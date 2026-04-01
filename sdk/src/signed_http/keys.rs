//! Ed25519 key utilities for signed HTTP.
//!
//! Signed HTTP (Leader <=> Tool) uses Ed25519 message signing keys at the
//! application layer. These keys are not Sui transaction keys:
//! - Sui tx keys authorize on-chain transactions.
//! - Signed HTTP keys authorize off-chain HTTP messages.
//!
//! Keeping these roles separate is strongly recommended. A compromise of a tool
//! container/config should not automatically grant on-chain spending power.
//!
//! ## Accepted encodings (private key)
//! This module accepts common encodings for a 32-byte Ed25519 secret key:
//! - `hex` (64 hex chars, optional `0x` prefix)
//! - `base64` / `base64url` (with or without padding) for 32 raw bytes
//! - Sui keytool format: `base64(0x00 || sk32)` (33 bytes) where `0x00` is the
//!   Ed25519 scheme flag used by Sui.
//!
//! ## Example: generate, print, and parse
//! ```
//! use nexus_sdk::signed_http::keys::*;
//!
//! let keypair = Ed25519Keypair::generate();
//! let sk_hex = keypair.private_key_hex();
//! let parsed = Ed25519Keypair::from_private_key_hex_or_base64(&sk_hex).unwrap();
//! assert_eq!(parsed.public_key_bytes(), keypair.public_key_bytes());
//! ```

use {
    base64::{engine::general_purpose, Engine as _},
    ed25519_dalek::SigningKey,
    rand::RngCore as _,
    thiserror::Error,
};

/// Ed25519 signing key material used for message signing.
///
/// This is intentionally minimal and does not include any Sui-specific key types.
#[derive(Clone)]
pub struct Ed25519Keypair {
    signing: SigningKey,
}

impl Ed25519Keypair {
    /// Generate a new random Ed25519 signing key.
    pub fn generate() -> Self {
        let mut sk = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut sk);
        Self {
            signing: SigningKey::from_bytes(&sk),
        }
    }

    /// Construct from an existing `ed25519_dalek::SigningKey`.
    pub fn from_signing_key(signing: SigningKey) -> Self {
        Self { signing }
    }

    /// Parse a private key from hex/base64/base64url formats (see module docs).
    pub fn from_private_key_hex_or_base64(raw: &str) -> Result<Self, KeyParseError> {
        parse_ed25519_signing_key(raw).map(Self::from_signing_key)
    }

    /// Borrow the underlying Ed25519 signing key.
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing
    }

    /// Return the raw 32-byte secret key bytes.
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    /// Return the raw 32-byte public key bytes.
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Hex-encode the 32-byte secret key.
    pub fn private_key_hex(&self) -> String {
        hex::encode(self.private_key_bytes())
    }

    /// Hex-encode the 32-byte public key.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public_key_bytes())
    }
}

/// Key parsing errors for Ed25519 message-signing keys.
#[derive(Debug, Error)]
pub enum KeyParseError {
    #[error("invalid hex private key: {0}")]
    InvalidHex(String),
    #[error("invalid base64/base64url private key: {0}")]
    InvalidBase64(String),
    #[error("unsupported key scheme flag 0x{flag:02x} (expected 0x00 for ed25519)")]
    UnsupportedSuiKeySchemeFlag { flag: u8 },
    #[error(
        "invalid private key length {len}, expected 32 bytes (raw ed25519) or 33 bytes (0x00 + key)"
    )]
    InvalidLength { len: usize },
}

/// Parse an Ed25519 message-signing key from hex/base64/base64url.
///
/// See module docs for accepted formats.
pub fn parse_ed25519_signing_key(raw: &str) -> Result<SigningKey, KeyParseError> {
    let raw = raw.trim();
    let raw_no_0x = raw.strip_prefix("0x").unwrap_or(raw);

    // Try hex first when the input looks like hex. This avoids accidentally
    // interpreting arbitrary base64 as hex.
    let looks_like_hex = raw.starts_with("0x")
        || ((raw_no_0x.len() == 64 || raw_no_0x.len() == 66)
            && raw_no_0x.chars().all(|c| c.is_ascii_hexdigit()));

    if looks_like_hex {
        let bytes = hex::decode(raw_no_0x).map_err(|e| KeyParseError::InvalidHex(e.to_string()))?;
        return match bytes.len() {
            32 => Ok(SigningKey::from_bytes(
                &<[u8; 32]>::try_from(bytes.as_slice()).expect("length checked"),
            )),
            33 => signing_key_from_sui_flagged_bytes(
                &<[u8; 33]>::try_from(bytes.as_slice()).expect("length checked"),
            ),
            len => Err(KeyParseError::InvalidLength { len }),
        };
    }

    // Try base64 / base64url (with/without padding).
    let try_b64 = |engine: &general_purpose::GeneralPurpose| -> Option<Vec<u8>> {
        engine.decode(raw.as_bytes()).ok()
    };

    let bytes = try_b64(&general_purpose::STANDARD)
        .or_else(|| try_b64(&general_purpose::STANDARD_NO_PAD))
        .or_else(|| try_b64(&general_purpose::URL_SAFE))
        .or_else(|| try_b64(&general_purpose::URL_SAFE_NO_PAD))
        .ok_or_else(|| {
            KeyParseError::InvalidBase64("expected base64/base64url data".to_string())
        })?;

    match bytes.len() {
        32 => Ok(SigningKey::from_bytes(
            &<[u8; 32]>::try_from(bytes.as_slice()).expect("length checked"),
        )),
        33 => signing_key_from_sui_flagged_bytes(
            &<[u8; 33]>::try_from(bytes.as_slice()).expect("length checked"),
        ),
        len => Err(KeyParseError::InvalidLength { len }),
    }
}

fn signing_key_from_sui_flagged_bytes(bytes: &[u8; 33]) -> Result<SigningKey, KeyParseError> {
    const ED25519_FLAG: u8 = 0x00;
    if bytes[0] != ED25519_FLAG {
        return Err(KeyParseError::UnsupportedSuiKeySchemeFlag { flag: bytes[0] });
    }
    let mut sk = [0u8; 32];
    sk.copy_from_slice(&bytes[1..]);
    Ok(SigningKey::from_bytes(&sk))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_key_bytes(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn parse_hex_roundtrip() {
        let keypair = Ed25519Keypair::generate();
        let hex = keypair.private_key_hex();
        let parsed = Ed25519Keypair::from_private_key_hex_or_base64(&hex).unwrap();
        assert_eq!(parsed.public_key_bytes(), keypair.public_key_bytes());
    }

    #[test]
    fn parse_base64_roundtrip() {
        let bytes = make_key_bytes(7);
        let b64 = general_purpose::STANDARD.encode(bytes);
        let parsed = Ed25519Keypair::from_private_key_hex_or_base64(&b64).unwrap();
        assert_eq!(parsed.private_key_bytes(), bytes);
    }

    #[test]
    fn parse_sui_keytool_base64_format() {
        let bytes = make_key_bytes(9);
        let mut flagged = [0u8; 33];
        flagged[0] = 0x00;
        flagged[1..].copy_from_slice(&bytes);
        let b64 = general_purpose::STANDARD.encode(flagged);
        let parsed = Ed25519Keypair::from_private_key_hex_or_base64(&b64).unwrap();
        assert_eq!(parsed.private_key_bytes(), bytes);
    }

    #[test]
    fn parse_invalid_hex_reports_error() {
        assert!(matches!(
            Ed25519Keypair::from_private_key_hex_or_base64("0xzzzz"),
            Err(KeyParseError::InvalidHex(_))
        ));
    }

    #[test]
    fn parse_invalid_length_reports_error() {
        let bytes = vec![0u8; 31];
        let b64 = general_purpose::STANDARD.encode(bytes);
        assert!(matches!(
            Ed25519Keypair::from_private_key_hex_or_base64(&b64),
            Err(KeyParseError::InvalidLength { len: 31 })
        ));
    }

    #[test]
    fn parse_unsupported_sui_flag_reports_error() {
        let mut flagged = [0u8; 33];
        flagged[0] = 0x01;
        let b64 = general_purpose::STANDARD.encode(flagged);
        assert!(matches!(
            Ed25519Keypair::from_private_key_hex_or_base64(&b64),
            Err(KeyParseError::UnsupportedSuiKeySchemeFlag { flag: 0x01 })
        ));
    }
}
