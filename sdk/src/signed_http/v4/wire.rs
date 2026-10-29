//! Signed HTTP v4 header encoding and authentication transcripts.

pub use crate::signed_http::v3::wire::{
    LeaderKeyResolver,
    CANONICAL_TOOL_RESPONSE_CONTENT_TYPE,
    HEADER_INPUT_HASH,
    HEADER_LEADER_ID,
    HEADER_LEADER_KEY_ID,
    HEADER_LEADER_SIGNATURE,
    HEADER_NONCE,
    HEADER_SIGNATURE_VERSION,
    HEADER_TOOL_SIGNATURE,
};
use {
    super::error::SignedHttpError,
    base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _},
    ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey},
};

pub const HEADER_TOOL_ID: &str = "X-Nexus-Tool-Id";
pub const SIGNATURE_VERSION_V4: &str = "4";
pub const SIGNED_HTTP_V4_LEADER_REQUEST_DOMAIN: &[u8] = b"nexus.signed_http.v4.leader_request";
pub const SHA256_LEN: usize = 32;
pub const ED25519_SIGNATURE_LEN: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedRequestHeaders {
    pub leader_id: String,
    pub leader_key_id: u64,
    pub tool_id: String,
    pub input_hash: String,
    pub leader_signature: String,
    pub nonce: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedResponseHeaders {
    pub tool_signature: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RequestHeadersRef<'a> {
    pub signature_version: Option<&'a str>,
    pub leader_id: Option<&'a str>,
    pub leader_key_id: Option<&'a str>,
    pub tool_id: Option<&'a str>,
    pub input_hash: Option<&'a str>,
    pub leader_signature: Option<&'a str>,
    pub nonce: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ResponseHeadersRef<'a> {
    pub signature_version: Option<&'a str>,
    pub tool_signature: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedRequest {
    pub leader_id: String,
    pub leader_key_id: u64,
    pub tool_id: [u8; SHA256_LEN],
    pub input_hash: [u8; SHA256_LEN],
    pub leader_signature: [u8; ED25519_SIGNATURE_LEN],
    pub nonce: [u8; SHA256_LEN],
}

impl EncodedRequestHeaders {
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V4.to_string()),
            (HEADER_LEADER_ID, self.leader_id.clone()),
            (HEADER_LEADER_KEY_ID, self.leader_key_id.to_string()),
            (HEADER_TOOL_ID, self.tool_id.clone()),
            (HEADER_INPUT_HASH, self.input_hash.clone()),
            (HEADER_LEADER_SIGNATURE, self.leader_signature.clone()),
            (HEADER_NONCE, self.nonce.clone()),
        ]
    }
}

impl EncodedResponseHeaders {
    pub fn to_pairs(&self) -> Vec<(&'static str, String)> {
        vec![
            (HEADER_SIGNATURE_VERSION, SIGNATURE_VERSION_V4.to_string()),
            (HEADER_TOOL_SIGNATURE, self.tool_signature.clone()),
        ]
    }
}

impl<'a> RequestHeadersRef<'a> {
    pub fn from_getter(mut get: impl FnMut(&str) -> Option<&'a str>) -> Self {
        Self {
            signature_version: get(HEADER_SIGNATURE_VERSION),
            leader_id: get(HEADER_LEADER_ID),
            leader_key_id: get(HEADER_LEADER_KEY_ID),
            tool_id: get(HEADER_TOOL_ID),
            input_hash: get(HEADER_INPUT_HASH),
            leader_signature: get(HEADER_LEADER_SIGNATURE),
            nonce: get(HEADER_NONCE),
        }
    }
}

impl<'a> ResponseHeadersRef<'a> {
    pub fn from_getter(mut get: impl FnMut(&str) -> Option<&'a str>) -> Self {
        Self {
            signature_version: get(HEADER_SIGNATURE_VERSION),
            tool_signature: get(HEADER_TOOL_SIGNATURE),
        }
    }
}

pub fn sign_request(
    leader_id: impl Into<String>,
    leader_key_id: u64,
    tool_id: [u8; SHA256_LEN],
    nonce: [u8; SHA256_LEN],
    input_hash: [u8; SHA256_LEN],
    signing_key: &SigningKey,
) -> EncodedRequestHeaders {
    let message = leader_request_message(&tool_id, &nonce, &input_hash);
    let signature = signing_key.sign(&message).to_bytes();
    EncodedRequestHeaders {
        leader_id: leader_id.into(),
        leader_key_id,
        tool_id: URL_SAFE_NO_PAD.encode(tool_id),
        input_hash: URL_SAFE_NO_PAD.encode(input_hash),
        leader_signature: URL_SAFE_NO_PAD.encode(signature),
        nonce: URL_SAFE_NO_PAD.encode(nonce),
    }
}

pub fn authenticate_request(
    headers: RequestHeadersRef<'_>,
    keys: &dyn LeaderKeyResolver,
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
    let tool_id = decode_array::<SHA256_LEN>(headers.tool_id, HEADER_TOOL_ID)?;
    let input_hash = decode_array::<SHA256_LEN>(headers.input_hash, HEADER_INPUT_HASH)?;
    let leader_signature =
        decode_array::<ED25519_SIGNATURE_LEN>(headers.leader_signature, HEADER_LEADER_SIGNATURE)?;
    let nonce = decode_array::<SHA256_LEN>(headers.nonce, HEADER_NONCE)?;
    let public_key = keys
        .leader_public_key(&leader_id, leader_key_id)
        .ok_or_else(|| SignedHttpError::UnknownLeaderKey {
            leader_id: leader_id.clone(),
            leader_key_id,
        })?;
    verify_signature(
        public_key,
        &leader_request_message(&tool_id, &nonce, &input_hash),
        leader_signature,
        format!("leader {leader_id} key {leader_key_id}"),
    )?;
    Ok(AuthenticatedRequest {
        leader_id,
        leader_key_id,
        tool_id,
        input_hash,
        leader_signature,
        nonce,
    })
}

pub fn leader_request_message(
    tool_id: &[u8; SHA256_LEN],
    nonce: &[u8; SHA256_LEN],
    input_hash: &[u8; SHA256_LEN],
) -> Vec<u8> {
    let mut message =
        Vec::with_capacity(SIGNED_HTTP_V4_LEADER_REQUEST_DOMAIN.len() + 3 * SHA256_LEN);
    message.extend_from_slice(SIGNED_HTTP_V4_LEADER_REQUEST_DOMAIN);
    message.extend_from_slice(tool_id);
    message.extend_from_slice(nonce);
    message.extend_from_slice(input_hash);
    message
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
    tool_public_key: [u8; SHA256_LEN],
) -> Result<[u8; ED25519_SIGNATURE_LEN], SignedHttpError> {
    require_version(headers.signature_version)?;
    let signature =
        decode_array::<ED25519_SIGNATURE_LEN>(headers.tool_signature, HEADER_TOOL_SIGNATURE)?;
    let message = crate::commitments::tool_signature_message(leader_signature, nonce, result_bytes);
    verify_signature(
        tool_public_key,
        &message,
        signature,
        "Tool response key".to_string(),
    )?;
    Ok(signature)
}

fn required<'a>(value: Option<&'a str>, header: &'static str) -> Result<&'a str, SignedHttpError> {
    value.ok_or(SignedHttpError::MissingHeader(header))
}

fn require_version(version: Option<&str>) -> Result<(), SignedHttpError> {
    let version = required(version, HEADER_SIGNATURE_VERSION)?;
    if version != SIGNATURE_VERSION_V4 {
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
    public_key: [u8; SHA256_LEN],
    message: &[u8],
    signature: [u8; ED25519_SIGNATURE_LEN],
    identity: String,
) -> Result<(), SignedHttpError> {
    let public_key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| SignedHttpError::InvalidPublicKey { identity })?;
    public_key
        .verify_strict(message, &Signature::from_bytes(&signature))
        .map_err(|_| SignedHttpError::InvalidSignature)
}
