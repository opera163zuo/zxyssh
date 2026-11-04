//! Content-only binary transfer for paired Agents.
//!
//! Bytes cross the MCP wire inside the request body — the service never reads a
//! path from the machine running the Agent, and never writes one to it. That is
//! the whole reason this module exists: the host-local `sftp_upload` and
//! `sftp_download` tools touch the disk of the machine running zxySSH and stay
//! refused for remote Agents.

use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::sftp::helpers::sha256_hex;

/// Decoded bytes accepted by one `sftp_put_bytes` call. The HTTP body limit is
/// 16 MiB and base64 inflates by a third, so this leaves room for the envelope.
pub const MAX_PUT_BYTES: usize = 8 * 1024 * 1024;

/// Decoded bytes returned by one `sftp_get_bytes` call.
pub const MAX_GET_BYTES: usize = 4 * 1024 * 1024;

/// Default `sftp_get_bytes` range length.
pub const DEFAULT_GET_BYTES: usize = 1024 * 1024;

/// Largest base64 text that can still decode to `MAX_PUT_BYTES`.
const MAX_PUT_BASE64_LEN: usize = MAX_PUT_BYTES / 3 * 4 + 4;

/// Accepts a hex digest in either case, with or without a `sha256:` prefix.
pub fn normalise_digest(declared: &str) -> Result<String, String> {
    let trimmed = declared.trim().to_ascii_lowercase();
    let hex = trimmed
        .strip_prefix("sha256:")
        .unwrap_or(trimmed.as_str())
        .trim();
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("sha256 must be 64 hexadecimal characters of the decoded payload".to_string());
    }
    Ok(hex.to_string())
}

/// Decode a requested upload and refuse it before anything is written unless the
/// caller's digest matches the bytes it actually sent.
pub fn decode_payload(data_base64: &str, declared_sha256: &str) -> Result<Vec<u8>, String> {
    let declared = normalise_digest(declared_sha256)?;

    // Cheap bound first: a too-large body should not be decoded and allocated.
    if data_base64.len() > MAX_PUT_BASE64_LEN {
        return Err(format!(
            "data_base64 is too large to decode (limit: {} bytes after decoding)",
            MAX_PUT_BYTES
        ));
    }

    let bytes = STANDARD
        .decode(data_base64)
        .map_err(|error| format!("data_base64 is not valid standard base64: {error}"))?;

    if bytes.len() > MAX_PUT_BYTES {
        return Err(format!(
            "decoded payload is {} bytes; limit is {}",
            bytes.len(),
            MAX_PUT_BYTES
        ));
    }

    let actual = sha256_hex(&bytes);
    if actual != declared {
        return Err(format!(
            "sha256 does not describe the payload that was sent (declared {declared}, decoded bytes hash to {actual}, {} bytes)",
            bytes.len()
        ));
    }

    Ok(bytes)
}

/// Clamp a requested read range to what one call may return.
pub fn clamp_range(requested_length: Option<u64>) -> usize {
    match requested_length {
        None => DEFAULT_GET_BYTES,
        Some(0) => DEFAULT_GET_BYTES,
        Some(length) => length.min(MAX_GET_BYTES as u64) as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sftp::helpers::sha256_hex;

    #[test]
    fn digest_forms_are_normalised_before_comparison() {
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(normalise_digest(empty).unwrap(), empty);
        assert_eq!(normalise_digest(&empty.to_uppercase()).unwrap(), empty);
        assert_eq!(
            normalise_digest(&format!(" sha256:{empty} ")).unwrap(),
            empty
        );
        for bad in ["", "f00d", &empty.repeat(2), &"z".repeat(64)] {
            assert!(normalise_digest(bad).is_err(), "accepted {bad} as a digest");
        }
    }

    #[test]
    fn decode_requires_the_digest_of_the_bytes_actually_sent() {
        let bytes = vec![0x7f, b'E', b'L', b'F', 0x01, 0xff, 0x00];
        let encoded = STANDARD.encode(&bytes);
        assert_eq!(
            decode_payload(&encoded, &sha256_hex(&bytes)).unwrap(),
            bytes
        );

        let error = decode_payload(&encoded, &sha256_hex(b"something else")).unwrap_err();
        assert!(error.contains("does not describe the payload"), "{error}");
        assert!(error.contains("7 bytes"), "{error}");
        assert!(
            !error.contains(&encoded),
            "the payload itself leaked into the error"
        );
    }

    #[test]
    fn decode_rejects_malformed_base64_without_loosening_the_alphabet() {
        for body in ["not base64!", "aGVsbG8gd29ybGQ", "aGVsbG8=\n"] {
            let error = decode_payload(body, &sha256_hex(b"")).unwrap_err();
            assert!(
                error.contains("base64") || error.contains("sha256"),
                "unexpected error for {body}: {error}"
            );
        }
    }

    #[test]
    fn an_oversized_body_is_refused_before_it_is_decoded() {
        // A string of this length cannot decode under the cap, and constructing
        // the decoded Vec is exactly what the check is meant to avoid.
        let padded = "A".repeat(MAX_PUT_BASE64_LEN + 1);
        let error = decode_payload(&padded, &sha256_hex(b"")).unwrap_err();
        assert!(error.contains("too large"), "{error}");

        // Heap-allocated: an 8 MiB stack temporary would overflow a test thread.
        let payload = vec![7u8; MAX_PUT_BYTES];
        let just_under = STANDARD.encode(&payload);
        assert_eq!(
            decode_payload(&just_under, &sha256_hex(&payload))
                .unwrap()
                .len(),
            MAX_PUT_BYTES
        );
    }

    #[test]
    fn read_ranges_default_and_cap_rather_than_following_the_caller() {
        assert_eq!(clamp_range(None), DEFAULT_GET_BYTES);
        assert_eq!(clamp_range(Some(0)), DEFAULT_GET_BYTES);
        assert_eq!(clamp_range(Some(128)), 128);
        assert_eq!(clamp_range(Some(u64::MAX)), MAX_GET_BYTES);
    }
}
