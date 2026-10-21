//! End-to-end tests for the LAN MCP transport: a real listener on an ephemeral
//! loopback port, exercised through the same JSON-RPC surface an Agent uses.
//!
//! These run as unit tests rather than under `tests/` because the paired-Agent
//! registry needs the deterministic, in-memory device key that only the
//! `cfg(test)` crypto path provides.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use serde_json::{json, Value};

use crate::mcp::pairing::PairingRegistry;
use crate::mcp::server::McpServer;
use crate::session::SessionManager;
use crate::storage::Database;

struct TestService {
    base_url: String,
    registry: Arc<std::sync::Mutex<PairingRegistry>>,
    database: Arc<Database>,
    server: tokio::task::JoinHandle<()>,
    /// Held so the temporary database outlives the service.
    _temp: tempfile::TempDir,
}

impl TestService {
    async fn start() -> Self {
        let temp = tempfile::tempdir().expect("temp dir");
        let database =
            Arc::new(Database::new_at(temp.path().join("lan-mcp.db")).expect("test database"));
        let registry = Arc::new(std::sync::Mutex::new(
            PairingRegistry::load(database.clone()).expect("empty registry"),
        ));
        let session_manager = Arc::new(SessionManager::new(database.clone()));
        let app: Router = McpServer::new(database.clone(), session_manager)
            .with_pairing_registry(registry.clone())
            .router_with_pairing();

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("ephemeral loopback bind");
        let address: SocketAddr = listener.local_addr().expect("local address");
        let server = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await;
        });

        Self {
            base_url: format!("http://{address}"),
            registry,
            database,
            server,
            _temp: temp,
        }
    }

    /// Mint a code through the registry the way `zxyssh mcp pair-code` does,
    /// proving the service picks it up from storage rather than memory.
    fn mint_code(&self) -> String {
        let now = chrono::Utc::now().timestamp();
        self.registry
            .lock()
            .unwrap()
            .create_code(now)
            .expect("pairing code")
    }

    async fn pair(&self, name: &str, code: &str) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/pair", self.base_url))
            .json(&json!({ "name": name, "code": code }))
            .send()
            .await
            .expect("pair request")
    }

    async fn pair_token(&self, name: &str) -> String {
        let response = self.pair(name, &self.mint_code()).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let body: Value = response.json().await.expect("pair body");
        body["token"].as_str().expect("issued token").to_string()
    }

    async fn rpc(&self, token: Option<&str>, payload: Value) -> reqwest::Response {
        let mut request = reqwest::Client::new()
            .post(format!("{}/mcp", self.base_url))
            .json(&payload);
        if let Some(token) = token {
            request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
        }
        request.send().await.expect("mcp request")
    }

    fn shutdown(&self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn unauthenticated_mcp_requests_are_rejected() {
    let service = TestService::start().await;
    let response = service
        .rpc(
            None,
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        )
        .await;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()["cache-control"], "no-store");
    service.shutdown();
}

#[tokio::test]
async fn health_stays_public_and_mcp_needs_a_paired_agent() {
    let service = TestService::start().await;
    let health = reqwest::Client::new()
        .get(format!("{}/health", service.base_url))
        .send()
        .await
        .expect("health request");
    assert_eq!(health.status(), reqwest::StatusCode::OK);

    let token = service.pair_token("hermes-remote").await;
    let response = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
        )
        .await;
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.expect("initialize body");
    assert_eq!(body["result"]["serverInfo"]["name"], "zxyssh");
    assert!(body["result"]["protocolVersion"].is_string());

    let wrong_token = service
        .rpc(
            Some("vsh_not_a_real_credential"),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        )
        .await;
    assert_eq!(wrong_token.status(), reqwest::StatusCode::UNAUTHORIZED);
    service.shutdown();
}

#[tokio::test]
async fn tools_list_and_an_authorized_read_only_tool_call() {
    let service = TestService::start().await;
    let token = service.pair_token("hermes-tools").await;

    let listed = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
        )
        .await;
    let listed: Value = listed.json().await.expect("tools/list body");
    let tools = listed["result"]["tools"].as_array().unwrap();
    let names = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(names.len(), 7);
    for name in [
        "server_list",
        "session_create",
        "session_send_input",
        "session_sudo",
        "session_read",
        "session_resize",
        "session_kill",
    ] {
        assert!(names.contains(name), "missing {name}");
    }
    for name in ["exec", "rg", "sftp_upload", "server_add", "server_delete"] {
        assert!(!names.contains(name), "unexpected {name}");
    }

    let forbidden = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":30,"method":"tools/call",
                   "params":{"name":"exec","arguments":{"command":"id","session_id":"none"}}}),
        )
        .await;
    let forbidden: Value = forbidden.json().await.expect("forbidden tool body");
    assert_eq!(forbidden["error"]["code"], -32601);

    let called = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
                   "params":{"name":"server_list","arguments":{}}}),
        )
        .await;
    let called: Value = called.json().await.expect("tool call body");
    assert!(called["error"].is_null(), "{called}");
    let servers: Value =
        serde_json::from_str(called["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    if let Some(server) = servers.as_array().and_then(|items| items.first()) {
        assert_eq!(
            server
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>(),
            ["id", "name"].into_iter().map(str::to_string).collect()
        );
    }

    // The audit trail records which Agent caused the operation.
    let events = service
        .database
        .agent_activity_list(Some(0), None, 50)
        .expect("activity");
    let started = events
        .iter()
        .find(|stored| stored.event.tool == "server_list")
        .expect("recorded tool call");
    let agent = started.event.agent.as_deref().expect("audited agent");
    assert!(agent.starts_with("hermes-tools"), "{agent}");
    assert!(
        agent.contains("hermes-tools ("),
        "id is not recorded: {agent}"
    );
    service.shutdown();
}

#[tokio::test]
async fn paired_agents_cannot_terminate_all_sessions() {
    let service = TestService::start().await;
    let token = service.pair_token("hermes-kill-boundary").await;

    let rejected = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":31,"method":"tools/call",
                   "params":{"name":"session_kill","arguments":{"session_id":"unowned","all":true}}}),
        )
        .await;
    let rejected: Value = rejected.json().await.expect("session kill boundary body");
    assert_eq!(rejected["error"]["code"], -32602);
    assert_eq!(
        rejected["error"]["message"],
        "Paired Agents may only terminate their own sessions"
    );

    service.shutdown();
}

#[tokio::test]
async fn pairing_code_is_single_use_and_wrong_codes_are_rejected() {
    let service = TestService::start().await;
    let code = service.mint_code();

    let rejected = service.pair("attacker", "000001").await;
    assert_eq!(rejected.status(), reqwest::StatusCode::UNAUTHORIZED);

    let accepted = service.pair("hermes-one", &code).await;
    assert_eq!(accepted.status(), reqwest::StatusCode::OK);

    let reused = service.pair("hermes-two", &code).await;
    assert_eq!(reused.status(), reqwest::StatusCode::UNAUTHORIZED);
    service.shutdown();
}

#[tokio::test]
async fn expired_pairing_code_is_rejected() {
    let service = TestService::start().await;
    // Minted at the epoch, so its TTL expired long before the service clock.
    let expired = service
        .registry
        .lock()
        .unwrap()
        .create_code(0)
        .expect("pairing code");
    let response = service.pair("hermes-late", &expired).await;
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
    service.shutdown();
}

#[tokio::test]
async fn revocation_stops_a_live_token_without_restarting() {
    let service = TestService::start().await;
    let token = service.pair_token("hermes-revocable").await;

    let before = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":5,"method":"ping","params":{}}),
        )
        .await;
    assert_eq!(before.status(), reqwest::StatusCode::OK);

    let id = service
        .registry
        .lock()
        .unwrap()
        .list()
        .first()
        .expect("paired agent")
        .id
        .clone();
    assert!(service.registry.lock().unwrap().revoke(&id).unwrap());

    let after = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":6,"method":"ping","params":{}}),
        )
        .await;
    assert_eq!(after.status(), reqwest::StatusCode::UNAUTHORIZED);
    service.shutdown();
}

#[tokio::test]
async fn notifications_get_an_empty_accepted_response() {
    let service = TestService::start().await;
    let token = service.pair_token("hermes-notify").await;
    let response = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}),
        )
        .await;
    assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
    assert!(response.text().await.expect("body").is_empty());
    service.shutdown();
}

#[tokio::test]
async fn unknown_methods_are_reported_as_jsonrpc_errors() {
    let service = TestService::start().await;
    let token = service.pair_token("hermes-unknown").await;
    let response = service
        .rpc(
            Some(&token),
            json!({"jsonrpc":"2.0","id":7,"method":"tools/nope","params":{}}),
        )
        .await;
    let body: Value = response.json().await.expect("error body");
    assert_eq!(body["error"]["code"], -32601);
    service.shutdown();
}

#[tokio::test]
async fn pairing_requests_are_rate_limited_per_peer() {
    let service = TestService::start().await;
    service.mint_code();
    let mut statuses = Vec::new();
    for _ in 0..14 {
        let response = service.pair("flooder", "999999").await;
        statuses.push(response.status());
    }
    assert!(
        statuses.contains(&reqwest::StatusCode::TOO_MANY_REQUESTS),
        "expected the limiter to engage: {statuses:?}"
    );
    service.shutdown();
}

#[tokio::test]
async fn malformed_pairing_bodies_are_rejected_without_touching_the_code() {
    let service = TestService::start().await;
    let code = service.mint_code();
    let response = reqwest::Client::new()
        .post(format!("{}/pair", service.base_url))
        .json(&json!({ "name": "hermes", "code": code, "extra": true }))
        .send()
        .await
        .expect("pair request");
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // The untouched code still pairs.
    let accepted = service.pair("hermes", &code).await;
    assert_eq!(accepted.status(), reqwest::StatusCode::OK);
    service.shutdown();
}
