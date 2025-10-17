use {
    super::{
        serde_parsers::{
            deserialize_sui_address,
            deserialize_sui_u64,
            serialize_sui_address,
            serialize_sui_u64,
        },
        TypeName,
    },
    crate::sui,
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize},
    serde_json::{Map as JsonMap, Value},
};

/// Representation of `nexus_workflow::dag::DagExecutionConfig`.
#[derive(Clone, Debug, Serialize)]
pub struct DagExecutionConfig {
    pub dag: sui::ObjectID,
    pub network: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub gas_price: u64,
    #[serde(default)]
    pub entry_group: Value,
    #[serde(default)]
    pub inputs: Value,
    #[serde(
        deserialize_with = "deserialize_sui_address",
        serialize_with = "serialize_sui_address"
    )]
    pub invoker: sui::Address,
}

/// Representation of `nexus_workflow::scheduler::Task`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Task {
    pub id: sui::UID,
    #[serde(
        deserialize_with = "deserialize_sui_address",
        serialize_with = "serialize_sui_address"
    )]
    pub owner: sui::Address,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub constraints: Value,
    pub execution: LinearPolicy,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub objects: Value,
}

/// Minimal representation of `nexus_primitives::linear_policy::LinearPolicy`.
#[derive(Clone, Debug, Serialize)]
pub struct LinearPolicy {
    pub policy: Policy,
    #[serde(default)]
    pub sequence: Vec<PolicySymbol>,
}

/// Minimal representation of `nexus_primitives::policy::Policy`.
#[derive(Clone, Debug, Serialize)]
pub struct Policy {
    pub id: sui::UID,
    pub dfa: ConfiguredAutomaton,
    #[serde(default)]
    pub alphabet_index: Value,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub state_index: u64,
    #[serde(default)]
    pub data: Value,
}

/// Minimal representation of `nexus_primitives::automaton::ConfiguredAutomaton`.
#[derive(Clone, Debug, Serialize)]
pub struct ConfiguredAutomaton {
    pub id: sui::UID,
    #[serde(default)]
    pub dfa: Value,
}

/// Representation of `nexus_primitives::policy::Symbol`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
pub struct PolicySymbol {
    pub kind: u8,
    #[serde(default)]
    pub witness: Option<MoveTypeName>,
    #[serde(default)]
    pub uid: Option<sui::ObjectID>,
}

/// TypeName that can deserialize from Move struct wrappers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct MoveTypeName {
    pub name: String,
}

impl<'de> Deserialize<'de> for MoveTypeName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Inner {
            name: String,
        }

        let value = Value::deserialize(deserializer)?;
        let unwrapped = unwrap_move_value(value);
        serde_json::from_value::<Inner>(unwrapped)
            .map(|inner| Self { name: inner.name })
            .map_err(serde::de::Error::custom)
    }
}

impl From<MoveTypeName> for TypeName {
    fn from(move_name: MoveTypeName) -> Self {
        TypeName {
            name: move_name.name,
        }
    }
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

impl<'de> Deserialize<'de> for LinearPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Inner {
            policy: Policy,
            #[serde(default)]
            sequence: Vec<PolicySymbol>,
        }

        let inner: Inner = deserialize_move_struct(deserializer)?;

        Ok(Self {
            policy: inner.policy,
            sequence: inner.sequence,
        })
    }
}

impl<'de> Deserialize<'de> for Policy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Inner {
            id: sui::UID,
            dfa: ConfiguredAutomaton,
            #[serde(default)]
            alphabet_index: Value,
            #[serde(
                deserialize_with = "deserialize_sui_u64",
                serialize_with = "serialize_sui_u64"
            )]
            state_index: u64,
            #[serde(default)]
            data: Value,
        }

        let inner: Inner = deserialize_move_struct(deserializer)?;

        Ok(Self {
            id: inner.id,
            dfa: inner.dfa,
            alphabet_index: inner.alphabet_index,
            state_index: inner.state_index,
            data: inner.data,
        })
    }
}

impl<'de> Deserialize<'de> for ConfiguredAutomaton {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Inner {
            id: sui::UID,
            #[serde(default)]
            dfa: Value,
        }

        let inner: Inner = deserialize_move_struct(deserializer)?;

        Ok(Self {
            id: inner.id,
            dfa: inner.dfa,
        })
    }
}

impl<'de> Deserialize<'de> for DagExecutionConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Inner {
            dag: sui::ObjectID,
            network: sui::ObjectID,
            #[serde(
                deserialize_with = "deserialize_sui_u64",
                serialize_with = "serialize_sui_u64"
            )]
            gas_price: u64,
            #[serde(default)]
            entry_group: Value,
            #[serde(default)]
            inputs: Value,
            #[serde(
                deserialize_with = "deserialize_sui_address",
                serialize_with = "serialize_sui_address"
            )]
            invoker: sui::Address,
        }

        let inner: Inner = deserialize_move_struct(deserializer)?;

        Ok(Self {
            dag: inner.dag,
            network: inner.network,
            gas_price: inner.gas_price,
            entry_group: inner.entry_group,
            inputs: inner.inputs,
            invoker: inner.invoker,
        })
    }
}

pub(crate) fn deserialize_move_struct<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    let unwrapped = unwrap_move_value(value);
    serde_json::from_value(unwrapped).map_err(serde::de::Error::custom)
}

fn unwrap_move_value(value: Value) -> Value {
    match value {
        Value::Object(map) => unwrap_move_object(map),
        Value::Array(values) => Value::Array(values.into_iter().map(unwrap_move_value).collect()),
        other => other,
    }
}

fn unwrap_move_object(mut map: JsonMap<String, Value>) -> Value {
    if map.len() == 1 {
        if let Some(value) = map.remove("data") {
            return unwrap_move_value(value);
        }

        if let Some(value) = map.remove("value") {
            return unwrap_move_value(value);
        }

        if let Some(value) = map.remove("contents") {
            return unwrap_move_value(value);
        }
    }

    let mut recursively_unwrapped = JsonMap::new();
    for (key, value) in map.into_iter() {
        recursively_unwrapped.insert(key, unwrap_move_value(value));
    }

    if let Some(Value::Object(fields)) = recursively_unwrapped.remove("fields") {
        let mut merged = JsonMap::new();

        for (key, value) in fields.into_iter() {
            merged.insert(key, value);
        }

        for (key, value) in recursively_unwrapped.into_iter() {
            if !is_metadata_key(&key) {
                merged.insert(key, value);
            }
        }

        return unwrap_move_object(merged);
    }

    if let Some(unwrapped_value) = extract_wrapped_value(&recursively_unwrapped) {
        return unwrap_move_value(unwrapped_value);
    }

    Value::Object(recursively_unwrapped)
}

fn extract_wrapped_value(map: &JsonMap<String, Value>) -> Option<Value> {
    let Some(value) = map.get("value") else {
        return None;
    };

    if map
        .keys()
        .filter(|key| key.as_str() != "value")
        .all(|key| is_value_wrapper_key(key))
    {
        return Some(value.clone());
    }

    None
}

fn is_metadata_key(key: &str) -> bool {
    matches!(
        key,
        "type"
            | "has_public_transfer"
            | "hasPublicTransfer"
            | "dataType"
            | "data_type"
            | "objectType"
            | "bcs"
    )
}

fn is_value_wrapper_key(key: &str) -> bool {
    matches!(
        key,
        "name"
            | "key"
            | "type"
            | "objectType"
            | "object_type"
            | "has_public_transfer"
            | "hasPublicTransfer"
            | "dataType"
            | "data_type"
            | "bcs"
    )
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        serde_json::json,
        sui::{move_ident_str, MoveStructTag, MoveTypeTag, ObjectID},
    };

    #[test]
    fn linear_policy_deserializes_from_wrapped_move_struct() {
        let policy_id = ObjectID::from_hex_literal("0x2").expect("valid object id");
        let dfa_id = ObjectID::from_hex_literal("0x3").expect("valid object id");
        let sequence_symbol_uid = ObjectID::from_hex_literal("0x4").expect("valid object id");

        let expected_policy = Policy {
            id: sui::UID::new(policy_id),
            dfa: ConfiguredAutomaton {
                id: sui::UID::new(dfa_id),
                dfa: json!({
                    "type": MoveStructTag {
                        address: *sui::FRAMEWORK_PACKAGE_ID,
                        module: move_ident_str!("dummy").into(),
                        name: move_ident_str!("Config").into(),
                        type_params: vec![MoveTypeTag::Address],
                    }
                }),
            },
            alphabet_index: Value::Null,
            state_index: 5,
            data: json!({ "extra": "payload" }),
        };

        let expected_sequence = vec![PolicySymbol {
            kind: 1,
            witness: None,
            uid: Some(sequence_symbol_uid),
        }];

        let expected = LinearPolicy {
            policy: expected_policy.clone(),
            sequence: expected_sequence.clone(),
        };

        let wrapped = wrap_move_value(serde_json::to_value(&expected).expect("serialize policy"));

        let parsed: LinearPolicy =
            serde_json::from_value(wrapped).expect("deserialize wrapped policy");

        assert_eq!(parsed.policy.id, expected.policy.id);
        assert_eq!(parsed.policy.dfa.id, expected.policy.dfa.id);
        assert_eq!(parsed.policy.dfa.dfa, expected.policy.dfa.dfa);
        assert_eq!(parsed.policy.alphabet_index, expected.policy.alphabet_index);
        assert_eq!(parsed.policy.state_index, expected.policy.state_index);
        assert_eq!(parsed.policy.data, expected.policy.data);
        assert_eq!(parsed.sequence, expected.sequence);
    }

    #[test]
    fn dag_execution_config_deserializes_from_wrapped_move_struct() {
        let dag_id = ObjectID::from_hex_literal("0xabc").expect("valid object id");
        let network_id = ObjectID::from_hex_literal("0xdef").expect("valid object id");
        let invoker = sui::Address::random_for_testing_only();

        let expected = DagExecutionConfig {
            dag: dag_id,
            network: network_id,
            gas_price: 1000,
            entry_group: json!({"name": "default"}),
            inputs: json!({}),
            invoker,
        };

        let wrapped = wrap_move_value(serde_json::to_value(&expected).expect("serialize config"));

        let parsed: DagExecutionConfig =
            serde_json::from_value(wrapped).expect("deserialize wrapped config");

        assert_eq!(parsed.dag, expected.dag);
        assert_eq!(parsed.network, expected.network);
        assert_eq!(parsed.gas_price, expected.gas_price);
        assert_eq!(parsed.entry_group, expected.entry_group);
        assert_eq!(parsed.inputs, expected.inputs);
        assert_eq!(parsed.invoker, expected.invoker);
    }

    fn wrap_move_value(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let wrapped_fields = map
                    .into_iter()
                    .map(|(k, v)| (k, wrap_move_value(v)))
                    .collect::<JsonMap<String, Value>>();

                let mut struct_map = JsonMap::new();
                struct_map.insert("type".into(), Value::String("0x1::dummy::Struct".into()));
                struct_map.insert("dataType".into(), Value::String("moveObject".into()));
                struct_map.insert("has_public_transfer".into(), Value::Bool(false));
                struct_map.insert("fields".into(), Value::Object(wrapped_fields));

                let mut data_fields = JsonMap::new();
                data_fields.insert("fields".into(), Value::Object(struct_map));

                let mut field_wrapper = JsonMap::new();
                field_wrapper.insert("data".into(), Value::Object(data_fields));

                let mut dynamic_field = JsonMap::new();
                dynamic_field.insert("name".into(), Value::String("dummy_name".into()));
                dynamic_field.insert("value".into(), Value::Object(field_wrapper));

                Value::Object(dynamic_field)
            }
            Value::Array(items) => {
                let wrapped_items = items.into_iter().map(wrap_move_value).collect::<Vec<_>>();
                let mut contents = JsonMap::new();
                contents.insert("contents".into(), Value::Array(wrapped_items));
                let mut map = JsonMap::new();
                map.insert("value".into(), Value::Object(contents));
                Value::Object(map)
            }
            other => other,
        }
    }
}
