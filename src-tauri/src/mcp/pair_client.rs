//! Reference pairing client.
//!
//! An Agent that can run this binary exchanges a pairing code for a bearer
//! credential and opens the sealed delivery itself, so the credential never
//! appears on the wire in the clear. Clients that implement the protocol
//! directly get the same guarantee; clients that send no key fall back to the
//! cleartext delivery the service has always offered.
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use super::pair_crypto::{self, public_key_b64};

/// A credential obtained by pairing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedAgent {
    pub agent_id: String,
    pub name: String,
    pub token: String,
    /// Whether the credential arrived sealed to this client's ephemeral key.
    pub sealed: bool,
}

/// How long to wait for an operator to approve a pairing request, and how often
/// to ask. Approval is a human action, so the default budget is the request's
/// own lifetime rather than a few seconds.
pub const DEFAULT_APPROVAL_WAIT: Duration = Duration::from_secs(900);
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

fn field(body: &Value, key: &str) -> Option<String> {
    body.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Pair with a zxySSH MCP listener at `endpoint` (for example
/// `http://192.168.0.96:8765`) using a code minted by the operator.
pub async fn pair(
    endpoint: &str,
    name: &str,
    code: &str,
    wait: Duration,
    poll: Duration,
) -> Result<PairedAgent> {
    let endpoint = endpoint.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build pairing client")?;
    let key_pair = pair_crypto::ephemeral_private_key()?;

    let response = client
        .post(format!("{endpoint}/pair"))
        .json(&json!({
            "name": name,
            "code": code,
            "clientPublicKey": public_key_b64(&key_pair)?,
        }))
        .send()
        .await
        .context("pairing request failed")?;
    let status = response.status();
    // Pairing failures come back as plain text, so read the body before asking
    // for JSON; an unreadable-shape error would hide the service's own reason.
    let body_text = response.text().await.context("read pairing response")?;
    let status_label = status.as_u16();
    if status != reqwest::StatusCode::OK && status != reqwest::StatusCode::ACCEPTED {
        return Err(anyhow!(
            "pairing was rejected with HTTP {status_label}: {}",
            body_text.trim()
        ));
    }
    let body: Value = serde_json::from_str(&body_text)
        .with_context(|| format!("pairing returned HTTP {status_label} in an unreadable shape"))?;
    if status == reqwest::StatusCode::OK {
        // An older listener issued the credential immediately.
        let token =
            field(&body, "token").ok_or_else(|| anyhow!("pairing response had no token"))?;
        return Ok(PairedAgent {
            agent_id: field(&body, "agentId").unwrap_or_default(),
            name: field(&body, "name").unwrap_or_else(|| name.to_string()),
            token,
            sealed: false,
        });
    }
    let request_id =
        field(&body, "requestId").ok_or_else(|| anyhow!("pairing response had no requestId"))?;
    await_delivery(&client, endpoint, &request_id, key_pair, name, wait, poll).await
}

/// Ask a listener to pair this machine with no code in hand. Nothing exists for
/// this Agent until an operator approves the request, so the caller waits —
/// which is the point: no secret has to travel through the Agent's own chat log
/// for the pairing to happen.
pub async fn request_pairing(
    endpoint: &str,
    name: &str,
    wait: Duration,
    poll: Duration,
) -> Result<PairedAgent> {
    let endpoint = endpoint.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build pairing client")?;
    let key_pair = pair_crypto::ephemeral_private_key()?;

    let response = client
        .post(format!("{endpoint}/pair/request"))
        .json(&json!({ "name": name, "clientPublicKey": public_key_b64(&key_pair)? }))
        .send()
        .await
        .context("pairing request failed")?;
    let status = response.status();
    let body_text = response.text().await.context("read pairing response")?;
    if status != reqwest::StatusCode::ACCEPTED {
        return Err(anyhow!(
            "pairing request was refused with HTTP {}: {}",
            status.as_u16(),
            body_text.trim()
        ));
    }
    let body: Value = serde_json::from_str(&body_text).with_context(|| {
        format!(
            "pairing request returned HTTP {} in an unreadable shape",
            status.as_u16()
        )
    })?;
    let request_id = field(&body, "requestId")
        .ok_or_else(|| anyhow!("pairing request response had no requestId"))?;
    await_delivery(&client, endpoint, &request_id, key_pair, name, wait, poll).await
}

/// Poll until the service hands over the credential, exactly once.
#[allow(clippy::too_many_arguments)]
async fn await_delivery(
    client: &reqwest::Client,
    endpoint: &str,
    request_id: &str,
    key_pair: ring::agreement::EphemeralPrivateKey,
    name: &str,
    wait: Duration,
    poll: Duration,
) -> Result<PairedAgent> {
    let deadline = std::time::Instant::now() + wait;
    loop {
        if std::time::Instant::now() >= deadline {
            return Err(anyhow!(
                "no operator approved pairing request {request_id} in time"
            ));
        }
        tokio::time::sleep(poll).await;
        let response = client
            .post(format!("{endpoint}/pair/status"))
            .json(&json!({ "requestId": request_id }))
            .send()
            .await
            .context("pairing status poll failed")?;
        let status = response.status();
        let body_text = response
            .text()
            .await
            .context("read pairing status response")?;
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(anyhow!("pairing request {request_id} is no longer known"));
        }
        if status != reqwest::StatusCode::OK {
            return Err(anyhow!(
                "pairing status returned HTTP {}: {}",
                status.as_u16(),
                body_text.trim()
            ));
        }
        let body: Value = serde_json::from_str(&body_text).with_context(|| {
            format!(
                "pairing status returned HTTP {} in an unreadable shape",
                status.as_u16()
            )
        })?;
        match field(&body, "status").as_deref() {
            Some("delivered") => {
                let token = match (field(&body, "sealedToken"), field(&body, "serverPublicKey")) {
                    (Some(sealed), Some(server_public_key)) => {
                        pair_crypto::open_sealed(key_pair, &server_public_key, request_id, &sealed)?
                    }
                    _ => field(&body, "token")
                        .ok_or_else(|| anyhow!("approved pairing carried no credential"))?,
                };
                return Ok(PairedAgent {
                    agent_id: field(&body, "agentId").unwrap_or_default(),
                    name: field(&body, "name").unwrap_or_else(|| name.to_string()),
                    sealed: body.get("sealedToken").is_some(),
                    token,
                });
            }
            Some("refused") | Some("rejected") => {
                return Err(anyhow!("an operator refused this pairing request"))
            }
            Some("expired") => {
                return Err(anyhow!(
                    "the pairing request expired before an operator approved it"
                ))
            }
            _ => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_credential_reports_whether_it_was_sealed() {
        let paired = PairedAgent {
            agent_id: "a1".to_string(),
            name: "hermes".to_string(),
            token: "vsh_x".to_string(),
            sealed: true,
        };
        assert!(paired.sealed);
        assert_eq!(paired.token, "vsh_x");
    }

    #[test]
    fn field_extracts_only_strings() {
        let body = json!({ "token": "vsh_x", "count": 2 });
        assert_eq!(field(&body, "token").as_deref(), Some("vsh_x"));
        assert_eq!(field(&body, "count"), None);
        assert_eq!(field(&body, "absent"), None);
    }
}
