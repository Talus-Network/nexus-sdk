//! Confirmed transaction receipts for scheduler mutations.

use {
    crate::{
        scheduler::{OccurrenceRef, OccurrenceSource, WithdrawalReason},
        sui,
    },
    serde::{Deserialize, Serialize},
};

/// Reference to one confirmed Sui transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionReference {
    digest: sui::types::Digest,
    checkpoint: u64,
}

impl TransactionReference {
    pub(crate) const fn new(digest: sui::types::Digest, checkpoint: u64) -> Self {
        Self { digest, checkpoint }
    }

    /// Returns the transaction digest.
    pub const fn digest(&self) -> &sui::types::Digest {
        &self.digest
    }

    /// Returns the checkpoint that contains the transaction.
    pub const fn checkpoint(&self) -> u64 {
        self.checkpoint
    }
}

/// One occurrence allocated by a scheduler mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledOccurrence {
    reference: OccurrenceRef,
    start_time_ms: u64,
    deadline_ms: Option<u64>,
    priority_fee_percentage: u64,
    source: OccurrenceSource,
}

impl ScheduledOccurrence {
    pub(crate) const fn new(
        reference: OccurrenceRef,
        start_time_ms: u64,
        deadline_ms: Option<u64>,
        priority_fee_percentage: u64,
        source: OccurrenceSource,
    ) -> Self {
        Self {
            reference,
            start_time_ms,
            deadline_ms,
            priority_fee_percentage,
            source,
        }
    }

    /// Returns the allocated occurrence identity.
    pub const fn reference(&self) -> OccurrenceRef {
        self.reference
    }

    /// Returns the requested absolute start timestamp.
    pub const fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the optional absolute dispatch deadline.
    pub const fn deadline_ms(&self) -> Option<u64> {
        self.deadline_ms
    }

    /// Returns the dispatch priority fee percentage.
    pub const fn priority_fee_percentage(&self) -> u64 {
        self.priority_fee_percentage
    }

    /// Returns how the occurrence was allocated.
    pub const fn source(&self) -> OccurrenceSource {
        self.source
    }
}

/// One scheduled occurrence withdrawn before dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawnOccurrence {
    reference: OccurrenceRef,
    reason: WithdrawalReason,
}

impl WithdrawnOccurrence {
    pub(crate) const fn new(reference: OccurrenceRef, reason: WithdrawalReason) -> Self {
        Self { reference, reason }
    }

    /// Returns the withdrawn occurrence identity.
    pub const fn reference(&self) -> OccurrenceRef {
        self.reference
    }

    /// Returns why the occurrence left its Schedule.
    pub const fn reason(&self) -> WithdrawalReason {
        self.reason
    }
}

/// Net Schedule changes confirmed by one transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDelta {
    scheduled: Vec<ScheduledOccurrence>,
    withdrawn: Vec<WithdrawnOccurrence>,
    advertised: Option<OccurrenceRef>,
}

impl ScheduleDelta {
    pub(crate) fn new(
        scheduled: Vec<ScheduledOccurrence>,
        withdrawn: Vec<WithdrawnOccurrence>,
        advertised: Option<OccurrenceRef>,
    ) -> Self {
        Self {
            scheduled,
            withdrawn,
            advertised,
        }
    }

    /// Returns every occurrence allocated by the transaction.
    pub fn scheduled(&self) -> &[ScheduledOccurrence] {
        &self.scheduled
    }

    /// Returns every occurrence withdrawn by the transaction.
    pub fn withdrawn(&self) -> &[WithdrawnOccurrence] {
        &self.withdrawn
    }

    /// Returns the final new advertisement when the transaction changed it.
    pub const fn advertised(&self) -> Option<OccurrenceRef> {
        self.advertised
    }

    /// Returns whether no allocation, withdrawal, or advertisement changed.
    pub fn is_empty(&self) -> bool {
        self.scheduled.is_empty() && self.withdrawn.is_empty() && self.advertised.is_none()
    }
}

/// Result of one confirmed Task mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskMutationReceipt {
    transaction: TransactionReference,
    task_id: sui::types::Address,
    delta: ScheduleDelta,
}

impl TaskMutationReceipt {
    pub(crate) const fn new(
        transaction: TransactionReference,
        task_id: sui::types::Address,
        delta: ScheduleDelta,
    ) -> Self {
        Self {
            transaction,
            task_id,
            delta,
        }
    }

    /// Returns the confirmed transaction reference.
    pub const fn transaction(&self) -> &TransactionReference {
        &self.transaction
    }

    /// Returns the mutated Task identifier.
    pub const fn task_id(&self) -> sui::types::Address {
        self.task_id
    }

    /// Returns the net Schedule changes in the transaction.
    pub const fn delta(&self) -> &ScheduleDelta {
        &self.delta
    }
}

/// Result of aborting an expired runtime execution for an occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbortReceipt {
    transaction: TransactionReference,
    occurrence: OccurrenceRef,
    execution_id: sui::types::Address,
}

impl AbortReceipt {
    pub(crate) const fn new(
        transaction: TransactionReference,
        occurrence: OccurrenceRef,
        execution_id: sui::types::Address,
    ) -> Self {
        Self {
            transaction,
            occurrence,
            execution_id,
        }
    }

    /// Returns the confirmed transaction reference.
    pub const fn transaction(&self) -> &TransactionReference {
        &self.transaction
    }

    /// Returns the occurrence whose execution was aborted.
    pub const fn occurrence(&self) -> OccurrenceRef {
        self.occurrence
    }

    /// Returns the aborted deterministic execution identifier.
    pub const fn execution_id(&self) -> sui::types::Address {
        self.execution_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    #[test]
    fn mutation_receipts_preserve_transaction_and_schedule_changes() {
        let task_id = address("0x20");
        let reference = OccurrenceRef::new(task_id, 4);
        let transaction = TransactionReference::new(sui::types::Digest::new([7; 32]), 12);
        assert_eq!(transaction.digest(), &sui::types::Digest::new([7; 32]));
        assert_eq!(transaction.checkpoint(), 12);

        let scheduled = ScheduledOccurrence::new(
            reference,
            100,
            Some(120),
            30,
            OccurrenceSource::Recurring { iteration: 2 },
        );
        assert_eq!(scheduled.reference(), reference);
        assert_eq!(scheduled.start_time_ms(), 100);
        assert_eq!(scheduled.deadline_ms(), Some(120));
        assert_eq!(scheduled.priority_fee_percentage(), 30);
        assert_eq!(
            scheduled.source(),
            OccurrenceSource::Recurring { iteration: 2 }
        );

        let withdrawn = WithdrawnOccurrence::new(reference, WithdrawalReason::RecurrenceReplaced);
        assert_eq!(withdrawn.reference(), reference);
        assert_eq!(withdrawn.reason(), WithdrawalReason::RecurrenceReplaced);

        let delta = ScheduleDelta::new(
            vec![scheduled.clone()],
            vec![withdrawn.clone()],
            Some(reference),
        );
        assert_eq!(delta.scheduled(), &[scheduled]);
        assert_eq!(delta.withdrawn(), &[withdrawn]);
        assert_eq!(delta.advertised(), Some(reference));
        assert!(!delta.is_empty());
        assert!(ScheduleDelta::default().is_empty());

        let receipt = TaskMutationReceipt::new(transaction.clone(), task_id, delta.clone());
        assert_eq!(receipt.transaction(), &transaction);
        assert_eq!(receipt.task_id(), task_id);
        assert_eq!(receipt.delta(), &delta);
    }

    #[test]
    fn abort_receipt_identifies_the_occurrence_and_runtime_object() {
        let transaction = TransactionReference::new(sui::types::Digest::new([8; 32]), 13);
        let occurrence = OccurrenceRef::new(address("0x21"), 5);
        let execution_id = address("0x22");
        let receipt = AbortReceipt::new(transaction.clone(), occurrence, execution_id);

        assert_eq!(receipt.transaction(), &transaction);
        assert_eq!(receipt.occurrence(), occurrence);
        assert_eq!(receipt.execution_id(), execution_id);
    }
}
