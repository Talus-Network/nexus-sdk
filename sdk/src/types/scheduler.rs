//! This module contains a struct representation of the task used by the scheduler.
//! It provides a way to serialize and deserialize the task and any helper structures.
use {
    super::{
        serde_parsers::{deserialize_sui_u64, serialize_sui_u64},
        strip_fields_owned,
        tap::{AgentId, InterfaceVersion, SkillId},
        AgentVertexAuthorizationTemplate,
        MoveOption,
        TypeName,
    },
    crate::{
        nexus::{
            crawler::{Bag, Map, ObjectBag, TableVec},
            models::DagInputPort,
        },
        sui,
        types::NexusData,
    },
    serde::{Deserialize, Deserializer, Serialize, Serializer},
};

/// Representation of `nexus_interface::agent::ExecutionSelection`.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "variant", content = "fields")]
pub enum ExecutionSelection {
    AgentSkill {
        agent_id: AgentId,
        #[serde(
            deserialize_with = "deserialize_sui_u64",
            serialize_with = "serialize_sui_u64"
        )]
        skill_id: SkillId,
        selected_dag: MoveOption<sui::types::Address>,
    },
    DefaultAgent {
        dag_id: sui::types::Address,
    },
}

impl<'de> Deserialize<'de> for ExecutionSelection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum Wire {
                AgentSkill {
                    agent_id: AgentId,
                    skill_id: SkillId,
                    selected_dag: MoveOption<sui::types::Address>,
                },
                DefaultAgent {
                    dag_id: sui::types::Address,
                },
            }

            return match Wire::deserialize(deserializer)? {
                Wire::AgentSkill {
                    agent_id,
                    skill_id,
                    selected_dag,
                } => Ok(Self::AgentSkill {
                    agent_id,
                    skill_id,
                    selected_dag,
                }),
                Wire::DefaultAgent { dag_id } => Ok(Self::DefaultAgent { dag_id }),
            };
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        parse_execution_selection_value(value).map_err(serde::de::Error::custom)
    }
}

fn parse_execution_selection_value(
    value: serde_json::Value,
) -> serde_json::Result<ExecutionSelection> {
    #[derive(Deserialize)]
    struct AgentSkillFields {
        agent_id: AgentId,
        #[serde(deserialize_with = "deserialize_sui_u64")]
        skill_id: SkillId,
        selected_dag: MoveOption<sui::types::Address>,
    }

    #[derive(Deserialize)]
    struct DefaultAgentFields {
        dag_id: sui::types::Address,
    }

    let value = strip_fields_owned(value);
    let serde_json::Value::Object(mut object) = value else {
        return Err(serde::de::Error::custom(
            "ExecutionSelection must be a Move enum object",
        ));
    };

    let fields = object
        .remove("fields")
        .or_else(|| object.remove("@fields"))
        .map(strip_fields_owned);

    let mut variant = object
        .remove("_variant_name")
        .or_else(|| object.remove("@variant"))
        .or_else(|| object.remove("variant"))
        .or_else(|| object.remove("type"))
        .and_then(|value| value.as_str().map(ToOwned::to_owned));

    let payload = if let Some(fields) = fields {
        fields
    } else if variant.is_none() && object.len() == 1 {
        let (name, fields) = object.into_iter().next().expect("len checked");
        if matches!(name.as_str(), "AgentSkill" | "DefaultAgent") {
            variant = Some(name);
            strip_fields_owned(fields)
        } else {
            serde_json::Value::Object(serde_json::Map::from_iter([(name, fields)]))
        }
    } else {
        serde_json::Value::Object(object)
    };

    let variant = variant.or_else(|| {
        let serde_json::Value::Object(object) = &payload else {
            return None;
        };
        if object.contains_key("agent_id") || object.contains_key("skill_id") {
            Some("AgentSkill".to_string())
        } else if object.contains_key("dag_id") {
            Some("DefaultAgent".to_string())
        } else {
            None
        }
    });

    match variant.as_deref() {
        Some("AgentSkill") => {
            let fields: AgentSkillFields = serde_json::from_value(payload)?;
            Ok(ExecutionSelection::AgentSkill {
                agent_id: fields.agent_id,
                skill_id: fields.skill_id,
                selected_dag: fields.selected_dag,
            })
        }
        Some("DefaultAgent") => {
            let fields: DefaultAgentFields = serde_json::from_value(payload)?;
            Ok(ExecutionSelection::DefaultAgent {
                dag_id: fields.dag_id,
            })
        }
        Some(other) => Err(serde::de::Error::custom(format!(
            "unsupported ExecutionSelection variant {other}"
        ))),
        None => Err(serde::de::Error::custom(
            "ExecutionSelection missing variant tag or recognizable fields",
        )),
    }
}

impl ExecutionSelection {
    pub fn dag_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::AgentSkill { selected_dag, .. } => selected_dag.0,
            Self::DefaultAgent { dag_id } => Some(*dag_id),
        }
    }
}

/// Representation of `nexus_interface::agent::AgentExecutionConfig`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AgentExecutionConfig {
    pub selection: ExecutionSelection,
    pub network: sui::types::Address,
    pub entry_group: SchedulerEntryGroup,
    pub inputs: Map<TypeName, Map<DagInputPort, NexusData>>,
    pub invoker: sui::types::Address,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub priority_fee_per_gas_unit: u64,
    pub authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
}

/// Representation of `nexus_scheduler::scheduler::Task`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Task {
    pub id: sui::types::Address,
    pub owner: sui::types::Address,
    pub agent_id: AgentId,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub skill_id: SkillId,
    pub interface_version: InterfaceVersion,
    pub metadata: Metadata,
    pub constraints: Policy<ConstraintsData>,
    pub execution: Policy<ExecutionData>,
    pub state: TaskState,
    pub data: Bag,
    pub objects: ObjectBag,
}

/// Minimal representation of `nexus_primitives::policy::Policy`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Policy<T> {
    pub id: sui::types::Address,
    pub dfa: ConfiguredAutomaton,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub state_index: u64,
    pub data: T,
}

/// Minimal representation of `nexus_primitives::automaton::ConfiguredAutomaton`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ConfiguredAutomaton {
    pub id: sui::types::Address,
    pub dfa: DeterministicAutomaton,
}

/// Deterministic automaton backing a policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeterministicAutomaton {
    pub states: TableVec<u64>,
    pub alphabet: TableVec<PolicySymbol>,
    pub transition: TableVec<TableVec<u64>>,
    pub accepting: TableVec<bool>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub start: u64,
}

/// Representation of `nexus_primitives::policy::Symbol`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PolicySymbol {
    Witness(TypeName),
    Uid(sui::types::Address),
}

// TODO: BCS and JSON standardization
// TODO: https://github.com/Talus-Network/nexus-sdk/issues/364
impl<'de> Deserialize<'de> for PolicySymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Non-human readable formats (BCS) use the standard enum layout.
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum Standard {
                Witness(TypeName),
                Uid(sui::types::Address),
            }

            return match Standard::deserialize(deserializer)? {
                Standard::Witness(name) => Ok(PolicySymbol::Witness(name)),
                Standard::Uid(uid) => Ok(PolicySymbol::Uid(uid)),
            };
        }

        // Human readable formats (JSON) use the { variant, fields: { pos0 } } shape.
        #[derive(Deserialize)]
        struct Fields<T> {
            #[serde(rename = "pos0")]
            pos0: T,
        }

        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let object = value
            .as_object()
            .ok_or_else(|| serde::de::Error::custom("PolicySymbol must be an object"))?;

        let variant = object
            .get("variant")
            .or_else(|| object.get("@variant"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| serde::de::Error::custom("PolicySymbol missing variant tag"))?;

        let fields = object
            .get("fields")
            .or_else(|| object.get("@fields"))
            .cloned()
            .or_else(|| {
                object.get("pos0").cloned().map(|pos0| {
                    let mut map = serde_json::Map::new();
                    map.insert("pos0".to_string(), pos0);
                    serde_json::Value::Object(map)
                })
            })
            .ok_or_else(|| serde::de::Error::custom("PolicySymbol missing fields payload"))?;

        match variant {
            "Witness" => {
                let fields: Fields<TypeName> =
                    serde_json::from_value(fields).map_err(serde::de::Error::custom)?;
                Ok(PolicySymbol::Witness(fields.pos0))
            }
            "Uid" => {
                let fields: Fields<sui::types::Address> =
                    serde_json::from_value(fields).map_err(serde::de::Error::custom)?;
                Ok(PolicySymbol::Uid(fields.pos0))
            }
            other => Err(serde::de::Error::custom(format!(
                "Unknown policy symbol variant: {other}"
            ))),
        }
    }
}

impl Serialize for PolicySymbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Non-human readable formats (BCS) use the standard enum layout.
        if !serializer.is_human_readable() {
            #[derive(Serialize)]
            enum Standard<'a> {
                Witness(&'a TypeName),
                Uid(sui::types::Address),
            }

            return match self {
                PolicySymbol::Witness(name) => Standard::Witness(name).serialize(serializer),
                PolicySymbol::Uid(uid) => Standard::Uid(*uid).serialize(serializer),
            };
        }

        // Human readable formats (JSON) use the { variant, fields: { pos0 } } shape.
        #[derive(Serialize)]
        struct Fields<'a, T> {
            #[serde(rename = "pos0")]
            pos0: &'a T,
        }

        #[derive(Serialize)]
        struct Tagged<'a, T> {
            variant: &'a str,
            fields: Fields<'a, T>,
        }

        match self {
            PolicySymbol::Witness(name) => Tagged {
                variant: "Witness",
                fields: Fields { pos0: name },
            }
            .serialize(serializer),
            PolicySymbol::Uid(uid) => Tagged {
                variant: "Uid",
                fields: Fields { pos0: uid },
            }
            .serialize(serializer),
        }
    }
}

impl PolicySymbol {
    pub fn as_witness(&self) -> Option<&TypeName> {
        match self {
            PolicySymbol::Witness(name) => Some(name),
            PolicySymbol::Uid(_) => None,
        }
    }

    pub fn as_uid(&self) -> Option<&sui::types::Address> {
        match self {
            PolicySymbol::Uid(uid) => Some(uid),
            PolicySymbol::Witness(_) => None,
        }
    }

    /// Returns true when the witness matches the fully-qualified name.
    pub fn matches_qualified_name(&self, expected: &str) -> bool {
        self.as_witness()
            .map(|name| name.matches_qualified_name(expected))
            .unwrap_or(false)
    }
}

/// Scheduled task state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskState {
    Active,
    Paused,
    Canceled,
    Completed,
    Failed,
}

impl<'de> Deserialize<'de> for TaskState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Standard {
            Active,
            Paused,
            Canceled,
            Completed,
            Failed,
        }

        if !deserializer.is_human_readable() {
            return match Standard::deserialize(deserializer)? {
                Standard::Active => Ok(TaskState::Active),
                Standard::Paused => Ok(TaskState::Paused),
                Standard::Canceled => Ok(TaskState::Canceled),
                Standard::Completed => Ok(TaskState::Completed),
                Standard::Failed => Ok(TaskState::Failed),
            };
        }

        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;

        let variant = match value {
            serde_json::Value::String(variant) => variant,
            serde_json::Value::Object(object) => object
                .get("variant")
                .or_else(|| object.get("@variant"))
                .and_then(|value| value.as_str())
                .ok_or_else(|| serde::de::Error::custom("TaskState missing variant tag"))?
                .to_string(),
            other => {
                return Err(serde::de::Error::custom(format!(
                    "unexpected value for TaskState: {other}"
                )))
            }
        };

        match variant.as_str() {
            "Active" => Ok(TaskState::Active),
            "Paused" => Ok(TaskState::Paused),
            "Canceled" => Ok(TaskState::Canceled),
            "Completed" => Ok(TaskState::Completed),
            "Failed" => Ok(TaskState::Failed),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["Active", "Paused", "Canceled", "Completed", "Failed"],
            )),
        }
    }
}

impl Serialize for TaskState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        enum Standard<'a> {
            Active,
            Paused,
            Canceled,
            Completed,
            Failed,
            #[allow(dead_code)]
            #[serde(skip)]
            _Marker(&'a ()),
        }

        if !serializer.is_human_readable() {
            return match self {
                TaskState::Active => Standard::Active.serialize(serializer),
                TaskState::Paused => Standard::Paused.serialize(serializer),
                TaskState::Canceled => Standard::Canceled.serialize(serializer),
                TaskState::Completed => Standard::Completed.serialize(serializer),
                TaskState::Failed => Standard::Failed.serialize(serializer),
            };
        }

        match self {
            TaskState::Active => "Active".serialize(serializer),
            TaskState::Paused => "Paused".serialize(serializer),
            TaskState::Canceled => "Canceled".serialize(serializer),
            TaskState::Completed => "Completed".serialize(serializer),
            TaskState::Failed => "Failed".serialize(serializer),
        }
    }
}

/// Marker data stored in the constraints policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConstraintsData {}

/// Marker data stored in the execution policy.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ExecutionData {}

/// Task metadata wrapper (Map<String, String>).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub values: Map<String, String>,
}

/// Representation of `nexus_interface::graph::EntryGroup`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchedulerEntryGroup {
    pub name: String,
}

/// Representation of `nexus_primitives::automaton::TransitionKey`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct TransitionKey<State, Symbol> {
    pub state: Option<State>,
    pub symbol: Symbol,
}

/// Representation of `nexus_primitives::automaton::TransitionConfigKey`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct TransitionConfigKey<State, Symbol> {
    pub transition: TransitionKey<State, Symbol>,
    pub config: TypeName,
}

#[cfg(test)]
mod tests {
    use {super::*, rand::thread_rng, serde_json::json};

    #[test]
    fn policy_deserializes_from_json() {
        let policy_id = sui::types::Address::from_static("0x2");
        let dfa_id = sui::types::Address::from_static("0x3");

        let expected = Policy {
            id: policy_id,
            dfa: ConfiguredAutomaton {
                id: dfa_id,
                dfa: DeterministicAutomaton {
                    states: TableVec::new(sui::types::Address::from_static("0x10"), 1),
                    alphabet: TableVec::new(sui::types::Address::from_static("0x11"), 1),
                    transition: TableVec::new(sui::types::Address::from_static("0x12"), 1),
                    accepting: TableVec::new(sui::types::Address::from_static("0x13"), 1),
                    start: 0,
                },
            },
            state_index: 5,
            data: ConstraintsData::default(),
        };

        let parsed: Policy<ConstraintsData> =
            serde_json::from_value(serde_json::to_value(&expected).unwrap()).unwrap();

        assert_eq!(parsed.id, expected.id);
        assert_eq!(parsed.dfa.id, expected.dfa.id);
        assert_eq!(parsed.dfa.dfa, expected.dfa.dfa);
        assert_eq!(parsed.state_index, expected.state_index);
        assert_eq!(parsed.data, expected.data);
    }

    #[test]
    fn task_state_deserializes_all_scheduler_states() {
        for (name, expected) in [
            ("Active", TaskState::Active),
            ("Paused", TaskState::Paused),
            ("Canceled", TaskState::Canceled),
            ("Completed", TaskState::Completed),
            ("Failed", TaskState::Failed),
        ] {
            let from_string: TaskState = serde_json::from_value(json!(name)).unwrap();
            assert_eq!(from_string, expected);

            let from_variant: TaskState = serde_json::from_value(json!({
                "variant": name
            }))
            .unwrap();
            assert_eq!(from_variant, expected);

            assert_eq!(serde_json::to_value(&expected).unwrap(), json!(name));
        }
    }

    #[test]
    fn default_agent_execution_config_deserializes_from_json() {
        let mut rng = rand::thread_rng();
        let dag_id = sui::types::Address::generate(&mut rng);
        let network_id = sui::types::Address::generate(&mut rng);
        let invoker = sui::types::Address::generate(&mut rng);

        let expected = AgentExecutionConfig {
            selection: ExecutionSelection::DefaultAgent { dag_id },
            network: network_id,
            entry_group: SchedulerEntryGroup {
                name: "default".to_string(),
            },
            inputs: Map::default(),
            invoker,
            priority_fee_per_gas_unit: 1000,
            authorization_templates: vec![],
        };

        let parsed: AgentExecutionConfig =
            serde_json::from_value(serde_json::to_value(&expected).unwrap()).unwrap();

        assert_eq!(parsed.selection.dag_id(), Some(dag_id));
        assert_eq!(parsed.network, expected.network);
        assert_eq!(
            parsed.priority_fee_per_gas_unit,
            expected.priority_fee_per_gas_unit
        );
        assert_eq!(parsed.entry_group, expected.entry_group);
        assert_eq!(parsed.inputs, expected.inputs);
        assert_eq!(parsed.invoker, expected.invoker);
    }

    #[test]
    fn default_agent_execution_config_deserializes_from_move_json() {
        let mut rng = rand::thread_rng();
        let dag_id = sui::types::Address::generate(&mut rng);
        let network_id = sui::types::Address::generate(&mut rng);
        let invoker = sui::types::Address::generate(&mut rng);

        let expected = AgentExecutionConfig {
            selection: ExecutionSelection::DefaultAgent { dag_id },
            network: network_id,
            entry_group: SchedulerEntryGroup {
                name: "default".to_string(),
            },
            inputs: Map::default(),
            invoker,
            priority_fee_per_gas_unit: 1000,
            authorization_templates: vec![],
        };
        let mut value = serde_json::to_value(&expected).unwrap();
        value["selection"] = json!({ "dag_id": dag_id });

        let parsed: AgentExecutionConfig = serde_json::from_value(value).unwrap();

        assert_eq!(parsed, expected);
    }

    #[test]
    fn execution_selection_deserializes_from_variant_wrapper_json() {
        let mut rng = rand::thread_rng();
        let dag_id = sui::types::Address::generate(&mut rng);

        let parsed: ExecutionSelection = serde_json::from_value(json!({
            "DefaultAgent": {
                "dag_id": dag_id
            }
        }))
        .unwrap();

        assert_eq!(parsed, ExecutionSelection::DefaultAgent { dag_id });
    }

    #[test]
    fn agent_execution_config_deserializes_from_json() {
        let mut rng = rand::thread_rng();
        let agent_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);
        let network_id = sui::types::Address::generate(&mut rng);
        let invoker = sui::types::Address::generate(&mut rng);
        let recipient_id = sui::types::Address::generate(&mut rng);

        let expected = AgentExecutionConfig {
            selection: ExecutionSelection::AgentSkill {
                agent_id,
                skill_id: 2,
                selected_dag: MoveOption(Some(dag_id)),
            },
            network: network_id,
            entry_group: SchedulerEntryGroup {
                name: "agent".to_string(),
            },
            inputs: Map::default(),
            invoker,
            priority_fee_per_gas_unit: 1000,
            authorization_templates: vec![AgentVertexAuthorizationTemplate {
                skill_id: 2,
                vertex: "cap_first".to_string(),
                recipient_id,
            }],
        };

        let parsed: AgentExecutionConfig =
            serde_json::from_value(serde_json::to_value(&expected).unwrap()).unwrap();

        assert_eq!(parsed.selection, expected.selection);
        assert_eq!(parsed.selection.dag_id(), Some(dag_id));
        assert_eq!(
            parsed.authorization_templates,
            expected.authorization_templates
        );
    }

    #[test]
    fn agent_execution_config_deserializes_from_move_json() {
        let mut rng = rand::thread_rng();
        let agent_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);
        let network_id = sui::types::Address::generate(&mut rng);
        let invoker = sui::types::Address::generate(&mut rng);

        let expected = AgentExecutionConfig {
            selection: ExecutionSelection::AgentSkill {
                agent_id,
                skill_id: 2,
                selected_dag: MoveOption(Some(dag_id)),
            },
            network: network_id,
            entry_group: SchedulerEntryGroup {
                name: "agent".to_string(),
            },
            inputs: Map::default(),
            invoker,
            priority_fee_per_gas_unit: 1000,
            authorization_templates: vec![],
        };
        let mut value = serde_json::to_value(&expected).unwrap();
        value["selection"] = json!({
            "agent_id": agent_id,
            "skill_id": "2",
            "selected_dag": serde_json::to_value(MoveOption(Some(dag_id))).unwrap(),
        });

        let parsed: AgentExecutionConfig = serde_json::from_value(value).unwrap();

        assert_eq!(parsed, expected);
    }

    #[test]
    fn policy_symbol_deserializes_enum_witness() {
        let json = json!({
            "variant": "Witness",
            "fields": { "pos0": { "name": "0x1::module::Type" } }
        });

        let sym: PolicySymbol = serde_json::from_value(json).unwrap();
        assert!(matches!(sym, PolicySymbol::Witness(name) if name.name == "0x1::module::Type"));
    }

    #[test]
    fn policy_symbol_deserializes_enum_uid() {
        let mut rng = thread_rng();
        let addr = sui::types::Address::generate(&mut rng);
        let json = json!({
            "variant": "Uid",
            "fields": { "pos0": addr }
        });

        let sym: PolicySymbol = serde_json::from_value(json).unwrap();
        assert!(matches!(sym, PolicySymbol::Uid(uid) if uid == addr));
    }

    #[test]
    fn policy_symbol_deserializes_bcs_witness() {
        let witness = TypeName::new("0x1::module::Type");
        let bytes = bcs::to_bytes(&PolicySymbol::Witness(witness.clone())).unwrap();

        let parsed: PolicySymbol = bcs::from_bytes(&bytes).unwrap();
        assert!(matches!(parsed, PolicySymbol::Witness(name) if name == witness));
    }

    #[test]
    fn policy_symbol_deserializes_bcs_uid() {
        let mut rng = thread_rng();
        let addr = sui::types::Address::generate(&mut rng);
        let bytes = bcs::to_bytes(&PolicySymbol::Uid(addr)).unwrap();

        let parsed: PolicySymbol = bcs::from_bytes(&bytes).unwrap();
        assert!(matches!(parsed, PolicySymbol::Uid(uid) if uid == addr));
    }
}
