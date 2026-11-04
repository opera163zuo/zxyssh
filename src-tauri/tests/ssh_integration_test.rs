//! Opt-in, pinned-host OpenSSH regression suite. Run scripts/test-ssh-compatibility.sh.
//! No test uses the user's server database, SSH keys, or trust store.
use anyhow::{Context, Result};
use std::{env, fs, path::PathBuf, sync::Arc, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use zxyssh_core::ssh::{FingerprintStore, HostKeyCheck, PtyConfig, SshClient};

fn required(name: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| panic!("{name} missing; run scripts/test-ssh-compatibility.sh"))
}
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(required("SSH_TEST_FIXTURE_DIR")).join(name)
}

struct Fixture {
    client: SshClient,
    output: mpsc::Receiver<Vec<u8>>,
    store: Arc<FingerprintStore>,
    _directory: tempfile::TempDir,
    host: String,
    port: u16,
}
impl Fixture {
    fn new(port_name: &str, key_name: &str) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let store = Arc::new(FingerprintStore::new_at(
            directory.path().join("trust.json"),
        )?);
        let host = required("SSH_TEST_HOST");
        let port = required(port_name).parse()?;
        let key =
            russh::keys::PublicKey::from_openssh(&fs::read_to_string(fixture_path(key_name))?)?;
        store.save(
            &host,
            port,
            &key.fingerprint(russh::keys::HashAlg::Sha256).to_string(),
            key.algorithm().as_ref(),
            None,
        )?;
        let (sender, output) = mpsc::channel(64);
        let mut client = SshClient::new(sender);
        client.set_host_key_check(HostKeyCheck::new(store.clone(), &host, port));
        Ok(Self {
            client,
            output,
            store,
            _directory: directory,
            host,
            port,
        })
    }
    async fn password(&mut self) -> Result<()> {
        self.client
            .connect_password(
                &self.host,
                self.port,
                &required("SSH_TEST_USER"),
                &required("SSH_TEST_PASSWORD"),
            )
            .await
    }
    async fn key(&mut self, name: &str, passphrase: Option<&str>) -> Result<()> {
        self.client
            .connect_key(
                &self.host,
                self.port,
                &required("SSH_TEST_USER"),
                &fs::read_to_string(fixture_path(name))?,
                passphrase,
            )
            .await
    }
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn authentication_matrix() -> Result<()> {
    for (port, host_key) in [
        ("SSH_TEST_PORT", "host.pub"),
        ("SSH_TEST_PAM_PORT", "host.pub"),
        ("SSH_TEST_COMPAT_PORT", "host-rsa.pub"),
    ] {
        let mut fixture = Fixture::new(port, host_key)?;
        fixture
            .password()
            .await
            .with_context(|| format!("Password/PAM policy {port}"))?;
        assert_eq!(
            fixture.client.exec_command("whoami").await?.trim(),
            required("SSH_TEST_USER")
        );
        fixture.client.disconnect().await?;
        for (name, passphrase) in [
            ("ed25519", None),
            ("rsa", None),
            ("ecdsa", None),
            ("encrypted", Some("fixture-key-passphrase")),
            ("rsa-pem", None),
        ] {
            let mut fixture = Fixture::new(port, host_key)?;
            fixture
                .key(name, passphrase)
                .await
                .with_context(|| format!("Key {name}, policy {port}"))?;
            assert_eq!(
                fixture.client.exec_command("printf key-ok").await?,
                "key-ok"
            );
            fixture.client.disconnect().await?;
            println!("PASS authentication: {port} / {name}");
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn unknown_changed_and_wrong_credentials_fail_closed() -> Result<()> {
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.store.clear()?;
    assert!(fixture.password().await.is_err());
    assert!(!fixture.client.is_connected().await);
    fixture.store.save(
        &fixture.host,
        fixture.port,
        "SHA256:not-the-server-key",
        "ssh-ed25519",
        None,
    )?;
    assert!(fixture.password().await.is_err());
    assert!(!fixture.client.is_connected().await);
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    assert!(fixture
        .client
        .connect_password(
            &fixture.host,
            fixture.port,
            &required("SSH_TEST_USER"),
            "intentionally-wrong"
        )
        .await
        .is_err());
    assert!(!fixture.client.is_connected().await);
    assert!(fixture
        .key("encrypted", Some("wrong-passphrase"))
        .await
        .is_err());
    assert!(!fixture.client.is_connected().await);
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn exec_eof_unicode_exit_status_and_channel_reuse() -> Result<()> {
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.password().await?;
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(5),
            fixture.client.exec_command("cat; printf eof-ok")
        )
        .await??,
        "eof-ok"
    );
    assert_eq!(
        fixture
            .client
            .exec_command_with_stdin("cat", Some("中文 input"))
            .await?,
        "中文 input\n"
    );
    assert_eq!(
        fixture
            .client
            .exec_command("python3 -c \"print('中文測試'*50,end='')\"")
            .await?,
        "中文測試".repeat(50)
    );
    for index in 0..24 {
        assert_eq!(
            fixture
                .client
                .exec_command(&format!("printf run-{index}"))
                .await?,
            format!("run-{index}")
        );
    }
    let result = fixture
        .client
        .exec_command_result("printf failure; exit 7", None)
        .await?;
    assert_eq!(result.output, "failure");
    assert_eq!(result.exit_code, 7);
    // EOF is not process completion: do not mistake a still-running job for success.
    let result = fixture
        .client
        .exec_command_result("exec 1>&- 2>&-; sleep 3; exit 9", None)
        .await?;
    assert_eq!(result.exit_code, 9);
    fixture.client.disconnect().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn excessive_exec_output_is_bounded_and_connection_survives() -> Result<()> {
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.password().await?;
    let error = fixture
        .client
        .exec_command("python3 -c \"print('x'*9000000)\"")
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("8 MiB"));
    assert_eq!(fixture.client.exec_command("printf alive").await?, "alive");
    fixture.client.disconnect().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn pty_unicode_resize_large_output_and_exit() -> Result<()> {
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.password().await?;
    fixture
        .client
        .open_shell(Some(PtyConfig {
            cols: 101,
            rows: 37,
            ..Default::default()
        }))
        .await?;
    fixture.client.resize_pty(113, 41).await?;
    fixture.client.send_data(b"stty -echo; stty size; python3 -c \"print('x'*262144)\"; printf '\\nPTY_DONE\\n'; exit\n").await?;
    let output = tokio::time::timeout(Duration::from_secs(15), async {
        let mut result = Vec::new();
        while let Some(chunk) = fixture.output.recv().await {
            result.extend(chunk);
            if result
                .windows(b"PTY_DONE\r\n".len())
                .any(|part| part == b"PTY_DONE\r\n")
            {
                break;
            }
        }
        result
    })
    .await?;
    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("41 113"));
    assert!(output.iter().filter(|&&byte| byte == b'x').count() >= 262144);
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.client.is_shell_open().await {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert!(fixture.client.is_connected().await);
    fixture.client.open_shell(None).await?;
    assert!(fixture.client.is_shell_open().await);
    fixture.client.disconnect().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn sftp_binary_unicode_roundtrip_and_reuse() -> Result<()> {
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.password().await?;
    let data: Vec<u8> = (0..1_048_576).map(|index| (index % 251) as u8).collect();
    for index in 0..10 {
        let sftp = fixture.client.open_sftp_session().await?;
        let path = format!("/home/audit/中文 文件-{index}.bin");
        let mut file = sftp.create(&path).await?;
        file.write_all(&data).await?;
        file.shutdown().await?;
        drop(file);
        let mut file = sftp.open(&path).await?;
        let mut actual = Vec::new();
        file.read_to_end(&mut actual).await?;
        assert_eq!(actual, data);
        drop(file);
        sftp.remove_file(&path).await?;
        sftp.close().await?;
    }
    assert_eq!(
        fixture.client.exec_command("printf after-sftp").await?,
        "after-sftp"
    );
    fixture.client.disconnect().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn verified_upload_lands_whole_or_not_at_all() -> Result<()> {
    use zxyssh_core::sftp::helpers::{read_remote_range, sha256_hex, write_verified_remote_file};

    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.password().await?;
    let sftp = fixture.client.open_sftp_session().await?;

    // Not valid UTF-8, so nothing on this path can be confused with the text tools.
    let data: Vec<u8> = (0..524_288).map(|index| (index % 256) as u8).collect();
    let digest = sha256_hex(&data);
    let path = "/home/audit/blob.bin";

    write_verified_remote_file(&sftp, path, &data, &digest, true, "landed")
        .await
        .expect("verified upload should succeed");
    assert_eq!(sftp.read(path).await?, data);

    // A declared digest the target does not hold must fail, and the failure must
    // not leave either the real path or the temporary file behind.
    let wrong = sha256_hex(b"described by a different digest");
    let error = write_verified_remote_file(&sftp, path, &data, &wrong, true, "refused")
        .await
        .expect_err("a mismatching digest must fail");
    assert!(error.contains("Verification failed"), "{error}");
    assert!(
        sftp.metadata(format!("{path}.zxyssh-part-refused"))
            .await
            .is_err(),
        "the rejected temporary file was left on the target"
    );
    assert_eq!(
        sftp.read(path).await?,
        data,
        "the failure touched the real path"
    );

    // Ranges page without the caller having to fetch the whole file.
    let (window, total) = read_remote_range(&sftp, path, 4096, 8192)
        .await
        .expect("a bounded range should read cleanly");
    assert_eq!(total, data.len() as u64);
    assert_eq!(window, data[4096..12_288]);
    let (tail, _) = read_remote_range(&sftp, path, (data.len() - 10) as u64, 1000)
        .await
        .expect("a range clipped by the end of the file should still read");
    assert_eq!(tail.len(), 10);

    assert!(read_remote_range(&sftp, path, data.len() as u64 + 1, 16)
        .await
        .expect_err("offset past end should fail")
        .contains("past the end"));

    // Parents are not minted as a side effect of writing a file.
    let nested = "/home/audit/absent/deep/blob.bin";
    assert!(
        write_verified_remote_file(&sftp, nested, &data, &digest, true, "nested")
            .await
            .expect_err("a missing parent directory should fail")
            .contains("Parent directory")
    );

    // mode=create_new refuses an existing file before writing anything.
    assert!(
        write_verified_remote_file(&sftp, path, &data, &digest, false, "exists")
            .await
            .expect_err("create_new should refuse an existing path")
            .contains("already exists")
    );

    // The payload moves onto the path instead of being written through it, so a
    // symlink there is replaced rather than followed into whatever it points at.
    let mut victim = sftp
        .create("/home/audit/victim")
        .await
        .map_err(|e| anyhow::anyhow!("victim create: {e}"))?;
    victim
        .write_all(b"original")
        .await
        .map_err(|e| anyhow::anyhow!("victim write: {e}"))?;
    victim
        .shutdown()
        .await
        .map_err(|e| anyhow::anyhow!("victim close: {e}"))?;
    drop(victim);
    // Made through the shell because this server answers SSH_FXP_SYMLINK with a
    // bare failure. What matters is only that a link exists at the target path.
    fixture
        .client
        .exec_command("ln -s /home/audit/victim /home/audit/link")
        .await
        .map_err(|e| anyhow::anyhow!("symlink: {e}"))?;
    write_verified_remote_file(
        &sftp,
        "/home/audit/link",
        b"replaced",
        &sha256_hex(b"replaced"),
        true,
        "link",
    )
    .await
    .expect("an upload onto a symlink path should land");
    let victim = sftp
        .read("/home/audit/victim")
        .await
        .map_err(|e| anyhow::anyhow!("victim read: {e}"))?;
    assert_eq!(
        victim, b"original",
        "the upload was written through the symlink into its target"
    );
    let link = sftp
        .read("/home/audit/link")
        .await
        .map_err(|e| anyhow::anyhow!("link read: {e}"))?;
    assert_eq!(link, b"replaced");
    sftp.remove_file("/home/audit/link")
        .await
        .map_err(|e| anyhow::anyhow!("link remove: {e}"))?;
    sftp.remove_file("/home/audit/victim")
        .await
        .map_err(|e| anyhow::anyhow!("victim remove: {e}"))?;

    // A directory target is refused rather than opened.
    assert!(write_verified_remote_file(
        &sftp,
        "/home/audit",
        b"hi",
        &sha256_hex(b"hi"),
        true,
        "dir"
    )
    .await
    .expect_err("a directory is not a file")
    .contains("directory"));

    sftp.remove_file(path).await?;
    sftp.close().await?;
    fixture.client.disconnect().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "isolated OpenSSH fixture required"]
async fn streaming_download_and_same_size_directory_sync() -> Result<()> {
    use zxyssh_core::sftp::sync::download_remote_file_streaming;
    use zxyssh_core::sftp::{
        transfer_directory_to_sftp, DirectoryTransferMode, DirectoryTransferOptions,
    };
    let mut fixture = Fixture::new("SSH_TEST_PORT", "host.pub")?;
    fixture.password().await?;
    let sftp = fixture.client.open_sftp_session().await?;
    let source = tempfile::tempdir()?;
    let destination = tempfile::tempdir()?;
    let payload: Vec<u8> = (0..600_123).map(|index| (index % 251) as u8).collect();
    fs::write(source.path().join("binary.bin"), &payload)?;
    fs::write(source.path().join("same.txt"), b"old!")?;
    let remote = format!("/home/audit/helper-{}", uuid::Uuid::new_v4());
    let options = DirectoryTransferOptions {
        excluded_paths: vec![],
        respect_gitignore: false,
        delete_extra: false,
    };
    let first = transfer_directory_to_sftp(
        &sftp,
        source.path(),
        &remote,
        DirectoryTransferMode::Sync,
        &options,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    assert_eq!(first.uploaded_files, 2);
    fs::write(source.path().join("same.txt"), b"new!")?;
    let second = transfer_directory_to_sftp(
        &sftp,
        source.path(),
        &remote,
        DirectoryTransferMode::Sync,
        &options,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    assert_eq!(second.uploaded_files, 1);
    assert_eq!(second.skipped_files, 1);
    for (name, expected) in [
        ("binary.bin", payload.as_slice()),
        ("same.txt", b"new!".as_slice()),
    ] {
        let path = destination.path().join(name);
        let count = download_remote_file_streaming(&sftp, &format!("{remote}/{name}"), &path)
            .await
            .map_err(anyhow::Error::msg)?;
        assert_eq!(count, expected.len() as u64);
        let actual = fs::read(path)?;
        assert_eq!(actual.len(), expected.len());
        assert!(actual == expected, "streamed file differs: {name}");
    }
    sftp.close().await?;
    fixture.client.disconnect().await?;
    Ok(())
}
