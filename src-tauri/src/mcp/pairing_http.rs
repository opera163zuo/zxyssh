//! HTTP surface for LAN Agent pairing: the one-time-code endpoint plus the
//! bearer-token guard in front of every other route.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard};

use axum::extract::{FromRequest, Request, State};
use axum::http::header::{AUTHORIZATION, CACHE_CONTROL, WWW_AUTHENTICATE};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use super::pairing::{
    AuthenticatedAgent, PairRequestStatus, PairStarted, PairingError, PairingRegistry,
};
use super::server::McpState;

const PAIR_WINDOW_SECONDS: u64 = 60;
const MAX_PAIR_REQUESTS_PER_PEER: u32 = 10;
/// Polling is how an Agent learns its request was approved, so it needs a wider
/// budget than pairing while still bounding a flood per peer.
pub const MAX_POLL_REQUESTS_PER_PEER: u32 = 60;
const MAX_TRACKED_PEERS: usize = 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairRequest {
    name: String,
    code: String,
    /// Ephemeral X25519 public key (base64url). With it, the credential comes
    /// back sealed and only this Agent can read it. Without it, pairing still
    /// works but the credential is delivered in the clear.
    #[serde(default, alias = "clientPublicKey")]
    client_public_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairPollRequest {
    #[serde(alias = "requestId")]
    request_id: String,
}

/// Body of a codeless pairing request. The key is required, not optional: a
/// request that will be answered by a human must not be able to come back
/// carrying its credential in the clear.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsentRequest {
    name: String,
    #[serde(alias = "clientPublicKey")]
    client_public_key: String,
}

/// Reject pairing floods before they reach a code's own attempt counter.
/// Pairing codes are six digits, so this is the outer bound on guessing.
#[derive(Default)]
pub struct PairRateLimiter {
    peers: HashMap<String, (u32, u64)>,
    quota: Option<u32>,
}

impl PairRateLimiter {
    pub fn with_quota(quota: u32) -> Self {
        Self {
            peers: HashMap::new(),
            quota: Some(quota),
        }
    }

    fn allow(&mut self, peer: &str, now_seconds: u64) -> bool {
        let window = now_seconds / PAIR_WINDOW_SECONDS;
        let entry = self.peers.entry(peer.to_string()).or_insert((0, window));
        if entry.1 != window {
            *entry = (0, window);
        }
        if entry.0 >= self.quota.unwrap_or(MAX_PAIR_REQUESTS_PER_PEER) {
            return false;
        }
        entry.0 += 1;
        if self.peers.len() > MAX_TRACKED_PEERS {
            self.peers.retain(|_, (_, seen)| *seen == window);
        }
        true
    }
}

fn peer_key(request: &Request) -> String {
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

/// Tokens and pairing material must not be cached by the Agent's HTTP client
/// or any proxy in front of the service.
fn no_store(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        HeaderName::from_static("pragma"),
        HeaderValue::from_static("no-cache"),
    );
    response
}

fn reject(status: StatusCode, message: &str) -> Response {
    no_store((status, message.to_string()).into_response())
}

fn unauthorized() -> Response {
    let mut response = reject(StatusCode::UNAUTHORIZED, "Unauthorized");
    response
        .headers_mut()
        .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

fn lock_registry(
    registry: &Arc<Mutex<PairingRegistry>>,
) -> Result<MutexGuard<'_, PairingRegistry>, ()> {
    registry.lock().map_err(|_| ())
}

pub async fn handle_pair(State(state): State<McpState>, request: Request) -> Response {
    let Some(registry) = &state.pairing_registry else {
        return reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Pairing registry unavailable",
        );
    };

    let peer = peer_key(&request);
    let allowed = state
        .pairing_limiter
        .lock()
        .map(|mut limiter| limiter.allow(&peer, now_seconds()))
        .unwrap_or(false);
    if !allowed {
        return reject(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pairing attempts, retry shortly",
        );
    }

    let body = match Json::<PairRequest>::from_request(request, &state).await {
        Ok(Json(body)) => body,
        Err(_) => {
            return reject(
                StatusCode::BAD_REQUEST,
                "Expected a JSON body with name and code",
            )
        }
    };

    let mut registry = match lock_registry(registry) {
        Ok(registry) => registry,
        Err(_) => return registry_unavailable(),
    };
    // Codes are minted by `zxyssh mcp pair-code` and by the desktop UI, which are
    // separate processes from this listener, so read the persisted registry
    // before consuming one and before deciding whether a request is queued.
    if registry.reload().is_err() {
        return registry_unavailable();
    }
    let now = chrono::Utc::now().timestamp();
    match registry.begin_pairing(
        &body.name,
        &body.code,
        body.client_public_key.as_deref(),
        &peer,
        now,
    ) {
        // The bearer token is issued exactly once. Only hashes are retained, so
        // it cannot be recovered from the service afterwards.
        Ok(PairStarted::Issued(issued)) => no_store(
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "agentId": issued.id,
                    "name": issued.name,
                    "token": issued.token,
                    "tokenType": "Bearer",
                })),
            )
                .into_response(),
        ),
        Ok(PairStarted::Awaiting { request_id }) => no_store(
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "requestId": request_id,
                    "status": "awaiting_approval",
                    "pollPath": "/pair/status",
                })),
            )
                .into_response(),
        ),
        Err(PairingError::InvalidName) => reject(StatusCode::BAD_REQUEST, "Agent name is required"),
        Err(PairingError::InvalidClientKey) => reject(
            StatusCode::BAD_REQUEST,
            "clientPublicKey must be a base64url-encoded 32-byte X25519 key",
        ),
        Err(PairingError::Storage(_)) => registry_unavailable(),
        Err(_) => reject(StatusCode::UNAUTHORIZED, "Pairing code rejected"),
    }
}

/// Start a pairing with no code. The request waits on the operator's approve
/// button — in the desktop or via `zxyssh mcp requests approve` — and no
/// credential exists until it is pressed. This is the path for an Agent that
/// must not be handed a secret through its own chat log.
pub async fn handle_pair_request(State(state): State<McpState>, request: Request) -> Response {
    let Some(registry) = &state.pairing_registry else {
        return reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Pairing registry unavailable",
        );
    };

    let peer = peer_key(&request);
    let allowed = state
        .pairing_limiter
        .lock()
        .map(|mut limiter| limiter.allow(&peer, now_seconds()))
        .unwrap_or(false);
    if !allowed {
        return reject(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pairing attempts, retry shortly",
        );
    }

    let body = match Json::<ConsentRequest>::from_request(request, &state).await {
        Ok(Json(body)) => body,
        Err(_) => {
            return reject(
                StatusCode::BAD_REQUEST,
                "Expected a JSON body with name and clientPublicKey",
            )
        }
    };

    let mut registry = match lock_registry(registry) {
        Ok(registry) => registry,
        Err(_) => return registry_unavailable(),
    };
    // The operator may approve from another process, so read the persisted
    // registry before queueing and before counting what is already waiting.
    if registry.reload().is_err() {
        return registry_unavailable();
    }
    let now = chrono::Utc::now().timestamp();
    match registry.begin_request(&body.name, &body.client_public_key, &peer, now) {
        Ok(request_id) => no_store(
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({
                    "requestId": request_id,
                    "status": "awaiting_consent",
                    "pollPath": "/pair/status",
                })),
            )
                .into_response(),
        ),
        Err(PairingError::InvalidName) => reject(StatusCode::BAD_REQUEST, "Agent name is required"),
        Err(PairingError::InvalidClientKey) => reject(
            StatusCode::BAD_REQUEST,
            "clientPublicKey is required and must be a base64url-encoded 32-byte X25519 key",
        ),
        Err(PairingError::TooManyRequests) => reject(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pairing requests are already waiting for the operator",
        ),
        Err(_) => registry_unavailable(),
    }
}

/// Delivery endpoint for a queued pairing request. It answers nothing about
/// other requests, and the sealed credential is returned only once.
pub async fn handle_pair_status(State(state): State<McpState>, request: Request) -> Response {
    let Some(registry) = &state.pairing_registry else {
        return reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Pairing registry unavailable",
        );
    };

    let allowed = state
        .pairing_poll_limiter
        .lock()
        .map(|mut limiter| limiter.allow(&peer_key(&request), now_seconds()))
        .unwrap_or(false);
    if !allowed {
        return reject(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many pairing polls, retry shortly",
        );
    }

    let body = match Json::<PairPollRequest>::from_request(request, &state).await {
        Ok(Json(body)) => body,
        Err(_) => {
            return reject(
                StatusCode::BAD_REQUEST,
                "Expected a JSON body with requestId",
            )
        }
    };

    let mut registry = match lock_registry(registry) {
        Ok(registry) => registry,
        Err(_) => return registry_unavailable(),
    };
    if registry.reload().is_err() {
        return registry_unavailable();
    }
    let now = chrono::Utc::now().timestamp();
    let poll = match registry.poll_request(&body.request_id, now) {
        Ok(Some(poll)) => poll,
        Ok(None) => return reject(StatusCode::NOT_FOUND, "Unknown pairing request"),
        Err(PairingError::Storage(_)) => return registry_unavailable(),
        Err(_) => return reject(StatusCode::INTERNAL_SERVER_ERROR, "Pairing failed"),
    };
    let mut body = serde_json::json!({ "status": status_name(poll.status) });
    if poll.status == PairRequestStatus::Delivered {
        if let (Some(id), Some(name)) = (poll.agent_id, poll.agent_name) {
            body["agentId"] = serde_json::Value::String(id);
            body["name"] = serde_json::Value::String(name);
        }
        if let (Some(public_key), Some(sealed)) = (poll.server_public_key, poll.sealed_credential) {
            body["serverPublicKey"] = serde_json::Value::String(public_key);
            body["sealedToken"] = serde_json::Value::String(sealed);
            body["tokenType"] = serde_json::Value::String("Bearer-sealed".to_string());
        }
    }
    no_store((StatusCode::OK, Json(body)).into_response())
}

fn status_name(status: PairRequestStatus) -> &'static str {
    match status {
        // The wire name stays `awaiting_approval` so Agents written against 2.1.0
        // keep parsing it; the state now means "sealing, poll again".
        PairRequestStatus::AwaitingDelivery => "awaiting_approval",
        PairRequestStatus::AwaitingConsent => "awaiting_consent",
        PairRequestStatus::Refused => "refused",
        PairRequestStatus::Approved => "approved",
        PairRequestStatus::Delivered => "delivered",
        PairRequestStatus::Expired => "expired",
    }
}

fn registry_unavailable() -> Response {
    reject(
        StatusCode::SERVICE_UNAVAILABLE,
        "Pairing registry is unreadable; refusing to serve",
    )
}

/// Require a valid, non-revoked Agent bearer token on every route except the
/// health probe and the pairing endpoint itself.
pub async fn require_paired_agent(
    State(state): State<McpState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    if matches!(
        path.as_str(),
        "/pair" | "/pair/request" | "/pair/status" | "/health"
    ) {
        return no_store(next.run(request).await);
    }

    let Some(registry) = &state.pairing_registry else {
        return reject(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Pairing registry unavailable",
        );
    };

    let token = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty());

    let Some(token) = token else {
        return unauthorized();
    };

    let agent = {
        let Ok(mut registry) = lock_registry(registry) else {
            return registry_unavailable();
        };
        // Reload so an administrator's `revoke` or `rotate` takes effect on the
        // next request rather than after a service restart. The guard is dropped
        // here; it must not be held across `next.run`.
        if registry.reload().is_err() {
            return registry_unavailable();
        }
        registry.authenticate(token)
    };

    match agent {
        Some(agent) => {
            let mut request = request;
            request.extensions_mut().insert(agent);
            no_store(next.run(request).await)
        }
        None => unauthorized(),
    }
}

/// Name and id of the authenticated Agent for the audit trail. Requests that
/// arrive without the LAN middleware (the loopback desktop gateway) are local.
pub fn audit_label(agent: Option<&AuthenticatedAgent>) -> String {
    match agent {
        Some(agent) => format!("{} ({})", agent.name, agent.id),
        None => "local".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_limiter_blocks_after_the_window_quota() {
        let mut limiter = PairRateLimiter::default();
        for _ in 0..MAX_PAIR_REQUESTS_PER_PEER {
            assert!(limiter.allow("203.0.113.7", 100));
        }
        assert!(!limiter.allow("203.0.113.7", 100));
        // A different peer is unaffected, and the window resets.
        assert!(limiter.allow("198.51.100.9", 100));
        assert!(limiter.allow("203.0.113.7", 100 + PAIR_WINDOW_SECONDS));
    }

    #[test]
    fn unknown_peer_is_shared_and_limited() {
        let mut limiter = PairRateLimiter::default();
        for _ in 0..MAX_PAIR_REQUESTS_PER_PEER {
            assert!(limiter.allow("unknown", 0));
        }
        assert!(!limiter.allow("unknown", 0));
    }
}
