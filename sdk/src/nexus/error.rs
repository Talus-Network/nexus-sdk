//! Common error types for Nexus-related functionality.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexusError {
    #[error("Sui wallet error: {0}")]
    Wallet(anyhow::Error),
    #[error("Client configuration error: {0}")]
    Configuration(String),
    #[error("Transaction building error: {0}")]
    TransactionBuilding(anyhow::Error),
    #[error("RPC error: {0}")]
    Rpc(anyhow::Error),
    #[error("Parsing error: {0}")]
    Parsing(anyhow::Error),
    #[error("Timeout error: {0}")]
    Timeout(anyhow::Error),
    #[error("Channel error: {0}")]
    Channel(anyhow::Error),
    #[error("Storage error: {0}")]
    Storage(anyhow::Error),
    /// The requested occurrence has no observable lifecycle.
    #[error("Occurrence '{task_id}:{occurrence_id}' was not found")]
    OccurrenceNotFound {
        /// Identifier of the owning Task.
        task_id: crate::sui::types::Address,
        /// Identifier allocated within the Task.
        occurrence_id: u64,
    },
    /// The requested occurrence has not created its runtime object.
    #[error("Occurrence '{task_id}:{occurrence_id}' has not been dispatched")]
    OccurrenceNotDispatched {
        /// Identifier of the owning Task.
        task_id: crate::sui::types::Address,
        /// Identifier allocated within the Task.
        occurrence_id: u64,
    },
    /// Required lifecycle evidence is not visible yet.
    #[error("Occurrence '{task_id}:{occurrence_id}' lifecycle is incomplete: {message}")]
    OccurrenceLifecycleIncomplete {
        /// Identifier of the owning Task.
        task_id: crate::sui::types::Address,
        /// Identifier allocated within the Task.
        occurrence_id: u64,
        /// Missing lifecycle evidence.
        message: String,
    },
    /// Observed lifecycle evidence violates an occurrence invariant.
    #[error("Occurrence '{task_id}:{occurrence_id}' lifecycle is inconsistent: {message}")]
    OccurrenceLifecycleInconsistent {
        /// Identifier of the owning Task.
        task_id: crate::sui::types::Address,
        /// Identifier allocated within the Task.
        occurrence_id: u64,
        /// Violated lifecycle invariant.
        message: String,
    },
    /// Watching an occurrence exceeded its configured duration.
    #[error(
        "Timed out watching occurrence '{}:{}'",
        last_snapshot.reference.task_id,
        last_snapshot.reference.occurrence_id
    )]
    OccurrenceWatchTimedOut {
        /// Most recent complete observation.
        last_snapshot: Box<super::scheduler::OccurrenceSnapshot>,
    },
}
