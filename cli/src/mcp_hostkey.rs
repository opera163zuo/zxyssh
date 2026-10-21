//! Headless SSH host-key administration for the LAN MCP service.
//!
//! zxySSH verifies host keys TOFU-style and refuses to send credentials to an
//! untrusted key. The desktop UI normally handles that first approval, so a
//! headless server needs an explicit, administrator-driven path: `probe` shows
//! the key the server presents, `trust` pins it only after the operator repeats
//! that fingerprint back.

use anyhow::{bail, Result};
use clap::Subcommand;
use std::sync::Arc;
use zxyssh_core::ssh::{FingerprintStore, HostKeyCheck, HostKeyRejection, SshClient};

/// `zxyssh mcp hostkey …`
#[derive(Subcommand)]
pub enum HostkeyCommand {
    /// Show the host key the server presents and whether it is trusted
    Probe {
        host: String,
        #[arg(long, default_value_t = 22)]
        port: u16,
    },
    /// Pin the presented key after repeating its fingerprint
    Trust {
        host: String,
        #[arg(long, default_value_t = 22)]
        port: u16,
        /// Fingerprint exactly as `probe` printed it, e.g. SHA256:...
        #[arg(long)]
        fingerprint: String,
    },
    /// List every trusted host key
    List,
}

pub fn run(command: HostkeyCommand) -> Result<()> {
    match command {
        HostkeyCommand::Probe { host, port } => probe(&host, port),
        HostkeyCommand::Trust {
            host,
            port,
            fingerprint,
        } => trust(&host, port, &fingerprint),
        HostkeyCommand::List => list(),
    }
}

fn store() -> Result<Arc<FingerprintStore>> {
    Ok(Arc::new(
        FingerprintStore::new().map_err(|error| anyhow::anyhow!(error))?,
    ))
}

/// The key the server currently presents, whether or not it is trusted yet.
async fn presented_key(
    store: Arc<FingerprintStore>,
    host: &str,
    port: u16,
) -> Result<(String, String)> {
    let (output_tx, _output_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
    let mut client = SshClient::new(output_tx);
    client.set_host_key_check(HostKeyCheck::new(store.clone(), host.to_string(), port));

    match client.connect_handshake(host, port).await {
        Ok(key) => {
            let _ = store.touch(host, port);
            Ok((key.fingerprint, key.algorithm))
        }
        Err(error) => match client.take_host_key_rejection().await {
            Some(HostKeyRejection::Unknown {
                fingerprint,
                algorithm,
            }) => Ok((fingerprint, algorithm)),
            Some(HostKeyRejection::Changed {
                stored_fingerprint,
                fingerprint,
                ..
            }) => bail!(
                "HOST_KEY_CHANGED for {host}:{port}: stored {stored_fingerprint}, presented \
                 {fingerprint}. Do not trust this key until you have investigated."
            ),
            Some(rejection) => bail!(rejection.error_message(host, port)),
            None => bail!("Could not reach {host}:{port}: {error}"),
        },
    }
}

fn block_on_probe(host: &str, port: u16) -> Result<(String, String)> {
    let store = store()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(presented_key(store, host, port))
}

pub fn probe(host: &str, port: u16) -> Result<()> {
    let (fingerprint, algorithm) = block_on_probe(host, port)?;
    let trusted = store()?
        .get(host, port)
        .map(|record| record.fingerprint)
        .unwrap_or_else(|| "not trusted yet".to_string());
    println!("{host}:{port} presents {algorithm} {fingerprint}");
    println!("Currently stored: {trusted}");
    if fingerprint == trusted {
        println!("This key is already trusted; nothing to do.");
    } else {
        println!(
            "Verify it out of band, then pin it: \
             zxyssh mcp hostkey trust {host} --port {port} --fingerprint {fingerprint}"
        );
    }
    Ok(())
}

/// Pin the presented key only when the operator supplies the same fingerprint,
/// so `trust` cannot silently accept a key the console never displayed.
pub fn trust(host: &str, port: u16, expected: &str) -> Result<()> {
    let (fingerprint, algorithm) = block_on_probe(host, port)?;
    if fingerprint != expected.trim() {
        bail!(
            "Refused: {host}:{port} presents {fingerprint}, not {expected}. \
             Re-probe and compare out of band."
        );
    }
    let record = store()?.save(host, port, &fingerprint, &algorithm, None)?;
    println!(
        "Trusted {algorithm} {fingerprint} for {host}:{port} ({}).",
        record.id
    );
    Ok(())
}

pub fn list() -> Result<()> {
    let records = store()?.list();
    if records.is_empty() {
        println!("No host keys are trusted yet.");
        return Ok(());
    }
    for record in records {
        println!(
            "{}:{}  {}  {}",
            record.host, record.port, record.algorithm, record.fingerprint
        );
    }
    Ok(())
}
