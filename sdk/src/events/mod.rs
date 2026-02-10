use {
    crate::{sui, types::*, ToolFqn},
    anyhow::{bail, Result},
    serde::{Deserialize, Serialize},
};

mod fetching;
mod graphql;
mod parsing;

pub use {fetching::*, graphql::*, parsing::*};

/// Distribution metadata for distributed events. This contains metadata about
/// the event deadline as well as the priority in which leaders should attempt
/// to execute the event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistributedEventMetadata {
    /// The timestamp by which the event should be executed.
    #[serde(
        rename = "deadline_ms",
        deserialize_with = "deserialize_sui_u64_to_datetime",
        serialize_with = "serialize_datetime_to_sui_u64"
    )]
    pub deadline: chrono::DateTime<chrono::Utc>,
    /// The priority list of leader addresses.
    pub leaders: Vec<sui::types::Address>,
    /// The task ID.
    pub task_id: sui::types::Address,
}

/// Struct holding the Sui event ID, the event generic arguments and the data
/// as one of [NexusEventKind].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NexusEvent {
    /// The event transaction digest and event sequence.
    pub id: (sui::types::Digest, u64),
    /// If the `T in NexusEvent<T>` is also a generic, this field holds the
    /// generic type. Note that this can be nested indefinitely.
    pub generics: Vec<sui::types::TypeTag>,
    /// The event data.
    pub data: NexusEventKind,
    /// If the event is a distributed event, this field holds the distribution
    /// metadata.
    pub distribution: Option<DistributedEventMetadata>,
}

macro_rules! events {
    (
        $(
            $struct_name:ident => $variant:ident, $name:expr
        ),* $(,)?
    ) => {

        // == enum NexusEventKind ==

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(tag = "_nexus_event_type", content = "event")]
        pub enum NexusEventKind {
            $(
                #[serde(rename = $name)]
                $variant($struct_name),
            )*
        }

        impl NexusEventKind {
            /// Returns the name of the event kind as a string.
            pub fn name(&self) -> String {
                match self {
                    $(
                        NexusEventKind::$variant(_) => stringify!($struct_name).to_string(),
                    )*
                }
            }
        }

        // == Parsing from BCS ==

        pub(super) fn parse_bcs(name: &str, bytes: &[u8]) -> Result<NexusEventKind> {
            #[derive(Deserialize)]
            struct Wrapper<T> {
                event: T,
            }

            match name {
                $(
                    stringify!($struct_name) => {
                        let obj: Wrapper<$struct_name> = bcs::from_bytes(bytes)?;
                        Ok(NexusEventKind::$variant(obj.event))
                    }
                )*
                _ => bail!("Unknown event: {}", name),
            }
        }
    };
}

// Enumeration with all available events coming from the on-chain part of
// Nexus. Also includes BCS parsing implementations.
events! {
    RequestScheduledOccurrenceEvent => RequestScheduledOccurrence, "RequestScheduledOccurrenceEvent",
    RequestScheduledWalkEvent => RequestScheduledWalk, "RequestScheduledWalkEvent",
    OccurrenceScheduledEvent => OccurrenceScheduled, "OccurrenceScheduledEvent",
    RequestWalkExecutionEvent => RequestWalkExecution, "RequestWalkExecutionEvent",
    AnnounceInterfacePackageEvent => AnnounceInterfacePackage, "AnnounceInterfacePackageEvent",
    ToolRegisteredEvent => ToolRegistered, "ToolRegisteredEvent",
    ToolUnregisteredEvent => ToolUnregistered, "ToolUnregisteredEvent",
    WalkAdvancedEvent => WalkAdvanced, "WalkAdvancedEvent",
    WalkFailedEvent => WalkFailed, "WalkFailedEvent",
    EndStateReachedEvent => EndStateReached, "EndStateReachedEvent",
    ExecutionFinishedEvent => ExecutionFinished, "ExecutionFinishedEvent",
    MissedOccurrenceEvent => MissedOccurrence, "MissedOccurrenceEvent",
    TaskCreatedEvent => TaskCreated, "TaskCreatedEvent",
    TaskPausedEvent => TaskPaused, "TaskPausedEvent",
    TaskResumedEvent => TaskResumed, "TaskResumedEvent",
    TaskCanceledEvent => TaskCanceled, "TaskCanceledEvent",
    OccurrenceConsumedEvent => OccurrenceConsumed, "OccurrenceConsumedEvent",
    PeriodicScheduleConfiguredEvent => PeriodicScheduleConfigured, "PeriodicScheduleConfiguredEvent",
    FoundingLeaderCapCreatedEvent => FoundingLeaderCapCreated, "FoundingLeaderCapCreatedEvent",
    GasSettlementUpdateEvent => GasSettlementUpdate, "GasSettlementUpdateEvent",
    PreKeyVaultCreatedEvent => PreKeyVaultCreated, "PreKeyVaultCreatedEvent",
    PreKeyRequestedEvent => PreKeyRequested, "PreKeyRequestedEvent",
    PreKeyFulfilledEvent => PreKeyFulfilled, "PreKeyFulfilledEvent",
    PreKeyAssociatedEvent => PreKeyAssociated, "PreKeyAssociatedEvent",
    DAGCreatedEvent => DAGCreated, "DAGCreatedEvent",
    ToolRegistryCreatedEvent => ToolRegistryCreated, "ToolRegistryCreatedEvent",

    // These events are unused for now.
    // "DAGVertexAdded" => DAGVertexAdded(serde_json::Value),
    // "DAGEdgeAdded" => DAGEdgeAdded(serde_json::Value),
    // "DAGOutputAdded" => DAGOutputAdded(serde_json::Value),
    // "DAGEntryVertexInputPortAdded" => DAGEntryVertexInputPortAdded(serde_json::Value),
    // "DAGDefaultValueAdded" => DAGDefaultValueAdded(serde_json::Value),
    // "LeaderClaimedGas" => LeaderClaimedGas(serde_json::Value),
    // "AllowedOwnerAdded" => AllowedOwnerAdded(serde_json::Value),
    // "AllowedOwnerRemoved" => AllowedOwnerRemoved(serde_json::Value),
}

// == Event definitions ==

/// Fired by the on-chain part of Nexus when a DAG vertex execution is
/// requested.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestWalkExecutionEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub invoker: sui::types::Address,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub walk_index: u64,
    pub next_vertex: RuntimeVertex,
    pub evaluations: sui::types::Address,
    /// This field defines the package ID, module and name of the Agent that
    /// holds the DAG. Used to confirm the tool evaluation with the Agent.
    pub worksheet_from_type: TypeName,
}

/// Fired via the Nexus `interface` package when a new Agent is registered.
/// Provides the agent's interface so that we can invoke
/// `confirm_tool_eval_for_walk` on it.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AnnounceInterfacePackageEvent {
    pub shared_objects: Vec<SharedObjectRef>,
}

/// Fired by the Nexus Workflow when a new tool is registered so that the Leader
/// can also register it in Redis. This way the Leader knows how and where to
/// evaluate the tool.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolRegisteredEvent {
    pub tool: sui::types::Address,
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
}

/// Fired by the Nexus Workflow when a tool is unregistered. The Leader should
/// remove the tool definition from its Redis registry.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolUnregisteredEvent {
    pub tool: sui::types::Address,
    /// The tool domain, name and version. See [ToolFqn] for more information.
    pub fqn: ToolFqn,
}

/// Fired by the Nexus Workflow when a walk has advanced. This event is used to
/// inspect DAG execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkAdvancedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
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
    pub variant_ports_to_data: PortsData,
}

/// Fired by the Nexus Workflow when a walk has failed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WalkFailedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
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
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
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
    pub variant_ports_to_data: PortsData,
}

/// Fired by the Nexus Workflow when all walks have halted in their end states
/// and there is no more work to be done. This event is used to inspect DAG
/// execution process.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionFinishedEvent {
    pub dag: sui::types::Address,
    pub execution: sui::types::Address,
    pub has_any_walk_failed: bool,
    pub has_any_walk_succeeded: bool,
}

/// Request wrapper emitted when scheduling an occurrence.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestScheduledOccurrenceEvent {
    pub request: OccurrenceScheduledEvent,
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

/// Request wrapper emitted when scheduling a walk execution.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RequestScheduledWalkEvent {
    pub request: RequestWalkExecutionEvent,
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

/// Fired when a scheduler occurrence is enqueued; used as the payload of
/// `RequestScheduledOccurrenceEvent`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OccurrenceScheduledEvent {
    pub task: sui::types::Address,
    pub generator: PolicySymbol,
}

/// Emitted when a scheduled occurrence misses its deadline and is pruned.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MissedOccurrenceEvent {
    pub task: sui::types::Address,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub start_time_ms: u64,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub deadline_ms: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub pruned_at: u64,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub priority_fee_per_gas_unit: u64,
    pub generator: PolicySymbol,
}

/// Emitted after a scheduler task object is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCreatedEvent {
    pub task: sui::types::Address,
    pub owner: sui::types::Address,
}

/// Emitted when scheduling for a task is paused.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskPausedEvent {
    pub task: sui::types::Address,
}

/// Emitted when scheduling for a task is resumed.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskResumedEvent {
    pub task: sui::types::Address,
}

/// Emitted when scheduling for a task is canceled.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskCanceledEvent {
    pub task: sui::types::Address,
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
    pub task: sui::types::Address,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub start_time_ms: u64,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub deadline_ms: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub priority_fee_per_gas_unit: u64,
    pub generator: PolicySymbol,
    #[serde(
        deserialize_with = "deserialize_sui_u64",
        serialize_with = "serialize_sui_u64"
    )]
    pub executed_at: u64,
}

/// Emitted whenever the periodic schedule is configured or cleared.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeriodicScheduleConfiguredEvent {
    pub task: sui::types::Address,
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
    pub priority_fee_per_gas_unit: Option<u64>,
    #[serde(
        deserialize_with = "deserialize_sui_option_u64",
        serialize_with = "serialize_sui_option_u64"
    )]
    pub last_generated_start_ms: Option<u64>,
}

/// Fired by the Nexus Workflow when a new founding LeaderCap is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FoundingLeaderCapCreatedEvent {
    pub leader_cap: sui::types::Address,
    pub network: sui::types::Address,
}

/// Fired by the Gas service when the gas settlement is updated. This event is
/// used to determine whether a tool invocation was paid for by the caller.
/// Combination of `execution` and `vertex` uniquely identifies the tool
/// invocation.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GasSettlementUpdateEvent {
    pub execution: sui::types::Address,
    pub tool_fqn: ToolFqn,
    pub vertex: RuntimeVertex,
    pub was_settled: bool,
}

/// Fired when the leader claims gas from a user's budget.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeaderClaimedGasEvent {
    pub network: sui::types::Address,
    pub amount: u64,
    /// Optional reason for auditing purposes.
    #[serde(default)]
    pub purpose: String,
}

/// Fired by the Nexus Workflow when a new pre key vault is created. This happens
/// on initial network setup.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyVaultCreatedEvent {
    pub vault: sui::types::Address,
    pub crypto_cap: sui::types::Address,
}

/// Fired by the Nexus Workflow when a pre key is requested. The pre key bytes
/// are still empty at this point and will be fulfilled by the leader.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyRequestedEvent {
    /// The address of the user that requested the pre key.
    pub requested_by: sui::types::Address,
}

/// Fired by the Nexus Workflow when a pre key request is fulfilled by the
/// leader. Carries the pending pre key bytes that the user can then associate.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyFulfilledEvent {
    /// The address of the user that requested the pre key.
    pub requested_by: sui::types::Address,
    /// Bytes of the fulfilled pre key.
    #[serde(
        deserialize_with = "deserialize_encoded_bytes",
        serialize_with = "serialize_encoded_bytes"
    )]
    pub pre_key_bytes: Vec<u8>,
}

/// Fired by the Nexus Workflow when a pre key is associated with a user.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreKeyAssociatedEvent {
    /// The address of the user the pre key is associated with.
    pub claimed_by: sui::types::Address,
    /// Bytes of the pre key.
    #[serde(
        deserialize_with = "deserialize_encoded_bytes",
        serialize_with = "serialize_encoded_bytes"
    )]
    pub pre_key: Vec<u8>,
    /// Bytes of the initial message.
    #[serde(
        deserialize_with = "deserialize_encoded_bytes",
        serialize_with = "serialize_encoded_bytes"
    )]
    pub initial_message: Vec<u8>,
}

/// Fired by the Nexus Workflow when a new DAG is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DAGCreatedEvent {
    /// Address of the created DAG.
    pub dag: sui::types::Address,
}

/// Fired by the Nexus Workflow when a new ToolRegistry is created.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ToolRegistryCreatedEvent {
    /// Address of the created ToolRegistry.
    pub registry: sui::types::Address,
    /// Address of the relevant slashing cap.
    pub slashing_cap: sui::types::Address,
}

#[cfg(test)]
mod tests {
    use super::*;

    events!(
        DummyEvent => Dummy, "DummyEvent",
        AnotherEvent => Another, "AnotherEvent",
    );

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct DummyEvent {
        pub value: u32,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct AnotherEvent {
        pub text: String,
    }

    #[test]
    fn test_nexus_event_kind_name_helper() {
        let dummy = DummyEvent { value: 42 };
        let another = AnotherEvent {
            text: "hello".to_string(),
        };

        let kind_dummy = NexusEventKind::Dummy(dummy.clone());
        let kind_another = NexusEventKind::Another(another.clone());

        assert_eq!(kind_dummy.name(), "DummyEvent");
        assert_eq!(kind_another.name(), "AnotherEvent");
    }

    #[test]
    fn test_nexus_event_kind_enum_generation() {
        let dummy = DummyEvent { value: 1 };
        let another = AnotherEvent {
            text: "abc".to_string(),
        };

        let kind_dummy = NexusEventKind::Dummy(dummy.clone());
        let kind_another = NexusEventKind::Another(another.clone());

        match kind_dummy {
            NexusEventKind::Dummy(ev) => assert_eq!(ev, dummy),
            _ => panic!("Expected Dummy variant"),
        }

        match kind_another {
            NexusEventKind::Another(ev) => assert_eq!(ev, another),
            _ => panic!("Expected Another variant"),
        }
    }

    #[test]
    fn test_nexus_event_kind_bcs_deser() {
        let dummy = DummyEvent { value: 99 };
        let another = AnotherEvent {
            text: "xyz".to_string(),
        };

        let dummy_bytes = bcs::to_bytes(&dummy).unwrap();
        let another_bytes = bcs::to_bytes(&another).unwrap();

        let parsed_dummy = parse_bcs("DummyEvent", &dummy_bytes).unwrap();
        let parsed_another = parse_bcs("AnotherEvent", &another_bytes).unwrap();

        match parsed_dummy {
            NexusEventKind::Dummy(ev) => assert_eq!(ev, dummy),
            _ => panic!("Expected Dummy variant"),
        }

        match parsed_another {
            NexusEventKind::Another(ev) => assert_eq!(ev, another),
            _ => panic!("Expected Another variant"),
        }
    }

    #[test]
    fn test_parse_bcs_unknown_event() {
        let dummy = DummyEvent { value: 123 };
        let bytes = bcs::to_bytes(&dummy).unwrap();
        let result = parse_bcs("UnknownEvent", &bytes);
        assert!(result.is_err());
    }
}
