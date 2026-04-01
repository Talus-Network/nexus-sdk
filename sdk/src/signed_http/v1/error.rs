//! Signed HTTP v1 errors.

use {super::wire::SIG_VERSION_V1, thiserror::Error};

#[derive(Debug, Error)]
pub enum SignedHttpError {
    #[error("unsupported signature version '{0}', expected '{SIG_VERSION_V1}'")]
    UnsupportedVersion(String),
    #[error("missing required header '{0}'")]
    MissingHeader(&'static str),
    #[error("invalid base64url in header '{header}': {source}")]
    InvalidBase64 {
        header: &'static str,
        #[source]
        source: base64::DecodeError,
    },
    #[error("invalid signature length {0}, expected 64")]
    InvalidSignatureLength(usize),
    #[error("invalid json in signed input: {0}")]
    InvalidSignedInputJson(#[source] serde_json::Error),
    #[error("unknown leader key (leader_id={leader_id}, leader_kid={leader_kid})")]
    UnknownLeaderKey { leader_id: String, leader_kid: u64 },
    #[error("invalid ed25519 public key (leader_id={leader_id}, leader_kid={leader_kid})")]
    InvalidLeaderPublicKey { leader_id: String, leader_kid: u64 },
    #[error("unknown tool key (tool_id={tool_id}, tool_kid={tool_kid})")]
    UnknownToolKey { tool_id: String, tool_kid: u64 },
    #[error("invalid ed25519 public key (tool_id={tool_id}, tool_kid={tool_kid})")]
    InvalidToolPublicKey { tool_id: String, tool_kid: u64 },
    #[error("invalid signature")]
    InvalidSignature,
    #[error("tool_id mismatch (claimed '{claimed}', expected '{expected}')")]
    ToolIdMismatch { claimed: String, expected: String },
    #[error("method mismatch (claimed '{claimed}', actual '{actual}')")]
    MethodMismatch { claimed: String, actual: String },
    #[error("path mismatch (claimed '{claimed}', actual '{actual}')")]
    PathMismatch { claimed: String, actual: String },
    #[error("query mismatch (claimed '{claimed}', actual '{actual}')")]
    QueryMismatch { claimed: String, actual: String },
    #[error("invalid body_sha256 hex: {0}")]
    InvalidBodySha256Hex(String),
    #[error("invalid req_sig_input_sha256 hex: {0}")]
    InvalidReqSigInputSha256Hex(String),
    #[error("body hash mismatch")]
    BodyHashMismatch,
    #[error("response is not bound to the expected request")]
    RequestBindingMismatch,
    #[error("status mismatch (claimed {claimed}, actual {actual})")]
    StatusMismatch { claimed: u16, actual: u16 },
    #[error("exp_ms must be >= iat_ms")]
    InvalidTimeWindow,
    #[error("request is not yet valid (iat_ms={iat_ms}, now_ms={now_ms})")]
    NotYetValid { iat_ms: u64, now_ms: u64 },
    #[error("request expired (exp_ms={exp_ms}, now_ms={now_ms})")]
    Expired { exp_ms: u64, now_ms: u64 },
    #[error("validity window too large ({validity_ms}ms > {max_validity_ms}ms)")]
    ValidityTooLarge {
        validity_ms: u64,
        max_validity_ms: u64,
    },
    #[error("invalid allowed-leaders file: {0}")]
    InvalidAllowedLeadersFile(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
