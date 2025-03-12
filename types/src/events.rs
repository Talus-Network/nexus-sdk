use {
    crate::{idents::primitives, sui, types::*, ToolFqn},
    serde::{Deserialize, Serialize},
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
    #[serde(rename = "WalkAdvancedEvent")]
    WalkAdvanced(WalkAdvancedEvent),
    #[serde(rename = "EndStateReachedEvent")]
    EndStateReached(EndStateReachedEvent),
    #[serde(rename = "ExecutionFinishedEvent")]
    ExecutionFinished(ExecutionFinishedEvent),
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
}

// == Event definitions ==

/// Fired by the on-chain part of Nexus when a DAG vertex execution is
/// requested.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestWalkExecutionEvent {
    pub dag: sui::ObjectID,
    pub execution: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
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
        deserialize_with = "deserialize_bytes_to_url",
        serialize_with = "serialize_url_to_bytes"
    )]
    pub url: reqwest::Url,
    #[serde(
        deserialize_with = "deserialize_bytes_to_json_value",
        serialize_with = "serialize_json_value_to_bytes"
    )]
    pub input_schema: serde_json::Value,
    #[serde(
        deserialize_with = "deserialize_bytes_to_json_value",
        serialize_with = "serialize_json_value_to_bytes"
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

/// Fired by the Nexus Workflow when a walk has advanced. This event is used to
/// inspect DAG execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkAdvancedEvent {
    pub dag: sui::ObjectID,
    pub execution: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub walk_index: u64,
    /// Which vertex was just executed.
    pub vertex: TypeName,
    /// Which output variant was evaluated.
    pub variant: TypeName,
    /// What data is associated with the variant.
    // TODO: the deser can be improved but it requires some bigger changes in
    // the object crawler as well as porting the crawler to this crate.
    pub variant_ports_to_data: serde_json::Value,
}

/// Fired by the Nexus Workflow when a walk has halted in an end state. This
/// event is used to inspect DAG execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EndStateReachedEvent {
    pub dag: sui::ObjectID,
    pub execution: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub walk_index: u64,
    /// Which vertex was just executed.
    pub vertex: TypeName,
    /// Which output variant was evaluated.
    pub variant: TypeName,
    /// What data is associated with the variant.
    // TODO: the deser can be improved but it requires some bigger changes in
    // the object crawler as well as porting the crawler to this crate.
    pub variant_ports_to_data: serde_json::Value,
}

/// Fired by the Nexus Workflow when all walks have halted in their end states
/// and there is no more work to be done. This event is used to inspect DAG
/// execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionFinishedEvent {
    pub dag: sui::ObjectID,
    pub execution: sui::ObjectID,
    pub has_any_walk_failed: bool,
    pub has_any_walk_succeeded: bool,
}

// == Parsing ==

/// Parse [`sui::Event`] into [`NexusEvent`]. We check that the module and name
/// of the event wrapper are what we expect. Then we add the event name as a
/// field in the json object with the [`NEXUS_EVENT_TYPE_TAG`] key. This way we
/// can automatically deserialize into the correct [`NexusEventKind`].
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

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    fn dummy_event(
        name: sui::Identifier,
        data: serde_json::Value,
        generics: Vec<sui::MoveTypeTag>,
    ) -> sui::Event {
        sui::Event {
            id: sui::EventID {
                tx_digest: sui::TransactionDigest::random(),
                event_seq: 42,
            },
            package_id: sui::ObjectID::random(),
            transaction_module: sui::move_ident_str!("primitives").into(),
            sender: sui::ObjectID::random().into(),
            bcs: vec![],
            timestamp_ms: None,
            type_: sui::MoveStructTag {
                address: *sui::ObjectID::random(),
                name: primitives::Event::EVENT_WRAPPER.name.into(),
                module: primitives::Event::EVENT_WRAPPER.module.into(),
                type_params: vec![sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                    address: *sui::ObjectID::random(),
                    name,
                    module: sui::move_ident_str!("dag").into(),
                    type_params: generics,
                }))],
            },
            parsed_json: data,
        }
    }

    #[test]
    fn test_sui_event_desers_into_nexus_event() {
        let dag = sui::ObjectID::random();
        let execution = sui::ObjectID::random();
        let evaluations = sui::ObjectID::random();

        let generic = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::ObjectID::random(),
            name: sui::move_ident_str!("Foo").into(),
            module: sui::move_ident_str!("bar").into(),
            type_params: vec![],
        }));

        let event = dummy_event(
            sui::move_ident_str!("RequestWalkExecutionEvent").into(),
            serde_json::json!({
                "event":{
                    "dag": dag.to_string(),
                    "execution": execution.to_string(),
                    "walk_index": "42",
                    "next_vertex": {
                        "name": "foo",
                    },
                    "evaluations": evaluations.to_string(),
                    "worksheet_from_type": {
                        "name": "bar",
                    },
                }
            }),
            vec![generic.clone()],
        );

        let event: NexusEvent = event.try_into().unwrap();

        assert_eq!(event.generics, vec![generic]);
        assert_matches!(event.data, NexusEventKind::RequestWalkExecution(e)
            if e.dag == dag &&
                e.execution == execution &&
                e.evaluations == evaluations &&
                e.walk_index == 42 &&
                e.next_vertex.name == "foo".to_string() &&
                e.worksheet_from_type.name == "bar".to_string()
        );
    }
}
