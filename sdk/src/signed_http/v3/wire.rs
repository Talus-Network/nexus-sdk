use {
    super::error::SignedHttpError,
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _},
    ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey},
    serde::{Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
    },
};

pub const HEADER_SIGNATURE_VERSION: &str = "X-Nexus-Sig-V";
pub const HEADER_LEADER_ID: &str = "X-Nexus-Leader-Id";
pub const HEADER_LEADER_KEY_ID: &str = "X-Nexus-Leader-Key-Id";
pub const HEADER_INPUT_HASH: &str = "X-Nexus-Input-Hash";
pub const HEADER_LEADER_SIGNATURE: &str = "X-Nexus-Leader-Signature";
pub const HEADER_NONCE: &str = "X-Nexus-Nonce";
pub const HEADER_CONTEXT_SIGNATURE: &str = "nexus-context-sig";
pub const HEADER_CONTEXT_TIMESTAMP: &str = "nexus-context-timestamp";
pub const HEADER_TOOL_SIGNATURE: &str = "X-Nexus-Tool-Signature";
pub const SIGNATURE_VERSION_V3: &str = "3";
pub const CANONICAL_TOOL_RESPONSE_CONTENT_TYPE: &str =
    "application/vnd.nexus.canonical-tool-response+bcs";

const SHA256_LEN: usize = 32;
const ED25519_SIGNATURE_LEN: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedRequestHeaders {
    pub leader_id: String,
    pub leader_key_id: u64,
    pub input_hash: String,
    pub leader_signature: String,
    pub nonce: String,
    pub context_signature: String,
    pub context_timestamp: String,
}

impl EncodedRequestHeaders {
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V3.to_string()),
            (HEADER_LEADER_ID, self.leader_id.clone()),
            (HEADER_LEADER_KEY_ID, self.leader_key_id.to_string()),
            (HEADER_INPUT_HASH, self.input_hash.clone()),
            (HEADER_LEADER_SIGNATURE, self.leader_signature.clone()),
            (HEADER_NONCE, self.nonce.clone()),
            (HEADER_CONTEXT_SIGNATURE, self.context_signature.clone()),
            (HEADER_CONTEXT_TIMESTAMP, self.context_timestamp.clone()),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedResponseHeaders {
    pub tool_signature: String,
}

impl EncodedResponseHeaders {
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V3.to_string()),
            (HEADER_TOOL_SIGNATURE, self.tool_signature.clone()),
        ]
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RequestHeadersRef<'a> {
    pub signature_version: Option<&'a str>,
    pub leader_id: Option<&'a str>,
    pub leader_key_id: Option<&'a str>,
    pub input_hash: Option<&'a str>,
    pub leader_signature: Option<&'a str>,
    pub nonce: Option<&'a str>,
    pub context_signature: Option<&'a str>,
    pub context_timestamp: Option<&'a str>,
}

impl<'a> RequestHeadersRef<'a> {
    pub fn from_getter(mut get: impl FnMut(&str) -> Option<&'a str>) -> Self {
        Self {
            signature_version: get(HEADER_SIGNATURE_VERSION),
            leader_id: get(HEADER_LEADER_ID),
            leader_key_id: get(HEADER_LEADER_KEY_ID),
            input_hash: get(HEADER_INPUT_HASH),
            leader_signature: get(HEADER_LEADER_SIGNATURE),
            nonce: get(HEADER_NONCE),
            context_signature: get(HEADER_CONTEXT_SIGNATURE),
            context_timestamp: get(HEADER_CONTEXT_TIMESTAMP),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseHeadersRef<'a> {
    pub signature_version: Option<&'a str>,
    pub tool_signature: Option<&'a str>,
}

impl<'a> ResponseHeadersRef<'a> {
    pub fn from_getter(mut get: impl FnMut(&str) -> Option<&'a str>) -> Self {
        Self {
            signature_version: get(HEADER_SIGNATURE_VERSION),
            tool_signature: get(HEADER_TOOL_SIGNATURE),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedRequest {
    pub leader_id: String,
    pub leader_key_id: u64,
    pub input_hash: [u8; SHA256_LEN],
    pub leader_signature: [u8; ED25519_SIGNATURE_LEN],
    pub nonce: [u8; SHA256_LEN],
    pub context_timestamp_ms: u64,
}

pub trait LeaderKeyResolver: Send + Sync {
    fn leader_public_key(&self, leader_id: &str, leader_key_id: u64) -> Option<[u8; 32]>;
}

pub fn sign_request(
    leader_id: impl Into<String>,
    leader_key_id: u64,
    input_hash: [u8; SHA256_LEN],
    nonce: [u8; SHA256_LEN],
    tool_fqn: &str,
    context_timestamp_ms: u64,
    signing_key: &SigningKey,
) -> EncodedRequestHeaders {
    let signature = signing_key.sign(&input_hash).to_bytes();
    let context_signature = signing_key
        .sign(&context_digest(tool_fqn, nonce, context_timestamp_ms))
        .to_bytes();
    EncodedRequestHeaders {
        leader_id: leader_id.into(),
        leader_key_id,
        input_hash: URL_SAFE_NO_PAD.encode(input_hash),
        leader_signature: URL_SAFE_NO_PAD.encode(signature),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
        context_signature: URL_SAFE_NO_PAD.encode(context_signature),
        context_timestamp: context_timestamp_ms.to_string(),
    }
}

pub fn authenticate_request(
    headers: RequestHeadersRef<'_>,
    keys: &dyn LeaderKeyResolver,
    tool_fqn: &str,
    now_ms: u64,
    context_timestamp_window_ms: u64,
) -> Result<AuthenticatedRequest, SignedHttpError> {
    require_version(headers.signature_version)?;
    let leader_id = required(headers.leader_id, HEADER_LEADER_ID)?.to_string();
    let leader_key_id_raw = required(headers.leader_key_id, HEADER_LEADER_KEY_ID)?;
    let leader_key_id = leader_key_id_raw
        .parse()
        .map_err(|_| SignedHttpError::InvalidInteger {
            header: HEADER_LEADER_KEY_ID,
            value: leader_key_id_raw.to_string(),
        })?;
    let input_hash = decode_array::<SHA256_LEN>(headers.input_hash, HEADER_INPUT_HASH)?;
    let leader_signature =
        decode_array::<ED25519_SIGNATURE_LEN>(headers.leader_signature, HEADER_LEADER_SIGNATURE)?;
    let nonce = decode_array::<SHA256_LEN>(headers.nonce, HEADER_NONCE)?;
    let context_timestamp_raw = required(headers.context_timestamp, HEADER_CONTEXT_TIMESTAMP)?;
    if context_timestamp_raw.is_empty()
        || !context_timestamp_raw
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    {
        return Err(SignedHttpError::InvalidInteger {
            header: HEADER_CONTEXT_TIMESTAMP,
            value: context_timestamp_raw.to_string(),
        });
    }
    let context_timestamp_ms: u64 =
        context_timestamp_raw
            .parse()
            .map_err(|_| SignedHttpError::InvalidInteger {
                header: HEADER_CONTEXT_TIMESTAMP,
                value: context_timestamp_raw.to_string(),
            })?;
    let context_signature =
        decode_array::<ED25519_SIGNATURE_LEN>(headers.context_signature, HEADER_CONTEXT_SIGNATURE)?;
    let public_key = keys
        .leader_public_key(&leader_id, leader_key_id)
        .ok_or_else(|| SignedHttpError::UnknownLeaderKey {
            leader_id: leader_id.clone(),
            leader_key_id,
        })?;
    verify_signature(
        public_key,
        &input_hash,
        leader_signature,
        format!("leader {leader_id} key {leader_key_id}"),
    )?;

    if context_timestamp_ms.abs_diff(now_ms) > context_timestamp_window_ms {
        return Err(SignedHttpError::TimestampOutsideWindow {
            timestamp_ms: context_timestamp_ms,
            now_ms,
            window_ms: context_timestamp_window_ms,
        });
    }
    verify_signature(
        public_key,
        &context_digest(tool_fqn, nonce, context_timestamp_ms),
        context_signature,
        format!("leader {leader_id} key {leader_key_id} Tool context"),
    )?;

    Ok(AuthenticatedRequest {
        leader_id,
        leader_key_id,
        input_hash,
        leader_signature,
        nonce,
        context_timestamp_ms,
    })
}

pub fn sign_response(
    leader_signature: &[u8; ED25519_SIGNATURE_LEN],
    nonce: &[u8; SHA256_LEN],
    result_bytes: &[u8],
    signing_key: &SigningKey,
) -> EncodedResponseHeaders {
    let message = crate::commitments::tool_signature_message(leader_signature, nonce, result_bytes);
    EncodedResponseHeaders {
        tool_signature: URL_SAFE_NO_PAD.encode(signing_key.sign(&message).to_bytes()),
    }
}

pub fn verify_response(
    headers: ResponseHeadersRef<'_>,
    leader_signature: &[u8; ED25519_SIGNATURE_LEN],
    nonce: &[u8; SHA256_LEN],
    result_bytes: &[u8],
    tool_public_key: [u8; 32],
) -> Result<[u8; ED25519_SIGNATURE_LEN], SignedHttpError> {
    require_version(headers.signature_version)?;
    let tool_signature =
        decode_array::<ED25519_SIGNATURE_LEN>(headers.tool_signature, HEADER_TOOL_SIGNATURE)?;
    verify_signature(
        tool_public_key,
        &crate::commitments::tool_signature_message(leader_signature, nonce, result_bytes),
        tool_signature,
        "tool".to_string(),
    )?;
    Ok(tool_signature)
}

pub fn tool_signature_message(
    leader_signature: &[u8; ED25519_SIGNATURE_LEN],
    nonce: &[u8; SHA256_LEN],
    result_bytes: &[u8],
) -> Vec<u8> {
    crate::commitments::tool_signature_message(leader_signature, nonce, result_bytes)
}

pub fn sha256(bytes: &[u8]) -> [u8; SHA256_LEN] {
    Sha256::digest(bytes).into()
}

/// Hashes the BCS encoding of the receiver's FQN, nonce, and Unix-ms timestamp.
pub fn context_digest(
    tool_fqn: &str,
    nonce: [u8; SHA256_LEN],
    timestamp_ms: u64,
) -> [u8; SHA256_LEN] {
    let bytes = bcs::to_bytes(&(tool_fqn.to_owned(), nonce, timestamp_ms))
        .expect("BCS encoding a String, fixed byte array, and u64 cannot fail");
    sha256(&bytes)
}

fn required<'a>(value: Option<&'a str>, header: &'static str) -> Result<&'a str, SignedHttpError> {
    value.ok_or(SignedHttpError::MissingHeader(header))
}

fn require_version(version: Option<&str>) -> Result<(), SignedHttpError> {
    let version = required(version, HEADER_SIGNATURE_VERSION)?;
    if version != SIGNATURE_VERSION_V3 {
        return Err(SignedHttpError::UnsupportedVersion(version.to_string()));
    }
    Ok(())
}

fn decode_array<const N: usize>(
    encoded: Option<&str>,
    header: &'static str,
) -> Result<[u8; N], SignedHttpError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(required(encoded, header)?)
        .map_err(|source| SignedHttpError::InvalidBase64 { header, source })?;
    bytes
        .as_slice()
        .try_into()
        .map_err(|_| SignedHttpError::InvalidLength {
            header,
            actual: bytes.len(),
            expected: N,
        })
}

fn verify_signature(
    public_key: [u8; 32],
    message: &[u8],
    signature: [u8; 64],
    identity: String,
) -> Result<(), SignedHttpError> {
    let public_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| SignedHttpError::InvalidPublicKey { identity })?;
    public_key
        .verify_strict(message, &Signature::from_bytes(&signature))
        .map_err(|_| SignedHttpError::InvalidSignature)
}

#[derive(Clone, Debug)]
pub struct AllowedLeaders {
    keys: HashMap<(String, u64), [u8; 32]>,
    source_path: Option<PathBuf>,
}

impl AllowedLeaders {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, SignedHttpError> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        let file: AllowedLeadersFileV1 = serde_json::from_slice(&bytes)
            .map_err(|error| SignedHttpError::InvalidAllowedLeadersFile(error.to_string()))?;
        let mut allowed = Self::try_from(file)?;
        allowed.source_path = Some(path.to_path_buf());
        Ok(allowed)
    }

    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

impl LeaderKeyResolver for AllowedLeaders {
    fn leader_public_key(&self, leader_id: &str, leader_key_id: u64) -> Option<[u8; 32]> {
        self.keys
            .get(&(leader_id.to_string(), leader_key_id))
            .copied()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AllowedLeadersFileV1 {
    pub version: u8,
    pub leaders: Vec<AllowedLeaderFileV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AllowedLeaderFileV1 {
    pub leader_id: String,
    pub keys: Vec<AllowedLeaderKeyFileV1>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AllowedLeaderKeyFileV1 {
    pub kid: u64,
    pub public_key: String,
}

impl TryFrom<AllowedLeadersFileV1> for AllowedLeaders {
    type Error = SignedHttpError;

    fn try_from(file: AllowedLeadersFileV1) -> Result<Self, Self::Error> {
        if file.version != 1 {
            return Err(SignedHttpError::InvalidAllowedLeadersFile(format!(
                "unsupported version {}, expected 1",
                file.version
            )));
        }
        let mut keys = HashMap::new();
        for leader in file.leaders {
            for key in leader.keys {
                let public_key = decode_public_key(&key.public_key)?;
                if keys
                    .insert((leader.leader_id.clone(), key.kid), public_key)
                    .is_some()
                {
                    return Err(SignedHttpError::InvalidAllowedLeadersFile(format!(
                        "duplicate leader key leader_id={} kid={}",
                        leader.leader_id, key.kid
                    )));
                }
            }
        }
        Ok(Self {
            keys,
            source_path: None,
        })
    }
}

fn decode_public_key(value: &str) -> Result<[u8; 32], SignedHttpError> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    let bytes = hex::decode(raw).map_err(|error| {
        SignedHttpError::InvalidAllowedLeadersFile(format!(
            "invalid Ed25519 public key '{value}': {error}"
        ))
    })?;
    bytes.as_slice().try_into().map_err(|_| {
        SignedHttpError::InvalidAllowedLeadersFile(format!(
            "invalid Ed25519 public key length {}, expected 32",
            bytes.len()
        ))
    })
}
