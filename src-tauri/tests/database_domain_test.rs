//! Domain invariants of the storage layer against the real production schema.
//! These run through `Database`, so they exercise `init_schema()` and the
//! per-connection `foreign_keys` pragma rather than a hand-copied DDL.
use zxyssh_core::storage::models::{AuthType, Server};
use zxyssh_core::storage::{Database, Group, TunnelConfig, TunnelType};

fn database() -> (tempfile::TempDir, Database) {
    let dir = tempfile::tempdir().expect("isolated test directory");
    let database = Database::new_at(dir.path().join("zxyssh.db")).expect("open test database");
    (dir, database)
}

fn server(name: &str) -> Server {
    Server {
        id: String::new(),
        name: name.to_string(),
        host: format!("{name}.example.com"),
        port: 22,
        username: "root".to_string(),
        auth_type: AuthType::Password,
        credential_id: None,
        group_id: None,
        tags: vec![],
        created_at: 0,
        updated_at: 0,
        jump_host_id: None,
        post_login_command: None,
        agent_forwarding: false,
    }
}

fn group(name: &str) -> Group {
    Group {
        id: String::new(),
        name: name.to_string(),
        parent_id: None,
        color: "#808080".to_string(),
    }
}

fn tunnel(server_id: &str, local_port: u16) -> TunnelConfig {
    TunnelConfig {
        id: String::new(),
        server_id: server_id.to_string(),
        tunnel_type: TunnelType::Local,
        local_host: "127.0.0.1".to_string(),
        local_port,
        remote_host: Some("127.0.0.1".to_string()),
        remote_port: Some(80),
        auto_start: false,
        enabled: true,
    }
}

#[test]
fn server_delete_cascades_declared_foreign_keys() {
    let (_dir, database) = database();
    let mut host = server("tunnel-host");
    database.server_add(&mut host).unwrap();
    let mut tunnel = tunnel(&host.id, 8080);
    database.tunnel_config_add(&mut tunnel).unwrap();
    assert_eq!(database.tunnel_config_list(&host.id).unwrap().len(), 1);

    database.server_delete(&host.id).unwrap();

    assert!(database.tunnel_config_list(&host.id).unwrap().is_empty());
}

#[test]
fn group_delete_detaches_child_groups_and_members() {
    let (_dir, database) = database();
    let mut parent = group("Parent");
    database.group_add(&mut parent).unwrap();
    let mut child = group("Child");
    child.parent_id = Some(parent.id.clone());
    database.group_add(&mut child).unwrap();
    let mut member = server("member");
    member.group_id = Some(parent.id.clone());
    database.server_add(&mut member).unwrap();

    database.group_delete(&parent.id).unwrap();

    let groups = database.group_list().unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, child.id);
    assert_eq!(groups[0].parent_id, None);
    assert_eq!(
        database.server_get(&member.id).unwrap().unwrap().group_id,
        None
    );
}

#[test]
fn server_delete_detaches_jump_hosts_and_keeps_recording_rows() {
    let (dir, database) = database();
    let mut jump_host = server("jump");
    database.server_add(&mut jump_host).unwrap();
    let mut dependent = server("dependent");
    dependent.jump_host_id = Some(jump_host.id.clone());
    database.server_add(&mut dependent).unwrap();
    let mut tunnel = tunnel(&jump_host.id, 8080);
    database.tunnel_config_add(&mut tunnel).unwrap();
    // A recording row from a build that still had session recording: the
    // metadata outlives the site it belonged to.
    let raw = rusqlite::Connection::open(dir.path().join("zxyssh.db")).unwrap();
    raw.execute(
        "INSERT INTO recordings (id, session_id, server_id, started_at, ended_at, file_path)
         VALUES ('r-1', 'session-jump', ?1, 10, 20, '/device-only/jump.cast')",
        [&jump_host.id],
    )
    .unwrap();

    database.server_delete(&jump_host.id).unwrap();

    assert_eq!(
        database
            .server_get(&dependent.id)
            .unwrap()
            .unwrap()
            .jump_host_id,
        None
    );
    assert!(database
        .tunnel_config_list(&jump_host.id)
        .unwrap()
        .is_empty());
    let kept = raw.query_row(
        "SELECT server_id FROM recordings WHERE id = 'r-1'",
        [],
        |row| row.get::<_, String>(0),
    );
    assert_eq!(kept.unwrap(), jump_host.id);
}
