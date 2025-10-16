use {
    crate::{idents::primitives, sui, types::*, ToolFqn},
    serde::{de::DeserializeOwned, Deserialize, Serialize},
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct RequestScheduledExecution<T>
where
    T: Clone,
{
    pub request: T,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub priority: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub request_ms: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub start_ms: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub deadline_ms: u64,
}

/// Enumeration with all available events coming from the on-chain part of
/// Nexus.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "_nexus_event_type", content = "event")]
pub enum NexusEventKind {
    #[serde(rename = "RequestScheduledExecution")]
    RequestScheduledExecution(RequestScheduledExecution<RequestWalkExecutionEvent>),
    #[serde(rename = "OccurrenceScheduledExecution")]
    OccurrenceScheduled(RequestScheduledExecution<OccurrenceScheduledEvent>),
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
    #[serde(rename = "WalkFailedEvent")]
    WalkFailed(WalkFailedEvent),
    #[serde(rename = "EndStateReachedEvent")]
    EndStateReached(EndStateReachedEvent),
    #[serde(rename = "ExecutionFinishedEvent")]
    ExecutionFinished(ExecutionFinishedEvent),
    #[serde(rename = "MissedOccurrenceEvent")]
    MissedOccurrence(MissedOccurrenceEvent),
    #[serde(rename = "TaskCreatedEvent")]
    TaskCreated(TaskCreatedEvent),
    #[serde(rename = "TaskPausedEvent")]
    TaskPaused(TaskPausedEvent),
    #[serde(rename = "TaskResumedEvent")]
    TaskResumed(TaskResumedEvent),
    #[serde(rename = "TaskCanceledEvent")]
    TaskCanceled(TaskCanceledEvent),
    #[serde(rename = "OccurrenceConsumedEvent")]
    OccurrenceConsumed(OccurrenceConsumedEvent),
    #[serde(rename = "PeriodicScheduleConfiguredEvent")]
    PeriodicScheduleConfigured(PeriodicScheduleConfiguredEvent),
    #[serde(rename = "FoundingLeaderCapCreatedEvent")]
    FoundingLeaderCapCreated(FoundingLeaderCapCreatedEvent),
    #[serde(rename = "GasSettlementUpdateEvent")]
    GasSettlementUpdate(GasSettlementUpdateEvent),
    #[serde(rename = "PreKeyVaultCreatedEvent")]
    PreKeyVaultCreated(PreKeyVaultCreatedEvent),
    #[serde(rename = "PreKeyClaimedEvent")]
    PreKeyClaimed(PreKeyClaimedEvent),
    #[serde(rename = "PreKeyAssociatedEvent")]
    PreKeyAssociated(PreKeyAssociatedEvent),
    // These events are unused for now.
    #[serde(rename = "ToolRegistryCreatedEvent")]
    ToolRegistryCreated(serde_json::Value),
    #[serde(rename = "DAGCreatedEvent")]
    DAGCreated(serde_json::Value),
    #[serde(rename = "DAGVertexAddedEvent")]
    DAGVertexAdded(serde_json::Value),
    #[serde(rename = "DAGEdgeAddedEvent")]
    DAGEdgeAdded(serde_json::Value),
    #[serde(rename = "DAGOutputAddedEvent")]
    DAGOutputAdded(serde_json::Value),
    #[serde(rename = "DAGEntryVertexInputPortAddedEvent")]
    DAGEntryVertexInputPortAdded(serde_json::Value),
    #[serde(rename = "DAGDefaultValueAddedEvent")]
    DAGDefaultValueAdded(serde_json::Value),
    #[serde(rename = "LeaderClaimedGasEvent")]
    LeaderClaimedGas(serde_json::Value),
    #[serde(rename = "AllowedOwnerAddedEvent")]
    AllowedOwnerAdded(serde_json::Value),
    #[serde(rename = "AllowedOwnerRemovedEvent")]
    AllowedOwnerRemoved(serde_json::Value),
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
    pub next_vertex: RuntimeVertex,
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
    pub vertex: RuntimeVertex,
    /// Which output variant was evaluated.
    pub variant: TypeName,
    /// What data is associated with the variant.
    pub variant_ports_to_data: serde_json::Value,
}

/// Fored by the Nexus Workflow when a walk has failed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkFailedEvent {
    pub dag: sui::ObjectID,
    pub execution: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub walk_index: u64,
    /// Which vertex was being executed when the failure happened.
    pub vertex: RuntimeVertex,
    /// The error message associated with the failure.
    pub reason: String,
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
    pub vertex: RuntimeVertex,
    /// Which output variant was evaluated.
    pub variant: TypeName,
    /// What data is associated with the variant.
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

/// Fired when a scheduler occurrence is enqueued (wrapped in `RequestScheduledExecution`).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OccurrenceScheduledEvent {
    pub task: sui::ObjectID,
    pub from_periodic: bool,
}

/// Emitted when a scheduled occurrence misses its deadline and is pruned.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MissedOccurrenceEvent {
    pub task: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub start_time: u64,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub deadline: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub pruned_at: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub gas_price: u64,
}

/// Emitted after a scheduler task object is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCreatedEvent {
    pub task: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_address",
        serialize_with = "serialize_sui_address"
    )]
    pub owner: sui::Address,
}

/// Emitted when scheduling for a task is paused.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskPausedEvent {
    pub task: sui::ObjectID,
}

/// Emitted when scheduling for a task is resumed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskResumedEvent {
    pub task: sui::ObjectID,
}

/// Emitted when scheduling for a task is canceled.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCanceledEvent {
    pub task: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub cleared_occurrences: u64,
    pub had_periodic: bool,
}

/// Emitted whenever a pending occurrence is consumed for execution.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OccurrenceConsumedEvent {
    pub task: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub start_time: u64,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub deadline: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub gas_price: u64,
    pub from_periodic: bool,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub executed_at: u64,
}

/// Emitted whenever the periodic schedule is configured or cleared.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeriodicScheduleConfiguredEvent {
    pub task: sui::ObjectID,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub period_ms: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub deadline_offset_ms: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub max_iterations: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub generated: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub gas_price: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub last_generated_start: Option<u64>,
}

/// Fired by the Nexus Workflow when a new founding LeaderCap is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundingLeaderCapCreatedEvent {
    pub leader_cap: sui::ObjectID,
    pub network: sui::ObjectID,
}

/// Fired by the Gas service when the gas settlement is updated. This event is
/// used to determine whether a tool invocation was paid for by the caller.
/// Combination of `execution` and `vertex` uniquely identifies the tool
/// invocation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GasSettlementUpdateEvent {
    pub execution: sui::ObjectID,
    pub tool_fqn: ToolFqn,
    pub vertex: RuntimeVertex,
    pub was_settled: bool,
}

/// Fired by the Nexus Workflow when a new pre key vault is created. This happens
/// on initial network setup.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyVaultCreatedEvent {
    pub vault: sui::ObjectID,
    pub crypto_cap: sui::ObjectID,
}

/// Fired by the Nexus Workflow when a pre key is claimed. Notifies about the
/// remaining pre keys in the vault after the claim.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyClaimedEvent {
    /// How many pre keys are left in the vault after this claim.
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub remaining: u64,
}

/// Fired by the Nexus Workflow when a pre key is associated with a user.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyAssociatedEvent {
    /// The address of the user the pre key is associated with.
    pub claimed_by: sui::ObjectID,
    /// Bytes of the pre key.
    pub pre_key: Vec<u8>,
    /// Bytes of the initial message.
    pub initial_message: Vec<u8>,
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

        let mut event_kind_name = name.to_string();
        // TODO: Fix the implementation
        if event_kind_name == "RequestScheduledExecution" {
            let mut is_occurrence =
                if let Some(sui::MoveTypeTag::Struct(inner)) = type_params.get(0) {
                    inner.name.as_str() == "OccurrenceScheduledEvent"
                } else {
                    false
                };

            if !is_occurrence {
                if let Some(event) = payload.get("event") {
                    if let Some(request) = event.get("request") {
                        is_occurrence = request.get("dag").is_none()
                            && request.get("execution").is_none()
                            && request.get("task").is_some();
                    }
                }
            }

            if is_occurrence {
                event_kind_name = "OccurrenceScheduledExecution".to_string();
            }
        }

        payload
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Event payload could not be accessed"))?
            .insert(NEXUS_EVENT_TYPE_TAG.to_string(), event_kind_name.into());

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
            bcs: sui::BcsEvent::new(vec![]),
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
                        "variant": "Plain",
                        "fields": {
                            "vertex": { "name": "foo" },
                        }
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
                matches!(&e.next_vertex, RuntimeVertex::Plain { vertex } if vertex.name == "foo") &&
                e.worksheet_from_type.name == *"bar"
        );
    }

    #[test]
    fn test_sui_event_desers_into_nexus_event_with_schedule_wrapper() {
        let dag = sui::ObjectID::random();
        let execution = sui::ObjectID::random();
        let evaluations = sui::ObjectID::random();

        let inner = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::ObjectID::random(),
            name: sui::move_ident_str!("RequestWalkExecutionEvent").into(),
            module: sui::move_ident_str!("dag").into(),
            type_params: vec![],
        }));

        let outer = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::ObjectID::random(),
            name: sui::move_ident_str!("RequestScheduledExecution").into(),
            module: sui::move_ident_str!("scheduler").into(),
            type_params: vec![inner.clone()],
        }));

        let event = dummy_event(
            sui::move_ident_str!("RequestScheduledExecution").into(),
            serde_json::json!({
                "event":{
                    "request": {
                        "dag": dag.to_string(),
                        "execution": execution.to_string(),
                        "walk_index": "42",
                        "next_vertex": {
                            "variant": "Plain",
                            "fields": {
                                "vertex": { "name": "foo" },
                            }
                        },
                        "evaluations": evaluations.to_string(),
                        "worksheet_from_type": {
                            "name": "bar",
                        },
                    },
                    "priority": "7",
                    "request_ms": "1",
                    "start_ms": "2",
                    "deadline_ms": "3",
                }
            }),
            vec![outer.clone()],
        );

        let event: NexusEvent = event.try_into().unwrap();

        assert_eq!(event.generics, vec![outer]);
        assert_matches!(event.data, NexusEventKind::RequestScheduledExecution(scheduled)
            if scheduled.request.dag == dag &&
                scheduled.request.execution == execution &&
                scheduled.request.evaluations == evaluations &&
                scheduled.request.walk_index == 42 &&
                matches!(&scheduled.request.next_vertex, RuntimeVertex::Plain { vertex } if vertex.name == "foo") &&
                scheduled.request.worksheet_from_type.name == *"bar" &&
                scheduled.priority == 7 &&
                scheduled.request_ms == 1 &&
                scheduled.start_ms == 2 &&
                scheduled.deadline_ms == 3
        );
    }

    #[test]
    fn test_sui_event_desers_into_occurrence_scheduled_event() {
        let task = sui::ObjectID::random();

        let inner = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::ObjectID::random(),
            name: sui::move_ident_str!("OccurrenceScheduledEvent").into(),
            module: sui::move_ident_str!("scheduler").into(),
            type_params: vec![],
        }));

        let outer = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::ObjectID::random(),
            name: sui::move_ident_str!("RequestScheduledExecution").into(),
            module: sui::move_ident_str!("scheduler").into(),
            type_params: vec![inner.clone()],
        }));

        let event = dummy_event(
            sui::move_ident_str!("RequestScheduledExecution").into(),
            serde_json::json!({
                "event":{
                    "request": {
                        "task": task.to_string(),
                        "from_periodic": true
                    },
                    "priority": "4",
                    "request_ms": "11",
                    "start_ms": "22",
                    "deadline_ms": "33",
                }
            }),
            vec![outer.clone()],
        );

        let event: NexusEvent = event.try_into().unwrap();

        assert_eq!(event.generics, vec![outer]);
        assert_matches!(event.data, NexusEventKind::OccurrenceScheduled(scheduled)
            if scheduled.request.task == task &&
                scheduled.request.from_periodic &&
                scheduled.priority == 4 &&
                scheduled.request_ms == 11 &&
                scheduled.start_ms == 22 &&
                scheduled.deadline_ms == 33
        );
    }
}
