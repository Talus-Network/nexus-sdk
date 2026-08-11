//! Shared transcript hashing for on-chain verification and signed HTTP.

use sha2::{Digest as _, Sha256};

pub const RAW_OUTPUT_DOMAIN: &[u8] = b"nexus.direct.v1.raw-output";
pub const TOOL_RESPONSE_DOMAIN: &[u8] = b"nexus.direct.v1.tool-response";

pub fn output_sha256(canonical_response_bcs: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(RAW_OUTPUT_DOMAIN);
    hasher.update(canonical_response_bcs);
    hasher.finalize().into()
}

pub fn tool_signature_message(
    leader_signature: &[u8; 64],
    nonce: &[u8; 32],
    canonical_response_bcs: &[u8],
) -> Vec<u8> {
    let mut message = Vec::with_capacity(TOOL_RESPONSE_DOMAIN.len() + 64 + 32 + 32);
    message.extend_from_slice(TOOL_RESPONSE_DOMAIN);
    message.extend_from_slice(leader_signature);
    message.extend_from_slice(nonce);
    message.extend_from_slice(&output_sha256(canonical_response_bcs));
    message
}
