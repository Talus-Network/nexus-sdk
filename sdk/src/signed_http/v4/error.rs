//! Errors returned while parsing or verifying signed HTTP v4 headers.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignedHttpError {
    #[error("unsupported signature version '{0}', expected '4'")]
    UnsupportedVersion(String),
    #[error("missing required header '{0}'")]
    MissingHeader(&'static str),
    #[error("invalid integer in header '{header}': {value}")]
    InvalidInteger { header: &'static str, value: String },
    #[error("invalid base64url in header '{header}': {source}")]
    InvalidBase64 {
        header: &'static str,
        #[source]
        source: base64::DecodeError,
    },
    #[error("invalid byte length in header '{header}': got {actual}, expected {expected}")]
    InvalidLength {
        header: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("unknown leader key (leader_id={leader_id}, leader_key_id={leader_key_id})")]
    UnknownLeaderKey {
        leader_id: String,
        leader_key_id: u64,
    },
    #[error("invalid ed25519 public key for {identity}")]
    InvalidPublicKey { identity: String },
    #[error("invalid ed25519 signature")]
    InvalidSignature,
}
