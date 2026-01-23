//! Signed HTTP v1 wire types and header encoding.
//!
//! This module defines the v1 signed-HTTP header names (`X-Nexus-Sig-*`), the JSON claim structs
//! used for `/invoke` requests/responses, and helpers for encoding/decoding and verifying
//! signatures.
//!
//! Signature headers encode raw bytes using base64url (no padding).
//!
//! Time fields are expressed as milliseconds since the Unix epoch (UTC):
//! - `iat_ms`: "issued at" / start of validity window
//! - `exp_ms`: expiry / end of validity window
//! - `now_ms`: verifier wall-clock time

use {
    super::error::SignedHttpError,
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _},
    ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey},
    serde::{Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    },
};

/// Signature protocol version header.
pub const HEADER_SIG_VERSION: &str = "X-Nexus-Sig-V";
/// Signed claims bytes (base64url, no padding).
pub const HEADER_SIG_INPUT: &str = "X-Nexus-Sig-Input";
/// Ed25519 signature bytes (base64url, no padding).
pub const HEADER_SIG: &str = "X-Nexus-Sig";

/// Protocol version string for v1.
pub const SIG_VERSION_V1: &str = "1";

pub(super) const DOMAIN_REQUEST_V1: &[u8] = b"nexus.leader_tool.request.v1.";
pub(super) const DOMAIN_RESPONSE_V1: &[u8] = b"nexus.leader_tool.response.v1.";

/// Verification policy knobs.
///
/// These limits are intentionally separate from request/response claims:
/// - claims say what the sender intends:
///   - `iat_ms`: "issued at" / start of validity window (ms since Unix epoch, UTC)
///   - `exp_ms`: expiry / end of validity window (ms since Unix epoch, UTC)
/// - verification policy says what the verifier accepts (clock skew + max window)
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Current wall-clock time in milliseconds since the Unix epoch (UTC).
    pub now_ms: u64,
    /// Allowed clock skew when comparing `now_ms` to `iat_ms`/`exp_ms`.
    pub max_clock_skew_ms: u64,
    /// Maximum accepted validity window (`exp_ms - iat_ms`).
    pub max_validity_ms: u64,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            now_ms: now_ms(),
            max_clock_skew_ms: 30_000,
            max_validity_ms: 60_000,
        }
    }
}

/// Minimal request metadata that is bound into the signature.
///
/// These values should match what the Tool runtime sees:
/// - `path` should be the request path (e.g. `"/invoke"`).
/// - `query` should be the raw query string without the leading `?` (or empty).
#[derive(Clone, Debug)]
pub struct HttpRequestMeta<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: &'a str,
}

/// Signed claims for a Leader -> Tool invocation request.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvokeRequestClaimsV1 {
    /// On-chain Leader identifier.
    pub leader_id: String,
    /// Key id for Leader key rotation.
    pub leader_kid: u64,
    /// On-chain Tool identifier (the tool being invoked).
    pub tool_id: String,
    /// "Issued at" timestamp (start of validity window), milliseconds since the Unix epoch (UTC).
    pub iat_ms: u64,
    /// Expiry timestamp (end of validity window), milliseconds since the Unix epoch (UTC).
    pub exp_ms: u64,
    /// Uniqueness token for replay resistance.
    ///
    /// The verifier MUST track nonce usage (typically keyed by `(leader_id, nonce)`) to reject
    /// replays while still allowing safe retries.
    ///
    /// The Leader should generate a fresh nonce per invocation attempt (a UUID, or random bytes
    /// encoded as base64url/hex are both fine).
    pub nonce: String,
    /// HTTP method (e.g. `"POST"`).
    pub method: String,
    /// HTTP path (e.g. `"/invoke"`).
    pub path: String,
    /// Raw query string without `?` (or empty).
    pub query: String,
    /// Hex-encoded `sha256(body_bytes)`.
    pub body_sha256: String,
}

/// Signed claims for a Tool -> Leader invocation response.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InvokeResponseClaimsV1 {
    /// On-chain Tool identifier.
    pub tool_id: String,
    /// Key id for Tool key rotation.
    pub tool_kid: u64,
    /// "Issued at" timestamp (start of validity window), milliseconds since the Unix epoch (UTC).
    pub iat_ms: u64,
    /// Expiry timestamp (end of validity window), milliseconds since the Unix epoch (UTC).
    pub exp_ms: u64,
    /// Echoed request nonce (binds response to the request).
    pub nonce: String,
    /// Hex-encoded `sha256(request_sig_input_bytes)` (binds response to a specific request).
    pub req_sig_input_sha256: String,
    /// HTTP status code the signer claims to have produced.
    pub status: u16,
    /// Hex-encoded `sha256(body_bytes)`.
    pub body_sha256: String,
}

#[derive(Clone, Debug)]
pub struct EncodedSignatureHeadersV1 {
    pub sig_input_b64: String,
    pub sig_b64: String,
}

impl EncodedSignatureHeadersV1 {
    pub fn to_pairs(&self) -> [(&'static str, String); 3] {
        [
            (HEADER_SIG_VERSION, SIG_VERSION_V1.to_string()),
            (HEADER_SIG_INPUT, self.sig_input_b64.clone()),
            (HEADER_SIG, self.sig_b64.clone()),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct DecodedSignatureV1 {
    pub sig_input: Vec<u8>,
    pub signature: [u8; 64],
}

/// Decode signature headers into raw `sig_input` bytes and signature bytes.
///
/// This function performs:
/// - version checking (`X-Nexus-Sig-V` must match [`SIG_VERSION_V1`])
/// - base64url decoding for `sig_input` and signature
///
/// It does not parse the signed JSON claims or verify the signature. Use
/// [`verify_invoke_request_v1`] / [`verify_invoke_response_v1`] for full verification.
pub fn decode_signature_headers_v1(
    sig_v: Option<&str>,
    sig_input_b64: Option<&str>,
    sig_b64: Option<&str>,
) -> Result<DecodedSignatureV1, SignedHttpError> {
    let sig_v = sig_v.ok_or(SignedHttpError::MissingHeader(HEADER_SIG_VERSION))?;
    if sig_v != SIG_VERSION_V1 {
        return Err(SignedHttpError::UnsupportedVersion(sig_v.to_string()));
    }

    let sig_input_b64 = sig_input_b64.ok_or(SignedHttpError::MissingHeader(HEADER_SIG_INPUT))?;
    let sig_b64 = sig_b64.ok_or(SignedHttpError::MissingHeader(HEADER_SIG))?;

    let sig_input =
        URL_SAFE_NO_PAD
            .decode(sig_input_b64)
            .map_err(|e| SignedHttpError::InvalidBase64 {
                header: HEADER_SIG_INPUT,
                source: e,
            })?;

    let sig_bytes =
        URL_SAFE_NO_PAD
            .decode(sig_b64)
            .map_err(|e| SignedHttpError::InvalidBase64 {
                header: HEADER_SIG,
                source: e,
            })?;

    let signature: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|v: Vec<u8>| SignedHttpError::InvalidSignatureLength(v.len()))?;

    Ok(DecodedSignatureV1 {
        sig_input,
        signature,
    })
}

/// Encode raw `sig_input` bytes and signature bytes into header values.
pub fn encode_signature_headers_v1(
    sig_input: &[u8],
    signature: &[u8; 64],
) -> EncodedSignatureHeadersV1 {
    EncodedSignatureHeadersV1 {
        sig_input_b64: URL_SAFE_NO_PAD.encode(sig_input),
        sig_b64: URL_SAFE_NO_PAD.encode(signature),
    }
}

#[derive(Clone, Debug)]
pub struct VerifiedInvokeRequestV1 {
    pub claims: InvokeRequestClaimsV1,
    pub leader_public_key: [u8; 32],
    pub sig_input: Vec<u8>,
    pub sig_input_sha256: [u8; 32],
}

/// Verify a signed invocation request (Leader -> Tool).
///
/// This function verifies:
/// - the signed claims JSON parses as [`InvokeRequestClaimsV1`]
/// - `tool_id`, `method`, `path`, `query` match the actual HTTP request
/// - `body_sha256` matches the provided body bytes
/// - the time window is acceptable under [`VerifyOptions`]
/// - the signature is valid for a Leader public key found in [`AllowedLeadersV1`]
///
/// It returns [`VerifiedInvokeRequestV1`] containing:
/// - parsed claims
/// - the Leader public key bytes used for verification
/// - the raw `sig_input` bytes and its SHA-256 hash (for response binding)
pub fn verify_invoke_request_v1(
    decoded: DecodedSignatureV1,
    http: HttpRequestMeta<'_>,
    body: &[u8],
    expected_tool_id: &str,
    allowed_leaders: &AllowedLeadersV1,
    opts: &VerifyOptions,
) -> Result<VerifiedInvokeRequestV1, SignedHttpError> {
    let claims: InvokeRequestClaimsV1 = serde_json::from_slice(&decoded.sig_input)
        .map_err(SignedHttpError::InvalidSignedInputJson)?;

    if claims.tool_id != expected_tool_id {
        return Err(SignedHttpError::ToolIdMismatch {
            claimed: claims.tool_id,
            expected: expected_tool_id.to_string(),
        });
    }

    if claims.method != http.method {
        return Err(SignedHttpError::MethodMismatch {
            claimed: claims.method,
            actual: http.method.to_string(),
        });
    }

    if claims.path != http.path {
        return Err(SignedHttpError::PathMismatch {
            claimed: claims.path,
            actual: http.path.to_string(),
        });
    }

    if claims.query != http.query {
        return Err(SignedHttpError::QueryMismatch {
            claimed: claims.query,
            actual: http.query.to_string(),
        });
    }

    let body_sha256 = sha256(body);
    let claimed_body_sha256 = parse_hex_32(&claims.body_sha256)
        .map_err(|_| SignedHttpError::InvalidBodySha256Hex(claims.body_sha256.clone()))?;
    if body_sha256 != claimed_body_sha256 {
        return Err(SignedHttpError::BodyHashMismatch);
    }

    validate_time_window(claims.iat_ms, claims.exp_ms, opts)?;

    let leader_public_key = allowed_leaders
        .leader_public_key_bytes(&claims.leader_id, claims.leader_kid)
        .ok_or_else(|| SignedHttpError::UnknownLeaderKey {
            leader_id: claims.leader_id.clone(),
            leader_kid: claims.leader_kid,
        })?;

    let verifying_key = VerifyingKey::from_bytes(&leader_public_key).map_err(|_| {
        SignedHttpError::InvalidLeaderPublicKey {
            leader_id: claims.leader_id.clone(),
            leader_kid: claims.leader_kid,
        }
    })?;

    let msg = message_to_verify(DOMAIN_REQUEST_V1, &decoded.sig_input);
    let sig = Signature::from_bytes(&decoded.signature);
    verifying_key
        .verify_strict(&msg, &sig)
        .map_err(|_| SignedHttpError::InvalidSignature)?;

    let sig_input_sha256 = sha256(&decoded.sig_input);

    Ok(VerifiedInvokeRequestV1 {
        claims,
        leader_public_key,
        sig_input: decoded.sig_input,
        sig_input_sha256,
    })
}

#[derive(Clone, Debug)]
pub struct VerifiedInvokeResponseV1 {
    pub claims: InvokeResponseClaimsV1,
    pub tool_public_key: [u8; 32],
    pub sig_input: Vec<u8>,
    pub sig_input_sha256: [u8; 32],
}

/// Verify a signed invocation response (Tool -> Leader).
///
/// This function verifies:
/// - the signed claims JSON parses as [`InvokeResponseClaimsV1`]
/// - `tool_id` matches the expected tool
/// - `body_sha256` matches the provided body bytes
/// - the time window is acceptable under [`VerifyOptions`]
/// - the response is bound to the request (`req_sig_input_sha256` matches)
/// - the signature is valid for the provided Tool public key
///
/// Note: response verification does not know the actual HTTP status code.
/// Callers should additionally compare `claims.status` with the received HTTP status.
pub fn verify_invoke_response_v1(
    decoded: DecodedSignatureV1,
    body: &[u8],
    expected_tool_id: &str,
    expected_req_sig_input_sha256: [u8; 32],
    tool_public_key: [u8; 32],
    opts: &VerifyOptions,
) -> Result<VerifiedInvokeResponseV1, SignedHttpError> {
    let claims: InvokeResponseClaimsV1 = serde_json::from_slice(&decoded.sig_input)
        .map_err(SignedHttpError::InvalidSignedInputJson)?;

    if claims.tool_id != expected_tool_id {
        return Err(SignedHttpError::ToolIdMismatch {
            claimed: claims.tool_id,
            expected: expected_tool_id.to_string(),
        });
    }

    let body_sha256 = sha256(body);
    let claimed_body_sha256 = parse_hex_32(&claims.body_sha256)
        .map_err(|_| SignedHttpError::InvalidBodySha256Hex(claims.body_sha256.clone()))?;
    if body_sha256 != claimed_body_sha256 {
        return Err(SignedHttpError::BodyHashMismatch);
    }

    validate_time_window(claims.iat_ms, claims.exp_ms, opts)?;

    let claimed_req_hash = parse_hex_32(&claims.req_sig_input_sha256).map_err(|_| {
        SignedHttpError::InvalidReqSigInputSha256Hex(claims.req_sig_input_sha256.clone())
    })?;
    if claimed_req_hash != expected_req_sig_input_sha256 {
        return Err(SignedHttpError::RequestBindingMismatch);
    }

    let verifying_key = VerifyingKey::from_bytes(&tool_public_key).map_err(|_| {
        SignedHttpError::InvalidToolPublicKey {
            tool_id: expected_tool_id.to_string(),
            tool_kid: claims.tool_kid,
        }
    })?;

    let msg = message_to_verify(DOMAIN_RESPONSE_V1, &decoded.sig_input);
    let sig = Signature::from_bytes(&decoded.signature);
    verifying_key
        .verify_strict(&msg, &sig)
        .map_err(|_| SignedHttpError::InvalidSignature)?;

    let sig_input_sha256 = sha256(&decoded.sig_input);

    Ok(VerifiedInvokeResponseV1 {
        claims,
        tool_public_key,
        sig_input: decoded.sig_input,
        sig_input_sha256,
    })
}

/// Sign request claims as a v1 invocation request.
///
/// Returns `(sig_input_bytes, signature_bytes)`.
pub fn sign_invoke_request_v1(
    claims: &InvokeRequestClaimsV1,
    signing_key: &SigningKey,
) -> Result<(Vec<u8>, [u8; 64]), SignedHttpError> {
    let sig_input = serde_json::to_vec(claims).map_err(SignedHttpError::InvalidSignedInputJson)?;
    let msg = message_to_verify(DOMAIN_REQUEST_V1, &sig_input);
    let sig: Signature = signing_key.sign(&msg);
    Ok((sig_input, sig.to_bytes()))
}

/// Sign response claims as a v1 invocation response.
///
/// Returns `(sig_input_bytes, signature_bytes)`.
pub fn sign_invoke_response_v1(
    claims: &InvokeResponseClaimsV1,
    signing_key: &SigningKey,
) -> Result<(Vec<u8>, [u8; 64]), SignedHttpError> {
    let sig_input = serde_json::to_vec(claims).map_err(SignedHttpError::InvalidSignedInputJson)?;
    let msg = message_to_verify(DOMAIN_RESPONSE_V1, &sig_input);
    let sig: Signature = signing_key.sign(&msg);
    Ok((sig_input, sig.to_bytes()))
}

/// Hex-encode `sha256(data)`.
pub fn sha256_hex(data: &[u8]) -> String {
    hex::encode(sha256(data))
}

/// Compute `sha256(data)` and return the raw 32-byte digest.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub(super) fn message_to_verify(domain: &[u8], sig_input: &[u8]) -> Vec<u8> {
    let mut msg = Vec::with_capacity(domain.len() + sig_input.len());
    msg.extend_from_slice(domain);
    msg.extend_from_slice(sig_input);
    msg
}

pub(super) fn validate_time_window(
    iat_ms: u64,
    exp_ms: u64,
    opts: &VerifyOptions,
) -> Result<(), SignedHttpError> {
    if exp_ms < iat_ms {
        return Err(SignedHttpError::InvalidTimeWindow);
    }

    let validity_ms = exp_ms - iat_ms;
    if validity_ms > opts.max_validity_ms {
        return Err(SignedHttpError::ValidityTooLarge {
            validity_ms,
            max_validity_ms: opts.max_validity_ms,
        });
    }

    let now_ms = opts.now_ms;
    let skew = opts.max_clock_skew_ms;

    if iat_ms > now_ms.saturating_add(skew) {
        return Err(SignedHttpError::NotYetValid { iat_ms, now_ms });
    }

    if exp_ms < now_ms.saturating_sub(skew) {
        return Err(SignedHttpError::Expired { exp_ms, now_ms });
    }

    Ok(())
}

pub(super) fn parse_hex_32(s: &str) -> Result<[u8; 32], ()> {
    let bytes = hex::decode(s).map_err(|_| ())?;
    bytes.try_into().map_err(|_| ())
}

/// Current wall-clock time in milliseconds since UNIX epoch.
pub fn now_ms() -> u64 {
    let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return 0;
    };
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[derive(Clone, Debug)]
pub struct AllowedLeadersV1 {
    leaders: BTreeMap<String, BTreeMap<u64, [u8; 32]>>,
    source_path: Option<PathBuf>,
}

impl AllowedLeadersV1 {
    /// Load an allowlist from disk.
    ///
    /// The file must follow [`AllowedLeadersFileV1`] schema. This is intended to be
    /// provisioned out-of-band (e.g. at tool registration time).
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, SignedHttpError> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        let file: AllowedLeadersFileV1 = serde_json::from_slice(&bytes)
            .map_err(|e| SignedHttpError::InvalidAllowedLeadersFile(e.to_string()))?;
        let mut allowed = Self::try_from(file)?;
        allowed.source_path = Some(path.to_path_buf());
        Ok(allowed)
    }

    /// Look up a leader public key by `(leader_id, leader_kid)`.
    pub fn leader_public_key_bytes(&self, leader_id: &str, leader_kid: u64) -> Option<[u8; 32]> {
        self.leaders
            .get(leader_id)
            .and_then(|kids| kids.get(&leader_kid).copied())
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

/// JSON schema for allowlisting Leaders (tool-side).
///
/// This is the on-disk format consumed by [`AllowedLeadersV1::from_path`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AllowedLeadersFileV1 {
    pub version: u8,
    pub leaders: Vec<AllowedLeaderFileV1>,
}

/// Allowlisted leader entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AllowedLeaderFileV1 {
    pub leader_id: String,
    pub keys: Vec<AllowedLeaderKeyFileV1>,
}

/// Allowlisted Leader signing key entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AllowedLeaderKeyFileV1 {
    pub kid: u64,
    /// Hex-encoded 32-byte Ed25519 public key.
    pub public_key: String,
}

impl TryFrom<AllowedLeadersFileV1> for AllowedLeadersV1 {
    type Error = SignedHttpError;

    fn try_from(file: AllowedLeadersFileV1) -> Result<Self, Self::Error> {
        if file.version != 1 {
            return Err(SignedHttpError::InvalidAllowedLeadersFile(format!(
                "unsupported version {}, expected 1",
                file.version
            )));
        }

        let mut leaders: BTreeMap<String, BTreeMap<u64, [u8; 32]>> = BTreeMap::new();

        for leader in file.leaders {
            let mut keys: BTreeMap<u64, [u8; 32]> = BTreeMap::new();
            for key in leader.keys {
                let pk_bytes: [u8; 32] = hex::decode(&key.public_key)
                    .map_err(|e| {
                        SignedHttpError::InvalidAllowedLeadersFile(format!(
                            "leader_id={} kid={}: invalid public_key hex: {e}",
                            leader.leader_id, key.kid
                        ))
                    })?
                    .try_into()
                    .map_err(|_| {
                        SignedHttpError::InvalidAllowedLeadersFile(format!(
                            "leader_id={} kid={}: public_key must be 32 bytes",
                            leader.leader_id, key.kid
                        ))
                    })?;

                keys.insert(key.kid, pk_bytes);
            }
            leaders.insert(leader.leader_id, keys);
        }

        Ok(Self {
            leaders,
            source_path: None,
        })
    }
}
