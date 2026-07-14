//! Canonical wire helpers for the built-in RegisteredKey verifier.

use {
    crate::move_bindings::{
        interface::verifier::{CanonicalToolInput, RegisteredKeyAuxiliary},
        primitives::data::NexusData,
    },
    anyhow::{bail, Context as _},
    sha2::{Digest as _, Sha256},
    std::collections::HashMap,
};

pub const SHA256_LEN: usize = 32;
pub const ED25519_SIGNATURE_LEN: usize = 64;

/// Sort Tool input ports by raw UTF-8 bytes and project the exact Move BCS shape.
pub fn canonical_tool_inputs(input_ports: &HashMap<String, NexusData>) -> Vec<CanonicalToolInput> {
    let mut port_names = input_ports.keys().collect::<Vec<_>>();
    port_names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    port_names
        .into_iter()
        .map(|port_name| {
            CanonicalToolInput::new(
                port_name.as_bytes().to_vec(),
                input_ports
                    .get(port_name)
                    .expect("a key collected from the map must still exist")
                    .clone(),
            )
        })
        .collect()
}

/// BCS bytes hashed by both the leader and `dag::effective_input_payload_sha256`.
pub fn canonical_tool_inputs_bcs(
    input_ports: &HashMap<String, NexusData>,
) -> anyhow::Result<Vec<u8>> {
    bcs::to_bytes(&canonical_tool_inputs(input_ports)).map_err(Into::into)
}

/// `request_hash = SHA-256(BCS(vector<CanonicalToolInput>))`.
pub fn canonical_tool_inputs_sha256(
    input_ports: &HashMap<String, NexusData>,
) -> anyhow::Result<[u8; SHA256_LEN]> {
    Ok(sha256(&canonical_tool_inputs_bcs(input_ports)?))
}

pub fn result_sha256(result_bytes: &[u8]) -> [u8; SHA256_LEN] {
    sha256(result_bytes)
}

/// Tool signature message: `leader_signature || SHA-256(result_bytes)`.
pub fn tool_signature_message(
    leader_signature: &[u8; ED25519_SIGNATURE_LEN],
    result_bytes: &[u8],
) -> Vec<u8> {
    let mut message = Vec::with_capacity(ED25519_SIGNATURE_LEN + SHA256_LEN);
    message.extend_from_slice(leader_signature);
    message.extend_from_slice(&result_sha256(result_bytes));
    message
}

pub fn registered_key_auxiliary(
    input_hash: [u8; SHA256_LEN],
    leader_signature: [u8; ED25519_SIGNATURE_LEN],
    tool_signature: [u8; ED25519_SIGNATURE_LEN],
) -> RegisteredKeyAuxiliary {
    RegisteredKeyAuxiliary::new(
        input_hash.to_vec(),
        leader_signature.to_vec(),
        tool_signature.to_vec(),
    )
}

pub fn encode_registered_key_auxiliary(
    auxiliary: &RegisteredKeyAuxiliary,
) -> anyhow::Result<Vec<u8>> {
    validate_registered_key_auxiliary(auxiliary)?;
    bcs::to_bytes(auxiliary).map_err(Into::into)
}

pub fn decode_registered_key_auxiliary(bytes: &[u8]) -> anyhow::Result<RegisteredKeyAuxiliary> {
    let auxiliary: RegisteredKeyAuxiliary =
        bcs::from_bytes(bytes).context("RegisteredKey auxiliary is not exact BCS")?;
    validate_registered_key_auxiliary(&auxiliary)?;
    if bcs::to_bytes(&auxiliary)? != bytes {
        bail!("RegisteredKey auxiliary is not canonically encoded");
    }
    Ok(auxiliary)
}

pub fn validate_registered_key_auxiliary(auxiliary: &RegisteredKeyAuxiliary) -> anyhow::Result<()> {
    if auxiliary.input_hash.len() != SHA256_LEN {
        bail!("RegisteredKey input hash must be {SHA256_LEN} bytes");
    }
    if auxiliary.leader_signature.len() != ED25519_SIGNATURE_LEN {
        bail!("RegisteredKey leader signature must be {ED25519_SIGNATURE_LEN} bytes");
    }
    if auxiliary.tool_signature.len() != ED25519_SIGNATURE_LEN {
        bail!("RegisteredKey tool signature must be {ED25519_SIGNATURE_LEN} bytes");
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> [u8; SHA256_LEN] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline(value: &[u8]) -> NexusData {
        NexusData::new(b"inline".to_vec(), value.to_vec(), vec![])
    }

    #[test]
    fn canonical_input_hash_uses_raw_port_order_and_exact_move_shape() {
        let inputs = HashMap::from([
            ("z".to_string(), inline(b"Z")),
            ("aa".to_string(), inline(b"A")),
        ]);
        let canonical = canonical_tool_inputs(&inputs);
        assert_eq!(canonical[0].port_name, b"aa");
        assert_eq!(canonical[1].port_name, b"z");
        assert_eq!(
            hex::encode(canonical_tool_inputs_bcs(&inputs).unwrap()),
            "0202616106696e6c696e65014100017a06696e6c696e65015a00"
        );
        assert_eq!(
            hex::encode(canonical_tool_inputs_sha256(&inputs).unwrap()),
            "a74c4268147c51e92a2d25b8436ac425fca6ba475bed23df0c66015f8d3b71f8"
        );
    }

    #[test]
    fn auxiliary_roundtrips_and_rejects_wrong_lengths_or_trailing_bytes() {
        let auxiliary = registered_key_auxiliary([1; 32], [2; 64], [3; 64]);
        let encoded = encode_registered_key_auxiliary(&auxiliary).unwrap();
        assert_eq!(
            decode_registered_key_auxiliary(&encoded).unwrap(),
            auxiliary
        );

        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_registered_key_auxiliary(&trailing).is_err());
        assert!(
            encode_registered_key_auxiliary(&RegisteredKeyAuxiliary::new(
                vec![1; 31],
                vec![2; 64],
                vec![3; 64],
            ))
            .is_err()
        );
    }

    #[test]
    fn tool_message_is_leader_signature_followed_by_result_hash() {
        let leader_signature = [7; 64];
        let message = tool_signature_message(&leader_signature, b"result");
        assert_eq!(&message[..64], &leader_signature);
        assert_eq!(&message[64..], result_sha256(b"result"));
    }
}
