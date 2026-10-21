use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Reason a pairing or administration request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    InvalidCode,
    InvalidName,
    /// The registry could not be written. The in-memory change is rolled back.
    Storage(String),
    /// The persisted registry could not be read or decrypted. Callers must
    /// refuse to start rather than begin with an empty, unpaired registry.
    Corrupt(String),
}

pub const PAIRING_TTL_SECONDS: i64 = 300;
pub const MAX_PAIRING_ATTEMPTS: u8 = 5;
const TOKEN_PREFIX: &str = "vsh_";
const REGISTRY_SETTING: &str = "mcp_lan_agents_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingCode {
    value: String,
    expires_at: i64,
    attempts: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentRecord {
    id: String,
    name: String,
    token_hash: String,
    revoked: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RegistryState {
    agents: HashMap<String, AgentRecord>,
    pending: Option<PendingCode>,
}

fn digest_token(token: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(token.as_bytes()))
}

fn random_token() -> String {
    use base64::Engine as _;
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    format!(
        "{TOKEN_PREFIX}{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    )
}

#[derive(Debug, Clone)]
pub struct IssuedAgent {
    pub id: String,
    pub name: String,
    /// Shown only on pairing or rotation; never retained in the registry.
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedAgent {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub revoked: bool,
}

/// Pairing state for LAN MCP Agents: one expiring one-time code plus the set of
/// paired Agents. Only token hashes are retained, and the whole registry is
/// encrypted at rest with the device key.
pub struct PairingRegistry {
    state: RegistryState,
    database: Option<std::sync::Arc<crate::storage::Database>>,
}

impl PairingRegistry {
    pub fn new() -> Self {
        Self {
            state: RegistryState::default(),
            database: None,
        }
    }

    /// Load the persisted registry. A value that cannot be decrypted or parsed
    /// is an error, never a silent reset: an empty registry would let a new
    /// pairing code mint credentials over data the administrator still believes
    /// is protected.
    pub fn load(database: std::sync::Arc<crate::storage::Database>) -> Result<Self, PairingError> {
        let stored = database
            .get_setting(REGISTRY_SETTING)
            .map_err(|error| PairingError::Corrupt(error.to_string()))?;
        let state = match stored {
            Some(value) => {
                let payload = value.strip_prefix("enc:v1:").ok_or_else(|| {
                    PairingError::Corrupt("agent registry has an unknown format".to_string())
                })?;
                let crypto = crate::storage::crypto::Crypto::device().map_err(|error| {
                    PairingError::Corrupt(format!("device key unavailable: {error}"))
                })?;
                let clear = crypto
                    .decrypt_base64(payload)
                    .map_err(|error| PairingError::Corrupt(error.to_string()))?;
                serde_json::from_slice::<RegistryState>(&clear)
                    .map_err(|error| PairingError::Corrupt(error.to_string()))?
            }
            None => RegistryState::default(),
        };
        Ok(Self {
            state,
            database: Some(database),
        })
    }

    /// Re-read the registry from storage. A long-running service calls this
    /// before pairing and before authenticating, so codes minted and
    /// credentials revoked by `zxyssh mcp` in another process take effect
    /// immediately instead of waiting for a restart.
    pub fn reload(&mut self) -> Result<(), PairingError> {
        let Some(database) = &self.database else {
            return Ok(());
        };
        let fresh = Self::load(database.clone())?;
        self.state = fresh.state;
        Ok(())
    }

    fn persist(&self) -> Result<(), PairingError> {
        let Some(database) = &self.database else {
            return Ok(());
        };
        let data = serde_json::to_vec(&self.state)
            .map_err(|error| PairingError::Storage(error.to_string()))?;
        let encrypted = crate::storage::crypto::Crypto::device()
            .and_then(|crypto| crypto.encrypt_base64(&data))
            .map_err(|error| PairingError::Storage(error.to_string()))?;
        database
            .set_setting(REGISTRY_SETTING, &format!("enc:v1:{encrypted}"))
            .map_err(|error| PairingError::Storage(error.to_string()))
    }

    /// Mint a short-lived one-time code for the next Agent to pair with. Only a
    /// local administrator on the server console should ever see the return
    /// value.
    pub fn create_code(&mut self, now: i64) -> Result<String, PairingError> {
        use rand::Rng;
        let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000u32));
        let previous = self.state.pending.replace(PendingCode {
            value: code.clone(),
            expires_at: now + PAIRING_TTL_SECONDS,
            attempts: 0,
        });
        if let Err(error) = self.persist() {
            self.state.pending = previous;
            return Err(error);
        }
        Ok(code)
    }

    pub fn pending_expires_at(&self) -> Option<i64> {
        self.state.pending.as_ref().map(|code| code.expires_at)
    }

    pub fn pair(&mut self, name: &str, code: &str, now: i64) -> Result<IssuedAgent, PairingError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(PairingError::InvalidName);
        }
        let Some(pending) = self.state.pending.as_mut() else {
            return Err(PairingError::InvalidCode);
        };
        if now >= pending.expires_at || pending.attempts >= MAX_PAIRING_ATTEMPTS {
            self.state.pending = None;
            let _ = self.persist();
            return Err(PairingError::InvalidCode);
        }
        pending.attempts += 1;
        use subtle::ConstantTimeEq;
        if !bool::from(pending.value.as_bytes().ct_eq(code.as_bytes())) {
            let exhausted = pending.attempts >= MAX_PAIRING_ATTEMPTS;
            if exhausted {
                self.state.pending = None;
            }
            let _ = self.persist();
            return Err(PairingError::InvalidCode);
        }
        self.state.pending = None;
        let token = random_token();
        let token_hash = digest_token(&token);
        let id = uuid::Uuid::new_v4().to_string();
        let previous_agents = self.state.agents.clone();
        self.state.agents.insert(
            id.clone(),
            AgentRecord {
                id: id.clone(),
                name: name.into(),
                token_hash,
                revoked: false,
            },
        );
        if let Err(error) = self.persist() {
            self.state.agents = previous_agents;
            return Err(error);
        }
        Ok(IssuedAgent {
            id,
            name: name.into(),
            token,
        })
    }

    /// Map a bearer token to its Agent, or `None` when no active Agent owns it.
    /// Every candidate hash is compared so timing does not leak which Agent
    /// matched first.
    pub fn authenticate(&self, token: &str) -> Option<AuthenticatedAgent> {
        use subtle::ConstantTimeEq;
        let candidate = digest_token(token);
        let mut matched = None;
        for agent in self.state.agents.values() {
            let is_match = bool::from(agent.token_hash.as_bytes().ct_eq(candidate.as_bytes()));
            if is_match && !agent.revoked {
                matched = Some(AuthenticatedAgent {
                    id: agent.id.clone(),
                    name: agent.name.clone(),
                });
            }
        }
        matched
    }

    pub fn revoke(&mut self, id: &str) -> Result<bool, PairingError> {
        let mut agent = match self.state.agents.get(id) {
            Some(agent) => agent.clone(),
            None => return Ok(false),
        };
        if agent.revoked {
            return Ok(false);
        }
        agent.revoked = true;
        self.apply(|state| {
            state.agents.insert(id.to_string(), agent.clone());
        })
        .map(|()| true)
    }

    pub fn rename(&mut self, id: &str, name: &str) -> Result<bool, PairingError> {
        let name = name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(PairingError::InvalidName);
        }
        let mut agent = match self.state.agents.get(id) {
            Some(agent) => agent.clone(),
            None => return Ok(false),
        };
        agent.name = name.into();
        let id = id.to_string();
        self.apply(move |state| {
            state.agents.insert(id.clone(), agent.clone());
        })
        .map(|()| true)
    }

    /// Replace an Agent's credential. The previous token stops working as soon
    /// as this returns, so the caller must display the new token exactly once.
    pub fn rotate(&mut self, id: &str) -> Result<Option<IssuedAgent>, PairingError> {
        let agent = match self.state.agents.get(id) {
            Some(agent) if !agent.revoked => agent.clone(),
            _ => return Ok(None),
        };
        let token = random_token();
        let issued = IssuedAgent {
            id: agent.id.clone(),
            name: agent.name.clone(),
            token: token.clone(),
        };
        let mut agent = agent;
        agent.token_hash = digest_token(&token);
        let id = agent.id.clone();
        self.apply(move |state| {
            state.agents.insert(id.clone(), agent.clone());
        })
        .map(|()| Some(issued))
    }

    /// Mutate the registry and persist it, rolling back the in-memory state if
    /// the write fails so memory and disk never disagree.
    fn apply(&mut self, mutate: impl FnOnce(&mut RegistryState)) -> Result<(), PairingError> {
        let previous = self.state.clone();
        mutate(&mut self.state);
        if let Err(error) = self.persist() {
            self.state = previous;
            return Err(error);
        }
        Ok(())
    }

    /// Paired Agents that can still authenticate. LAN listeners count these,
    /// so revoking every Agent leaves the port closed on next start.
    pub fn active_agent_count(&self) -> usize {
        self.state
            .agents
            .values()
            .filter(|agent| !agent.revoked)
            .count()
    }

    pub fn list(&self) -> Vec<AgentSummary> {
        let mut agents: Vec<AgentSummary> = self
            .state
            .agents
            .values()
            .map(|agent| AgentSummary {
                id: agent.id.clone(),
                name: agent.name.clone(),
                revoked: agent.revoked,
            })
            .collect();
        agents.sort_by(|left, right| left.name.cmp(&right.name));
        agents
    }
}

impl Default for PairingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    fn temp_database() -> (tempfile::TempDir, std::sync::Arc<crate::storage::Database>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let database =
            crate::storage::Database::new_at(dir.path().join("pairing-test.db")).expect("database");
        (dir, std::sync::Arc::new(database))
    }

    #[test]
    fn pairing_code_is_single_use_and_token_is_returned_once() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        let issued = registry.pair("hermes-office", &code, NOW).unwrap();
        assert_eq!(issued.name, "hermes-office");
        assert_eq!(
            registry.pair("another-agent", &code, NOW).unwrap_err(),
            PairingError::InvalidCode
        );
        assert_eq!(
            registry.authenticate(&issued.token).unwrap().name,
            "hermes-office"
        );
    }

    #[test]
    fn pairing_code_expires_and_locks_after_five_failures() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        assert_eq!(
            registry
                .pair("late-agent", &code, NOW + PAIRING_TTL_SECONDS)
                .unwrap_err(),
            PairingError::InvalidCode
        );
        let code = registry.create_code(NOW).unwrap();
        for _ in 0..MAX_PAIRING_ATTEMPTS {
            assert_eq!(
                registry.pair("bad-agent", "000000", NOW).unwrap_err(),
                PairingError::InvalidCode
            );
        }
        assert_eq!(
            registry.pair("good-agent", &code, NOW).unwrap_err(),
            PairingError::InvalidCode
        );
    }

    #[test]
    fn revoked_agent_token_stops_authenticating() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        let issued = registry.pair("revoke-me", &code, NOW).unwrap();
        assert!(registry.authenticate(&issued.token).is_some());
        assert!(registry.revoke(&issued.id).unwrap());
        assert_eq!(registry.authenticate(&issued.token), None);
        assert_eq!(registry.active_agent_count(), 0);
    }

    #[test]
    fn registry_persists_encrypted_and_survives_restart() {
        let (_dir, database) = temp_database();
        let code = {
            let mut registry = PairingRegistry::load(database.clone()).unwrap();
            let code = registry.create_code(NOW).unwrap();
            registry.pair("hermes-lab", &code, NOW).unwrap();
            code
        };
        let raw = database.get_setting(REGISTRY_SETTING).unwrap().unwrap();
        assert!(raw.starts_with("enc:v1:"));
        assert!(!raw.contains("hermes-lab"));
        assert!(!raw.contains(&code));

        let mut reloaded = PairingRegistry::load(database.clone()).unwrap();
        assert_eq!(reloaded.list().len(), 1);
        assert_eq!(reloaded.active_agent_count(), 1);
        // The consumed code is not reusable after a restart.
        assert_eq!(
            reloaded.pair("attacker", &code, NOW).unwrap_err(),
            PairingError::InvalidCode
        );
    }

    #[test]
    fn rotate_invalidates_the_previous_token() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        let issued = registry.pair("rotating", &code, NOW).unwrap();
        let rotated = registry.rotate(&issued.id).unwrap().unwrap();
        assert!(rotated.token != issued.token);
        assert_eq!(registry.authenticate(&issued.token), None);
        assert_eq!(registry.authenticate(&rotated.token).unwrap().id, issued.id);
    }

    #[test]
    fn corrupt_registry_refuses_to_load() {
        let (_dir, database) = temp_database();
        database
            .set_setting(REGISTRY_SETTING, "enc:v1:not-a-ciphertext")
            .unwrap();
        let error = match PairingRegistry::load(database) {
            Ok(_) => panic!("a corrupt registry must not load"),
            Err(error) => error,
        };
        assert!(matches!(error, PairingError::Corrupt(_)));
    }

    #[test]
    fn plaintext_registry_is_not_accepted() {
        let (_dir, database) = temp_database();
        database
            .set_setting(REGISTRY_SETTING, "{\"agents\":{}}")
            .unwrap();
        assert!(matches!(
            PairingRegistry::load(database),
            Err(PairingError::Corrupt(_))
        ));
    }
}
