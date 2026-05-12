use {
    crate::{
        nexus::{
            crawler::Crawler,
            models::{
                Dag,
                DagInputPort,
                DagPortDataBcs,
                DagVertexEvaluationsBcs,
                DagVertexInputPort,
            },
        },
        sui::types::Address,
        types::{DataStorage, NexusData, RuntimeVertex, Storable, StorageKind, TypeName},
    },
    anyhow::{anyhow, bail},
    serde::{Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::collections::{HashMap, HashSet},
};

const ERR_EVAL_VARIANT: &str = "_err_eval";
const INLINE_STORAGE_TAG: &[u8] = b"inline";
const WALRUS_STORAGE_TAG: &[u8] = b"walrus";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct CanonicalNexusDataWire {
    pub storage: Vec<u8>,
    pub one: Vec<u8>,
    pub many: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct RegisteredKeyTranscriptV1Wire {
    pub version: u8,
    pub execution: Address,
    pub walk_index: u64,
    pub vertex_name: Vec<u8>,
    pub tool_fqn: Vec<u8>,
    pub leader_cap_id: Address,
    pub request_leader_kid: u64,
    pub resolved_leader_kid: u64,
    pub response_tool_kid: u64,
    pub resolved_tool_kid: u64,
    pub request_method: Vec<u8>,
    pub request_path: Vec<u8>,
    pub request_query: Vec<u8>,
    pub request_sig_input_sha256: Vec<u8>,
    pub response_req_sig_input_sha256: Vec<u8>,
    pub request_body_sha256: Vec<u8>,
    pub response_body_sha256: Vec<u8>,
    pub request_signature: Vec<u8>,
    pub response_signature: Vec<u8>,
    pub response_status: u16,
    pub outcome: u8,
    pub payload_sha256: Vec<u8>,
}

pub fn canonical_request_body_sha256(
    input_ports: &HashMap<String, NexusData>,
) -> anyhow::Result<[u8; 32]> {
    let mut bytes = Vec::new();
    let mut port_names = input_ports.keys().cloned().collect::<Vec<_>>();
    port_names.sort();

    for port_name in port_names {
        bytes.extend_from_slice(&bcs::to_bytes(&port_name.as_bytes().to_vec())?);
        bytes.extend_from_slice(&bcs::to_bytes(&canonical_request_nexus_data_wire(
            input_ports
                .get(&port_name)
                .expect("port name collected from keys must exist"),
        )?)?);
    }

    Ok(hash_bytes(&bytes))
}

/// Derive the canonical request-body hash from on-chain DAG evaluations and defaults.
pub async fn derive_onchain_request_body_sha256(
    crawler: &Crawler,
    dag: &Dag,
    vertex_name: &TypeName,
    expected_vertex: &RuntimeVertex,
    evaluations_object_id: Address,
    declared_input_ports: &HashSet<String>,
) -> anyhow::Result<[u8; 32]> {
    let evaluations_response = crawler
        .get_object_contents_bcs::<DagVertexEvaluationsBcs<CanonicalNexusDataWire>>(
            evaluations_object_id,
        )
        .await?;
    let default_values = crawler
        .get_dynamic_fields_bcs::<DagVertexInputPort, CanonicalNexusDataWire>(
            dag.defaults_to_input_ports.id(),
            dag.defaults_to_input_ports.size(),
        )
        .await?;

    derive_request_body_sha256_from_onchain_data(
        vertex_name,
        expected_vertex,
        declared_input_ports,
        &evaluations_response.data.ports_to_data.into_inner(),
        &default_values,
    )
}

pub fn canonical_output_payload_sha256(
    output_variant: &str,
    output_ports_data: &HashMap<String, DataStorage>,
) -> anyhow::Result<[u8; 32]> {
    let mut bytes = bcs::to_bytes(&output_variant.as_bytes().to_vec())?;
    let mut port_names = output_ports_data.keys().cloned().collect::<Vec<_>>();
    port_names.sort();

    for port_name in port_names {
        bytes.extend_from_slice(&bcs::to_bytes(&port_name.as_bytes().to_vec())?);
        bytes.extend_from_slice(&bcs::to_bytes(&canonical_nexus_data_wire(
            output_ports_data
                .get(&port_name)
                .expect("port name collected from keys must exist"),
        )?)?);
    }

    Ok(hash_bytes(&bytes))
}

pub fn registered_key_payload_sha256(
    output_variant: &str,
    output_ports_data: &HashMap<String, DataStorage>,
) -> anyhow::Result<[u8; 32]> {
    if output_variant == ERR_EVAL_VARIANT {
        return Ok(hash_bytes(&terminal_err_eval_reason_bytes(
            output_variant,
            output_ports_data,
        )?));
    }

    canonical_output_payload_sha256(output_variant, output_ports_data)
}

pub fn canonical_nexus_data_wire(value: &DataStorage) -> anyhow::Result<CanonicalNexusDataWire> {
    let storage = match value.storage_kind() {
        StorageKind::Inline => INLINE_STORAGE_TAG.to_vec(),
        StorageKind::Walrus => WALRUS_STORAGE_TAG.to_vec(),
    };
    let (one, many) = match value.as_json() {
        serde_json::Value::Array(values) => (
            Vec::new(),
            values
                .iter()
                .map(serde_json::to_string)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(String::into_bytes)
                .collect(),
        ),
        other => (serde_json::to_string(other)?.into_bytes(), Vec::new()),
    };

    Ok(CanonicalNexusDataWire { storage, one, many })
}

pub fn canonical_request_nexus_data_wire(
    value: &NexusData,
) -> anyhow::Result<CanonicalNexusDataWire> {
    bcs::from_bytes::<CanonicalNexusDataWire>(&bcs::to_bytes(value)?).map_err(anyhow::Error::new)
}

#[allow(clippy::too_many_arguments)]
pub fn encode_registered_key_transcript_v1(
    execution: Address,
    walk_index: u64,
    vertex_name: &str,
    tool_fqn: &str,
    leader_cap_id: Address,
    transcript: &crate::signed_http::v1::engine::SignedInvokeTranscriptV1,
    output_variant: &str,
    output_ports_data: &HashMap<String, DataStorage>,
) -> anyhow::Result<Vec<u8>> {
    let payload_sha256 = registered_key_payload_sha256(output_variant, output_ports_data)?;
    let outcome = if output_variant == ERR_EVAL_VARIANT {
        1
    } else {
        0
    };

    bcs::to_bytes(&RegisteredKeyTranscriptV1Wire {
        version: 1,
        execution,
        walk_index,
        vertex_name: vertex_name.as_bytes().to_vec(),
        tool_fqn: tool_fqn.as_bytes().to_vec(),
        leader_cap_id,
        request_leader_kid: transcript.request_leader_kid,
        resolved_leader_kid: transcript.resolved_leader_kid,
        response_tool_kid: transcript.response_tool_kid,
        resolved_tool_kid: transcript.resolved_tool_kid,
        request_method: transcript.request_method.as_bytes().to_vec(),
        request_path: transcript.request_path.as_bytes().to_vec(),
        request_query: transcript.request_query.as_bytes().to_vec(),
        request_sig_input_sha256: transcript.request_sig_input_sha256.to_vec(),
        response_req_sig_input_sha256: transcript.response_req_sig_input_sha256.to_vec(),
        request_body_sha256: transcript.request_body_sha256.to_vec(),
        response_body_sha256: transcript.response_body_sha256.to_vec(),
        request_signature: transcript.request_signature.to_vec(),
        response_signature: transcript.response_signature.to_vec(),
        response_status: transcript.response_status,
        outcome,
        payload_sha256: payload_sha256.to_vec(),
    })
    .map_err(Into::into)
}

fn runtime_vertex_input(
    vertex_name: &TypeName,
    expected_vertex: &RuntimeVertex,
    declared_port_name: &str,
    evaluations: &HashMap<TypeName, DagPortDataBcs<CanonicalNexusDataWire>>,
    default_values: &HashMap<DagVertexInputPort, CanonicalNexusDataWire>,
) -> anyhow::Result<CanonicalNexusDataWire> {
    let port_key = TypeName::new(declared_port_name);
    if let Some(port_data) = evaluations.get(&port_key) {
        return match port_data {
            DagPortDataBcs::Single { data, .. } => Ok(data.clone()),
            DagPortDataBcs::Many { data, .. } => {
                let RuntimeVertex::WithIterator { iteration, .. } = expected_vertex else {
                    bail!("Expected single data for port '{declared_port_name}' in evaluations");
                };

                data.get(iteration)
                    .cloned()
                    .ok_or_else(|| anyhow!("No data for iteration '{iteration}'"))
            }
        };
    }

    default_values
        .get(&DagVertexInputPort {
            vertex: vertex_name.clone(),
            port: DagInputPort {
                name: declared_port_name.to_string(),
            },
        })
        .cloned()
        .ok_or_else(|| anyhow!("Missing input data for declared port '{declared_port_name}'"))
}

fn derive_request_body_sha256_from_onchain_data(
    vertex_name: &TypeName,
    expected_vertex: &RuntimeVertex,
    declared_input_ports: &HashSet<String>,
    evaluations: &HashMap<TypeName, DagPortDataBcs<CanonicalNexusDataWire>>,
    default_values: &HashMap<DagVertexInputPort, CanonicalNexusDataWire>,
) -> anyhow::Result<[u8; 32]> {
    let mut bytes = Vec::new();
    let mut ports = declared_input_ports.iter().cloned().collect::<Vec<_>>();
    ports.sort();

    for port_name in ports {
        let data = runtime_vertex_input(
            vertex_name,
            expected_vertex,
            &port_name,
            evaluations,
            default_values,
        )?;

        bytes.extend_from_slice(&bcs::to_bytes(&port_name.as_bytes().to_vec())?);
        bytes.extend_from_slice(&bcs::to_bytes(&data)?);
    }

    Ok(hash_bytes(&bytes))
}

fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn terminal_err_eval_reason_bytes(
    output_variant: &str,
    output_ports_data: &HashMap<String, DataStorage>,
) -> anyhow::Result<Vec<u8>> {
    if output_variant != ERR_EVAL_VARIANT {
        bail!("expected terminal err_eval output variant, got {output_variant}");
    }

    let reason = output_ports_data
        .get("reason")
        .ok_or_else(|| anyhow!("missing reason port in terminal err_eval output"))?;
    let Some(reason) = reason.as_json().as_str() else {
        bail!("terminal err_eval reason must be a JSON string");
    };
    Ok(reason.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{signed_http::v1::engine::SignedInvokeTranscriptV1, types::NexusData},
        serde_json::json,
    };

    fn inline_data(value: serde_json::Value) -> DataStorage {
        NexusData::new_inline(value).commit_inline_plain()
    }

    fn transcript() -> SignedInvokeTranscriptV1 {
        SignedInvokeTranscriptV1 {
            leader_id: "0x1111".to_string(),
            request_leader_kid: 7,
            resolved_leader_kid: 7,
            response_tool_kid: 9,
            resolved_tool_kid: 9,
            request_method: "POST".to_string(),
            request_path: "/dummy/invoke".to_string(),
            request_query: "".to_string(),
            request_sig_input_sha256: [1; 32],
            response_req_sig_input_sha256: [1; 32],
            request_body_sha256: [2; 32],
            response_body_sha256: [3; 32],
            request_signature: [4; 64],
            response_signature: [5; 64],
            response_status: 200,
        }
    }

    #[test]
    fn canonical_request_body_sha256_is_order_stable() {
        let ordered = HashMap::from([
            ("a".to_string(), NexusData::new_inline(json!("one"))),
            ("b".to_string(), NexusData::new_inline(json!(["two", 3]))),
        ]);
        let shuffled = HashMap::from([
            ("b".to_string(), NexusData::new_inline(json!(["two", 3]))),
            ("a".to_string(), NexusData::new_inline(json!("one"))),
        ]);

        assert_eq!(
            canonical_request_body_sha256(&ordered).expect("ordered hash"),
            canonical_request_body_sha256(&shuffled).expect("shuffled hash"),
        );
    }

    #[test]
    fn canonical_request_nexus_data_wire_uses_explicit_shape() {
        let input_ports = HashMap::from([
            ("a".to_string(), NexusData::new_inline(json!("one"))),
            ("b".to_string(), NexusData::new_inline(json!(["two", 3]))),
        ]);

        for port_name in ["a", "b"] {
            let actual =
                canonical_request_nexus_data_wire(input_ports.get(port_name).expect("port data"))
                    .expect("canonical request wire");
            let expected = match port_name {
                "a" => CanonicalNexusDataWire {
                    storage: INLINE_STORAGE_TAG.to_vec(),
                    one: br#""one""#.to_vec(),
                    many: Vec::new(),
                },
                "b" => CanonicalNexusDataWire {
                    storage: INLINE_STORAGE_TAG.to_vec(),
                    one: Vec::new(),
                    many: vec![br#""two""#.to_vec(), b"3".to_vec()],
                },
                _ => unreachable!(),
            };
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn canonical_nexus_data_wire_preserves_inline_one_and_many_shapes() {
        let scalar = canonical_nexus_data_wire(&inline_data(json!("one"))).expect("scalar wire");
        let many = canonical_nexus_data_wire(&inline_data(json!(["one", 2]))).expect("many wire");

        assert_eq!(
            scalar,
            CanonicalNexusDataWire {
                storage: INLINE_STORAGE_TAG.to_vec(),
                one: br#""one""#.to_vec(),
                many: Vec::new(),
            }
        );
        assert_eq!(
            many,
            CanonicalNexusDataWire {
                storage: INLINE_STORAGE_TAG.to_vec(),
                one: Vec::new(),
                many: vec![br#""one""#.to_vec(), b"2".to_vec()],
            }
        );
    }

    #[test]
    fn registered_key_payload_sha256_uses_reason_bytes_for_err_eval() {
        let output_ports_data =
            HashMap::from([("reason".to_string(), inline_data(json!("failure")))]);

        assert_eq!(
            registered_key_payload_sha256(ERR_EVAL_VARIANT, &output_ports_data)
                .expect("payload hash"),
            hash_bytes(b"failure"),
        );
    }

    #[test]
    fn derive_request_body_sha256_from_onchain_data_uses_defaults_and_iteration() {
        let vertex_name = TypeName::new("demo");
        let expected_vertex = RuntimeVertex::with_iterator("demo", 2, 4);
        let declared_input_ports = HashSet::from([
            "iter".to_string(),
            "missing".to_string(),
            "single".to_string(),
        ]);
        let iter_value = CanonicalNexusDataWire {
            storage: INLINE_STORAGE_TAG.to_vec(),
            one: b"2".to_vec(),
            many: Vec::new(),
        };
        let single_value = CanonicalNexusDataWire {
            storage: INLINE_STORAGE_TAG.to_vec(),
            one: br#""value""#.to_vec(),
            many: Vec::new(),
        };
        let default_value = CanonicalNexusDataWire {
            storage: INLINE_STORAGE_TAG.to_vec(),
            one: br#""fallback""#.to_vec(),
            many: Vec::new(),
        };
        let evaluations = HashMap::from([
            (
                TypeName::new("iter"),
                DagPortDataBcs::Many {
                    _variant_name: "Many".to_string(),
                    data: crate::nexus::models::BcsMap {
                        contents: vec![crate::nexus::models::BcsMapEntry {
                            key: 2,
                            value: iter_value.clone(),
                        }],
                    },
                    total_iterations: 4,
                },
            ),
            (
                TypeName::new("single"),
                DagPortDataBcs::Single {
                    _variant_name: "Single".to_string(),
                    data: single_value.clone(),
                    is_static: false,
                },
            ),
        ]);
        let default_values = HashMap::from([(
            DagVertexInputPort {
                vertex: vertex_name.clone(),
                port: DagInputPort {
                    name: "missing".to_string(),
                },
            },
            default_value.clone(),
        )]);

        let actual = derive_request_body_sha256_from_onchain_data(
            &vertex_name,
            &expected_vertex,
            &declared_input_ports,
            &evaluations,
            &default_values,
        )
        .expect("request-body hash");

        let mut expected_bytes = Vec::new();
        for (port_name, data) in [
            ("iter", iter_value),
            ("missing", default_value),
            ("single", single_value),
        ] {
            expected_bytes
                .extend_from_slice(&bcs::to_bytes(&port_name.as_bytes().to_vec()).unwrap());
            expected_bytes.extend_from_slice(&bcs::to_bytes(&data).unwrap());
        }

        assert_eq!(actual, hash_bytes(&expected_bytes));
    }

    #[test]
    fn encode_registered_key_transcript_v1_roundtrips() {
        let output_ports_data =
            HashMap::from([("message".to_string(), inline_data(json!("success")))]);
        let encoded = encode_registered_key_transcript_v1(
            "0x5".parse().expect("execution"),
            3,
            "dummy",
            "xyz.dummy.tool@1",
            "0x7".parse().expect("leader cap"),
            &transcript(),
            "ok",
            &output_ports_data,
        )
        .expect("transcript");

        let decoded: RegisteredKeyTranscriptV1Wire =
            bcs::from_bytes(&encoded).expect("registered-key transcript");
        assert_eq!(decoded.walk_index, 3);
        assert_eq!(decoded.vertex_name, b"dummy".to_vec());
        assert_eq!(decoded.tool_fqn, b"xyz.dummy.tool@1".to_vec());
        assert_eq!(decoded.request_method, b"POST".to_vec());
        assert_eq!(decoded.response_status, 200);
        assert_eq!(decoded.outcome, 0);
    }
}
