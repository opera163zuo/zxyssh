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

use super::pairing::{AuthenticatedAgent, PairingError, PairingRegistry};
use super::server::McpState;

const PAIR_WINDOW_SECONDS: u64 = 60;
const MAX_PAIR_REQUESTS_PER_PEER: u32 = 10;
const MAX_TRACKED_PEERS: usize = 1_024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairRequest {
    name: String,
    code: String,
}

/// Reject pairing floods before they reach a code's own attempt counter.
/// Pairing codes are six digits, so this is the outer bound on guessing.
#[derive(Default)]
pub struct PairRateLimiter {
    peers: HashMap<String, (u32, u64)>,
}

impl PairRateLimiter {
    fn allow(&mut self, peer: &str, now_seconds: u64) -> bool {
        let window = now_seconds / PAIR_WINDOW_SECONDS;
        let entry = self.peers.entry(peer.to_string()).or_insert((0, window));
        if entry.1 != window {
            *entry = (0, window);
        }
        if entry.0 >= MAX_PAIR_REQUESTS_PER_PEER {
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

    let allowed = state
        .pairing_limiter
        .lock()
        .map(|mut limiter| limiter.allow(&peer_key(&request), now_seconds()))
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
    // Codes are minted by `zxyssh mcp pair-code`, which runs in a separate
    // process, so read the persisted registry before consuming one.
    if registry.reload().is_err() {
        return registry_unavailable();
    }
    let now = chrono::Utc::now().timestamp();
    match registry.pair(&body.name, &body.code, now) {
        // The bearer token is issued exactly once. Only hashes are retained, so
        // it cannot be recovered from the service afterwards.
        Ok(issued) => no_store(
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
        Err(PairingError::InvalidName) => reject(StatusCode::BAD_REQUEST, "Agent name is required"),
        Err(PairingError::Storage(_)) => registry_unavailable(),
        Err(_) => reject(StatusCode::UNAUTHORIZED, "Pairing code rejected"),
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
    if path == "/pair" || path == "/health" {
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
