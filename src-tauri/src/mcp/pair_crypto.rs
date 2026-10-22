//! Confidentiality for the credential a paired Agent receives over the LAN
//! listener.
//!
//! A pairing code is six digits, so it cannot key the channel: anyone who
//! captures one pairing exchange could brute-force a code-derived key offline
//! and recover the long-lived bearer token. Instead the Agent contributes an
//! ephemeral X25519 public key, the service derives a per-request shared
//! secret with its own fresh ephemeral key, and only the token travels sealed
//! under that secret. A passive listener on the network segment sees public
//! keys and ciphertext, and cannot recover the credential.
//!
//! The pairing code itself still crosses the wire in the clear while the
//! listener is plain HTTP; sealing protects the credential, not the code.
use anyhow::{anyhow, Context, Result};
use base64::Engine as _;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::agreement::{agree_ephemeral, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use ring::hkdf::{KeyType as HkdfKeyType, Salt, HKDF_SHA256};
use ring::rand::{SecureRandom, SystemRandom};

/// Domain separation for every derived secret in the pairing channel.
pub const CONTEXT: &[u8] = b"zxyssh-pair-v1";

const KEY_LEN: usize = 32;
const PUBLIC_KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

struct Key256;

impl HkdfKeyType for Key256 {
    fn len(&self) -> usize {
        KEY_LEN
    }
}

/// The service half of one pairing exchange: the sealed credential plus the
/// public key the Agent needs to re-derive the same secret.
pub struct PairingEnvelope {
    pub public_key: String,
    pub sealed: String,
}

/// Generate a fresh X25519 private key for one pairing exchange. Each key is
/// consumed by the agreement, so it can never be reused for a second request.
pub fn ephemeral_private_key() -> Result<EphemeralPrivateKey> {
    EphemeralPrivateKey::generate(&X25519, &SystemRandom::new())
        .map_err(|_| anyhow!("could not generate a pairing key"))
}

/// Base64url of a private key's public half, for a JSON body.
pub fn public_key_b64(private_key: &EphemeralPrivateKey) -> Result<String> {
    private_key
        .compute_public_key()
        .map(|public| encode(public.as_ref()))
        .map_err(|_| anyhow!("could not derive the pairing public key"))
}

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn decode(value: &str) -> Result<Vec<u8>> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .context("pairing material is not base64url")
}

fn derive_key(shared_secret: &[u8], request_id: &str) -> Result<LessSafeKey> {
    let secret = Salt::new(HKDF_SHA256, request_id.as_bytes()).extract(shared_secret);
    let mut bytes = [0u8; KEY_LEN];
    secret
        .expand(&[CONTEXT], Key256)
        .and_then(|okm| okm.fill(&mut bytes))
        .map_err(|_| anyhow!("could not derive the pairing key"))?;
    let unbound =
        UnboundKey::new(&AES_256_GCM, &bytes).map_err(|_| anyhow!("pairing key was rejected"))?;
    Ok(LessSafeKey::new(unbound))
}

fn random_nonce() -> Result<[u8; NONCE_LEN]> {
    let mut bytes = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow!("no random source available"))?;
    Ok(bytes)
}

/// Seal `plaintext` for the Agent holding `peer_public_key`, consuming
/// `our_private_key` for exactly this exchange.
pub fn seal_for_agent(
    our_private_key: EphemeralPrivateKey,
    peer_public_key: &str,
    request_id: &str,
    plaintext: &str,
) -> Result<PairingEnvelope> {
    let peer_bytes = decode(peer_public_key)?;
    if peer_bytes.len() != PUBLIC_KEY_LEN {
        return Err(anyhow!("pairing public key must be 32 bytes"));
    }
    let public_key = public_key_b64(&our_private_key)?;
    let peer = UnparsedPublicKey::new(&X25519, peer_bytes);
    agree_ephemeral(our_private_key, &peer, |shared| {
        let key = derive_key(shared, request_id)?;
        let nonce_bytes = random_nonce()?;
        let mut buffer = plaintext.as_bytes().to_vec();
        key.seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce_bytes),
            Aad::from(CONTEXT),
            &mut buffer,
        )
        .map_err(|_| anyhow!("pairing seal failed"))?;
        let mut blob = nonce_bytes.to_vec();
        blob.extend_from_slice(&buffer);
        Ok(PairingEnvelope {
            public_key,
            sealed: encode(&blob),
        })
    })
    .map_err(|_| anyhow!("pairing key agreement failed"))?
}

/// Open a sealed pairing blob with the private key whose public half was sent
/// in the pairing request. Used by the reference client and by tests.
pub fn open_sealed(
    our_private_key: EphemeralPrivateKey,
    peer_public_key: &str,
    request_id: &str,
    sealed: &str,
) -> Result<String> {
    let peer_bytes = decode(peer_public_key)?;
    if peer_bytes.len() != PUBLIC_KEY_LEN {
        return Err(anyhow!("pairing public key must be 32 bytes"));
    }
    let blob = decode(sealed)?;
    if blob.len() <= NONCE_LEN {
        return Err(anyhow!("sealed pairing blob is truncated"));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce_bytes)
        .map_err(|_| anyhow!("sealed pairing blob carried a bad nonce"))?;
    let peer = UnparsedPublicKey::new(&X25519, peer_bytes);
    let mut buffer = ciphertext.to_vec();
    agree_ephemeral(our_private_key, &peer, |shared| {
        let key = derive_key(shared, request_id)?;
        let clear = key
            .open_in_place(nonce, Aad::from(CONTEXT), &mut buffer)
            .map_err(|_| anyhow!("sealed pairing blob did not decrypt"))?;
        String::from_utf8(clear.to_vec()).context("pairing token is not valid UTF-8")
    })
    .map_err(|_| anyhow!("pairing key agreement failed"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_token_round_trips_to_the_matching_key_pair() {
        let client = ephemeral_private_key().unwrap();
        let client_public = public_key_b64(&client).unwrap();
        let service = ephemeral_private_key().unwrap();
        let envelope =
            seal_for_agent(service, &client_public, "req-1", "vsh_secret-token").unwrap();
        assert_eq!(envelope.public_key.len(), 43);
        let opened = open_sealed(client, &envelope.public_key, "req-1", &envelope.sealed).unwrap();
        assert_eq!(opened, "vsh_secret-token");
    }

    #[test]
    fn a_different_key_pair_cannot_open_the_envelope() {
        let client = ephemeral_private_key().unwrap();
        let stranger = ephemeral_private_key().unwrap();
        let envelope = seal_for_agent(
            ephemeral_private_key().unwrap(),
            &public_key_b64(&client).unwrap(),
            "req-1",
            "vsh_secret-token",
        )
        .unwrap();
        assert!(open_sealed(stranger, &envelope.public_key, "req-1", &envelope.sealed).is_err());
    }

    #[test]
    fn request_id_is_part_of_the_key_derivation() {
        let client = ephemeral_private_key().unwrap();
        let envelope = seal_for_agent(
            ephemeral_private_key().unwrap(),
            &public_key_b64(&client).unwrap(),
            "req-1",
            "vsh_secret-token",
        )
        .unwrap();
        assert!(open_sealed(client, &envelope.public_key, "req-2", &envelope.sealed).is_err());
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let client = ephemeral_private_key().unwrap();
        let envelope = seal_for_agent(
            ephemeral_private_key().unwrap(),
            &public_key_b64(&client).unwrap(),
            "req-1",
            "vsh_secret-token",
        )
        .unwrap();
        let mut blob = decode(&envelope.sealed).unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        assert!(open_sealed(client, &envelope.public_key, "req-1", &encode(&blob)).is_err());
    }

    #[test]
    fn short_or_malformed_peer_keys_are_refused() {
        assert!(seal_for_agent(
            ephemeral_private_key().unwrap(),
            "not-base64!",
            "req-1",
            "x"
        )
        .is_err());
        assert!(seal_for_agent(
            ephemeral_private_key().unwrap(),
            &encode(&[0u8; 31]),
            "req-1",
            "x"
        )
        .is_err());
    }
}
