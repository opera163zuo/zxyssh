//! `zxyssh mcp` — run zxySSH's MCP endpoint as a network service and
//! administer the Agents that pair with it.
//!
//! The service owns the SSH/SFTP work: a remote Agent only speaks MCP HTTP and
//! never needs a local zxySSH, SSH client, or copy of the credential store.

use anyhow::{bail, Context, Result};
use axum::Router;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use zxyssh_core::mcp::pairing::{AgentSummary, IssuedAgent, PairRequestSummary, PairingRegistry};
use zxyssh_core::mcp::McpServer;
use zxyssh_core::{Database, SessionManager};

const DEFAULT_BIND: &str = "127.0.0.1:8765";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceConfig {
    pub bind: SocketAddr,
    /// Non-loopback listeners are refused unless the operator says so explicitly.
    pub allow_lan: bool,
}

impl ServiceConfig {
    pub fn from_env() -> Result<Self> {
        let bind = std::env::var("ZXYSSH_MCP_BIND")
            .unwrap_or_else(|_| DEFAULT_BIND.to_string())
            .parse::<SocketAddr>()
            .context("ZXYSSH_MCP_BIND must be a socket address such as 127.0.0.1:8765")?;
        let allow_lan = match std::env::var("ZXYSSH_MCP_ALLOW_LAN")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "0" | "false" | "no" => false,
            "1" | "true" | "yes" => true,
            other => bail!("ZXYSSH_MCP_ALLOW_LAN must be true or false, got {other}"),
        };
        Ok(Self { bind, allow_lan })
    }
}

/// Decide whether a listener address may be opened. Loopback is always allowed;
/// reaching beyond it needs both an explicit opt-in and at least one Agent that
/// can authenticate, so a freshly installed service never boots an open port.
pub fn validate_listener(bind: SocketAddr, active_agents: usize, allow_lan: bool) -> Result<()> {
    if bind.ip().is_loopback() {
        return Ok(());
    }
    if !allow_lan {
        bail!(
            "Refusing to bind {bind} without ZXYSSH_MCP_ALLOW_LAN=true; \
             loopback (127.0.0.1) is the default"
        );
    }
    if bind.ip() == IpAddr::from([0, 0, 0, 0]) || bind.ip().is_unspecified() {
        bail!("Bind a specific private interface address instead of {bind}");
    }
    if active_agents == 0 {
        bail!("Refusing a non-loopback MCP listener until at least one Agent has been paired");
    }
    Ok(())
}

fn registry(database: Arc<Database>) -> Result<Arc<std::sync::Mutex<PairingRegistry>>> {
    let registry = PairingRegistry::load(database)
        .map_err(|error| anyhow::anyhow!("Paired Agent registry is unusable: {error:?}"))?;
    Ok(Arc::new(std::sync::Mutex::new(registry)))
}

pub fn build_router(
    database: Arc<Database>,
    session_manager: Arc<SessionManager>,
    registry: Arc<std::sync::Mutex<PairingRegistry>>,
) -> Router {
    McpServer::new(database, session_manager)
        .with_pairing_registry(registry)
        .router_with_pairing()
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}

fn open_database() -> Result<Arc<Database>> {
    Ok(Arc::new(
        Database::new().context("Failed to open the zxySSH database")?,
    ))
}

pub fn serve() -> Result<()> {
    let config = ServiceConfig::from_env()?;
    let database = open_database()?;
    let registry = registry(database.clone())?;
    let active_agents = registry
        .lock()
        .map_err(|_| anyhow::anyhow!("pairing registry lock poisoned"))?
        .active_agent_count();
    validate_listener(config.bind, active_agents, config.allow_lan)?;

    let status = zxyssh_core::mcp::lan_status::LanListenerStatus {
        bind: config.bind.to_string(),
        allow_lan: config.allow_lan,
        credential_sealing: true,
        pid: std::process::id(),
        started_at: unix_now(),
        active_agents,
    };
    // A missing status file only costs the desktop a "not running" read, so this
    // never blocks the listener.
    if let Err(error) = zxyssh_core::mcp::lan_status::write_status(&status) {
        eprintln!("Warning: could not publish the listener status: {error}");
    }

    let session_manager = Arc::new(SessionManager::new(database.clone()));
    let app = build_router(database, session_manager, registry);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to create the MCP service runtime")?;
    let result = runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(config.bind)
            .await
            .with_context(|| format!("Failed to bind {}", config.bind))?;
        println!("zxySSH MCP service listening on http://{}/mcp", config.bind);
        println!(
            "Pair an Agent with `zxyssh mcp pair-code`; {} Agent(s) paired.",
            active_agents
        );
        println!("Risky commands fail closed: this transport has no human approval UI.");
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("MCP service stopped with an error")
    });
    zxyssh_core::mcp::lan_status::clear_status();
    result
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install the SIGINT handler");
    };
    #[cfg(unix)]
    let terminate = async {
        let mut signals = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install the SIGTERM handler");
        signals.recv().await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    println!("Shutdown requested, closing the MCP service");
}

/// `zxyssh mcp …`
#[derive(clap::Subcommand)]
pub enum McpCommand {
    /// Run the MCP HTTP service in the foreground (systemd Type=simple)
    Serve,
    /// Mint a one-time pairing code for the next Agent
    PairCode,
    /// List paired Agents and their revocation state
    Agents,
    /// Revoke an Agent credential; its token is rejected on the next request
    Revoke {
        /// Agent id or unambiguous id prefix
        agent: String,
    },
    /// Rename an Agent record
    Rename {
        /// Agent id or unambiguous id prefix
        agent: String,
        /// New display name
        name: String,
    },
    /// Replace an Agent credential and print the new token once
    Rotate {
        /// Agent id or unambiguous id prefix
        agent: String,
    },
    /// Trust, probe and list SSH host keys (headless TOFU)
    Hostkey {
        #[command(subcommand)]
        command: crate::mcp_hostkey::HostkeyCommand,
    },
    /// Save the credential a saved server profile uses, from piped stdin
    Credential {
        #[command(subcommand)]
        command: CredentialCommand,
    },
    /// Pair this machine with a running listener
    Pair {
        /// Listener URL; defaults to the address published by the running service
        #[arg(long)]
        endpoint: Option<String>,
        /// Name this Agent reports to the service
        #[arg(long)]
        name: String,
        /// Seconds to wait for the sealed credential to be delivered
        #[arg(long, default_value_t = 900)]
        wait: u64,
        /// Ask the operator to approve this pairing instead of spending a code,
        /// so no secret has to pass through the Agent's own chat log
        #[arg(long)]
        no_code: bool,
    },
    /// List and decide the codeless pairing requests waiting on an operator
    Requests {
        #[command(subcommand)]
        command: RequestsCommand,
    },
}

/// `zxyssh mcp credential …` — headless provisioning for the profiles Agents
/// connect with. Secrets come from stdin and never appear in arguments, logs or
/// shell history; they are stored encrypted with the device key.
#[derive(clap::Subcommand)]
pub enum CredentialCommand {
    /// Store or replace a saved server's credential
    Set {
        /// Saved server name, as it appears in `server_list`
        server: String,
        #[arg(long, default_value = "password")]
        auth_type: String,
        /// Private-key file on this host instead of pasting the key
        #[arg(long)]
        key_path: Option<String>,
    },
    /// Forget a saved server's credential
    Clear { server: String },
}

/// `zxyssh mcp requests …` — decide the pairings that arrived with no code.
/// This is the headless equivalent of the desktop's approve button.
#[derive(clap::Subcommand)]
pub enum RequestsCommand {
    /// List the requests waiting for a decision
    List,
    /// Approve one: its credential is created now and delivered to the Agent
    Approve {
        /// Request id or unambiguous id prefix
        request: String,
    },
    /// Refuse one: nothing is ever issued for it
    Reject {
        /// Request id or unambiguous id prefix
        request: String,
    },
}

pub fn run(command: McpCommand) -> Result<()> {
    match command {
        McpCommand::Serve => serve(),
        McpCommand::Hostkey { command } => crate::mcp_hostkey::run(command),
        McpCommand::Credential { command } => credential(command),
        McpCommand::PairCode => {
            let code = pair_code()?;
            println!("One-time pairing code: {code}");
            println!(
                "Valid for {} seconds, usable once. The Agent posts it to /pair.",
                zxyssh_core::mcp::pairing::PAIRING_TTL_SECONDS
            );
            println!("This code is not written to any log or file.");
            Ok(())
        }
        McpCommand::Agents => {
            let agents = list_agents()?;
            if agents.is_empty() {
                println!("No Agents are paired yet. Run `zxyssh mcp pair-code`.");
                return Ok(());
            }
            for agent in agents {
                let status = if agent.revoked { "revoked" } else { "active" };
                println!("{}  {status:<8}  {}", agent.id, agent.name);
            }
            Ok(())
        }
        McpCommand::Revoke { agent } => {
            if revoke(&agent)? {
                println!("Revoked. Requests with that credential are now rejected.");
                Ok(())
            } else {
                bail!("That Agent is already revoked")
            }
        }
        McpCommand::Rename { agent, name } => {
            if rename(&agent, &name)? {
                println!("Renamed to {name}.");
                Ok(())
            } else {
                bail!("Rename failed")
            }
        }
        McpCommand::Pair {
            endpoint,
            name,
            wait,
            no_code,
        } => {
            let discovered = if endpoint.is_none() {
                zxyssh_core::mcp::lan_status::read_status()
                    .map(|status| format!("http://{}", status.bind))
            } else {
                None
            };
            let endpoint = resolve_pair_endpoint(endpoint, discovered);
            if no_code {
                pair_by_request(&endpoint, &name, wait)
            } else {
                pair_against_endpoint(&endpoint, &name, wait)
            }
        }
        McpCommand::Requests { command } => requests(command),
        McpCommand::Rotate { agent } => match rotate(&agent)? {
            Some(issued) => {
                println!("Agent {} was issued a new credential:", issued.name);
                println!("{}", issued.token);
                println!("Shown once. The service stores only its hash.");
                Ok(())
            }
            None => bail!("Nothing to rotate: that Agent is unknown or revoked"),
        },
    }
}

fn read_secret_from_stdin() -> Result<String> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        bail!("Pipe the secret into stdin; an interactive prompt could echo it");
    }
    let mut data = Vec::new();
    std::io::stdin()
        .take(1_048_577)
        .read_to_end(&mut data)
        .context("Could not read the secret from stdin")?;
    let text = String::from_utf8(data).context("The secret must be UTF-8")?;
    Ok(text.trim_end_matches(['\r', '\n']).to_string())
}

fn credential(command: CredentialCommand) -> Result<()> {
    match command {
        CredentialCommand::Set {
            server,
            auth_type,
            key_path,
        } => {
            let database = open_database()?;
            if database.server_get_by_name(&server)?.is_none() {
                bail!("No saved server is named {server}; create it first with the MCP server_add tool or the desktop app");
            }
            let secret = read_secret_from_stdin()?;
            let (auth_type, credential, passphrase, stored_path) = match auth_type.as_str() {
                "password" => {
                    if key_path.is_some() {
                        bail!("--key-path does not apply to password authentication")
                    }
                    if secret.is_empty() {
                        bail!("The password read from stdin was empty")
                    }
                    ("password".to_string(), secret, None, None)
                }
                "key" | "key_with_passphrase" => match key_path {
                    Some(path) => {
                        if !std::path::Path::new(&path).exists() {
                            bail!("Key file {path} does not exist on this host")
                        }
                        let auth = if secret.is_empty() {
                            "key"
                        } else {
                            "key_with_passphrase"
                        };
                        (
                            auth.to_string(),
                            String::new(),
                            (!secret.is_empty()).then_some(secret),
                            Some(path),
                        )
                    }
                    None => {
                        if secret.is_empty() {
                            bail!("Paste the private key into stdin, or pass --key-path")
                        }
                        ("key".to_string(), secret, None, None)
                    }
                },
                other => {
                    bail!("auth_type must be password, key or key_with_passphrase, got {other}")
                }
            };
            database.credential_save(
                &server,
                &auth_type,
                &credential,
                passphrase.as_deref(),
                stored_path.as_deref(),
            )?;
            println!("Saved a {auth_type} credential for {server}.");
            println!("It is encrypted with this host's device key and is never echoed back.");
            Ok(())
        }
        CredentialCommand::Clear { server } => {
            open_database()?
                .credential_delete(&server)
                .map_err(|error| anyhow::anyhow!("Could not delete the credential: {error}"))?;
            println!("Removed the credential for {server}.");
            Ok(())
        }
    }
}

fn lock<'a>(
    holder: &'a std::sync::Mutex<PairingRegistry>,
) -> Result<std::sync::MutexGuard<'a, PairingRegistry>> {
    holder
        .lock()
        .map_err(|_| anyhow::anyhow!("pairing registry lock poisoned"))
}

/// Resolve a full Agent id or an unambiguous prefix; UUIDs are hostile to type
/// on a server console.
fn resolve_id(agents: &[AgentSummary], reference: &str) -> Result<String> {
    let matches: Vec<&AgentSummary> = agents
        .iter()
        .filter(|agent| agent.id.starts_with(reference))
        .collect();
    match matches.len() {
        1 => Ok(matches[0].id.clone()),
        0 => bail!("No paired Agent id starts with {reference}"),
        count => bail!("{count} Agents match {reference}; use a longer id prefix"),
    }
}

/// Resolve a full request id or an unambiguous prefix of one that is waiting.
fn resolve_request(requests: &[PairRequestSummary], reference: &str) -> Result<String> {
    let matches: Vec<&PairRequestSummary> = requests
        .iter()
        .filter(|request| request.id.starts_with(reference))
        .collect();
    match matches.len() {
        1 => Ok(matches[0].id.clone()),
        0 => bail!("No waiting pairing request id starts with {reference}"),
        count => bail!("{count} requests match {reference}; use a longer id prefix"),
    }
}

/// List and decide the pairings that arrived with no code. Approving is the
/// moment a credential comes into existence, so a request that is never
/// answered leaves nothing behind to revoke.
fn requests(command: RequestsCommand) -> Result<()> {
    let holder = registry(open_database()?)?;
    let mut registry = lock(&holder)?;
    match command {
        RequestsCommand::List => {
            let waiting = registry.pending_requests();
            if waiting.is_empty() {
                println!("No pairing requests are waiting.");
                return Ok(());
            }
            for request in waiting {
                println!(
                    "{}  {:<24}  {}",
                    request.id, request.peer, request.agent_name
                );
            }
            Ok(())
        }
        RequestsCommand::Approve { request } => {
            let id = resolve_request(&registry.pending_requests(), &request)?;
            let now = unix_now();
            match registry
                .approve_request(&id, now)
                .map_err(|error| anyhow::anyhow!("Could not approve the request: {error:?}"))?
            {
                Some(approval) => {
                    println!(
                        "Approved {} (agent id {}).",
                        approval.agent.name, approval.agent.id
                    );
                    if let Some(token) = approval.token {
                        println!("Credential for the operator to place in the Agent's managed secret store:");
                        println!("{token}");
                        println!("Shown once. Do not paste it into chat or logs.");
                    } else {
                        println!("Credential was sealed to the Agent's key; it can fetch it now.");
                    }
                    Ok(())
                }
                None => bail!("That request is no longer waiting for a decision"),
            }
        }
        RequestsCommand::Reject { request } => {
            let id = resolve_request(&registry.pending_requests(), &request)?;
            let now = unix_now();
            if registry
                .reject_request(&id, now)
                .map_err(|error| anyhow::anyhow!("Could not refuse the request: {error:?}"))?
            {
                println!("Refused. Nothing was issued.");
                Ok(())
            } else {
                bail!("That request is no longer waiting for a decision")
            }
        }
    }
}

/// Mint a one-time code for the next Agent. Safe to run while the service is
/// already up: the service re-reads the registry before it consumes a code.
pub fn pair_code() -> Result<String> {
    let holder = registry(open_database()?)?;
    let mut registry = lock(&holder)?;
    registry
        .create_code(unix_now())
        .map_err(|error| anyhow::anyhow!("Could not store the pairing code: {error:?}"))
}

pub fn list_agents() -> Result<Vec<AgentSummary>> {
    let holder = registry(open_database()?)?;
    let registry = lock(&holder)?;
    Ok(registry.list())
}

pub fn revoke(reference: &str) -> Result<bool> {
    let holder = registry(open_database()?)?;
    let mut registry = lock(&holder)?;
    let id = resolve_id(&registry.list(), reference)?;
    registry
        .revoke(&id)
        .map_err(|error| anyhow::anyhow!("Could not persist the revocation: {error:?}"))
}

pub fn rename(reference: &str, name: &str) -> Result<bool> {
    let holder = registry(open_database()?)?;
    let mut registry = lock(&holder)?;
    let id = resolve_id(&registry.list(), reference)?;
    registry
        .rename(&id, name)
        .map_err(|error| anyhow::anyhow!("Could not persist the rename: {error:?}"))
}

/// Issue a replacement credential. The old token stops working for the next
/// request the service handles.
pub fn rotate(reference: &str) -> Result<Option<IssuedAgent>> {
    let holder = registry(open_database()?)?;
    let mut registry = lock(&holder)?;
    let id = resolve_id(&registry.list(), reference)?;
    registry
        .rotate(&id)
        .map_err(|error| anyhow::anyhow!("Could not persist the new credential: {error:?}"))
}

fn resolve_pair_endpoint(endpoint: Option<String>, discovered: Option<String>) -> String {
    endpoint
        .or(discovered)
        .unwrap_or_else(|| format!("http://{DEFAULT_BIND}"))
}

/// Become an Agent of a running listener. The pairing code is read from stdin so
/// it stays out of arguments, shell history and logs.
fn pair_against_endpoint(endpoint: &str, name: &str, wait: u64) -> Result<()> {
    let code = read_secret_from_stdin()?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to create the pairing runtime")?;
    let paired = runtime.block_on(zxyssh_core::mcp::pair_client::pair(
        endpoint,
        name,
        code.trim(),
        std::time::Duration::from_secs(wait),
        zxyssh_core::mcp::pair_client::DEFAULT_POLL_INTERVAL,
    ))?;
    print_paired(&paired);
    Ok(())
}

/// Ask for a pairing with no code in hand. The operator decides in the desktop
/// or via `zxyssh mcp requests approve`, and the credential arrives sealed to a
/// key generated here — so nothing secret passes through the Agent's chat log.
fn pair_by_request(endpoint: &str, name: &str, wait: u64) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("Failed to create the pairing runtime")?;
    println!("Asking {endpoint} to pair as {name}; waiting for an operator to approve it…");
    let paired = runtime.block_on(zxyssh_core::mcp::pair_client::request_pairing(
        endpoint,
        name,
        std::time::Duration::from_secs(wait),
        zxyssh_core::mcp::pair_client::DEFAULT_POLL_INTERVAL,
    ))?;
    print_paired(&paired);
    Ok(())
}

fn print_paired(paired: &zxyssh_core::mcp::pair_client::PairedAgent) {
    println!("Paired as {} (agent id {})", paired.name, paired.agent_id);
    println!(
        "Credential{}: {}",
        if paired.sealed {
            " arrived sealed"
        } else {
            " arrived in the clear"
        },
        paired.token
    );
    println!("Shown once. Store it now; the service keeps only its hash.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn pair_endpoint_prefers_explicit_override() {
        assert_eq!(
            resolve_pair_endpoint(
                Some("http://example.test:9000".to_string()),
                Some("http://192.168.0.96:8765".to_string()),
            ),
            "http://example.test:9000"
        );
    }

    #[test]
    fn pair_endpoint_discovers_the_live_listener() {
        assert_eq!(
            resolve_pair_endpoint(None, Some("http://192.168.0.96:8765".to_string())),
            "http://192.168.0.96:8765"
        );
    }

    #[test]
    fn pair_endpoint_falls_back_to_loopback_without_a_listener_status() {
        assert_eq!(resolve_pair_endpoint(None, None), "http://127.0.0.1:8765");
    }

    #[test]
    fn loopback_always_opens_without_pairing() {
        assert!(validate_listener(SocketAddr::from((Ipv4Addr::LOCALHOST, 8765)), 0, false).is_ok());
        assert!(validate_listener(SocketAddr::from((Ipv6Addr::LOCALHOST, 8765)), 0, false).is_ok());
    }

    #[test]
    fn rejects_wildcard_bind_even_when_opted_in() {
        let wildcard = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8765));
        let error = validate_listener(wildcard, 1, true).unwrap_err();
        assert!(error.to_string().contains("specific private interface"));
    }

    #[test]
    fn refuses_lan_listener_without_explicit_opt_in() {
        let lan = "192.168.1.20:8765".parse().unwrap();
        let error = validate_listener(lan, 1, false).unwrap_err();
        assert!(error.to_string().contains("ZXYSSH_MCP_ALLOW_LAN"));
    }

    #[test]
    fn refuses_lan_listener_until_an_agent_is_paired() {
        let lan = "192.168.1.20:8765".parse().unwrap();
        let error = validate_listener(lan, 0, true).unwrap_err();
        assert!(error.to_string().contains("at least one Agent"));
        assert!(validate_listener(lan, 1, true).is_ok());
    }

    #[test]
    fn parses_the_bind_environment() {
        // `from_env` reads process state, so assert only on the parser pieces.
        let parsed: SocketAddr = "0.0.0.0:9999".parse().unwrap();
        assert_eq!(parsed.port(), 9999);
        let invalid = "192.168.1.20".parse::<SocketAddr>();
        assert!(invalid.is_err());
    }
}
