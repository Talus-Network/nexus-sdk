use {
    crate::{idents::primitives, sui, ToolFqn},
    serde::{Deserialize, Serialize},
    std::hash::Hash,
};

/// Struct holding the Sui event ID, the event generic arguments and the data
/// as one of [NexusEventKind].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NexusEvent {
    /// The event transaction digest and event sequence. Useful to filter down
    /// events.
    pub id: sui::EventID,
    /// If the `T in NexusEvent<T>` is also a generic, this field holds the
    /// generic type. Note that this can be nested indefinitely.
    pub generics: Vec<sui::MoveTypeTag>,
    /// The event data.
    pub data: NexusEventKind,
}

/// This allows us to deserialize SuiEvent into [NexusEvent] and match the
/// corresponding event kind to one of [NexusEventKind].
const NEXUS_EVENT_TYPE_TAG: &str = "_nexus_event_type";

/// Enumeration with all available events coming from the on-chain part of
/// Nexus.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "_nexus_event_type", content = "event")]
pub enum NexusEventKind {
    #[serde(rename = "RequestWalkExecutionEvent")]
    RequestWalkExecution(RequestWalkExecutionEvent),
    #[serde(rename = "AnnounceInterfacePackageEvent")]
    AnnounceInterfacePackage(AnnounceInterfacePackageEvent),
    #[serde(rename = "OffChainToolRegisteredEvent")]
    OffChainToolRegistered(OffChainToolRegisteredEvent),
    #[serde(rename = "OnChainToolRegisteredEvent")]
    OnChainToolRegistered(OnChainToolRegisteredEvent),
    #[serde(rename = "ToolUnregisteredEvent")]
    ToolUnregistered(ToolUnregisteredEvent),
    // These events are unused for now.
    #[serde(rename = "FoundingLeaderCapCreatedEvent")]
    FoundingLeaderCapCreated(serde_json::Value),
    #[serde(rename = "ToolRegistryCreatedEvent")]
    ToolRegistryCreated(serde_json::Value),
    #[serde(rename = "DAGCreatedEvent")]
    DAGCreated(serde_json::Value),
    #[serde(rename = "DAGVertexAddedEvent")]
    DAGVertexAdded(serde_json::Value),
    #[serde(rename = "DAGEdgeAddedEvent")]
    DAGEdgeAdded(serde_json::Value),
    #[serde(rename = "DAGEntryVertexAddedEvent")]
    DAGEntryVertexAdded(serde_json::Value),
    #[serde(rename = "DAGDefaultValueAddedEvent")]
    DAGDefaultValueAdded(serde_json::Value),
    #[serde(rename = "EndStateReachedEvent")]
    EndStateReached(serde_json::Value),
    #[serde(rename = "ExecutionFinishedEvent")]
    ExecutionFinished(serde_json::Value),
    #[serde(rename = "WalkAdvancedEvent")]
    WalkAdvanced(serde_json::Value),
}

/// Useful struct as quite a few structs coming from on-chain events are just
/// `{ name: String }`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TypeName {
    pub name: String,
}

// == Event definitions ==

/// Fired by the on-chain part of Nexus when a DAG vertex execution is
/// requested.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestWalkExecutionEvent {
    pub dag: sui::ObjectID,
    pub execution: sui::ObjectID,
    #[serde(
        deserialize_with = "parsers::deserialize_sui_u64",
        serialize_with = "parsers::serialize_sui_u64"
    )]
    pub walk_index: u64,
    pub next_vertex: TypeName,
    pub evaluations: sui::ObjectID,
    /// This field defines the package ID, module and name of the Agent that
    /// holds the DAG. Used to confirm the tool evaluation with the Agent.
    pub worksheet_from_type: TypeName,
}

/// Fired via the Nexus `interface` package when a new Agent is registered.
/// Provides the agent's interface so that we can invoke
/// `confirm_tool_eval_for_walk` on it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnnounceInterfacePackageEvent {
    pub shared_objects: Vec<sui::ObjectID>,
}

/// Fired by the Nexus Workflow when a new off-chain tool is registered so that
/// the Leader can also register it in Redis. This way the Leader knows how and
/// where to evaluate the tool.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OffChainToolRegisteredEvent {
    pub registry: sui::ObjectID,
    pub tool: sui::ObjectID,
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
    #[serde(
        deserialize_with = "parsers::deserialize_bytes_to_url",
        serialize_with = "parsers::serialize_url_to_bytes"
    )]
    pub url: reqwest::Url,
    #[serde(
        deserialize_with = "parsers::deserialize_bytes_to_json_value",
        serialize_with = "parsers::serialize_json_value_to_bytes"
    )]
    pub input_schema: serde_json::Value,
    #[serde(
        deserialize_with = "parsers::deserialize_bytes_to_json_value",
        serialize_with = "parsers::serialize_json_value_to_bytes"
    )]
    pub output_schema: serde_json::Value,
}

/// Fired by the Nexus Workflow when a new on-chain tool is registered so that
/// the Leader can also register it in Redis. This way the Leader knows how and
/// where to evaluate the tool.
// TODO: <https://github.com/Talus-Network/nexus-next/issues/96>
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OnChainToolRegisteredEvent {
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
}

/// Fired by the Nexus Workflow when a tool is unregistered. The Leader should
/// remove the tool definition from its Redis registry.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolUnregisteredEvent {
    pub tool: sui::ObjectID,
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
}

// == Useful impls ==

impl TryInto<NexusEvent> for sui::Event {
    type Error = anyhow::Error;

    fn try_into(self) -> anyhow::Result<NexusEvent> {
        let id = self.id;

        let sui::MoveStructTag {
            name,
            module,
            type_params,
            ..
        } = self.type_;

        if name != primitives::Event::EVENT_WRAPPER.name.into()
            || module != primitives::Event::EVENT_WRAPPER.module.into()
        {
            anyhow::bail!("Event is not a Nexus event");
        };

        // Extract the event name from its type parameters. This is used to
        // match the corresponding [NexusEventKind].
        let Some(sui::MoveTypeTag::Struct(type_param)) = type_params.into_iter().next() else {
            anyhow::bail!("Event is not a struct");
        };

        let sui::MoveStructTag {
            name, type_params, ..
        } = *type_param;

        // This allows us to insert the event name to the json object. This way
        // we can then automatically deserialize into the correct
        // [NexusEventKind].
        let mut payload = self.parsed_json;

        payload
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Event payload could not be accessed"))?
            .insert(NEXUS_EVENT_TYPE_TAG.to_string(), name.to_string().into());

        let data = match serde_json::from_value(payload) {
            Ok(data) => data,
            Err(e) => {
                anyhow::bail!("Could not deserialize event data for event '{name}': {e}");
            }
        };

        Ok(NexusEvent {
            id,
            generics: type_params,
            data,
        })
    }
}

mod parsers {
    use {
        super::*,
        serde::{de::Deserializer, ser::Serializer},
    };

    /// Deserialize a `Vec<u8>` into a [reqwest::Url].
    pub(super) fn deserialize_bytes_to_url<'de, D>(
        deserializer: D,
    ) -> Result<reqwest::Url, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        let url = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

        reqwest::Url::parse(&url).map_err(serde::de::Error::custom)
    }

    /// Inverse of [deserialize_bytes_to_url].
    pub(super) fn serialize_url_to_bytes<S>(
        value: &reqwest::Url,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let url = value.to_string();
        let bytes = url.into_bytes();

        bytes.serialize(serializer)
    }

    /// Deserialize a `Vec<u8>` into a [serde_json::Value].
    pub(super) fn deserialize_bytes_to_json_value<'de, D>(
        deserializer: D,
    ) -> Result<serde_json::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = Deserialize::deserialize(deserializer)?;
        let value = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

        serde_json::from_str(&value).map_err(serde::de::Error::custom)
    }

    /// Inverse of [deserialize_bytes_to_json_value].
    pub(super) fn serialize_json_value_to_bytes<S>(
        value: &serde_json::Value,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        let bytes = value.into_bytes();

        bytes.serialize(serializer)
    }

    /// Deserialize a `Vec<Vec<u8>>` into a `serde_json::Value`.
    ///
    /// If the outer `Vec` is len 1, it will be deserialized as a single JSON value.
    /// Otherwise it will be deserialized as a JSON array.
    #[allow(dead_code)]
    pub(super) fn deserialize_array_of_bytes_to_json_value<'de, D>(
        deserializer: D,
    ) -> Result<serde_json::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        let array_of_bytes: Vec<Vec<u8>> = Deserialize::deserialize(deserializer)?;
        let mut result = Vec::with_capacity(array_of_bytes.len());

        for bytes in array_of_bytes {
            let value = String::from_utf8(bytes).map_err(serde::de::Error::custom)?;

            // TODO: This is temporarily added here to automatically fallback to
            // a JSON String if we can't parse the bytes as JSON. In the future,
            // this should fail the execution.
            //
            // TODO: <https://github.com/Talus-Network/nexus-next/issues/97>
            let value = match serde_json::from_str(&value) {
                Ok(value) => value,
                Err(_) => serde_json::Value::String(value),
            };

            result.push(value);
        }

        match result.len() {
            1 => Ok(result.pop().expect("Len is 1")),
            _ => Ok(serde_json::Value::Array(result)),
        }
    }

    /// Inverse of [deserialize_array_of_bytes_to_json_value].
    #[allow(dead_code)]
    pub(super) fn serialize_json_value_to_array_of_bytes<S>(
        value: &serde_json::Value,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // The structure of the data here is TBD.
        //
        // TODO: <https://github.com/Talus-Network/nexus-next/issues/97>
        let array = match value {
            serde_json::Value::Array(array) => array,
            value => &vec![value.clone()],
        };

        let mut result = Vec::with_capacity(array.len());

        for value in array {
            let value = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
            let bytes = value.into_bytes();

            result.push(bytes);
        }

        result.serialize(serializer)
    }

    /// Custom parser for deserializing to a [u64] from Sui Events. They wrap this
    /// type as a string to avoid overflow.
    ///
    /// See [sui_sdk::rpc_types::SuiMoveValue] for more information.
    pub(super) fn deserialize_sui_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value: String = Deserialize::deserialize(deserializer)?;
        let value = value.parse::<u64>().map_err(serde::de::Error::custom)?;

        Ok(value)
    }

    /// Inverse of [deserialize_sui_u64] for indexing reasons.
    pub(super) fn serialize_sui_u64<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.to_string().serialize(serializer)
    }

    #[cfg(test)]
    mod tests {
        use {super::*, serde::Deserialize, serde_json::json};

        #[derive(Deserialize)]
        struct TestStruct {
            #[serde(deserialize_with = "deserialize_array_of_bytes_to_json_value")]
            value: serde_json::Value,
        }

        #[test]
        fn test_single_valid_json_number() {
            // This test supplies a single element.
            // The inner array [49, 50, 51] corresponds to the UTF-8 string "123".
            // "123" is valid JSON and parses to the number 123.
            let input = r#"
        {
            "value": [[49,50,51]]
        }
        "#;
            let result: TestStruct = serde_json::from_str(input).unwrap();
            assert_eq!(result.value, json!(123));
        }

        #[test]
        fn test_multiple_valid_json() {
            // Two elements:
            // First element: [34,118,97,108,117,101,34] corresponds to "\"value\""
            //   which is valid JSON and becomes the string "value".
            // Second element: [49,50,51] corresponds to "123" and becomes the number 123.
            // Since there is more than one element, the deserializer returns a JSON array.
            let input = r#"
        {
            "value": [
                [34,118,97,108,117,101,34],
                [49,50,51]
            ]
        }
        "#;
            let result: TestStruct = serde_json::from_str(input).unwrap();
            assert_eq!(result.value, json!(["value", 123]));
        }

        #[test]
        fn test_single_invalid_json_fallback() {
            // Single element with bytes for "hello": [104,101,108,108,111].
            // "hello" is not valid JSON (missing quotes), so the fallback
            // returns the string "hello" as a JSON string.
            let input = r#"
        {
            "value": [[104,101,108,108,111]]
        }
        "#;
            let result: TestStruct = serde_json::from_str(input).unwrap();
            assert_eq!(result.value, json!("hello"));
        }

        #[test]
        fn test_empty_array() {
            // An empty outer array should result in an empty JSON array.
            let input = r#"
        {
            "value": []
        }
        "#;
            let result: TestStruct = serde_json::from_str(input).unwrap();
            assert_eq!(result.value, json!([]));
        }
    }
}
