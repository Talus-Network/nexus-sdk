//! This module contains a struct representation of the task used by the scheduler.
//! It provides a way to serialize and deserialize the task and any helper structures.
use {
    super::{
        interface::version::InterfaceVersion,
        primitives::policy::Symbol as PolicySymbol,
        scheduler::scheduler::State as TaskState,
        serde_parsers::{deserialize_sui_u64, serialize_sui_u64},
        strip_fields_owned,
        tap::{AgentId, SkillId},
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

impl<'de> Deserialize<'de> for TaskState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum Wire {
                Active,
                Paused,
                Canceled,
                Completed,
                Failed,
            }

            return match Wire::deserialize(deserializer)? {
                Wire::Active => Ok(Self::Active),
                Wire::Paused => Ok(Self::Paused),
                Wire::Canceled => Ok(Self::Canceled),
                Wire::Completed => Ok(Self::Completed),
                Wire::Failed => Ok(Self::Failed),
            };
        }

        let value = serde_json::Value::deserialize(deserializer)?;
        parse_task_state_value(value).map_err(serde::de::Error::custom)
    }
}

fn parse_task_state_value(value: serde_json::Value) -> Result<TaskState, String> {
    let value = strip_fields_owned(value);

    if let serde_json::Value::String(name) = value {
        return task_state_from_variant_name(&name);
    }

    let serde_json::Value::Object(object) = value else {
        return Err("TaskState must be a string or Move enum object".to_string());
    };

    let variant = object
        .get("_variant_name")
        .or_else(|| object.get("@variant"))
        .or_else(|| object.get("variant"))
        .or_else(|| object.get("type"))
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| {
            if object.len() == 1 {
                object.keys().next().cloned()
            } else {
                None
            }
        })
        .ok_or_else(|| "TaskState missing variant tag".to_string())?;

    task_state_from_variant_name(&variant)
}

fn task_state_from_variant_name(name: &str) -> Result<TaskState, String> {
    match name {
        "Active" => Ok(TaskState::Active),
        "Paused" => Ok(TaskState::Paused),
        "Canceled" => Ok(TaskState::Canceled),
        "Completed" | "Exhausted" => Ok(TaskState::Completed),
        "Failed" => Ok(TaskState::Failed),
        other => Err(format!(
            "unknown TaskState variant `{other}`, expected one of `Active`, `Paused`, `Canceled`, `Completed`, `Failed`"
        )),
    }
}

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

// TODO: BCS and JSON standardization
// TODO: https://github.com/Talus-Network/nexus-sdk/issues/364
impl<'de> Deserialize<'de> for PolicySymbol {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            enum Standard {
                Witness(TypeName),
                Uid(crate::types::sui_framework::object::ID),
            }

            return match Standard::deserialize(deserializer)? {
                Standard::Witness(pos0) => Ok(PolicySymbol::Witness { pos0 }),
                Standard::Uid(pos0) => Ok(PolicySymbol::Uid { pos0 }),
            };
        }

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
                Ok(PolicySymbol::Witness { pos0: fields.pos0 })
            }
            "Uid" => {
                let fields: Fields<serde_json::Value> =
                    serde_json::from_value(fields).map_err(serde::de::Error::custom)?;
                let pos0 = serde_json::from_value(fields.pos0.clone()).or_else(|_| {
                    crate::types::parse_address_value(&fields.pos0)
                        .map_err(serde::de::Error::custom)?
                        .map(crate::types::sui_address_to_id)
                        .ok_or_else(|| serde::de::Error::custom("PolicySymbol Uid missing address"))
                })?;
                Ok(PolicySymbol::Uid { pos0 })
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
        S: Serializer,
    {
        if !serializer.is_human_readable() {
            #[derive(Serialize)]
            enum Standard<'a> {
                Witness(&'a TypeName),
                Uid(&'a crate::types::sui_framework::object::ID),
            }

            return match self {
                PolicySymbol::Witness { pos0 } => Standard::Witness(pos0).serialize(serializer),
                PolicySymbol::Uid { pos0 } => Standard::Uid(pos0).serialize(serializer),
            };
        }

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
            PolicySymbol::Witness { pos0 } => Tagged {
                variant: "Witness",
                fields: Fields { pos0 },
            }
            .serialize(serializer),
            PolicySymbol::Uid { pos0 } => Tagged {
                variant: "Uid",
                fields: Fields { pos0 },
            }
            .serialize(serializer),
        }
    }
}

impl PolicySymbol {
    pub fn witness(name: TypeName) -> Self {
        Self::Witness { pos0: name }
    }

    pub fn uid(uid: sui::types::Address) -> Self {
        Self::Uid {
            pos0: crate::types::move_binding_support::sui_address_to_id(uid),
        }
    }

    pub fn as_witness(&self) -> Option<&TypeName> {
        match self {
            PolicySymbol::Witness { pos0 } => Some(pos0),
            PolicySymbol::Uid { .. } => None,
        }
    }

    pub fn as_uid(&self) -> Option<&sui::types::Address> {
        match self {
            PolicySymbol::Uid { pos0 } => Some(&pos0.bytes),
            PolicySymbol::Witness { .. } => None,
        }
    }

    /// Returns true when the witness matches the fully-qualified name.
    pub fn matches_qualified_name(&self, expected: &str) -> bool {
        self.as_witness()
            .map(|name| name.matches_qualified_name(expected))
            .unwrap_or(false)
    }
}

impl std::hash::Hash for PolicySymbol {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Witness { pos0 } => {
                0u8.hash(state);
                pos0.hash(state);
            }
            Self::Uid { pos0 } => {
                1u8.hash(state);
                pos0.bytes.hash(state);
            }
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

            let from_move_json: TaskState =
                serde_json::from_value(json!({ "@variant": name })).unwrap();
            assert_eq!(from_move_json, expected);

            assert_eq!(serde_json::to_value(&expected).unwrap(), json!(name));
        }
    }

    #[test]
    fn task_state_deserializes_move_json_variant_wrappers() {
        assert_eq!(
            serde_json::from_value::<TaskState>(json!({ "_variant_name": "Active" })).unwrap(),
            TaskState::Active
        );
        assert_eq!(
            serde_json::from_value::<TaskState>(json!({ "variant": "Paused" })).unwrap(),
            TaskState::Paused
        );
        assert_eq!(
            serde_json::from_value::<TaskState>(json!({ "Canceled": {} })).unwrap(),
            TaskState::Canceled
        );
        assert_eq!(
            serde_json::from_value::<TaskState>(json!("Exhausted")).unwrap(),
            TaskState::Completed
        );
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
                vertex: "cap_first".into(),
                recipient_id: crate::types::move_binding_support::sui_address_to_id(recipient_id),
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
        assert!(matches!(sym, PolicySymbol::Witness { pos0 } if pos0.name == "0x1::module::Type"));
    }

    #[test]
    fn policy_symbol_deserializes_enum_witness_with_string_type_name() {
        let json = json!({
            "variant": "Witness",
            "fields": { "pos0": "0xa5::scheduler::QueueGeneratorWitness" }
        });

        let sym: PolicySymbol = serde_json::from_value(json).unwrap();
        assert!(matches!(
            sym,
            PolicySymbol::Witness { pos0 }
                if pos0.name == "0xa5::scheduler::QueueGeneratorWitness"
        ));
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
        assert!(matches!(sym, PolicySymbol::Uid { pos0 } if pos0.bytes == addr));
    }

    #[test]
    fn policy_symbol_deserializes_bcs_witness() {
        let witness = TypeName::new("0x1::module::Type");
        let bytes = bcs::to_bytes(&PolicySymbol::witness(witness.clone())).unwrap();

        let parsed: PolicySymbol = bcs::from_bytes(&bytes).unwrap();
        assert!(matches!(parsed, PolicySymbol::Witness { pos0 } if pos0 == witness));
    }

    #[test]
    fn policy_symbol_deserializes_bcs_uid() {
        let mut rng = thread_rng();
        let addr = sui::types::Address::generate(&mut rng);
        let bytes = bcs::to_bytes(&PolicySymbol::uid(addr)).unwrap();

        let parsed: PolicySymbol = bcs::from_bytes(&bytes).unwrap();
        assert!(matches!(parsed, PolicySymbol::Uid { pos0 } if pos0.bytes == addr));
    }
}
