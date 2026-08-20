//! Errors produced while authoring or operating scheduled Tasks.

#[cfg(feature = "nexus")]
use crate::nexus::error::NexusError;
use {
    crate::{scheduler::OccurrenceSnapshot, sui},
    std::error::Error,
    thiserror::Error,
};

/// Type erased source retained by a scheduler boundary error.
pub type ErrorSource = Box<dyn Error + Send + Sync + 'static>;

/// An invalid Task or Schedule authoring value.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScheduleError {
    /// A Task cannot reserve zero MIST for an occurrence.
    #[error("occurrence budget must be greater than zero")]
    ZeroOccurrenceBudget,

    /// A Task entry group cannot be empty.
    #[error("entry group must not be empty")]
    EmptyEntryGroup,

    /// An absolute deadline precedes the known absolute start.
    #[error("occurrence deadline {deadline_ms}ms precedes its start {start_time_ms}ms")]
    DeadlineBeforeStart {
        /// Known absolute start timestamp.
        start_time_ms: u64,
        /// Invalid absolute deadline timestamp.
        deadline_ms: u64,
    },

    /// The priority fee is outside the protocol range.
    #[error("priority fee percentage must be in {minimum}..={maximum}, got {percentage}")]
    PriorityFeeOutOfRange {
        /// Invalid percentage.
        percentage: u64,
        /// Inclusive protocol minimum.
        minimum: u64,
        /// Inclusive protocol maximum.
        maximum: u64,
    },

    /// A recurrence interval must advance time.
    #[error("recurrence interval must be greater than zero")]
    ZeroRecurrenceInterval,

    /// A finite recurrence must contain at least one occurrence.
    #[error("finite recurrence occurrence count must be greater than zero")]
    ZeroRecurrenceCount,

    /// The atomic create and schedule shortcut requires work.
    #[error("schedule must contain at least one occurrence or a recurrence")]
    EmptySchedule,

    /// Resolving a relative timestamp exceeded the `u64` timestamp range.
    #[error("resolving {field} from {base_ms}ms with offset {offset_ms}ms overflowed")]
    TimeOverflow {
        /// Name of the timestamp being resolved.
        field: &'static str,
        /// Timestamp to which the offset was applied.
        base_ms: u64,
        /// Relative offset that overflowed.
        offset_ms: u64,
    },

    /// A Task operation and funding controller cannot be combined.
    #[error("{message}")]
    IncompatibleFunding {
        /// Explanation of the invalid combination.
        message: &'static str,
    },
}

/// An error while resolving, submitting, or inspecting scheduler state.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SchedulerError {
    /// A Task or Schedule authoring value is invalid.
    #[error(transparent)]
    Schedule(#[from] ScheduleError),

    /// The configured Nexus client could not complete the operation.
    #[cfg(feature = "nexus")]
    #[error(transparent)]
    Client(Box<NexusError>),

    /// The requested Task object does not exist.
    #[error("Task '{task_id}' was not found")]
    TaskNotFound {
        /// Missing Task identifier.
        task_id: sui::types::Address,
    },

    /// The requested occurrence record does not exist.
    #[error("occurrence {occurrence_id} was not found on Task '{task_id}'")]
    OccurrenceNotFound {
        /// Owning Task identifier.
        task_id: sui::types::Address,
        /// Missing occurrence identifier.
        occurrence_id: u64,
    },

    /// The requested operation requires a dispatched occurrence.
    #[error("occurrence {occurrence_id} on Task '{task_id}' has not been dispatched")]
    OccurrenceNotDispatched {
        /// Owning Task identifier.
        task_id: sui::types::Address,
        /// Occurrence identifier.
        occurrence_id: u64,
    },

    /// A selected DAG does not contain the requested Task entry group.
    #[error(
        "DAG '{dag_id}' does not contain Task entry group '{entry_group}'; available groups: \
         {available:?}"
    )]
    TaskEntryGroupNotFound {
        /// Selected DAG identifier.
        dag_id: sui::types::Address,
        /// Requested entry group.
        entry_group: String,
        /// Entry groups published by the DAG.
        available: Vec<String>,
    },

    /// Task inputs do not exactly match the selected DAG entry group.
    #[error(
        "Task inputs do not match entry group '{entry_group}' on DAG '{dag_id}'; expected shape \
         {expected}, received shape {received}"
    )]
    TaskInputsMismatch {
        /// Selected DAG identifier.
        dag_id: sui::types::Address,
        /// Selected entry group.
        entry_group: String,
        /// Required input JSON shape with placeholder values.
        expected: String,
        /// Supplied input JSON shape without user values.
        received: String,
    },

    /// One provided Task input does not conform to the immutable Tool schema stored on its DAG vertex.
    #[error(
        "Task input '{vertex}.{port}' on DAG '{dag_id}' does not conform to MetaSchema; expected \
         {expected}, received {received}"
    )]
    TaskInputSchemaMismatch {
        /// Effective DAG identifier.
        dag_id: sui::types::Address,
        /// Entry vertex name.
        vertex: String,
        /// Entry input port name.
        port: String,
        /// Required cardinality and value kind.
        expected: Box<str>,
        /// Received cardinality and value kind.
        received: Box<str>,
    },

    /// A pinned Agent skill forbids a caller-selected DAG, even when it repeats the pinned ID.
    #[error(
        "Agent '{agent_id}' skill {skill_id} is pinned to DAG '{pinned_dag}' and rejects caller-selected DAG '{selected_dag}'"
    )]
    PinnedSkillDagSelectionConflict {
        /// Selected Agent.
        agent_id: sui::types::Address,
        /// Selected Agent-local skill.
        skill_id: u64,
        /// Authoritative registered DAG.
        pinned_dag: sui::types::Address,
        /// Rejected caller-selected DAG.
        selected_dag: sui::types::Address,
    },

    /// A runtime-selected Agent skill requires the Task author to choose a DAG.
    #[error("Agent '{agent_id}' skill {skill_id} requires a caller-selected DAG")]
    RuntimeSelectedSkillDagRequired {
        /// Selected Agent.
        agent_id: sui::types::Address,
        /// Selected Agent-local skill.
        skill_id: u64,
    },

    /// The configured signer cannot satisfy the Task controller.
    #[error("authority for Task '{task_id}' is unavailable: {message}")]
    AuthorityUnavailable {
        /// Task whose controller cannot be resolved.
        task_id: sui::types::Address,
        /// Human readable authority mismatch.
        message: String,
    },

    /// An onchain object did not have the required type or shape.
    #[error("object '{object_id}' is invalid: {message}")]
    InvalidObject {
        /// Invalid object identifier.
        object_id: sui::types::Address,
        /// Human readable schema mismatch.
        message: String,
    },

    /// Binary Canonical Serialization failed at the Move boundary.
    #[error("scheduler BCS conversion failed")]
    Bcs(#[source] bcs::Error),

    /// RPC or network transport failed.
    #[error("scheduler transport failed: {source}")]
    Transport {
        /// Original transport failure.
        #[source]
        source: ErrorSource,
    },

    /// Programmable transaction construction failed.
    #[error("scheduler transaction construction failed: {source}")]
    Transaction {
        /// Original construction failure.
        #[source]
        source: ErrorSource,
    },

    /// A submitted transaction was not confirmed as expected.
    #[error("scheduler transaction confirmation failed: {message}")]
    Confirmation {
        /// Confirmation failure details.
        message: String,
    },

    /// Confirmed chain data violates the scheduler's structural invariants.
    #[error("inconsistent scheduler chain state: {message}")]
    InconsistentChainState {
        /// Invariant violation details.
        message: String,
    },

    /// A scheduler query option is invalid.
    #[error("invalid scheduler request: {message}")]
    InvalidRequest {
        /// Invalid request details.
        message: String,
    },

    /// Watching an occurrence reached its timeout.
    #[error("timed out while waiting for occurrence lifecycle completion")]
    WatchTimedOut {
        /// Last object backed snapshot observed before timeout.
        last_snapshot: Box<OccurrenceSnapshot>,
    },
}

impl SchedulerError {
    pub(crate) fn transport<E>(source: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let source = source.into();
        Self::Transport {
            source: source.into_boxed_dyn_error(),
        }
    }

    pub(crate) fn transaction<E>(source: E) -> Self
    where
        E: Into<anyhow::Error>,
    {
        let source = source.into();
        Self::Transaction {
            source: source.into_boxed_dyn_error(),
        }
    }
}

#[cfg(feature = "nexus")]
impl From<NexusError> for SchedulerError {
    fn from(error: NexusError) -> Self {
        Self::Client(Box::new(error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "nexus")]
    #[test]
    fn client_errors_remain_typed() {
        let tag = crate::sui::types::StructTag::new(
            crate::sui::types::Address::from_static("0xa"),
            crate::sui::types::Identifier::from_static("witness"),
            crate::sui::types::Identifier::from_static("V2"),
            vec![],
        );
        let source: crate::nexus::error::NexusError =
            crate::nexus::error::ClientUpgradeRequired::new(
                crate::sui::types::Address::from_static("0xb"),
                tag,
                None,
            )
            .into();
        let error = SchedulerError::from(source);

        assert!(matches!(
            error,
            SchedulerError::Client(source)
                if matches!(
                    *source,
                    crate::nexus::error::NexusError::ClientUpgradeRequired(_)
                )
        ));
    }

    #[test]
    fn boundary_errors_retain_transport_and_transaction_sources() {
        let transport = SchedulerError::transport(anyhow::anyhow!("RPC unavailable"));
        assert_eq!(
            transport.to_string(),
            "scheduler transport failed: RPC unavailable"
        );
        assert_eq!(
            transport.source().map(ToString::to_string).as_deref(),
            Some("RPC unavailable")
        );

        let transaction = SchedulerError::transaction(anyhow::anyhow!("invalid PTB"));
        assert_eq!(
            transaction.to_string(),
            "scheduler transaction construction failed: invalid PTB"
        );
        assert_eq!(
            transaction.source().map(ToString::to_string).as_deref(),
            Some("invalid PTB")
        );
    }
}
