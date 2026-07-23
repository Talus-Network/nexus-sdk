use {
    crate::sui,
    anyhow::{bail, Result},
    serde::{Deserialize, Serialize},
};

mod parsing;
mod query;

pub use query::*;

fn deserialize_u64_to_datetime<'de, D>(
    deserializer: D,
) -> Result<chrono::DateTime<chrono::Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let timestamp = u64::deserialize(deserializer)?;
    chrono::DateTime::from_timestamp_millis(timestamp as i64)
        .ok_or_else(|| serde::de::Error::custom("datetime out of range"))
}

fn serialize_datetime_to_u64<S>(
    value: &chrono::DateTime<chrono::Utc>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(value.timestamp_millis() as u64)
}

fn deserialize_u64_to_duration<'de, D>(deserializer: D) -> Result<chrono::Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let millis = u64::deserialize(deserializer)?;
    Ok(chrono::Duration::milliseconds(millis as i64))
}

fn serialize_duration_to_u64<S>(value: &chrono::Duration, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_u64(value.num_milliseconds() as u64)
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

        pub(super) fn supports_event(name: &str) -> bool {
            matches!(name, $($name)|*)
        }
    };
}

// Enumeration with all available events coming from the on-chain part of
// Nexus. Also includes BCS parsing implementations.
events! {
    crate::move_bindings::scheduler::scheduler::OccurrenceAdvertised => OccurrenceAdvertised, "OccurrenceAdvertised",
    crate::move_bindings::scheduler::scheduler::OccurrenceDispatched => OccurrenceDispatched, "OccurrenceDispatched",
    crate::move_bindings::scheduler::scheduler::OccurrenceMissed => OccurrenceMissed, "OccurrenceMissed",
    crate::move_bindings::scheduler::scheduler::OccurrenceSettled => OccurrenceSettled, "OccurrenceSettled",
    crate::move_bindings::scheduler::scheduler::TaskCanceled => TaskCanceled, "TaskCanceled",
    crate::move_bindings::scheduler::scheduler::TaskClosed => TaskClosed, "TaskClosed",
    crate::move_bindings::scheduler::scheduler::TaskCreated => TaskCreated, "TaskCreated",
    crate::move_bindings::scheduler::scheduler::TaskPaused => TaskPaused, "TaskPaused",
    crate::move_bindings::scheduler::scheduler::TaskResumed => TaskResumed, "TaskResumed",
    crate::move_bindings::workflow::execution_events::RequestWalkExecutionEvent => RequestWalkExecution, "RequestWalkExecutionEvent",
    crate::move_bindings::interface::agent::AgentCreatedEvent => AgentCreated, "AgentCreatedEvent",
    crate::move_bindings::registry::agent_registry::SkillRegisteredEvent => SkillRegistered, "SkillRegisteredEvent",
    crate::move_bindings::registry::agent_registry::SkillContractRevisionedEvent => SkillContractRevisioned, "SkillContractRevisionedEvent",
    crate::move_bindings::registry::agent_registry::DefaultDagExecutorUpdatedEvent => DefaultDagExecutorUpdated, "DefaultDagExecutorUpdatedEvent",
    crate::move_bindings::workflow::execution_events::AgentSkillExecutionRequestedEvent => AgentSkillExecutionRequested, "AgentSkillExecutionRequestedEvent",
    crate::move_bindings::workflow::execution_events::AgentVertexAuthorizationRequiredEvent => AgentVertexAuthorizationRequired, "AgentVertexAuthorizationRequiredEvent",
    crate::move_bindings::interface::payment::AgentSkillPaymentCreatedEvent => AgentSkillPaymentCreated, "AgentSkillPaymentCreatedEvent",
    crate::move_bindings::interface::payment::ExecutionPaymentFeesRecordedEvent => ExecutionPaymentFeesRecorded, "ExecutionPaymentFeesRecordedEvent",
    crate::move_bindings::interface::payment::ExecutionPaymentToolCostSnapshottedEvent => ExecutionPaymentToolCostSnapshotted, "ExecutionPaymentToolCostSnapshottedEvent",
    crate::move_bindings::interface::payment::ExecutionPaymentVertexLockedEvent => ExecutionPaymentVertexLocked, "ExecutionPaymentVertexLockedEvent",
    crate::move_bindings::interface::payment::ExecutionPaymentVertexSettledEvent => ExecutionPaymentVertexSettled, "ExecutionPaymentVertexSettledEvent",
    crate::move_bindings::interface::payment::GasPaymentConsumedEvent => GasPaymentConsumed, "GasPaymentConsumedEvent",
    crate::move_bindings::interface::payment::ExecutionAccomplishedEvent => ExecutionAccomplished, "ExecutionAccomplishedEvent",
    crate::move_bindings::interface::payment::ExecutionRefundedEvent => ExecutionRefunded, "ExecutionRefundedEvent",
    crate::move_bindings::interface::payment::TaskExecutionPaymentCreated => TaskExecutionPaymentCreated, "TaskExecutionPaymentCreated",
    crate::move_bindings::interface::payment::TaskExecutionPaymentFinalized => TaskExecutionPaymentFinalized, "TaskExecutionPaymentFinalized",
    crate::move_bindings::interface::payment::TaskPaymentReserveCanceled => TaskPaymentReserveCanceled, "TaskPaymentReserveCanceled",
    crate::move_bindings::interface::payment::TaskPaymentReserveRefilled => TaskPaymentReserveRefilled, "TaskPaymentReserveRefilled",
    crate::move_bindings::registry::tool_registry::ToolRegisteredEvent => ToolRegistered, "ToolRegisteredEvent",
    crate::move_bindings::registry::tool_registry::ToolUnregisteredEvent => ToolUnregistered, "ToolUnregisteredEvent",
    crate::move_bindings::registry::priority_fee_vault::PriorityFeeSwapEvent => PriorityFeeSwap, "PriorityFeeSwapEvent",
    crate::move_bindings::workflow::execution_events::CommittedToolResultEvent => CommittedToolResult, "CommittedToolResultEvent",
    crate::move_bindings::workflow::execution_events::WalkAdvancedEvent => WalkAdvanced, "WalkAdvancedEvent",
    crate::move_bindings::workflow::execution_events::WalkFailedEvent => WalkFailed, "WalkFailedEvent",
    crate::move_bindings::workflow::execution_events::SubmissionFailureEvidenceRecordedEvent => SubmissionFailureEvidenceRecorded, "SubmissionFailureEvidenceRecordedEvent",
    crate::move_bindings::workflow::execution_events::TerminalErrEvalRecordedEvent => TerminalErrEvalRecorded, "TerminalErrEvalRecordedEvent",
    crate::move_bindings::workflow::execution_events::ToolVerificationResolved => ToolVerificationResolved, "ToolVerificationResolved",
    crate::move_bindings::workflow::execution_events::WalkPendingAbortEvent => WalkPendingAbort, "WalkPendingAbortEvent",
    crate::move_bindings::workflow::execution_events::WalkAbortedEvent => WalkAborted, "WalkAbortedEvent",
    crate::move_bindings::workflow::execution_events::WalkCancelledEvent => WalkCancelled, "WalkCancelledEvent",
    crate::move_bindings::workflow::execution_events::EndStateReachedEvent => EndStateReached, "EndStateReachedEvent",
    crate::move_bindings::workflow::execution_events::ExecutionFinishedEvent => ExecutionFinished, "ExecutionFinishedEvent",
    crate::move_bindings::workflow::execution_events::ExecutionPaymentRefilledEvent => ExecutionPaymentRefilled, "ExecutionPaymentRefilledEvent",
    crate::move_bindings::workflow::execution_events::ExecutionPaymentInsufficientSettlementEvent => ExecutionPaymentInsufficientSettlement, "ExecutionPaymentInsufficientSettlementEvent",
    crate::move_bindings::registry::priority_fee_vault::PriorityFeeDepositEvent => PriorityFeeDeposit, "PriorityFeeDepositEvent",
    crate::move_bindings::registry::leader_cap::FoundingLeaderCapCreatedEvent => FoundingLeaderCapCreated, "FoundingLeaderCapCreatedEvent",
    crate::move_bindings::registry::leader::LeaderCapIssuedEvent => LeaderCapIssued, "LeaderCapIssuedEvent",
    crate::move_bindings::registry::leader::LeaderClaimedEvent => LeaderClaimed, "LeaderClaimedEvent",
    crate::move_bindings::workflow::gas::PaymentInsufficientGasEvent => PaymentInsufficientGas, "PaymentInsufficientGasEvent",
    crate::move_bindings::workflow::gas::PaymentLockUpdateEvent => PaymentLockUpdate, "PaymentLockUpdateEvent",
    crate::move_bindings::workflow::gas::PaymentUnlockUpdateEvent => PaymentUnlockUpdate, "PaymentUnlockUpdateEvent",
    crate::move_bindings::interface::dag::DAGCreatedEvent => DAGCreated, "DAGCreatedEvent",
    crate::move_bindings::registry::tool_registry::ToolRegistryCreatedEvent => ToolRegistryCreated, "ToolRegistryCreatedEvent",
}
