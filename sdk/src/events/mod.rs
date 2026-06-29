use {
    crate::{sui, types::*},
    anyhow::{bail, Result},
    serde::{Deserialize, Serialize},
};

mod parsing;
mod polling;

pub use {parsing::*, polling::*};

/// Generated Move event types grouped by their source package/module.
pub mod generated {
    pub use crate::types::generated::{
        interface_types::{agent, dag, payment, scheduled_request},
        registry_types::{agent_registry, leader, leader_cap, tool_registry},
        scheduler_types::scheduler,
        workflow_types::{execution_events, gas},
    };
}

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
                    crate::types::generated::interface_types::scheduled_request::RequestScheduledExecution<
                        RequestWalkExecutionEvent,
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
                    stringify!($struct_name) => {
                        match bcs::from_bytes::<DistributedWrapper<$struct_name>>(bytes) {
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
                                 let obj: Wrapper<$struct_name> = bcs::from_bytes(bytes)?;

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

pub type OccurrenceScheduledEvent =
    crate::types::generated::scheduler_types::scheduler::OccurrenceScheduledEvent;
pub type RequestScheduledOccurrenceEvent =
    crate::types::generated::interface_types::scheduled_request::RequestScheduledExecution<
        OccurrenceScheduledEvent,
    >;

// Enumeration with all available events coming from the on-chain part of
// Nexus. Also includes BCS parsing implementations.
events! {
    RequestScheduledOccurrenceEvent => RequestScheduledOccurrence, "RequestScheduledOccurrenceEvent",
    OccurrenceScheduledEvent => OccurrenceScheduled, "OccurrenceScheduledEvent",
    RequestWalkExecutionEvent => RequestWalkExecution, "RequestWalkExecutionEvent",
    AgentCreatedEvent => AgentCreated, "AgentCreatedEvent",
    SkillRegisteredEvent => SkillRegistered, "SkillRegisteredEvent",
    SkillContractRevisionedEvent => SkillContractRevisioned, "SkillContractRevisionedEvent",
    DefaultDagExecutorUpdatedEvent => DefaultDagExecutorUpdated, "DefaultDagExecutorUpdatedEvent",
    AgentSkillExecutionRequestedEvent => AgentSkillExecutionRequested, "AgentSkillExecutionRequestedEvent",
    AgentVertexAuthorizationRequiredEvent => AgentVertexAuthorizationRequired, "AgentVertexAuthorizationRequiredEvent",
    AgentSkillPaymentCreatedEvent => AgentSkillPaymentCreated, "AgentSkillPaymentCreatedEvent",
    ExecutionPaymentReceiptCreatedEvent => ExecutionPaymentReceiptCreated, "ExecutionPaymentReceiptCreatedEvent",
    ExecutionPaymentReceiptResolvedEvent => ExecutionPaymentReceiptResolved, "ExecutionPaymentReceiptResolvedEvent",
    ScheduledPaymentReserveReceiptCreatedEvent => ScheduledPaymentReserveReceiptCreated, "ScheduledPaymentReserveReceiptCreatedEvent",
    GasPaymentConsumedEvent => GasPaymentConsumed, "GasPaymentConsumedEvent",
    ExecutionAccomplishedEvent => ExecutionAccomplished, "ExecutionAccomplishedEvent",
    ExecutionRefundedEvent => ExecutionRefunded, "ExecutionRefundedEvent",
    ScheduledSkillExecutionCreatedEvent => ScheduledSkillExecutionCreated, "ScheduledSkillExecutionCreatedEvent",
    ScheduledSkillExecutionPausedEvent => ScheduledSkillExecutionPaused, "ScheduledSkillExecutionPausedEvent",
    ScheduledSkillExecutionResumedEvent => ScheduledSkillExecutionResumed, "ScheduledSkillExecutionResumedEvent",
    ScheduledSkillExecutionCanceledEvent => ScheduledSkillExecutionCanceled, "ScheduledSkillExecutionCanceledEvent",
    ScheduledSkillPaymentRefilledEvent => ScheduledSkillPaymentRefilled, "ScheduledSkillPaymentRefilledEvent",
    ScheduledOccurrencePaymentCreatedEvent => ScheduledOccurrencePaymentCreated, "ScheduledOccurrencePaymentCreatedEvent",
    ScheduledSkillPaymentCanceledEvent => ScheduledSkillPaymentCanceled, "ScheduledSkillPaymentCanceledEvent",
    ScheduledOccurrencePaymentFinalizedEvent => ScheduledOccurrencePaymentFinalized, "ScheduledOccurrencePaymentFinalizedEvent",
    ToolRegisteredEvent => ToolRegistered, "ToolRegisteredEvent",
    ToolUnregisteredEvent => ToolUnregistered, "ToolUnregisteredEvent",
    CommittedToolResultEvent => CommittedToolResult, "CommittedToolResultEvent",
    WalkAdvancedEvent => WalkAdvanced, "WalkAdvancedEvent",
    WalkFailedEvent => WalkFailed, "WalkFailedEvent",
    TerminalErrEvalRecordedEvent => TerminalErrEvalRecorded, "TerminalErrEvalRecordedEvent",
    VerificationVerdictEvent => VerificationVerdictRecorded, "VerificationVerdictEvent",
    WalkAbortedEvent => WalkAborted, "WalkAbortedEvent",
    WalkCancelledEvent => WalkCancelled, "WalkCancelledEvent",
    EndStateReachedEvent => EndStateReached, "EndStateReachedEvent",
    ExecutionFinishedEvent => ExecutionFinished, "ExecutionFinishedEvent",
    MissedOccurrenceEvent => MissedOccurrence, "MissedOccurrenceEvent",
    OccurrenceConsumedEvent => OccurrenceConsumed, "OccurrenceConsumedEvent",
    PeriodicScheduleConfiguredEvent => PeriodicScheduleConfigured, "PeriodicScheduleConfiguredEvent",
    FoundingLeaderCapCreatedEvent => FoundingLeaderCapCreated, "FoundingLeaderCapCreatedEvent",
    LeaderCapIssuedEvent => LeaderCapIssued, "LeaderCapIssuedEvent",
    LeaderClaimedEvent => LeaderClaimed, "LeaderClaimedEvent",
    PaymentInsufficientGasEvent => PaymentInsufficientGas, "PaymentInsufficientGasEvent",
    PaymentLockUpdateEvent => PaymentLockUpdate, "PaymentLockUpdateEvent",
    PaymentUnlockUpdateEvent => PaymentUnlockUpdate, "PaymentUnlockUpdateEvent",
    DAGCreatedEvent => DAGCreated, "DAGCreatedEvent",
    ToolRegistryCreatedEvent => ToolRegistryCreated, "ToolRegistryCreatedEvent",
}

// == Generated event definitions ==

pub type RequestWalkExecutionEvent =
    crate::types::generated::workflow_types::execution_events::RequestWalkExecutionEvent;
pub type AgentCreatedEvent = crate::types::generated::interface_types::agent::AgentCreatedEvent;
pub type SkillRegisteredEvent =
    crate::types::generated::registry_types::agent_registry::SkillRegisteredEvent;
pub type SkillContractRevisionedEvent =
    crate::types::generated::registry_types::agent_registry::SkillContractRevisionedEvent;
pub type DefaultDagExecutorUpdatedEvent =
    crate::types::generated::registry_types::agent_registry::DefaultDagExecutorUpdatedEvent;
pub type AgentSkillExecutionRequestedEvent =
    crate::types::generated::workflow_types::execution_events::AgentSkillExecutionRequestedEvent;
pub type AgentVertexAuthorizationRequiredEvent =
    crate::types::generated::workflow_types::execution_events::AgentVertexAuthorizationRequiredEvent;
pub type AgentSkillPaymentCreatedEvent =
    crate::types::generated::interface_types::payment::AgentSkillPaymentCreatedEvent;
pub type ExecutionPaymentReceiptCreatedEvent =
    crate::types::generated::interface_types::payment::ExecutionPaymentReceiptCreatedEvent;
pub type ExecutionPaymentReceiptResolvedEvent =
    crate::types::generated::interface_types::payment::ExecutionPaymentReceiptResolvedEvent;
pub type ScheduledPaymentReserveReceiptCreatedEvent =
    crate::types::generated::interface_types::payment::ScheduledPaymentReserveReceiptCreatedEvent;
pub type GasPaymentConsumedEvent =
    crate::types::generated::interface_types::payment::GasPaymentConsumedEvent;
pub type ExecutionAccomplishedEvent =
    crate::types::generated::interface_types::payment::ExecutionAccomplishedEvent;
pub type ExecutionRefundedEvent =
    crate::types::generated::interface_types::payment::ExecutionRefundedEvent;
pub type ScheduledSkillExecutionCreatedEvent =
    crate::types::generated::scheduler_types::scheduler::ScheduledSkillExecutionCreatedEvent;
pub type ScheduledSkillExecutionPausedEvent =
    crate::types::generated::scheduler_types::scheduler::ScheduledSkillExecutionPausedEvent;
pub type ScheduledSkillExecutionResumedEvent =
    crate::types::generated::scheduler_types::scheduler::ScheduledSkillExecutionResumedEvent;
pub type ScheduledSkillExecutionCanceledEvent =
    crate::types::generated::scheduler_types::scheduler::ScheduledSkillExecutionCanceledEvent;
pub type ScheduledSkillPaymentRefilledEvent =
    crate::types::generated::interface_types::payment::ScheduledSkillPaymentRefilledEvent;
pub type ScheduledOccurrencePaymentCreatedEvent =
    crate::types::generated::interface_types::payment::ScheduledOccurrencePaymentCreatedEvent;
pub type ScheduledSkillPaymentCanceledEvent =
    crate::types::generated::interface_types::payment::ScheduledSkillPaymentCanceledEvent;
pub type ScheduledOccurrencePaymentFinalizedEvent =
    crate::types::generated::interface_types::payment::ScheduledOccurrencePaymentFinalizedEvent;
pub type ToolRegisteredEvent =
    crate::types::generated::registry_types::tool_registry::ToolRegisteredEvent;
pub type ToolUnregisteredEvent =
    crate::types::generated::registry_types::tool_registry::ToolUnregisteredEvent;
pub type CommittedToolResultEvent =
    crate::types::generated::workflow_types::execution_events::CommittedToolResultEvent;
pub type WalkAdvancedEvent =
    crate::types::generated::workflow_types::execution_events::WalkAdvancedEvent;
pub type WalkFailedEvent =
    crate::types::generated::workflow_types::execution_events::WalkFailedEvent;
pub type TerminalErrEvalRecordedEvent =
    crate::types::generated::workflow_types::execution_events::TerminalErrEvalRecordedEvent;
pub type VerificationVerdictEvent =
    crate::types::generated::workflow_types::execution_events::VerificationVerdictEvent;
pub type SubmissionFailureEvidenceRecordedEvent =
    crate::types::generated::workflow_types::execution_events::SubmissionFailureEvidenceRecordedEvent;
pub type WalkAbortedEvent =
    crate::types::generated::workflow_types::execution_events::WalkAbortedEvent;
pub type WalkCancelledEvent =
    crate::types::generated::workflow_types::execution_events::WalkCancelledEvent;
pub type EndStateReachedEvent =
    crate::types::generated::workflow_types::execution_events::EndStateReachedEvent;
pub type ExecutionFinishedEvent =
    crate::types::generated::workflow_types::execution_events::ExecutionFinishedEvent;
pub type MissedOccurrenceEvent =
    crate::types::generated::scheduler_types::scheduler::MissedOccurrenceEvent;
pub type OccurrenceConsumedEvent =
    crate::types::generated::scheduler_types::scheduler::OccurrenceConsumedEvent;
pub type PeriodicScheduleConfiguredEvent =
    crate::types::generated::scheduler_types::scheduler::PeriodicScheduleConfiguredEvent;
pub type FoundingLeaderCapCreatedEvent =
    crate::types::generated::registry_types::leader_cap::FoundingLeaderCapCreatedEvent;
pub type LeaderCapIssuedEvent =
    crate::types::generated::registry_types::leader::LeaderCapIssuedEvent;
pub type LeaderClaimedEvent = crate::types::generated::registry_types::leader::LeaderClaimedEvent;
pub type PaymentInsufficientGasEvent =
    crate::types::generated::workflow_types::gas::PaymentInsufficientGasEvent;
pub type PaymentLockUpdateEvent =
    crate::types::generated::workflow_types::gas::PaymentLockUpdateEvent;
pub type PaymentUnlockUpdateEvent =
    crate::types::generated::workflow_types::gas::PaymentUnlockUpdateEvent;
pub type DAGCreatedEvent = crate::types::generated::interface_types::dag::DAGCreatedEvent;
pub type ToolRegistryCreatedEvent =
    crate::types::generated::registry_types::tool_registry::ToolRegistryCreatedEvent;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestWalkContext {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub interface_revision: InterfaceVersion,
    pub scheduled_task_id: Option<sui::types::Address>,
    pub scheduled_occurrence_index: Option<u64>,
}

impl RequestWalkContext {
    pub fn skill_revision_key(&self) -> SkillRevisionKey {
        SkillRevisionKey {
            agent_id: self.agent_id,
            skill_id: self.skill_id,
            interface_revision: self.interface_revision,
        }
    }
}

impl RequestWalkExecutionEvent {
    pub fn skill_revision_key(&self) -> Option<SkillRevisionKey> {
        Some(SkillRevisionKey {
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
