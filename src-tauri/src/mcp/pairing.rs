use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Reason a pairing or administration request was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    InvalidCode,
    InvalidName,
    /// A presented key is not a usable X25519 public key. Checked before the
    /// pairing code is spent, so a malformed body cannot burn a code and then
    /// fail to deliver.
    InvalidClientKey,
    /// A record may only be deleted once its credential is revoked, so removing
    /// an entry can never silently drop a credential an Agent is still using.
    StillActive,
    /// The registry could not be written. The in-memory change is rolled back.
    Storage(String),
    /// The persisted registry could not be read or decrypted. Callers must
    /// refuse to start rather than begin with an empty, unpaired registry.
    Corrupt(String),
}

pub const PAIRING_TTL_SECONDS: i64 = 300;
pub const MAX_PAIRING_ATTEMPTS: u8 = 5;
/// How long an Agent may wait in the operator's approval queue before the
/// request is dropped and a fresh pairing code is needed.
pub const PAIR_REQUEST_TTL_SECONDS: i64 = 900;
/// Terminal or expired requests older than this are dropped from the registry so
/// it cannot grow without bound.
const PAIR_REQUEST_RETENTION_SECONDS: i64 = 24 * 3600;
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

/// Where a sealed pairing request stands. The wire names stay as they were in
/// 2.1.0 so an Agent that is already polling `/pair/status` keeps parsing them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairRequestStatus {
    /// The code was correct; the credential is being sealed for delivery.
    #[default]
    #[serde(rename = "awaiting_approval")]
    AwaitingDelivery,
    /// Sealed. The credential is waiting to be handed back exactly once.
    Approved,
    /// The credential was delivered to the Agent that requested it.
    Delivered,
    /// The Agent never polled before the request aged out.
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredPairRequest {
    id: String,
    agent_name: String,
    /// Source address of the request, for the operator to recognise the device.
    peer: String,
    created_at: i64,
    expires_at: i64,
    status: PairRequestStatus,
    /// X25519 key the credential is sealed to. Every queued request has one: a
    /// keyless client cannot read a sealed delivery, so approval mode refuses it.
    client_public_key: Option<String>,
    /// Retained until delivery so the sealed key agreement can be completed;
    /// the whole registry is encrypted at rest with the device key.
    pairing_code: String,
    agent_id: Option<String>,
    server_public_key: Option<String>,
    sealed_credential: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct RegistryState {
    agents: HashMap<String, AgentRecord>,
    pending: Option<PendingCode>,
    #[serde(default)]
    requests: HashMap<String, StoredPairRequest>,
}

fn digest_token(token: &str) -> String {
    use base64::Engine as _;
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(token.as_bytes()))
}

/// Agent names reach the approval queue as the operator's only description of the
/// device, so they are kept short and never blank.
fn valid_name(name: &str) -> Result<String, PairingError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 80 {
        return Err(PairingError::InvalidName);
    }
    Ok(name.to_string())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

/// A pairing request as the desktop UI and the CLI show it. Never carries
/// credential material.
/// Outcome of a pairing attempt that consumed a code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairStarted {
    /// Pairing completed in one step; the bearer token is in hand.
    Issued(IssuedAgent),
    /// The credential is sealed to the presented key and fetched from `/pair/status`.
    Awaiting { request_id: String },
}

/// Answer for an Agent polling its pairing request.
#[derive(Debug, Clone, Default)]
pub struct PairPoll {
    pub status: PairRequestStatus,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    /// Service half of the key agreement, for sealed delivery.
    pub server_public_key: Option<String>,
    /// Sealed credential, delivered once.
    pub sealed_credential: Option<String>,
}

/// A pairing key must decode from base64url to exactly one X25519 public key.
fn valid_client_key(public_key: &str) -> bool {
    use base64::Engine as _;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(public_key)
        .is_ok_and(|bytes| bytes.len() == 32)
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

    /// Consume the pending pairing code on a correct guess. Failure paths persist
    /// the attempt counter (and the spent code once it is exhausted) so a restart
    /// cannot hand a guesser a fresh budget; the success path leaves persistence to
    /// the caller so spending the code and recording the result is one write.
    fn consume_code(&mut self, code: &str, now: i64) -> Result<String, PairingError> {
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
            if pending.attempts >= MAX_PAIRING_ATTEMPTS {
                self.state.pending = None;
            }
            let _ = self.persist();
            return Err(PairingError::InvalidCode);
        }
        Ok(self
            .state
            .pending
            .take()
            .expect("pending code was present")
            .value)
    }

    pub fn pair(&mut self, name: &str, code: &str, now: i64) -> Result<IssuedAgent, PairingError> {
        let name = valid_name(name)?;
        let previous = self.state.clone();
        let _ = self.consume_code(code, now)?;
        let token = random_token();
        let token_hash = digest_token(&token);
        let id = uuid::Uuid::new_v4().to_string();
        let agent = AgentRecord {
            id: id.clone(),
            name: name.clone(),
            token_hash,
            revoked: false,
        };
        self.state.agents.insert(id.clone(), agent);
        if let Err(error) = self.persist() {
            self.state = previous;
            return Err(error);
        }
        Ok(IssuedAgent { id, name, token })
    }

    /// Spend a code. Without `client_public_key` the credential is issued in the
    /// response; with one it is sealed to that key and fetched from
    /// `/pair/status`, so a listener on the segment cannot read it.
    pub fn begin_pairing(
        &mut self,
        name: &str,
        code: &str,
        client_public_key: Option<&str>,
        peer: &str,
        now: i64,
    ) -> Result<PairStarted, PairingError> {
        let name = valid_name(name)?;
        if let Some(public_key) = client_public_key {
            if !valid_client_key(public_key) {
                return Err(PairingError::InvalidClientKey);
            }
        }
        if client_public_key.is_none() {
            return self.pair(&name, code, now).map(PairStarted::Issued);
        }
        let previous = self.state.clone();
        let pairing_code = self.consume_code(code, now)?;
        let request_id = uuid::Uuid::new_v4().to_string();
        self.state.requests.insert(
            request_id.clone(),
            StoredPairRequest {
                id: request_id.clone(),
                agent_name: name.clone(),
                peer: peer.to_string(),
                created_at: now,
                expires_at: now + PAIR_REQUEST_TTL_SECONDS,
                status: PairRequestStatus::AwaitingDelivery,
                client_public_key: client_public_key.map(str::to_string),
                pairing_code,
                agent_id: None,
                server_public_key: None,
                sealed_credential: None,
            },
        );
        self.prune_requests(now);
        if let Err(error) = self.persist() {
            self.state = previous;
            return Err(error);
        }
        // Nothing waits for a decision: the credential is sealed right away and
        // the Agent picks it up by polling `/pair/status`.
        self.finalize_request(&request_id, now)?;
        Ok(PairStarted::Awaiting { request_id })
    }

    /// Seal and release the credential for a request. The Agent record and the
    /// request transition are written together, so a failed write cannot leave a
    /// live credential with no owner or a finished request with none.
    fn finalize_request(
        &mut self,
        id: &str,
        now: i64,
    ) -> Result<Option<AgentSummary>, PairingError> {
        let Some(request) = self.state.requests.get(id) else {
            return Ok(None);
        };
        if request.status != PairRequestStatus::AwaitingDelivery {
            let summary = request.agent_id.as_ref().and_then(|agent_id| {
                self.state.agents.get(agent_id).map(|agent| AgentSummary {
                    id: agent.id.clone(),
                    name: agent.name.clone(),
                    revoked: agent.revoked,
                })
            });
            return Ok(summary);
        }
        if now >= request.expires_at {
            self.expire_request(id)?;
            return Ok(None);
        }
        let previous = self.state.clone();
        let token = random_token();
        let agent_id = uuid::Uuid::new_v4().to_string();
        let agent = AgentRecord {
            id: agent_id.clone(),
            name: request.agent_name.clone(),
            token_hash: digest_token(&token),
            revoked: false,
        };
        let summary = AgentSummary {
            id: agent_id.clone(),
            name: agent.name.clone(),
            revoked: false,
        };
        let client_public_key = request.client_public_key.clone();
        self.state.agents.insert(agent_id.clone(), agent);
        if let Some(entry) = self.state.requests.get_mut(id) {
            entry.status = PairRequestStatus::Approved;
            entry.agent_id = Some(agent_id);
            entry.pairing_code = String::new();
            {
                let key_pair = client_public_key.as_deref().ok_or_else(|| {
                    PairingError::Storage("queued request lost its client key".to_string())
                })?;
                let envelope = super::pair_crypto::seal_for_agent(
                    super::pair_crypto::ephemeral_private_key()
                        .map_err(|error| PairingError::Storage(error.to_string()))?,
                    key_pair,
                    id,
                    &token,
                )
                .map_err(|error| PairingError::Storage(error.to_string()))?;
                entry.server_public_key = Some(envelope.public_key);
                entry.sealed_credential = Some(envelope.sealed);
            }
        }
        if let Err(error) = self.persist() {
            self.state = previous;
            return Err(error);
        }
        Ok(Some(summary))
    }

    /// Read one step of a pairing request. An approved credential is released
    /// exactly once: the request moves to `Delivered` and keeps no material.
    pub fn poll_request(&mut self, id: &str, now: i64) -> Result<Option<PairPoll>, PairingError> {
        let Some(request) = self.state.requests.get(id) else {
            return Ok(None);
        };
        if request.status == PairRequestStatus::AwaitingDelivery && now >= request.expires_at {
            self.expire_request(id)?;
            return Ok(Some(PairPoll {
                status: PairRequestStatus::Expired,
                ..PairPoll::default()
            }));
        }
        if request.status != PairRequestStatus::Approved {
            return Ok(Some(PairPoll {
                status: request.status,
                ..PairPoll::default()
            }));
        }
        if request.sealed_credential.is_none() || request.server_public_key.is_none() {
            return Err(PairingError::Storage(
                "approved pairing request carries no sealed credential".to_string(),
            ));
        }
        let poll = PairPoll {
            status: PairRequestStatus::Delivered,
            agent_id: request.agent_id.clone(),
            agent_name: Some(request.agent_name.clone()),
            server_public_key: request.server_public_key.clone(),
            sealed_credential: request.sealed_credential.clone(),
        };
        self.apply(|state| {
            if let Some(entry) = state.requests.get_mut(id) {
                entry.status = PairRequestStatus::Delivered;
                entry.server_public_key = None;
                entry.sealed_credential = None;
            }
        })?;
        Ok(Some(poll))
    }

    fn expire_request(&mut self, id: &str) -> Result<(), PairingError> {
        self.apply(|state| {
            if let Some(entry) = state.requests.get_mut(id) {
                entry.status = PairRequestStatus::Expired;
                entry.pairing_code = String::new();
                entry.sealed_credential = None;
            }
        })
    }

    /// Drop requests that are terminal, and any that expired long enough ago that
    /// no client can still be polling for it.
    fn prune_requests(&mut self, now: i64) {
        let mut abandoned = Vec::new();
        self.state.requests.retain(|_, request| {
            if now - request.expires_at < PAIR_REQUEST_RETENTION_SECONDS {
                return true;
            }
            // Approved but never fetched: the credential was hashed and issued, and
            // no Agent will ever hold it. Drop the record rather than leave it live.
            if request.status == PairRequestStatus::Approved {
                if let Some(agent_id) = &request.agent_id {
                    abandoned.push(agent_id.clone());
                }
            }
            false
        });
        for agent_id in abandoned {
            self.state.agents.remove(&agent_id);
        }
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

    /// Delete a revoked Agent's record so the operator's list does not grow
    /// without bound. An active credential must be revoked first: deleting a
    /// live record would silently drop a credential its holder is still using
    /// while leaving its history behind.
    pub fn forget(&mut self, id: &str) -> Result<bool, PairingError> {
        match self.state.agents.get(id) {
            None => Ok(false),
            Some(agent) if !agent.revoked => Err(PairingError::StillActive),
            Some(_) => self
                .apply(|state| {
                    state.agents.remove(id);
                })
                .map(|()| true),
        }
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

    /// A well-formed client key, since the approval queue only accepts sealed
    /// delivery. The tests never need to open the envelope.
    fn client_key() -> String {
        let key_pair = super::super::pair_crypto::ephemeral_private_key().unwrap();
        super::super::pair_crypto::public_key_b64(&key_pair).unwrap()
    }

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
    fn a_malformed_client_key_is_refused_without_spending_the_code() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        for bogus in ["not-base64!!", "", "c2hvcnQ="] {
            assert_eq!(
                registry
                    .begin_pairing("hermes", &code, Some(bogus), "peer", NOW)
                    .unwrap_err(),
                PairingError::InvalidClientKey
            );
        }
        // Nothing was spent: the same code still opens a request with a real key.
        assert!(registry
            .begin_pairing("hermes", &code, Some(&client_key()), "peer", NOW)
            .is_ok());
    }

    #[test]
    fn a_spent_code_cannot_open_a_second_request() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        registry
            .begin_pairing("first", &code, Some(&client_key()), "peer", NOW)
            .unwrap();
        assert_eq!(
            registry
                .begin_pairing("second", &code, Some(&client_key()), "peer", NOW)
                .unwrap_err(),
            PairingError::InvalidCode
        );
        assert_eq!(registry.active_agent_count(), 1);
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
    fn only_a_revoked_agent_can_be_forgotten() {
        let mut registry = PairingRegistry::new();
        let code = registry.create_code(NOW).unwrap();
        let issued = registry.pair("forget-me", &code, NOW).unwrap();
        assert!(matches!(
            registry.forget(&issued.id),
            Err(PairingError::StillActive)
        ));
        assert!(registry.list().iter().any(|agent| agent.id == issued.id));
        registry.revoke(&issued.id).unwrap();
        assert!(registry.forget(&issued.id).unwrap());
        assert!(registry.list().is_empty());
        assert!(!registry.forget(&issued.id).unwrap());
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
