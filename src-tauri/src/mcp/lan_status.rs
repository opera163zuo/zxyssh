//! Snapshot of the LAN MCP listener, written by the service process so the
//! desktop can show what is exposed without reaching into another process.
//!
//! The file records no credential: bind address, opt-in flags, and counters
//! only. It is mode 0600 in the shared application data directory.
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub const LAN_STATUS_FILE: &str = "mcp-lan-status.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LanListenerStatus {
    /// Address the service is bound to, e.g. `192.168.0.96:8765`.
    pub bind: String,
    /// Whether the process was allowed to leave loopback.
    pub allow_lan: bool,
    /// Credentials are sealed to an ephemeral Agent key rather than returned in
    /// the clear.
    pub credential_sealing: bool,
    /// Operator switch for the approval queue.
    pub approval_required: bool,
    pub pid: u32,
    pub started_at: i64,
    pub active_agents: usize,
    pub pending_requests: usize,
}

pub fn lan_status_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from("com", "zxyssh", "zxySSH")
        .ok_or_else(|| anyhow::anyhow!("Could not determine zxySSH data directory"))?;
    Ok(project_dirs.data_dir().join(LAN_STATUS_FILE))
}

/// Publish the listener state. A failure here must not stop the service, which
/// is why the caller logs rather than unwraps.
pub fn write_status(status: &LanListenerStatus) -> Result<()> {
    let path = lan_status_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create MCP data directory")?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
        .context("open MCP LAN status file")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .context("secure MCP LAN status file")?;
    }
    let payload = serde_json::to_vec_pretty(status).context("serialize MCP LAN status")?;
    file.write_all(&payload).context("write MCP LAN status")?;
    Ok(())
}

/// Drop the published state when the listener stops, so a stale file cannot make
/// the desktop claim a port is open.
pub fn clear_status() {
    if let Ok(path) = lan_status_path() {
        let _ = fs::remove_file(path);
    }
}

/// Read the published state, or `None` when no live service wrote it. The pid is
/// checked so a crashed service that left its file behind does not read as
/// running.
pub fn read_status() -> Option<LanListenerStatus> {
    let path = lan_status_path().ok()?;
    let text = fs::read_to_string(path).ok()?;
    let status: LanListenerStatus = serde_json::from_str(&text).ok()?;
    if process_alive(status.pid) {
        Some(status)
    } else {
        None
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_through_json() {
        let status = LanListenerStatus {
            bind: "192.168.0.96:8765".to_string(),
            allow_lan: true,
            credential_sealing: true,
            approval_required: true,
            pid: 4242,
            started_at: 1_800_000_000,
            active_agents: 2,
            pending_requests: 1,
        };
        let text = serde_json::to_string(&status).unwrap();
        let back: LanListenerStatus = serde_json::from_str(&text).unwrap();
        assert_eq!(status, back);
        // The published shape is what the desktop reads, so field names are part
        // of the contract.
        assert!(text.contains("\"allowLan\""));
        assert!(text.contains("\"credentialSealing\""));
    }

    #[test]
    fn a_dead_service_is_not_reported_as_running() {
        assert!(!process_alive(u32::MAX));
    }
}
