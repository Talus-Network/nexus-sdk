//! Define models for Sui Move objects related to Nexus DAGs.

use {
    crate::{
        nexus::crawler::{DynamicMap, Map, Set},
        sui,
        types::{
            deserialize_sui_u64,
            parse_string_value,
            serialize_sui_u64,
            strip_fields_owned,
            MoveOption,
            NexusData,
            TypeName,
            VerifierConfig,
        },
        ToolFqn,
    },
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// Struct holding the DAG definition from our Move code.
///
/// See <sui/workflow/sources/dag.move:DAG> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Dag {
    pub vertices: DynamicMap<TypeName, DagVertexInfo>,
    pub defaults_to_input_ports: DynamicMap<DagVertexInputPort, NexusData>,
    pub edges: DynamicMap<TypeName, Vec<DagEdge>>,
    pub outputs: DynamicMap<TypeName, Vec<DagOutputVariantPort>>,
    #[serde(default, skip_serializing_if = "VerifierConfig::is_none")]
    pub leader_verifier: VerifierConfig,
    #[serde(default, skip_serializing_if = "VerifierConfig::is_none")]
    pub tool_verifier: VerifierConfig,
}

/// Struct holding the DAG execution information.
///
/// See <sui/workflow/sources/dag.move:DAGExecution> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagExecution {
    /// The address of the sender of the transaction to trigger this DAG
    /// execution.
    pub invoker: sui::types::Address,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagVertexInfo {
    pub kind: DagVertexKind,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_verifier_config",
        skip_serializing_if = "Option::is_none"
    )]
    pub leader_verifier: Option<VerifierConfig>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_verifier_config",
        skip_serializing_if = "Option::is_none"
    )]
    pub tool_verifier: Option<VerifierConfig>,
    #[serde(default, skip_serializing_if = "Set::is_empty")]
    pub input_ports: Set<DagInputPort>,
}

impl DagVertexInfo {
    pub fn declared_input_port_names(&self) -> Vec<String> {
        let mut ports = self
            .input_ports
            .inner()
            .iter()
            .map(|port| port.name.clone())
            .collect::<Vec<_>>();
        ports.sort();
        ports
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "_variant_name")]
pub enum DagVertexKind {
    OffChain { tool_fqn: ToolFqn },
    OnChain { tool_fqn: ToolFqn },
}

impl DagVertexKind {
    pub fn tool_fqn(&self) -> &ToolFqn {
        match self {
            DagVertexKind::OffChain { tool_fqn } => tool_fqn,
            DagVertexKind::OnChain { tool_fqn } => tool_fqn,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct DagVertexInputPort {
    pub vertex: TypeName,
    pub port: DagInputPort,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct DagInputPort {
    pub name: String,
}

impl<'de> Deserialize<'de> for DagInputPort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Standard {
            name: String,
        }

        if !deserializer.is_human_readable() {
            let parsed = Standard::deserialize(deserializer)?;
            return Ok(Self { name: parsed.name });
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        if let Ok(parsed) = serde_json::from_value::<Standard>(value.clone()) {
            return Ok(Self { name: parsed.name });
        }

        let value = strip_fields_owned(value);
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("DagInputPort must be an object"))?;
        let name = object
            .get("name")
            .ok_or_else(|| serde::de::Error::custom("DagInputPort missing name"))?;
        let name = parse_string_value(name)
            .map_err(serde::de::Error::custom)?
            .ok_or_else(|| serde::de::Error::custom("DagInputPort name did not parse"))?;

        Ok(Self { name })
    }
}

fn deserialize_optional_verifier_config<'de, D>(
    deserializer: D,
) -> Result<Option<VerifierConfig>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    MoveOption::<VerifierConfig>::deserialize(deserializer).map(|value| value.0)
}

/// Enum distinguishing between a plain vertex and a vertex with an iterator.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "_variant_name")]
pub enum DagPortData {
    Single { data: NexusData },
    Many { data: Map<String, NexusData> },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BcsMapEntry<K, V> {
    pub key: K,
    pub value: V,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct BcsMap<K, V> {
    pub contents: Vec<BcsMapEntry<K, V>>,
}

impl<K, V> BcsMap<K, V>
where
    K: Eq + std::hash::Hash,
{
    pub fn get(&self, key: &K) -> Option<&V> {
        self.contents
            .iter()
            .find(|entry| &entry.key == key)
            .map(|entry| &entry.value)
    }

    pub fn into_inner(self) -> HashMap<K, V> {
        self.contents
            .into_iter()
            .map(|entry| (entry.key, entry.value))
            .collect()
    }
}

/// BCS-facing DAG port data used for on-chain object decoding.
///
/// This keeps the raw Move enum layout in the SDK while allowing callers to
/// choose the decoded value shape they need.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(bound(
    deserialize = "T: serde::de::DeserializeOwned",
    serialize = "T: Serialize"
))]
pub enum DagPortDataBcs<T> {
    Single {
        #[allow(dead_code)]
        _variant_name: String,
        data: T,
        #[allow(dead_code)]
        is_static: bool,
    },
    Many {
        #[allow(dead_code)]
        _variant_name: String,
        data: BcsMap<u64, T>,
        #[allow(dead_code)]
        total_iterations: u64,
    },
}

/// Struct holding the evaluations for a vertex in the DAG.
///
/// See <sui/workflow/sources/dag.move:VertexEvaluations> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagVertexEvaluations {
    pub ports_to_data: Map<TypeName, DagPortData>,
}

/// BCS-facing DAG evaluations object used for on-chain object decoding.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(bound(
    deserialize = "T: serde::de::DeserializeOwned",
    serialize = "T: Serialize"
))]
pub struct DagVertexEvaluationsBcs<T> {
    #[allow(dead_code)]
    pub id: sui::types::Address,
    pub ports_to_data: BcsMap<TypeName, DagPortDataBcs<T>>,
}

/// Struct holding the edges in the DAG.
///
/// See <sui/workflow/sources/dag.move:Edge> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagEdge {
    pub from: DagOutputVariantPort,
}

/// Struct holding the output variant and port pair.
///
/// See <sui/workflow/sources/dag.move:OutputVariantPort> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagOutputVariantPort {
    pub variant: TypeName,
    pub port: TypeName,
}

// == `GasService` related types ==

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Scope {
    Execution(sui::types::Address),
    WorksheetType(TypeName),
    InvokerAddress(sui::types::Address),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct InvokerGas {
    pub vault: DynamicMap<Scope, GasFunds>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GasFunds {
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub bal: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub locked: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionGas {
    pub claimed_leader_gas: DynamicMap<Vec<u8>, ClaimedGas>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClaimedGas {
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub execution: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub priority: u64,
}
#[cfg(test)]
mod tests {
    use {super::*, crate::fqn, serde_json::json, std::str::FromStr};

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct TestWireValue {
        bytes: Vec<u8>,
    }

    #[test]
    fn test_dag_vertex_kind_offchain_serde() {
        let kind = DagVertexKind::OffChain {
            tool_fqn: fqn!("xyz.example.tool@1"),
        };
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: DagVertexKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind.tool_fqn(), deserialized.tool_fqn());
    }

    #[test]
    fn test_dag_vertex_kind_onchain_serde() {
        let kind = DagVertexKind::OnChain {
            tool_fqn: fqn!("xyz.example.tool@1"),
        };
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: DagVertexKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind.tool_fqn(), deserialized.tool_fqn());
    }

    #[test]
    fn test_dag_port_data_single_serde() {
        let port_data = DagPortData::Single {
            data: NexusData::new_inline(json!(1)),
        };
        let json = serde_json::to_string(&port_data).unwrap();
        let _deserialized: DagPortData = serde_json::from_str(&json).unwrap();
    }

    #[test]
<<<<<<< Updated upstream
    fn test_dag_input_port_deserializes_wrapped_name() {
        let parsed: DagInputPort = serde_json::from_value(json!({
            "name": {
                "fields": {
                    "ascii": [105, 110, 112, 117, 116]
                }
            }
        }))
        .unwrap();

        assert_eq!(
            parsed,
            DagInputPort {
                name: "input".to_string(),
            }
        );
    }

    #[test]
    fn test_dag_input_port_deserializes_plain_name() {
        let parsed: DagInputPort = serde_json::from_value(json!({
            "name": "plain_input"
        }))
        .unwrap();

        assert_eq!(
            parsed,
            DagInputPort {
                name: "plain_input".to_string(),
            }
        );
    }

    #[test]
=======
>>>>>>> Stashed changes
    fn test_dag_vertex_evaluations_bcs_roundtrip() {
        let value = DagVertexEvaluationsBcs {
            id: sui::types::Address::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000007",
            )
            .unwrap(),
            ports_to_data: BcsMap {
                contents: vec![
                    BcsMapEntry {
                        key: TypeName::new("plain_port"),
                        value: DagPortDataBcs::Single {
                            _variant_name: "Single".to_string(),
                            data: TestWireValue {
                                bytes: b"plain".to_vec(),
                            },
                            is_static: false,
                        },
                    },
                    BcsMapEntry {
                        key: TypeName::new("iter_port"),
                        value: DagPortDataBcs::Many {
                            _variant_name: "Many".to_string(),
                            data: BcsMap {
                                contents: vec![
                                    BcsMapEntry {
                                        key: 1,
                                        value: TestWireValue {
                                            bytes: b"one".to_vec(),
                                        },
                                    },
                                    BcsMapEntry {
                                        key: 2,
                                        value: TestWireValue {
                                            bytes: b"two".to_vec(),
                                        },
                                    },
                                ],
                            },
                            total_iterations: 2,
                        },
                    },
                ],
            },
        };

        let encoded = bcs::to_bytes(&value).unwrap();
        let decoded: DagVertexEvaluationsBcs<TestWireValue> = bcs::from_bytes(&encoded).unwrap();

        assert_eq!(decoded, value);
    }

    #[test]
    fn test_gas_funds_serde() {
        let gas_funds = GasFunds {
            bal: 1000,
            locked: 500,
        };
        let json = serde_json::to_string(&gas_funds).unwrap();
        let deserialized: GasFunds = serde_json::from_str(&json).unwrap();
        assert_eq!(gas_funds.bal, deserialized.bal);
        assert_eq!(gas_funds.locked, deserialized.locked);
    }

    #[test]
    fn test_claimed_gas_serde() {
        let claimed = ClaimedGas {
            execution: 2000,
            priority: 300,
        };
        let json = serde_json::to_string(&claimed).unwrap();
        let deserialized: ClaimedGas = serde_json::from_str(&json).unwrap();
        assert_eq!(claimed.execution, deserialized.execution);
        assert_eq!(claimed.priority, deserialized.priority);
    }
}
