//! Tauri commands for Agent activity and pairing management.

use std::sync::Arc;

use tauri::State;

use crate::storage::Database;

/// Read the encrypted Agent activity history for the desktop panel.
#[tauri::command]
pub fn agent_activity_list(
    db: State<'_, Arc<Database>>,
    after: Option<i64>,
    before: Option<i64>,
    limit: Option<u32>,
) -> Result<Vec<crate::storage::database::StoredAgentActivity>, String> {
    db.agent_activity_list(after, before, limit.unwrap_or(200))
        .map_err(|error| format!("Failed to read agent activity: {error}"))
}

/// Commands for the LAN Agent access queue: mint a pairing code, decide pending
/// requests, and manage the Agents that already hold a credential. These share
/// the encrypted registry with `zxyssh mcp`, so the desktop and the terminal are
/// two views of one list.
use crate::mcp::lan_status::{self, LanListenerStatus};
use crate::mcp::pairing::{
    AgentSummary, IssuedAgent, PairRequestSummary, PairingError, PairingRegistry,
};

fn registry(db: &State<'_, Arc<Database>>) -> Result<PairingRegistry, String> {
    PairingRegistry::load(db.inner().clone()).map_err(registry_error)
}

fn registry_error(error: PairingError) -> String {
    match error {
        PairingError::Corrupt(detail) => {
            format!("Agent registry is unreadable and was not modified: {detail}")
        }
        PairingError::Storage(detail) => format!("Agent registry could not be saved: {detail}"),
        PairingError::InvalidCode => "Pairing code rejected".to_string(),
        PairingError::InvalidName => "Agent name must be 1-80 characters".to_string(),
        PairingError::InvalidClientKey => {
            "clientPublicKey must be a base64url-encoded 32-byte X25519 key".to_string()
        }
        PairingError::StillActive => "Revoke this Agent before deleting its record".to_string(),
        PairingError::TooManyRequests => {
            "Too many pairing requests are already waiting for the operator".to_string()
        }
    }
}

/// A pairing code is shown once, in the desktop UI only, and expires on the
/// server side.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCode {
    pub code: String,
    pub expires_at: i64,
}

#[tauri::command]
pub fn mcp_pair_code_create(db: State<'_, Arc<Database>>) -> Result<PairingCode, String> {
    let mut registry = registry(&db)?;
    let now = chrono::Utc::now().timestamp();
    let code = registry.create_code(now).map_err(registry_error)?;
    let expires_at = registry
        .pending_expires_at()
        .unwrap_or(now + crate::mcp::pairing::PAIRING_TTL_SECONDS);
    Ok(PairingCode { code, expires_at })
}

#[tauri::command]
pub fn mcp_agents_list(db: State<'_, Arc<Database>>) -> Result<Vec<AgentSummary>, String> {
    let registry = registry(&db)?;
    Ok(registry.list())
}

/// Whether a pairing code is still armed and unspent. The listener consumes it
/// in another process, so the desktop asks this instead of assuming the code it
/// minted is still good — that is how the UI notices a pairing that just
/// happened and can show the new Agent right away.
#[tauri::command]
pub fn mcp_pair_code_pending(db: State<'_, Arc<Database>>) -> Result<bool, String> {
    let registry = registry(&db)?;
    let now = chrono::Utc::now().timestamp();
    Ok(registry
        .pending_expires_at()
        .is_some_and(|expires_at| expires_at > now))
}

/// Codeless pairing requests waiting on the operator, oldest first.
#[tauri::command]
pub fn mcp_pair_requests_list(
    db: State<'_, Arc<Database>>,
) -> Result<Vec<PairRequestSummary>, String> {
    let registry = registry(&db)?;
    Ok(registry.pending_requests())
}

/// Approve a codeless request: this is the moment its credential is created, and
/// the requestor collects it by polling `/pair/status`.
#[tauri::command]
pub fn mcp_pair_request_approve(
    db: State<'_, Arc<Database>>,
    request_id: String,
) -> Result<Option<AgentSummary>, String> {
    let mut registry = registry(&db)?;
    let now = chrono::Utc::now().timestamp();
    registry
        .approve_request(&request_id, now)
        .map_err(registry_error)
}

/// Refuse a codeless request. Nothing was issued, so there is nothing to revoke.
#[tauri::command]
pub fn mcp_pair_request_reject(
    db: State<'_, Arc<Database>>,
    request_id: String,
) -> Result<bool, String> {
    let mut registry = registry(&db)?;
    let now = chrono::Utc::now().timestamp();
    registry
        .reject_request(&request_id, now)
        .map_err(registry_error)
}

#[tauri::command]
pub fn mcp_agent_revoke(db: State<'_, Arc<Database>>, agent_id: String) -> Result<bool, String> {
    let mut registry = registry(&db)?;
    registry.revoke(&agent_id).map_err(registry_error)
}

/// Remove a revoked Agent's record from the registry. Active credentials have
/// to be revoked first, so this can never retire a live Agent by accident.
#[tauri::command]
pub fn mcp_agent_forget(db: State<'_, Arc<Database>>, agent_id: String) -> Result<bool, String> {
    let mut registry = registry(&db)?;
    registry.forget(&agent_id).map_err(registry_error)
}

#[tauri::command]
pub fn mcp_agent_rename(
    db: State<'_, Arc<Database>>,
    agent_id: String,
    name: String,
) -> Result<bool, String> {
    let mut registry = registry(&db)?;
    registry.rename(&agent_id, &name).map_err(registry_error)
}

/// Replace an Agent's credential. The new token is returned once and never
/// stored, so the UI must show it immediately and must not log it.
#[tauri::command]
pub fn mcp_agent_rotate(
    db: State<'_, Arc<Database>>,
    agent_id: String,
) -> Result<Option<IssuedAgent>, String> {
    let mut registry = registry(&db)?;
    registry.rotate(&agent_id).map_err(registry_error)
}

/// What the LAN service published about its listener, or `None` when no live
/// service is running.
#[tauri::command]
pub fn mcp_lan_status() -> Result<Option<LanListenerStatus>, String> {
    Ok(lan_status::read_status())
}
