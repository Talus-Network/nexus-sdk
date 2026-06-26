//! Define models for Sui Move objects related to Nexus DAGs.

use {
    crate::{
        events::RequestWalkContext,
        nexus::crawler::{DynamicMap, Map, Set},
        sui,
        types::{
            deserialize_move_option_field,
            deserialize_move_option_sui_u64_field,
            deserialize_sui_u64,
            parse_runtime_vertex_value,
            parse_string_value,
            parse_u64_value,
            strip_fields_owned,
            AgentId,
            InterfaceVersion,
            MoveOption,
            MoveVecSet,
            NexusData,
            SkillId,
            SkillRevisionKey,
            TypeName,
            VerifierConfig,
            VerifierMode,
        },
        ToolFqn,
    },
    anyhow::anyhow,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    std::collections::{HashMap, HashSet},
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
    #[serde(default, skip_serializing_if = "verifier_config_is_none")]
    pub leader_verifier: VerifierConfig,
    #[serde(default, skip_serializing_if = "verifier_config_is_none")]
    pub tool_verifier: VerifierConfig,
}

fn verifier_config_is_none(config: &VerifierConfig) -> bool {
    config.mode == VerifierMode::None && config.method.as_str().is_empty()
}

/// Struct holding the DAG execution information.
///
/// See <sui/workflow/sources/execution.move:DAGExecution> for documentation.
#[derive(Clone, Debug, Deserialize)]
pub struct DagExecution {
    /// The address of the sender of the transaction to trigger this DAG
    /// execution.
    pub invoker: sui::types::Address,
    #[serde(rename = "dag", alias = "dag_id")]
    pub dag_id: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(deserialize_with = "deserialize_sui_u64")]
    pub skill_id: SkillId,
    pub interface_version: InterfaceVersion,
    #[serde(default, deserialize_with = "deserialize_move_option_field")]
    pub scheduled_task_id: Option<sui::types::Address>,
    #[serde(default, deserialize_with = "deserialize_move_option_sui_u64_field")]
    pub scheduled_occurrence_index: Option<u64>,
    #[serde(default)]
    pub walks: Vec<DagExecutionWalk>,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub active_walks: u64,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub pending_abort_walks: u64,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub successful_walks: u64,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub failed_walks: u64,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub aborted_walks: u64,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub consumed_walks: u64,
    #[serde(default, deserialize_with = "deserialize_sui_u64")]
    pub cancelled_walks: u64,
}

impl DagExecution {
    pub fn skill_revision_key(&self) -> Option<SkillRevisionKey> {
        Some(SkillRevisionKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
        })
    }

    pub fn to_context(&self) -> anyhow::Result<Option<RequestWalkContext>> {
        Ok(Some(RequestWalkContext {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
            scheduled_task_id: self.scheduled_task_id,
            scheduled_occurrence_index: self.scheduled_occurrence_index,
        }))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum DagExecutionWalk {
    Active {
        next_vertex: crate::types::RuntimeVertex,
        timeout_ms: u64,
        requires_vertex_authorization_grant: bool,
        created_at: u64,
    },
    PendingSettlement {
        next_vertex: crate::types::RuntimeVertex,
        timeout_ms: u64,
        requires_vertex_authorization_grant: bool,
        created_at: u64,
    },
    Successful,
    Failed,
    Consumed {
        at_vertex: crate::types::RuntimeVertex,
    },
    PendingAbort {
        at_vertex: crate::types::RuntimeVertex,
    },
    Aborted {
        at_vertex: crate::types::RuntimeVertex,
    },
    Cancelled,
}

impl DagExecutionWalk {
    pub fn expired_active_vertex(&self, clock_ms: u64) -> Option<&crate::types::RuntimeVertex> {
        match self {
            Self::Active {
                next_vertex,
                timeout_ms,
                created_at,
                ..
            } if clock_ms >= created_at.saturating_add(timeout_ms.saturating_mul(2)) => {
                Some(next_vertex)
            }
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for DagExecutionWalk {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let value = strip_fields_owned(value);
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("DAGWalk must be an object or enum"))?;

        let (variant, fields) = if let Some(Value::String(variant)) = object
            .get("_variant_name")
            .or_else(|| object.get("@variant"))
            .or_else(|| object.get("variant"))
        {
            (variant.as_str(), Some(&value))
        } else if object.len() == 1 {
            let (variant, fields) = object.iter().next().expect("single entry exists");
            (variant.as_str(), Some(fields))
        } else {
            return Err(serde::de::Error::custom("DAGWalk missing variant"));
        };

        let fields = fields.map(strip_fields_ref).unwrap_or(&value);
        match variant {
            "Active" | "PendingSettlement" => {
                let fields = fields.as_object().ok_or_else(|| {
                    serde::de::Error::custom(format!("{variant} DAGWalk fields missing"))
                })?;
                let next_vertex =
                    parse_runtime_vertex_value(fields.get("next_vertex").ok_or_else(|| {
                        serde::de::Error::custom(format!("{variant} DAGWalk missing next_vertex"))
                    })?)
                    .map_err(serde::de::Error::custom)?
                    .ok_or_else(|| {
                        serde::de::Error::custom(format!("{variant} DAGWalk missing next_vertex"))
                    })?;
                let timeout_ms = parse_u64_value(fields.get("timeout_ms").ok_or_else(|| {
                    serde::de::Error::custom(format!("{variant} DAGWalk missing timeout_ms"))
                })?)
                .map_err(serde::de::Error::custom)?
                .ok_or_else(|| {
                    serde::de::Error::custom(format!("{variant} DAGWalk missing timeout_ms"))
                })?;
                let created_at = parse_u64_value(fields.get("created_at").ok_or_else(|| {
                    serde::de::Error::custom(format!("{variant} DAGWalk missing created_at"))
                })?)
                .map_err(serde::de::Error::custom)?
                .ok_or_else(|| {
                    serde::de::Error::custom(format!("{variant} DAGWalk missing created_at"))
                })?;
                let requires_vertex_authorization_grant = fields
                    .get("requires_vertex_authorization_grant")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                match variant {
                    "Active" => Ok(Self::Active {
                        next_vertex,
                        timeout_ms,
                        requires_vertex_authorization_grant,
                        created_at,
                    }),
                    "PendingSettlement" => Ok(Self::PendingSettlement {
                        next_vertex,
                        timeout_ms,
                        requires_vertex_authorization_grant,
                        created_at,
                    }),
                    _ => unreachable!(),
                }
            }
            "Successful" => Ok(Self::Successful),
            "Failed" => Ok(Self::Failed),
            "Cancelled" => Ok(Self::Cancelled),
            "Consumed" | "PendingAbort" | "Aborted" => {
                let fields = fields
                    .as_object()
                    .ok_or_else(|| serde::de::Error::custom("terminal DAGWalk fields missing"))?;
                let at_vertex =
                    parse_runtime_vertex_value(fields.get("at_vertex").ok_or_else(|| {
                        serde::de::Error::custom("terminal DAGWalk missing at_vertex")
                    })?)
                    .map_err(serde::de::Error::custom)?
                    .ok_or_else(|| {
                        serde::de::Error::custom("terminal DAGWalk missing at_vertex")
                    })?;
                match variant {
                    "Consumed" => Ok(Self::Consumed { at_vertex }),
                    "PendingAbort" => Ok(Self::PendingAbort { at_vertex }),
                    "Aborted" => Ok(Self::Aborted { at_vertex }),
                    _ => unreachable!(),
                }
            }
            _ => Err(serde::de::Error::custom(format!(
                "unsupported DAGWalk variant {variant}"
            ))),
        }
    }
}

fn strip_fields_ref(value: &Value) -> &Value {
    value
        .as_object()
        .and_then(|object| object.get("fields"))
        .unwrap_or(value)
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

/// BCS-facing DAG vertex metadata used for linked-table dynamic-field decoding.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct DagVertexInfoBcs {
    pub(crate) kind: DagVertexKindBcs,
    pub(crate) input_ports: MoveVecSet<DagInputPort>,
    #[allow(dead_code)]
    pub(crate) post_failure_action: MoveOption<PostFailureActionBcs>,
    pub(crate) leader_verifier: MoveOption<VerifierConfigBcs>,
    pub(crate) tool_verifier: MoveOption<VerifierConfigBcs>,
}

impl DagVertexInfoBcs {
    pub(crate) fn into_sdk(self) -> anyhow::Result<DagVertexInfo> {
        let input_ports = self
            .input_ports
            .contents
            .into_iter()
            .collect::<HashSet<_>>();
        Ok(DagVertexInfo {
            kind: self.kind.into_sdk()?,
            leader_verifier: self
                .leader_verifier
                .0
                .map(VerifierConfigBcs::into_sdk)
                .transpose()?,
            tool_verifier: self
                .tool_verifier
                .0
                .map(VerifierConfigBcs::into_sdk)
                .transpose()?,
            input_ports: input_ports.into_iter().collect(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum DagVertexKindBcs {
    OnChain {
        #[allow(dead_code)]
        _variant_name: String,
        tool_fqn: String,
    },
    OffChain {
        #[allow(dead_code)]
        _variant_name: String,
        tool_fqn: String,
    },
}

impl DagVertexKindBcs {
    fn into_sdk(self) -> anyhow::Result<DagVertexKind> {
        match self {
            Self::OnChain { tool_fqn, .. } => Ok(DagVertexKind::OnChain {
                tool_fqn: parse_dag_tool_fqn(tool_fqn)?,
            }),
            Self::OffChain { tool_fqn, .. } => Ok(DagVertexKind::OffChain {
                tool_fqn: parse_dag_tool_fqn(tool_fqn)?,
            }),
        }
    }
}

fn parse_dag_tool_fqn(value: String) -> anyhow::Result<ToolFqn> {
    value
        .parse::<ToolFqn>()
        .map_err(|error| anyhow!("DAG BCS tool FQN '{value}' did not parse: {error}"))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct VerifierConfigBcs {
    mode: VerifierModeBcs,
    method: String,
}

impl VerifierConfigBcs {
    fn into_sdk(self) -> anyhow::Result<VerifierConfig> {
        Ok(VerifierConfig {
            mode: match self.mode {
                VerifierModeBcs::None => VerifierMode::None,
                VerifierModeBcs::LeaderRegisteredKey => VerifierMode::LeaderRegisteredKey,
                VerifierModeBcs::ToolVerifierContract => VerifierMode::ToolVerifierContract,
            },
            method: self.method.into(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum VerifierModeBcs {
    None,
    LeaderRegisteredKey,
    ToolVerifierContract,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum PostFailureActionBcs {
    Terminate,
    TransientContinue,
}

/// BCS-facing `sui::linked_table::Node<K, V>`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(bound(
    deserialize = "K: serde::de::DeserializeOwned, V: serde::de::DeserializeOwned",
    serialize = "K: Serialize, V: Serialize"
))]
pub(crate) struct LinkedTableNodeBcs<K, V> {
    #[allow(dead_code)]
    pub(crate) prev: MoveOption<K>,
    #[allow(dead_code)]
    pub(crate) next: MoveOption<K>,
    pub(crate) value: V,
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

/// Raw on-chain `NexusData` wire shape for committed-result wake reads.
///
/// The public `NexusData` type eagerly parses payload bytes as JSON, but committed-result metadata
/// reads only need to skip over output payload bytes without inspecting them.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RawNexusDataBcs {
    #[allow(dead_code)]
    pub(crate) storage: Vec<u8>,
    #[allow(dead_code)]
    pub(crate) one: Vec<u8>,
    #[allow(dead_code)]
    pub(crate) many: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CommittedToolResultLeaderRecordBcs {
    pub(crate) commit_tx_digest: Vec<u8>,
    pub(crate) recipient: sui::types::Address,
    pub(crate) commit_gas_charge: MoveOption<u64>,
    pub(crate) settlement_gas_charge: MoveOption<u64>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct DagEdgeBcs {
    pub(crate) from: DagOutputVariantPort,
    #[allow(dead_code)]
    pub(crate) to: DagVertexInputPort,
    #[allow(dead_code)]
    pub(crate) kind: DagEdgeKindBcs,
}

impl DagEdgeBcs {
    pub(crate) fn into_sdk(self) -> DagEdge {
        DagEdge { from: self.from }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) enum DagEdgeKindBcs {
    Normal,
    ForEach,
    Collect,
    DoWhile,
    Break,
    Static,
}

/// Struct holding the output variant and port pair.
///
/// See <sui/workflow/sources/dag.move:OutputVariantPort> for documentation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DagOutputVariantPort {
    pub variant: TypeName,
    pub port: TypeName,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{events::RequestWalkContext, fqn, types::InterfaceVersion},
        serde_json::json,
        std::str::FromStr,
    };

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
    fn test_dag_vertex_info_bcs_converts_linked_table_node() {
        let value = LinkedTableNodeBcs {
            prev: MoveOption(None::<TypeName>),
            next: MoveOption(None::<TypeName>),
            value: DagVertexInfoBcs {
                kind: DagVertexKindBcs::OnChain {
                    _variant_name: "OnChain".to_string(),
                    tool_fqn: "demo.taluslabs.demo_onchain_vertex@1".to_string(),
                },
                input_ports: MoveVecSet {
                    contents: vec![
                        DagInputPort {
                            name: "prompt".to_string(),
                        },
                        DagInputPort {
                            name: "recipient".to_string(),
                        },
                    ],
                },
                post_failure_action: MoveOption(None),
                leader_verifier: MoveOption(None),
                tool_verifier: MoveOption(Some(VerifierConfigBcs {
                    mode: VerifierModeBcs::ToolVerifierContract,
                    method: "demo_verifier_v1".to_string(),
                })),
            },
        };

        let encoded = bcs::to_bytes(&value).unwrap();
        let decoded: LinkedTableNodeBcs<TypeName, DagVertexInfoBcs> =
            bcs::from_bytes(&encoded).unwrap();
        let vertex = decoded.value.into_sdk().unwrap();

        assert!(matches!(vertex.kind, DagVertexKind::OnChain { .. }));
        assert_eq!(
            vertex.kind.tool_fqn().to_string(),
            "demo.taluslabs.demo_onchain_vertex@1"
        );
        assert_eq!(
            vertex.declared_input_port_names(),
            vec!["prompt".to_string(), "recipient".to_string()]
        );
        assert_eq!(
            vertex.tool_verifier,
            Some(VerifierConfig {
                mode: VerifierMode::ToolVerifierContract,
                method: "demo_verifier_v1".into(),
            })
        );
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
    fn dag_execution_context_uses_required_agent_fields() {
        let execution = DagExecution {
            invoker: sui::types::Address::from_static("0x1"),
            dag_id: sui::types::Address::from_static("0xd"),
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            interface_version: InterfaceVersion(7),
            scheduled_task_id: Some(sui::types::Address::from_static("0xf")),
            scheduled_occurrence_index: Some(2),
            walks: Vec::new(),
            active_walks: 0,
            pending_abort_walks: 0,
            successful_walks: 0,
            failed_walks: 0,
            aborted_walks: 0,
            consumed_walks: 0,
            cancelled_walks: 0,
        };

        let context = execution
            .to_context()
            .expect("current agent context should parse")
            .expect("context is always present for active executions");

        assert_eq!(
            context,
            RequestWalkContext {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceVersion(7),
                scheduled_task_id: Some(sui::types::Address::from_static("0xf")),
                scheduled_occurrence_index: Some(2),
            }
        );
    }

    #[test]
    fn dag_execution_scheduled_occurrence_index_accepts_plain_option_json() {
        let execution: DagExecution = serde_json::from_value(serde_json::json!({
            "invoker": "0x1",
            "dag": "0xd",
            "agent_id": "0xa",
            "skill_id": "11",
            "interface_version": "7",
            "scheduled_task_id": { "vec": ["0xf"] },
            "scheduled_occurrence_index": { "vec": ["2"] },
            "active_walks": "1",
            "pending_abort_walks": "2",
            "successful_walks": "3",
            "failed_walks": "4",
            "aborted_walks": "5",
            "consumed_walks": "6",
            "cancelled_walks": "7"
        }))
        .expect("DAGExecution should parse current Move JSON fields");

        assert_eq!(execution.skill_id, 11);
        assert_eq!(execution.dag_id, sui::types::Address::from_static("0xd"));
        assert_eq!(
            execution.scheduled_task_id,
            Some(sui::types::Address::from_static("0xf"))
        );
        assert_eq!(execution.scheduled_occurrence_index, Some(2));
        assert_eq!(execution.active_walks, 1);
        assert_eq!(execution.pending_abort_walks, 2);
        assert_eq!(execution.successful_walks, 3);
        assert_eq!(execution.failed_walks, 4);
        assert_eq!(execution.aborted_walks, 5);
        assert_eq!(execution.consumed_walks, 6);
        assert_eq!(execution.cancelled_walks, 7);
    }

    #[test]
    fn dag_execution_walk_decodes_pending_settlement_variant() {
        let walk: DagExecutionWalk = serde_json::from_value(json!({
            "PendingSettlement": {
                "next_vertex": {
                    "Plain": {
                        "vertex": {
                            "name": "settling"
                        }
                    }
                },
                "timeout_ms": "5000",
                "requires_vertex_authorization_grant": true,
                "created_at": "42"
            }
        }))
        .expect("PendingSettlement DAGWalk should parse current Move JSON");

        assert_eq!(
            walk,
            DagExecutionWalk::PendingSettlement {
                next_vertex: crate::types::RuntimeVertex::plain("settling"),
                timeout_ms: 5000,
                requires_vertex_authorization_grant: true,
                created_at: 42,
            }
        );
    }
}
