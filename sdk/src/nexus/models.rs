//! Define models for Sui Move objects related to Nexus DAGs.

use {
    crate::{
        events::RequestWalkStandardTapContext,
        nexus::crawler::{DynamicMap, Map, Set},
        sui,
        types::{
            parse_runtime_vertex_value,
            parse_string_value,
            parse_u64_value,
            strip_fields_owned,
            AgentId,
            InterfaceRevision,
            MoveOption,
            MoveVecSet,
            NexusData,
            SkillId,
            TapEndpointKey,
            TapVertexAuthorizationPlan,
            TapVertexAuthorizationPlanEntry,
            TypeName,
            VerifierConfig,
            VerifierMode,
        },
        ToolFqn,
    },
    anyhow::{anyhow, bail},
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
    #[serde(default, skip_serializing_if = "VerifierConfig::is_none")]
    pub leader_verifier: VerifierConfig,
    #[serde(default, skip_serializing_if = "VerifierConfig::is_none")]
    pub tool_verifier: VerifierConfig,
}

/// Struct holding the DAG execution information.
///
/// See <sui/workflow/sources/dag.move:DAGExecution> for documentation.
#[derive(Clone, Debug, Deserialize)]
pub struct DagExecution {
    /// The address of the sender of the transaction to trigger this DAG
    /// execution.
    pub invoker: sui::types::Address,
    #[serde(default = "empty_move_option")]
    pub tap_agent_id: MoveOption<AgentId>,
    #[serde(default = "empty_move_option")]
    #[serde(deserialize_with = "deserialize_move_option_sui_u64")]
    pub tap_skill_id: MoveOption<SkillId>,
    #[serde(default = "empty_move_option")]
    pub tap_interface_revision: MoveOption<InterfaceRevision>,
    #[serde(default = "empty_move_option")]
    pub tap_payment_id: MoveOption<sui::types::Address>,
    #[serde(default = "empty_move_option")]
    pub tap_selected_dag_id: MoveOption<sui::types::Address>,
    #[serde(default = "empty_move_option")]
    pub tap_authorization_plan_commitment: MoveOption<Vec<u8>>,
    #[serde(default)]
    pub tap_authorization_plan: Vec<TapVertexAuthorizationPlanEntry>,
    #[serde(default = "empty_move_option")]
    pub tap_scheduled_task_id: MoveOption<sui::types::Address>,
    #[serde(default = "empty_move_option")]
    #[serde(deserialize_with = "deserialize_move_option_sui_u64")]
    pub tap_scheduled_occurrence_index: MoveOption<u64>,
    #[serde(default)]
    pub walks: Vec<DagExecutionWalk>,
}

impl DagExecution {
    pub fn endpoint_key(&self) -> Option<TapEndpointKey> {
        Some(TapEndpointKey {
            agent_id: self.tap_agent_id.0?,
            skill_id: self.tap_skill_id.0?,
            interface_revision: self.tap_interface_revision.0?,
        })
    }

    pub fn standard_tap_context(&self) -> anyhow::Result<Option<RequestWalkStandardTapContext>> {
        if self.tap_agent_id.0.is_none()
            && self.tap_skill_id.0.is_none()
            && self.tap_interface_revision.0.is_none()
            && self.tap_payment_id.0.is_none()
            && self.tap_selected_dag_id.0.is_none()
            && self.tap_authorization_plan_commitment.0.is_none()
            && self.tap_authorization_plan.is_empty()
            && self.tap_scheduled_task_id.0.is_none()
            && self.tap_scheduled_occurrence_index.0.is_none()
        {
            return Ok(None);
        }

        let Some(agent_id) = self.tap_agent_id.0 else {
            bail!("DAGExecution has partial standard TAP context: missing tap_agent_id");
        };
        let Some(skill_id) = self.tap_skill_id.0 else {
            bail!("DAGExecution has partial standard TAP context: missing tap_skill_id");
        };
        let Some(interface_revision) = self.tap_interface_revision.0 else {
            bail!("DAGExecution has partial standard TAP context: missing tap_interface_revision");
        };
        let Some(payment_id) = self.tap_payment_id.0 else {
            bail!("DAGExecution has partial standard TAP context: missing tap_payment_id");
        };
        let Some(selected_dag_id) = self.tap_selected_dag_id.0 else {
            bail!("DAGExecution has partial standard TAP context: missing tap_selected_dag_id");
        };

        Ok(Some(RequestWalkStandardTapContext {
            agent_id,
            skill_id,
            interface_revision,
            payment_id,
            selected_dag_id,
            authorization_plan_commitment: self.tap_authorization_plan_commitment.0.clone(),
            authorization_plan: TapVertexAuthorizationPlan(self.tap_authorization_plan.clone()),
            scheduled_task_id: self.tap_scheduled_task_id.0,
            scheduled_occurrence_index: self.tap_scheduled_occurrence_index.0,
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
            "Active" => {
                let fields = fields
                    .as_object()
                    .ok_or_else(|| serde::de::Error::custom("Active DAGWalk fields missing"))?;
                let next_vertex =
                    parse_runtime_vertex_value(fields.get("next_vertex").ok_or_else(|| {
                        serde::de::Error::custom("Active DAGWalk missing next_vertex")
                    })?)
                    .map_err(serde::de::Error::custom)?
                    .ok_or_else(|| {
                        serde::de::Error::custom("Active DAGWalk missing next_vertex")
                    })?;
                let timeout_ms = parse_u64_value(fields.get("timeout_ms").ok_or_else(|| {
                    serde::de::Error::custom("Active DAGWalk missing timeout_ms")
                })?)
                .map_err(serde::de::Error::custom)?
                .ok_or_else(|| serde::de::Error::custom("Active DAGWalk missing timeout_ms"))?;
                let created_at = parse_u64_value(fields.get("created_at").ok_or_else(|| {
                    serde::de::Error::custom("Active DAGWalk missing created_at")
                })?)
                .map_err(serde::de::Error::custom)?
                .ok_or_else(|| serde::de::Error::custom("Active DAGWalk missing created_at"))?;
                let requires_vertex_authorization_grant = fields
                    .get("requires_vertex_authorization_grant")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                Ok(Self::Active {
                    next_vertex,
                    timeout_ms,
                    requires_vertex_authorization_grant,
                    created_at,
                })
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
            method: self.method,
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

fn deserialize_move_option_sui_u64<'de, D>(deserializer: D) -> Result<MoveOption<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    if !deserializer.is_human_readable() {
        return MoveOption::<u64>::deserialize(deserializer);
    }

    fn parse_value(value: Value) -> Result<Option<u64>, String> {
        let value = strip_fields_owned(value);
        match value {
            Value::Null => Ok(None),
            Value::String(value) => value
                .parse::<u64>()
                .map(Some)
                .map_err(|e| format!("invalid number: {e}")),
            Value::Number(value) => value
                .as_u64()
                .map(Some)
                .ok_or_else(|| "expected unsigned number".to_string()),
            Value::Array(mut values) => values
                .drain(..)
                .next()
                .map(parse_value)
                .transpose()
                .map(|value| value.flatten()),
            Value::Object(mut object) => {
                if let Some(vec) = object.remove("vec").or_else(|| object.remove("Vec")) {
                    return parse_value(vec);
                }
                if let Some(inner) = object.remove("some").or_else(|| object.remove("Some")) {
                    return parse_value(inner);
                }
                if object.contains_key("none") || object.contains_key("None") {
                    return Ok(None);
                }
                Err("expected Move Option<u64>".to_string())
            }
            other => Err(format!("unexpected value for Option<u64>: {other}")),
        }
    }

    parse_value(Deserialize::deserialize(deserializer)?)
        .map(MoveOption)
        .map_err(serde::de::Error::custom)
}

fn empty_move_option<T>() -> MoveOption<T> {
    MoveOption(None)
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
        crate::{
            events::RequestWalkStandardTapContext,
            fqn,
            types::{InterfaceRevision, RuntimeVertex},
        },
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
                method: "demo_verifier_v1".to_string(),
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
    fn dag_execution_standard_tap_context_requires_complete_fields() {
        let execution = DagExecution {
            invoker: sui::types::Address::from_static("0x1"),
            tap_agent_id: MoveOption(Some(sui::types::Address::from_static("0xa"))),
            tap_skill_id: MoveOption(Some(11)),
            tap_interface_revision: MoveOption(Some(InterfaceRevision(7))),
            tap_payment_id: MoveOption(Some(sui::types::Address::from_static("0xd"))),
            tap_selected_dag_id: MoveOption(Some(sui::types::Address::from_static("0xe"))),
            tap_authorization_plan_commitment: MoveOption(Some(vec![1, 2, 3])),
            tap_authorization_plan: Vec::new(),
            tap_scheduled_task_id: MoveOption(Some(sui::types::Address::from_static("0xf"))),
            tap_scheduled_occurrence_index: MoveOption(Some(2)),
            walks: Vec::new(),
        };

        let context = execution
            .standard_tap_context()
            .expect("complete standard context should parse")
            .expect("context should be present");

        assert_eq!(
            context,
            RequestWalkStandardTapContext {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(7),
                payment_id: sui::types::Address::from_static("0xd"),
                selected_dag_id: sui::types::Address::from_static("0xe"),
                authorization_plan_commitment: Some(vec![1, 2, 3]),
                authorization_plan: TapVertexAuthorizationPlan::default(),
                scheduled_task_id: Some(sui::types::Address::from_static("0xf")),
                scheduled_occurrence_index: Some(2),
            }
        );
    }

    #[test]
    fn dag_execution_scheduled_occurrence_index_accepts_sui_move_json_string_option() {
        let execution: DagExecution = serde_json::from_value(serde_json::json!({
            "invoker": "0x1",
            "tap_skill_id": { "vec": ["11"] },
            "tap_scheduled_occurrence_index": { "vec": ["2"] }
        }))
        .expect("DAGExecution should parse Move JSON option-u64 strings");

        assert_eq!(execution.tap_skill_id.0, Some(11));
        assert_eq!(execution.tap_scheduled_occurrence_index.0, Some(2));
    }

    #[test]
    fn dag_execution_decodes_active_and_pending_abort_walks() {
        let execution: DagExecution = serde_json::from_value(json!({
            "invoker": "0x1",
            "walks": [
                {
                    "Active": {
                        "next_vertex": { "Plain": { "vertex": { "name": "payable" } } },
                        "timeout_ms": "30000",
                        "requires_vertex_authorization_grant": true,
                        "created_at": "7"
                    }
                },
                {
                    "_variant_name": "PendingAbort",
                    "at_vertex": { "Plain": { "vertex": { "name": "payable" } } }
                }
            ]
        }))
        .expect("walks should decode from Move JSON");

        assert_eq!(
            execution.walks[0],
            DagExecutionWalk::Active {
                next_vertex: RuntimeVertex::plain("payable"),
                timeout_ms: 30_000,
                requires_vertex_authorization_grant: true,
                created_at: 7,
            }
        );
        assert_eq!(
            execution.walks[1],
            DagExecutionWalk::PendingAbort {
                at_vertex: RuntimeVertex::plain("payable")
            }
        );
        assert_eq!(
            execution.walks[0].expired_active_vertex(60_007),
            Some(&RuntimeVertex::plain("payable"))
        );
        assert_eq!(execution.walks[0].expired_active_vertex(60_006), None);
    }

    #[test]
    fn dag_execution_walk_rejects_missing_and_unknown_variants() {
        let missing_variant = serde_json::from_value::<DagExecutionWalk>(json!({
            "next_vertex": { "Plain": { "vertex": { "name": "payable" } } },
            "timeout_ms": "30000"
        }))
        .expect_err("walk without a variant should fail");
        assert!(missing_variant.to_string().contains("missing variant"));

        let unsupported_variant = serde_json::from_value::<DagExecutionWalk>(json!({
            "Paused": {
                "at_vertex": { "Plain": { "vertex": { "name": "payable" } } }
            }
        }))
        .expect_err("unknown walk variant should fail");
        assert!(unsupported_variant
            .to_string()
            .contains("unsupported DAGWalk variant Paused"));
    }

    #[test]
    fn dag_execution_walk_requires_active_and_terminal_fields() {
        let missing_next_vertex = serde_json::from_value::<DagExecutionWalk>(json!({
            "Active": {
                "timeout_ms": "30000",
                "created_at": "7"
            }
        }))
        .expect_err("active walk without next_vertex should fail");
        assert!(missing_next_vertex
            .to_string()
            .contains("missing next_vertex"));

        let missing_timeout = serde_json::from_value::<DagExecutionWalk>(json!({
            "Active": {
                "next_vertex": { "Plain": { "vertex": { "name": "payable" } } },
                "created_at": "7"
            }
        }))
        .expect_err("active walk without timeout_ms should fail");
        assert!(missing_timeout.to_string().contains("missing timeout_ms"));

        let missing_created_at = serde_json::from_value::<DagExecutionWalk>(json!({
            "Active": {
                "next_vertex": { "Plain": { "vertex": { "name": "payable" } } },
                "timeout_ms": "30000"
            }
        }))
        .expect_err("active walk without created_at should fail");
        assert!(missing_created_at
            .to_string()
            .contains("missing created_at"));

        let missing_at_vertex = serde_json::from_value::<DagExecutionWalk>(json!({
            "Aborted": {}
        }))
        .expect_err("terminal walk without at_vertex should fail");
        assert!(missing_at_vertex.to_string().contains("missing at_vertex"));

        let null_next_vertex = serde_json::from_value::<DagExecutionWalk>(json!({
            "Active": {
                "next_vertex": null,
                "timeout_ms": "30000",
                "created_at": "7"
            }
        }))
        .expect_err("active walk with null next_vertex should fail");
        assert!(null_next_vertex.to_string().contains("missing next_vertex"));

        let null_at_vertex = serde_json::from_value::<DagExecutionWalk>(json!({
            "PendingAbort": {
                "at_vertex": null
            }
        }))
        .expect_err("terminal walk with null at_vertex should fail");
        assert!(null_at_vertex.to_string().contains("missing at_vertex"));
    }

    #[test]
    fn dag_execution_walk_decodes_terminal_variants_and_saturating_expiry() {
        let consumed = serde_json::from_value::<DagExecutionWalk>(json!({
            "Consumed": {
                "at_vertex": { "Plain": { "vertex": { "name": "payable" } } }
            }
        }))
        .expect("consumed walk should decode");
        let failed = serde_json::from_value::<DagExecutionWalk>(json!("Failed"))
            .expect_err("bare strings are not accepted Move JSON objects");
        let successful = serde_json::from_value::<DagExecutionWalk>(json!({
            "_variant_name": "Successful"
        }))
        .expect("successful walk should decode");
        let failed_object = serde_json::from_value::<DagExecutionWalk>(json!({
            "_variant_name": "Failed"
        }))
        .expect("failed walk should decode");
        let cancelled = serde_json::from_value::<DagExecutionWalk>(json!({
            "_variant_name": "Cancelled"
        }))
        .expect("cancelled walk should decode");
        let aborted = serde_json::from_value::<DagExecutionWalk>(json!({
            "Aborted": {
                "at_vertex": { "Plain": { "vertex": { "name": "payable" } } }
            }
        }))
        .expect("aborted walk should decode");
        let saturating_active = DagExecutionWalk::Active {
            next_vertex: RuntimeVertex::plain("payable"),
            timeout_ms: u64::MAX,
            requires_vertex_authorization_grant: false,
            created_at: u64::MAX,
        };

        assert_eq!(
            consumed,
            DagExecutionWalk::Consumed {
                at_vertex: RuntimeVertex::plain("payable")
            }
        );
        assert!(failed.to_string().contains("must be an object"));
        assert_eq!(successful, DagExecutionWalk::Successful);
        assert_eq!(failed_object, DagExecutionWalk::Failed);
        assert_eq!(cancelled, DagExecutionWalk::Cancelled);
        assert_eq!(
            aborted,
            DagExecutionWalk::Aborted {
                at_vertex: RuntimeVertex::plain("payable")
            }
        );
        assert_eq!(
            saturating_active.expired_active_vertex(u64::MAX),
            Some(&RuntimeVertex::plain("payable"))
        );
    }

    #[test]
    fn dag_execution_option_u64_accepts_move_json_shapes_and_rejects_invalid_values() {
        for (value, expected) in [
            (json!(null), None),
            (json!(7), Some(7)),
            (json!({ "some": 8 }), Some(8)),
            (json!({ "Some": "9" }), Some(9)),
            (json!({ "none": {} }), None),
            (json!({ "Vec": [] }), None),
        ] {
            let execution: DagExecution = serde_json::from_value(json!({
                "invoker": "0x1",
                "tap_skill_id": value,
            }))
            .expect("option u64 shape should parse");
            assert_eq!(execution.tap_skill_id.0, expected);
        }

        for value in [json!(-1), json!({ "bad": 1 }), json!(true)] {
            assert!(serde_json::from_value::<DagExecution>(json!({
                "invoker": "0x1",
                "tap_skill_id": value,
            }))
            .is_err());
        }
    }

    #[test]
    fn dag_execution_standard_tap_context_rejects_partial_fields() {
        let execution = DagExecution {
            invoker: sui::types::Address::from_static("0x1"),
            tap_agent_id: MoveOption(Some(sui::types::Address::from_static("0xa"))),
            tap_skill_id: MoveOption(None),
            tap_interface_revision: MoveOption(None),
            tap_payment_id: MoveOption(None),
            tap_selected_dag_id: MoveOption(None),
            tap_authorization_plan_commitment: MoveOption(None),
            tap_authorization_plan: Vec::new(),
            tap_scheduled_task_id: MoveOption(None),
            tap_scheduled_occurrence_index: MoveOption(None),
            walks: Vec::new(),
        };

        let error = execution
            .standard_tap_context()
            .expect_err("partial standard context should error");
        assert!(error.to_string().contains("missing tap_skill_id"));

        for (execution, expected) in [
            (
                DagExecution {
                    invoker: sui::types::Address::from_static("0x1"),
                    tap_agent_id: MoveOption(Some(sui::types::Address::from_static("0xa"))),
                    tap_skill_id: MoveOption(Some(11)),
                    tap_interface_revision: MoveOption(None),
                    tap_payment_id: MoveOption(None),
                    tap_selected_dag_id: MoveOption(None),
                    tap_authorization_plan_commitment: MoveOption(None),
                    tap_authorization_plan: Vec::new(),
                    tap_scheduled_task_id: MoveOption(None),
                    tap_scheduled_occurrence_index: MoveOption(None),
                    walks: Vec::new(),
                },
                "missing tap_interface_revision",
            ),
            (
                DagExecution {
                    invoker: sui::types::Address::from_static("0x1"),
                    tap_agent_id: MoveOption(Some(sui::types::Address::from_static("0xa"))),
                    tap_skill_id: MoveOption(Some(11)),
                    tap_interface_revision: MoveOption(Some(InterfaceRevision(7))),
                    tap_payment_id: MoveOption(None),
                    tap_selected_dag_id: MoveOption(None),
                    tap_authorization_plan_commitment: MoveOption(None),
                    tap_authorization_plan: Vec::new(),
                    tap_scheduled_task_id: MoveOption(None),
                    tap_scheduled_occurrence_index: MoveOption(None),
                    walks: Vec::new(),
                },
                "missing tap_payment_id",
            ),
            (
                DagExecution {
                    invoker: sui::types::Address::from_static("0x1"),
                    tap_agent_id: MoveOption(Some(sui::types::Address::from_static("0xa"))),
                    tap_skill_id: MoveOption(Some(11)),
                    tap_interface_revision: MoveOption(Some(InterfaceRevision(7))),
                    tap_payment_id: MoveOption(Some(sui::types::Address::from_static("0xd"))),
                    tap_selected_dag_id: MoveOption(None),
                    tap_authorization_plan_commitment: MoveOption(None),
                    tap_authorization_plan: Vec::new(),
                    tap_scheduled_task_id: MoveOption(None),
                    tap_scheduled_occurrence_index: MoveOption(None),
                    walks: Vec::new(),
                },
                "missing tap_selected_dag_id",
            ),
        ] {
            let error = execution.standard_tap_context().unwrap_err();
            assert!(
                error.to_string().contains(expected),
                "expected {expected:?}, got {error}"
            );
        }
    }

    #[test]
    fn dag_execution_missing_standard_tap_fields_default_to_none() {
        let execution: DagExecution = serde_json::from_value(json!({
            "invoker": "0x1"
        }))
        .expect("missing standard TAP fields should default to none");

        assert_eq!(execution.tap_agent_id.0, None);
        assert_eq!(execution.tap_skill_id.0, None);
        assert_eq!(execution.tap_interface_revision.0, None);
        assert_eq!(execution.tap_payment_id.0, None);
        assert_eq!(execution.tap_selected_dag_id.0, None);
        assert_eq!(execution.tap_authorization_plan_commitment.0, None);
        assert!(execution.tap_authorization_plan.is_empty());
        assert_eq!(execution.tap_scheduled_task_id.0, None);
        assert_eq!(execution.tap_scheduled_occurrence_index.0, None);
        assert_eq!(
            execution
                .standard_tap_context()
                .expect("empty standard TAP context should parse"),
            None
        );
    }
}
