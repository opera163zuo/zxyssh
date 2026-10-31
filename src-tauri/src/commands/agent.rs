//! Tauri commands backing the agent command-guard UI (approval dialog,
//! auto-approve window, and the persisted risk ruleset).

use std::sync::Arc;

use serde::Deserialize;
use tauri::State;

use crate::mcp::approval::ApprovalStatus;
use crate::mcp::approval::AUTO_APPROVE_UNTIL_KEY;
use crate::mcp::guard::{GuardConfig, GUARD_CONFIG_KEY};
use crate::storage::Database;

use crate::mcp::{AgentApprovalManager, ApprovalOutcome};

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

const MAX_AUTO_APPROVE_HOURS: f64 = 5.0;

fn auto_approve_ms(hours: Option<f64>) -> Result<Option<i64>, String> {
    let Some(hours) = hours else {
        return Ok(None);
    };
    if !hours.is_finite() || hours <= 0.0 || hours > MAX_AUTO_APPROVE_HOURS {
        return Err(format!(
            "autoApproveHours must be greater than 0 and at most {MAX_AUTO_APPROVE_HOURS}"
        ));
    }

    Ok(Some((hours * 3_600_000.0).round() as i64))
}

/// Decision payload sent from the approval dialog.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveApprovalRequest {
    /// Id of the pending approval to resolve.
    pub id: String,
    /// Whether the command was approved.
    pub approved: bool,
    /// When approved, open an auto-approve window of this many hours.
    #[serde(default)]
    pub auto_approve_hours: Option<f64>,
}

/// Resolve a pending approval and optionally open an auto-approve window.
#[tauri::command]
pub fn resolve_agent_approval(
    approvals: State<'_, Arc<AgentApprovalManager>>,
    db: State<'_, Arc<Database>>,
    request: ResolveApprovalRequest,
) -> Result<(), String> {
    let auto_ms = if request.approved {
        auto_approve_ms(request.auto_approve_hours)?
    } else {
        None
    };
    let outcome = if request.approved {
        ApprovalOutcome::Approved
    } else {
        ApprovalOutcome::Denied("Denied by user".to_string())
    };
    if approvals.resolve(&request.id, outcome, auto_ms) {
        if auto_ms.is_some() {
            if let Some(until) = approvals.status().auto_approve_until {
                if let Err(error) = db.set_setting(AUTO_APPROVE_UNTIL_KEY, &until.to_string()) {
                    log::warn!("Could not persist agent auto-approve deadline: {}", error);
                }
            }
        }
        Ok(())
    } else {
        Err("Approval request is no longer pending".to_string())
    }
}

/// Report the current auto-approve deadline (if any).
#[tauri::command]
pub fn get_agent_guard_status(approvals: State<'_, Arc<AgentApprovalManager>>) -> ApprovalStatus {
    approvals.status()
}

/// Close the auto-approve window immediately.
#[tauri::command]
pub fn cancel_agent_auto_approve(
    approvals: State<'_, Arc<AgentApprovalManager>>,
    db: State<'_, Arc<Database>>,
) -> Result<(), String> {
    db.set_setting(AUTO_APPROVE_UNTIL_KEY, "0")
        .map_err(|error| error.to_string())?;
    approvals.cancel_auto_approve();
    Ok(())
}

/// Read the persisted command-guard configuration.
#[tauri::command]
pub fn get_agent_guard_config(db: State<'_, Arc<Database>>) -> Result<GuardConfig, String> {
    let json = db
        .get_setting(GUARD_CONFIG_KEY)
        .map_err(|e| e.to_string())?;
    Ok(GuardConfig::from_stored_json(json.as_deref()))
}

/// Persist the command-guard configuration.
#[tauri::command]
pub fn set_agent_guard_config(
    db: State<'_, Arc<Database>>,
    config: GuardConfig,
) -> Result<(), String> {
    let json = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    db.set_setting(GUARD_CONFIG_KEY, &json)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Commands for the LAN Agent access queue: mint a pairing code, decide pending
/// requests, and manage the Agents that already hold a credential. These share
/// the encrypted registry with `zxyssh mcp`, so the desktop and the terminal are
/// two views of one list.
use crate::mcp::lan_status::{self, LanListenerStatus};
use crate::mcp::pairing::{AgentSummary, IssuedAgent, PairingError, PairingRegistry};

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

#[tauri::command]
pub fn mcp_agent_revoke(db: State<'_, Arc<Database>>, agent_id: String) -> Result<bool, String> {
    let mut registry = registry(&db)?;
    registry.revoke(&agent_id).map_err(registry_error)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_approve_duration_is_bounded_and_finite() {
        assert_eq!(auto_approve_ms(None).unwrap(), None);
        assert_eq!(auto_approve_ms(Some(5.0)).unwrap(), Some(18_000_000));
        assert!(auto_approve_ms(Some(5.01)).is_err());
        assert!(auto_approve_ms(Some(0.0)).is_err());
        assert!(auto_approve_ms(Some(f64::NAN)).is_err());
        assert!(auto_approve_ms(Some(f64::INFINITY)).is_err());
    }
}
