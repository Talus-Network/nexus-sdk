//! This module contains a struct representation of the task used by the scheduler.
//! It provides a way to serialize and deserialize the task and any helper structures.
use {
    super::{
        serde_parsers::{deserialize_sui_u64, serialize_sui_u64},
        TypeName,
    },
    crate::{
        nexus::crawler::{Bag, Map, ObjectBag},
        sui,
        types::NexusData,
    },
    serde::{Deserialize, Deserializer, Serialize},
};

/// Representation of `nexus_workflow::dag::DagExecutionConfig`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DagExecutionConfig {
    pub dag: sui::types::Address,
    pub network: sui::types::Address,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub gas_price: u64,
    pub entry_group: SchedulerEntryGroup,
    pub inputs: Map<String, NexusData>,
    pub invoker: sui::types::Address,
}

/// Representation of `nexus_workflow::scheduler::Task`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Task {
    pub id: sui::types::Address,
    pub owner: sui::types::Address,
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
    pub states: Vec<u64>,
    pub alphabet: Vec<PolicySymbol>,
    pub transition: Vec<Vec<u64>>,
    pub accepting: Vec<bool>,
    pub start: u64,
}

/// Representation of `nexus_primitives::policy::Symbol`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PolicySymbol {
    Witness(TypeName),
    Uid(sui::types::Address),
}

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

        // Human readable formats (GraphQL/JSON) use the { variant, fields: { pos0 } } shape.
        #[derive(Deserialize)]
        struct Tagged {
            variant: String,
            fields: serde_json::Value,
        }

        #[derive(Deserialize)]
        struct Fields<T> {
            #[serde(rename = "pos0")]
            pos0: T,
        }

        let tagged = Tagged::deserialize(deserializer)?;
        match tagged.variant.as_str() {
            "Witness" => {
                let fields: Fields<TypeName> =
                    serde_json::from_value(tagged.fields).map_err(serde::de::Error::custom)?;
                Ok(PolicySymbol::Witness(fields.pos0))
            }
            "Uid" => {
                let fields: Fields<sui::types::Address> =
                    serde_json::from_value(tagged.fields).map_err(serde::de::Error::custom)?;
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

        // Human readable formats (GraphQL/JSON) use the { variant, fields: { pos0 } } shape.
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

/// Scheduler task state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    Active,
    Paused,
    Canceled,
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

/// Representation of `nexus_workflow::dag::EntryGroup`.
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
                    states: vec![0],
                    alphabet: vec![PolicySymbol::Witness(TypeName::new("0x1::module::Type"))],
                    transition: vec![vec![0]],
                    accepting: vec![true],
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
    fn dag_execution_config_deserializes_from_json() {
        let mut rng = rand::thread_rng();
        let dag_id = sui::types::Address::generate(&mut rng);
        let network_id = sui::types::Address::generate(&mut rng);
        let invoker = sui::types::Address::generate(&mut rng);

        let expected = DagExecutionConfig {
            dag: dag_id,
            network: network_id,
            gas_price: 1000,
            entry_group: SchedulerEntryGroup {
                name: "default".to_string(),
            },
            inputs: Map::default(),
            invoker,
        };

        let parsed: DagExecutionConfig =
            serde_json::from_value(serde_json::to_value(&expected).unwrap()).unwrap();

        assert_eq!(parsed.dag, expected.dag);
        assert_eq!(parsed.network, expected.network);
        assert_eq!(parsed.gas_price, expected.gas_price);
        assert_eq!(parsed.entry_group, expected.entry_group);
        assert_eq!(parsed.inputs, expected.inputs);
        assert_eq!(parsed.invoker, expected.invoker);
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
