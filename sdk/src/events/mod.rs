use {
    crate::{
        sui,
        types::{
            interface::version::InterfaceVersion,
            workflow::execution_events::RequestWalkExecutionEvent,
            *,
        },
    },
    anyhow::{bail, Result},
    serde::{Deserialize, Serialize},
};

mod parsing;
mod polling;

pub use {parsing::*, polling::*};

/// Distribution metadata for distributed events. This contains metadata about
/// the event deadline as well as the priority in which leaders should attempt
/// to execute the event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DistributedEventMetadata {
    /// The execution window duration.
    #[serde(
        rename = "deadline_ms",
        deserialize_with = "deserialize_u64_to_duration",
        serialize_with = "serialize_duration_to_u64"
    )]
    pub deadline: chrono::Duration,
    /// The timestamp by which the event was requested.
    #[serde(
        rename = "requested_at_ms",
        deserialize_with = "deserialize_u64_to_datetime",
        serialize_with = "serialize_datetime_to_u64"
    )]
    pub requested_at: chrono::DateTime<chrono::Utc>,
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
            $event_ty:ty => $variant:ident, $name:expr
        ),* $(,)?
    ) => {

        // == enum NexusEventKind ==

        #[derive(Clone, Debug, Serialize, Deserialize)]
        #[serde(tag = "_nexus_event_type", content = "event")]
        pub enum NexusEventKind {
            $(
                #[serde(rename = $name)]
                $variant($event_ty),
            )*
        }

        impl NexusEventKind {
            /// Returns the name of the event kind as a string.
            pub fn name(&self) -> String {
                match self {
                    $(
                        NexusEventKind::$variant(_) => $name.to_string(),
                    )*
                }
            }
        }

        // == Parsing from BCS ==

        pub(super) fn parse_bcs(name: &str, bytes: &[u8]) -> Result<(NexusEventKind, Option<DistributedEventMetadata>)> {
            #[derive(Deserialize)]
            struct Wrapper<T> {
                event: T,
            }

            #[derive(Deserialize)]
            struct DistributedWrapper<T> {
                event: T,
                deadline_ms: u64,
                requested_at_ms: u64,
                task_id: sui::types::Address,
                leaders: Vec<sui::types::Address>,
            }

            if name == "RequestWalkExecutionEvent" {
                type ScheduledWalkRequest =
                    crate::types::interface::scheduled_request::RequestScheduledExecution<
                        crate::types::workflow::execution_events::RequestWalkExecutionEvent,
                    >;

                if let Ok(distributed) = bcs::from_bytes::<DistributedWrapper<ScheduledWalkRequest>>(bytes) {
                    let metadata = DistributedEventMetadata {
                        deadline: chrono::Duration::milliseconds(distributed.deadline_ms as i64),
                        requested_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(distributed.requested_at_ms as i64)
                            .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
                        task_id: distributed.task_id,
                        leaders: distributed.leaders,
                    };

                    return Ok((
                        NexusEventKind::RequestWalkExecution(distributed.event.request),
                        Some(metadata),
                    ));
                }
            }

            match name {
                $(
                    $name => {
                        match bcs::from_bytes::<DistributedWrapper<$event_ty>>(bytes) {
                            Ok(distributed) => {
                                let metadata = DistributedEventMetadata {
                                    deadline: chrono::Duration::milliseconds(distributed.deadline_ms as i64),
                                    requested_at: chrono::DateTime::<chrono::Utc>::from_timestamp_millis(distributed.requested_at_ms as i64)
                                        .ok_or_else(|| anyhow::anyhow!("Invalid timestamp"))?,
                                    task_id: distributed.task_id,
                                    leaders: distributed.leaders,
                                };

                                Ok((NexusEventKind::$variant(distributed.event), Some(metadata)))
                            }
                            Err(_) => {
                                 let obj: Wrapper<$event_ty> = bcs::from_bytes(bytes)?;

                                 Ok((NexusEventKind::$variant(obj.event), None))
                            }
                        }
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
    crate::types::interface::scheduled_request::RequestScheduledExecution<crate::types::scheduler::scheduler::OccurrenceScheduledEvent> => RequestScheduledOccurrence, "RequestScheduledOccurrenceEvent",
    crate::types::scheduler::scheduler::OccurrenceScheduledEvent => OccurrenceScheduled, "OccurrenceScheduledEvent",
    crate::types::workflow::execution_events::RequestWalkExecutionEvent => RequestWalkExecution, "RequestWalkExecutionEvent",
    crate::types::interface::agent::AgentCreatedEvent => AgentCreated, "AgentCreatedEvent",
    crate::types::registry::agent_registry::SkillRegisteredEvent => SkillRegistered, "SkillRegisteredEvent",
    crate::types::registry::agent_registry::SkillContractRevisionedEvent => SkillContractRevisioned, "SkillContractRevisionedEvent",
    crate::types::registry::agent_registry::DefaultDagExecutorUpdatedEvent => DefaultDagExecutorUpdated, "DefaultDagExecutorUpdatedEvent",
    crate::types::workflow::execution_events::AgentSkillExecutionRequestedEvent => AgentSkillExecutionRequested, "AgentSkillExecutionRequestedEvent",
    crate::types::workflow::execution_events::AgentVertexAuthorizationRequiredEvent => AgentVertexAuthorizationRequired, "AgentVertexAuthorizationRequiredEvent",
    crate::types::interface::payment::AgentSkillPaymentCreatedEvent => AgentSkillPaymentCreated, "AgentSkillPaymentCreatedEvent",
    crate::types::interface::payment::ExecutionPaymentReceiptCreatedEvent => ExecutionPaymentReceiptCreated, "ExecutionPaymentReceiptCreatedEvent",
    crate::types::interface::payment::ExecutionPaymentReceiptResolvedEvent => ExecutionPaymentReceiptResolved, "ExecutionPaymentReceiptResolvedEvent",
    crate::types::interface::payment::ScheduledPaymentReserveReceiptCreatedEvent => ScheduledPaymentReserveReceiptCreated, "ScheduledPaymentReserveReceiptCreatedEvent",
    crate::types::interface::payment::GasPaymentConsumedEvent => GasPaymentConsumed, "GasPaymentConsumedEvent",
    crate::types::interface::payment::ExecutionAccomplishedEvent => ExecutionAccomplished, "ExecutionAccomplishedEvent",
    crate::types::interface::payment::ExecutionRefundedEvent => ExecutionRefunded, "ExecutionRefundedEvent",
    crate::types::scheduler::scheduler::ScheduledSkillExecutionCreatedEvent => ScheduledSkillExecutionCreated, "ScheduledSkillExecutionCreatedEvent",
    crate::types::scheduler::scheduler::ScheduledSkillExecutionPausedEvent => ScheduledSkillExecutionPaused, "ScheduledSkillExecutionPausedEvent",
    crate::types::scheduler::scheduler::ScheduledSkillExecutionResumedEvent => ScheduledSkillExecutionResumed, "ScheduledSkillExecutionResumedEvent",
    crate::types::scheduler::scheduler::ScheduledSkillExecutionCanceledEvent => ScheduledSkillExecutionCanceled, "ScheduledSkillExecutionCanceledEvent",
    crate::types::interface::payment::ScheduledSkillPaymentRefilledEvent => ScheduledSkillPaymentRefilled, "ScheduledSkillPaymentRefilledEvent",
    crate::types::interface::payment::ScheduledOccurrencePaymentCreatedEvent => ScheduledOccurrencePaymentCreated, "ScheduledOccurrencePaymentCreatedEvent",
    crate::types::interface::payment::ScheduledSkillPaymentCanceledEvent => ScheduledSkillPaymentCanceled, "ScheduledSkillPaymentCanceledEvent",
    crate::types::interface::payment::ScheduledOccurrencePaymentFinalizedEvent => ScheduledOccurrencePaymentFinalized, "ScheduledOccurrencePaymentFinalizedEvent",
    crate::types::registry::tool_registry::ToolRegisteredEvent => ToolRegistered, "ToolRegisteredEvent",
    crate::types::registry::tool_registry::ToolUnregisteredEvent => ToolUnregistered, "ToolUnregisteredEvent",
    crate::types::workflow::execution_events::CommittedToolResultEvent => CommittedToolResult, "CommittedToolResultEvent",
    crate::types::workflow::execution_events::WalkAdvancedEvent => WalkAdvanced, "WalkAdvancedEvent",
    crate::types::workflow::execution_events::WalkFailedEvent => WalkFailed, "WalkFailedEvent",
    crate::types::workflow::execution_events::TerminalErrEvalRecordedEvent => TerminalErrEvalRecorded, "TerminalErrEvalRecordedEvent",
    crate::types::workflow::execution_events::VerificationVerdictEvent => VerificationVerdictRecorded, "VerificationVerdictEvent",
    crate::types::workflow::execution_events::WalkAbortedEvent => WalkAborted, "WalkAbortedEvent",
    crate::types::workflow::execution_events::WalkCancelledEvent => WalkCancelled, "WalkCancelledEvent",
    crate::types::workflow::execution_events::EndStateReachedEvent => EndStateReached, "EndStateReachedEvent",
    crate::types::workflow::execution_events::ExecutionFinishedEvent => ExecutionFinished, "ExecutionFinishedEvent",
    crate::types::scheduler::scheduler::MissedOccurrenceEvent => MissedOccurrence, "MissedOccurrenceEvent",
    crate::types::scheduler::scheduler::OccurrenceConsumedEvent => OccurrenceConsumed, "OccurrenceConsumedEvent",
    crate::types::scheduler::scheduler::PeriodicScheduleConfiguredEvent => PeriodicScheduleConfigured, "PeriodicScheduleConfiguredEvent",
    crate::types::registry::leader_cap::FoundingLeaderCapCreatedEvent => FoundingLeaderCapCreated, "FoundingLeaderCapCreatedEvent",
    crate::types::registry::leader::LeaderCapIssuedEvent => LeaderCapIssued, "LeaderCapIssuedEvent",
    crate::types::registry::leader::LeaderClaimedEvent => LeaderClaimed, "LeaderClaimedEvent",
    crate::types::workflow::gas::PaymentInsufficientGasEvent => PaymentInsufficientGas, "PaymentInsufficientGasEvent",
    crate::types::workflow::gas::PaymentLockUpdateEvent => PaymentLockUpdate, "PaymentLockUpdateEvent",
    crate::types::workflow::gas::PaymentUnlockUpdateEvent => PaymentUnlockUpdate, "PaymentUnlockUpdateEvent",
    crate::types::interface::dag::DAGCreatedEvent => DAGCreated, "DAGCreatedEvent",
    crate::types::registry::tool_registry::ToolRegistryCreatedEvent => ToolRegistryCreated, "ToolRegistryCreatedEvent",
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestWalkContext {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceVersion,
    pub scheduled_task_id: Option<sui::types::Address>,
    pub scheduled_occurrence_index: Option<u64>,
}

impl RequestWalkContext {
    pub fn skill_revision_key(&self) -> SkillRevisionLookupKey {
        SkillRevisionLookupKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }
}

impl RequestWalkExecutionEvent {
    pub fn skill_revision_key(&self) -> Option<SkillRevisionLookupKey> {
        Some(SkillRevisionLookupKey {
            agent_id: self.agent_id.clone().into(),
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
        })
    }

    pub fn to_context(&self) -> Result<Option<RequestWalkContext>> {
        Ok(Some(RequestWalkContext {
            agent_id: self.agent_id.clone().into(),
            skill_id: self.skill_id,
            interface_revision: self.interface_version,
            scheduled_task_id: self.scheduled_task_id.clone().0.map(Into::into),
            scheduled_occurrence_index: self.scheduled_occurrence_index.0,
        }))
    }
}
