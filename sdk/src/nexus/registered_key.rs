//! Canonical commitment helpers for the built-in RegisteredKey verifier.

use {
    crate::{
        move_bindings::{
            interface::{
                meta_schema::MetaSchema,
                verifier::{RegisteredKeyAuxiliary, ToolInvocationNoncePreimage},
            },
            sui_framework::object::ID,
        },
        sui,
        types::{NexusData, NexusValue},
    },
    anyhow::{bail, Context as _},
    sha2::{Digest as _, Sha256},
    std::collections::HashMap,
};

pub const SHA256_LEN: usize = 32;
pub const ED25519_SIGNATURE_LEN: usize = 64;
pub const INVOCATION_NONCE_DOMAIN: &[u8] = b"nexus.direct.v3.invocation-nonce";
pub use crate::commitments::{
    output_sha256,
    tool_signature_message,
    RAW_OUTPUT_DOMAIN,
    TOOL_RESPONSE_DOMAIN,
};

/// Returns the exact schema-ordered content commitment computed by Move.
pub fn canonical_tool_inputs_sha256(
    schema: &MetaSchema,
    input_ports: &HashMap<String, NexusData>,
) -> anyhow::Result<[u8; SHA256_LEN]> {
    schema.canonical_inputs_sha256(input_ports)
}

/// Returns the exact schema-ordered resolved-content commitment computed by Move.
pub fn canonical_resolved_tool_inputs_sha256(
    schema: &MetaSchema,
    input_ports: &HashMap<String, Vec<NexusValue>>,
) -> anyhow::Result<[u8; SHA256_LEN]> {
    schema.resolved_inputs_sha256(input_ports)
}

/// Deterministic identity for one logical off-chain Tool invocation.
pub fn invocation_nonce(
    execution_id: sui::types::Address,
    walk_index: u64,
    vertex_name: impl Into<Vec<u8>>,
    iteration: u64,
) -> anyhow::Result<[u8; SHA256_LEN]> {
    let preimage = ToolInvocationNoncePreimage::new(
        ID::new(execution_id),
        walk_index,
        vertex_name.into(),
        iteration,
    );
    let encoded = bcs::to_bytes(&preimage).context("failed to encode invocation nonce preimage")?;
    Ok(domain_sha256(INVOCATION_NONCE_DOMAIN, &encoded))
}

pub fn registered_key_auxiliary(
    input_hash: [u8; SHA256_LEN],
    nonce: [u8; SHA256_LEN],
    leader_signature: [u8; ED25519_SIGNATURE_LEN],
    tool_signature: [u8; ED25519_SIGNATURE_LEN],
) -> RegisteredKeyAuxiliary {
    RegisteredKeyAuxiliary::new(
        input_hash.to_vec(),
        nonce.to_vec(),
        leader_signature.to_vec(),
        tool_signature.to_vec(),
    )
}

pub fn validate_registered_key_auxiliary(auxiliary: &RegisteredKeyAuxiliary) -> anyhow::Result<()> {
    if auxiliary.input_hash.len() != SHA256_LEN {
        bail!("RegisteredKey input hash must be {SHA256_LEN} bytes");
    }
    if auxiliary.nonce.len() != SHA256_LEN {
        bail!("RegisteredKey nonce must be {SHA256_LEN} bytes");
    }
    if auxiliary.leader_signature.len() != ED25519_SIGNATURE_LEN {
        bail!("RegisteredKey leader signature must be {ED25519_SIGNATURE_LEN} bytes");
    }
    if auxiliary.tool_signature.len() != ED25519_SIGNATURE_LEN {
        bail!("RegisteredKey tool signature must be {ED25519_SIGNATURE_LEN} bytes");
    }
    Ok(())
}

fn domain_sha256(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_LEN] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::move_bindings::interface::meta_schema::{
            OutputVariantSchema,
            PortSchema,
            ValueKind,
        },
    };

    fn schema() -> MetaSchema {
        MetaSchema::new(
            ["z", "aa"]
                .into_iter()
                .map(|name| PortSchema::new(name.as_bytes().to_vec(), false, ValueKind::Data))
                .collect(),
            vec![OutputVariantSchema::new(b"ok".to_vec(), vec![])],
        )
    }

    fn object_schema() -> MetaSchema {
        MetaSchema::new(
            vec![PortSchema::new(
                b"object".to_vec(),
                false,
                ValueKind::Object,
            )],
            vec![OutputVariantSchema::new(b"ok".to_vec(), vec![])],
        )
    }

    fn object_inputs() -> HashMap<String, NexusData> {
        HashMap::from([(
            "object".to_string(),
            NexusData::object(sui::types::Address::from_static("0x1")),
        )])
    }

    fn inputs() -> HashMap<String, NexusData> {
        HashMap::from([
            ("aa".to_string(), NexusData::inline_data(b"A").unwrap()),
            ("z".to_string(), NexusData::inline_data(b"Z").unwrap()),
        ])
    }

    fn resolved_inputs() -> HashMap<String, Vec<NexusValue>> {
        inputs()
            .into_iter()
            .map(|(name, value)| (name, value.into_values().unwrap()))
            .collect()
    }

    #[test]
    fn direct_input_commitments_match_move_goldens() {
        let canonical = canonical_tool_inputs_sha256(&schema(), &inputs()).unwrap();
        let resolved =
            canonical_resolved_tool_inputs_sha256(&schema(), &resolved_inputs()).unwrap();
        assert_eq!(canonical, resolved);
        assert_eq!(
            hex::encode(canonical),
            "c4c6b94a64dfa36bbc1261eb8cd2ea030a0187cb6f66b34571be80aa1539f858"
        );
    }

    #[test]
    fn empty_many_is_rejected_before_registered_key_commitment() {
        let schema_for = |kind| {
            MetaSchema::new(
                vec![PortSchema::new(b"values".to_vec(), true, kind)],
                vec![OutputVariantSchema::new(b"ok".to_vec(), vec![])],
            )
        };
        let inputs = HashMap::from([(
            "values".to_string(),
            NexusData::new(b"nexus_value".to_vec(), Vec::new(), Vec::new()),
        )]);
        let data_error = canonical_tool_inputs_sha256(&schema_for(ValueKind::Data), &inputs)
            .expect_err("empty Many must fail before commitment");
        let object_error = canonical_tool_inputs_sha256(&schema_for(ValueKind::Object), &inputs)
            .expect_err("empty Many must fail before commitment");

        assert!(data_error.to_string().contains("does not conform"));
        assert!(object_error.to_string().contains("does not conform"));
    }

    #[test]
    fn compatible_port_relabeling_changes_the_authenticated_hash() {
        let z = NexusData::inline_data(b"Z")
            .unwrap()
            .to_json_value()
            .unwrap();
        let aa = NexusData::inline_data(b"A")
            .unwrap()
            .to_json_value()
            .unwrap();
        let schema = schema();
        let original_inputs = schema
            .resolved_inputs_from_json(&serde_json::json!({
                "ports": [
                    { "port_name": "z", "value": z },
                    { "port_name": "aa", "value": aa },
                ]
            }))
            .unwrap();
        let original = schema.resolved_inputs_sha256(&original_inputs).unwrap();
        let relabeled_inputs = schema
            .resolved_inputs_from_json(&serde_json::json!({
                "ports": [
                    { "port_name": "aa", "value": NexusData::inline_data(b"Z").unwrap().to_json_value().unwrap() },
                    { "port_name": "z", "value": NexusData::inline_data(b"A").unwrap().to_json_value().unwrap() },
                ]
            }))
            .unwrap();
        let relabeled = schema.resolved_inputs_sha256(&relabeled_inputs).unwrap();

        assert_ne!(original, relabeled);
    }

    #[test]
    fn worksheet_input_commitment_accepts_onchain_object_ports() {
        canonical_tool_inputs_sha256(&object_schema(), &object_inputs())
            .expect("on-chain Object input should produce a worksheet commitment");
    }

    #[test]
    fn resolved_input_commitment_rejects_offchain_object_ports() {
        let object_inputs = HashMap::from([(
            "object".to_string(),
            vec![NexusValue::object(sui::types::Address::from_static("0x1"))],
        )]);
        let error = canonical_resolved_tool_inputs_sha256(&object_schema(), &object_inputs)
            .expect_err("off-chain resolved input must remain Data-only");

        assert_eq!(
            error.to_string(),
            "off-chain input ports must contain opaque Data values"
        );
    }

    #[test]
    fn invocation_nonce_matches_move_golden() {
        assert_eq!(
            hex::encode(
                invocation_nonce(sui::types::Address::from_static("0xe"), 3, b"vertex", 0).unwrap()
            ),
            "7dc2b7b7addb953fc231afd71d9444beca7b253f1e45481bc50d4ac6cd0e4c32"
        );
    }

    #[test]
    fn tool_message_contains_the_direct_output_hash() {
        let message = tool_signature_message(&[7; 64], &[8; 32], b"result");
        assert!(message.starts_with(TOOL_RESPONSE_DOMAIN));
        assert_eq!(
            &message[TOOL_RESPONSE_DOMAIN.len() + 96..],
            output_sha256(b"result")
        );
    }
}
