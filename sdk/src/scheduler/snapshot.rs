//! Object backed Task and occurrence snapshots.

use {
    crate::{
        scheduler::{FailurePolicy, OccurrenceRef},
        sui,
    },
    serde::{Deserialize, Serialize},
    std::time::Duration,
};

const DEFAULT_WATCH_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const DEFAULT_WATCH_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Immutable controller of a Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskController {
    /// A Sui address controls the Task.
    Address {
        /// Controller address.
        address: sui::types::Address,
    },
    /// An Agent object controls the Task.
    Agent {
        /// Controller Agent identifier.
        agent_id: sui::types::Address,
    },
}

/// Durable lifecycle state of a Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// The Task may advertise and dispatch eligible work.
    Active,
    /// Dispatch is paused while future work is retained.
    Paused,
    /// Future work was canceled while in flight work may still settle.
    Canceled,
    /// Live resources were released while history was retained.
    Finalized,
}

/// Origin of an allocated occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OccurrenceSource {
    /// The controller scheduled the occurrence independently.
    Standalone,
    /// A recurrence allocated the occurrence.
    Recurring {
        /// Zero based recurrence iteration.
        iteration: u64,
    },
}

/// Reason an occurrence left its Schedule before dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalReason {
    /// A new recurrence replaced the recurrence candidate.
    RecurrenceReplaced,
    /// The controller cleared the recurrence.
    RecurrenceCleared,
    /// The controller canceled the Task.
    TaskCanceled,
}

/// Public lifecycle projection of one occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OccurrenceStatus {
    /// Allocated but not currently offered for dispatch.
    Pending,
    /// Currently offered to the leader network.
    Advertised,
    /// Dispatched while runtime work remains observable as incomplete.
    Executing,
    /// Runtime work is complete but scheduler settlement is pending.
    Finished,
    /// Scheduler settlement completed.
    Settled {
        /// Whether runtime execution succeeded.
        succeeded: bool,
    },
    /// The dispatch deadline elapsed before dispatch.
    Missed {
        /// Timestamp at which the scheduler recorded expiration.
        missed_at_ms: u64,
    },
    /// Removed from the Schedule before dispatch.
    Withdrawn {
        /// Reason the occurrence left its Schedule.
        reason: WithdrawalReason,
    },
}

impl OccurrenceStatus {
    /// Returns whether scheduler lifecycle processing is complete.
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Settled { .. } | Self::Missed { .. } | Self::Withdrawn { .. }
        )
    }
}

/// Observable workflow state related to one dispatched occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSnapshot {
    execution_id: sui::types::Address,
    created_at_ms: Option<u64>,
    active_walks: Option<u64>,
    pending_abort_walks: Option<u64>,
    pending_settlement_walks: Option<u64>,
    successful_walks: Option<u64>,
    failed_walks: Option<u64>,
    aborted_walks: Option<u64>,
}

pub(crate) struct ExecutionObservation {
    pub created_at_ms: u64,
    pub active_walks: u64,
    pub pending_abort_walks: u64,
    pub pending_settlement_walks: u64,
    pub successful_walks: u64,
    pub failed_walks: u64,
    pub aborted_walks: u64,
}

impl ExecutionSnapshot {
    pub(crate) const fn unavailable(execution_id: sui::types::Address) -> Self {
        Self {
            execution_id,
            created_at_ms: None,
            active_walks: None,
            pending_abort_walks: None,
            pending_settlement_walks: None,
            successful_walks: None,
            failed_walks: None,
            aborted_walks: None,
        }
    }

    pub(crate) const fn observed(
        execution_id: sui::types::Address,
        observation: ExecutionObservation,
    ) -> Self {
        Self {
            execution_id,
            created_at_ms: Some(observation.created_at_ms),
            active_walks: Some(observation.active_walks),
            pending_abort_walks: Some(observation.pending_abort_walks),
            pending_settlement_walks: Some(observation.pending_settlement_walks),
            successful_walks: Some(observation.successful_walks),
            failed_walks: Some(observation.failed_walks),
            aborted_walks: Some(observation.aborted_walks),
        }
    }

    /// Returns the deterministic workflow execution identifier.
    pub const fn execution_id(&self) -> sui::types::Address {
        self.execution_id
    }

    /// Returns the runtime creation timestamp when the object is available.
    pub const fn created_at_ms(&self) -> Option<u64> {
        self.created_at_ms
    }

    /// Returns the active walk count when the object is available.
    pub const fn active_walks(&self) -> Option<u64> {
        self.active_walks
    }

    /// Returns the pending abort walk count when the object is available.
    pub const fn pending_abort_walks(&self) -> Option<u64> {
        self.pending_abort_walks
    }

    /// Returns the pending settlement walk count when the object is available.
    pub const fn pending_settlement_walks(&self) -> Option<u64> {
        self.pending_settlement_walks
    }

    /// Returns the successful walk count when the object is available.
    pub const fn successful_walks(&self) -> Option<u64> {
        self.successful_walks
    }

    /// Returns the failed walk count when the object is available.
    pub const fn failed_walks(&self) -> Option<u64> {
        self.failed_walks
    }

    /// Returns the aborted walk count when the object is available.
    pub const fn aborted_walks(&self) -> Option<u64> {
        self.aborted_walks
    }
}

/// Current object backed view of one occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccurrenceSnapshot {
    pub(crate) reference: OccurrenceRef,
    pub(crate) source: OccurrenceSource,
    pub(crate) requested_start_time_ms: u64,
    pub(crate) effective_start_time_ms: Option<u64>,
    pub(crate) deadline_ms: Option<u64>,
    pub(crate) priority_fee_percentage: u64,
    pub(crate) dispatched_at_ms: Option<u64>,
    pub(crate) settled_at_ms: Option<u64>,
    pub(crate) status: OccurrenceStatus,
    pub(crate) execution: Option<ExecutionSnapshot>,
    pub(crate) observed_task_version: sui::types::Version,
}

impl OccurrenceSnapshot {
    /// Returns the stable occurrence identity.
    pub const fn reference(&self) -> OccurrenceRef {
        self.reference
    }

    /// Returns how the occurrence was allocated.
    pub const fn source(&self) -> OccurrenceSource {
        self.source
    }

    /// Returns the requested absolute start timestamp.
    pub const fn requested_start_time_ms(&self) -> u64 {
        self.requested_start_time_ms
    }

    /// Returns the latest effective start advertised to leaders.
    pub const fn effective_start_time_ms(&self) -> Option<u64> {
        self.effective_start_time_ms
    }

    /// Returns the optional absolute dispatch deadline.
    pub const fn deadline_ms(&self) -> Option<u64> {
        self.deadline_ms
    }

    /// Returns the dispatch priority fee percentage.
    pub const fn priority_fee_percentage(&self) -> u64 {
        self.priority_fee_percentage
    }

    /// Returns the dispatch timestamp when dispatch occurred.
    pub const fn dispatched_at_ms(&self) -> Option<u64> {
        self.dispatched_at_ms
    }

    /// Returns the settlement timestamp when settlement occurred.
    pub const fn settled_at_ms(&self) -> Option<u64> {
        self.settled_at_ms
    }

    /// Returns the projected public lifecycle state.
    pub const fn status(&self) -> OccurrenceStatus {
        self.status
    }

    /// Returns observable runtime data after dispatch.
    pub const fn execution(&self) -> Option<&ExecutionSnapshot> {
        self.execution.as_ref()
    }

    /// Returns the Task version used for this snapshot.
    pub const fn observed_task_version(&self) -> sui::types::Version {
        self.observed_task_version
    }
}

/// Current object backed view of one Task.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub(crate) task_id: sui::types::Address,
    pub(crate) controller: TaskController,
    pub(crate) status: TaskStatus,
    pub(crate) failure_policy: FailurePolicy,
    pub(crate) advertised: Option<OccurrenceRef>,
    pub(crate) allocated_occurrences: u64,
    pub(crate) pending_occurrences: u64,
    pub(crate) dispatched_occurrences: u64,
    pub(crate) in_flight_occurrences: u64,
    pub(crate) observed_version: sui::types::Version,
}

impl TaskSnapshot {
    /// Returns the Task identifier.
    pub const fn task_id(&self) -> sui::types::Address {
        self.task_id
    }

    /// Returns the immutable Task controller.
    pub const fn controller(&self) -> TaskController {
        self.controller
    }

    /// Returns the durable Task lifecycle state.
    pub const fn status(&self) -> TaskStatus {
        self.status
    }

    /// Returns the configured failure policy.
    pub const fn failure_policy(&self) -> FailurePolicy {
        self.failure_policy
    }

    /// Returns the occurrence currently offered to leaders.
    pub const fn advertised(&self) -> Option<OccurrenceRef> {
        self.advertised
    }

    /// Returns the number of occurrence identities allocated by the Task.
    pub const fn allocated_occurrences(&self) -> u64 {
        self.allocated_occurrences
    }

    /// Returns the number of pending occurrences.
    pub const fn pending_occurrences(&self) -> u64 {
        self.pending_occurrences
    }

    /// Returns the number of occurrences dispatched by the Task.
    pub const fn dispatched_occurrences(&self) -> u64 {
        self.dispatched_occurrences
    }

    /// Returns the number of dispatched occurrences awaiting settlement.
    pub const fn in_flight_occurrences(&self) -> u64 {
        self.in_flight_occurrences
    }

    /// Returns the object version used for this snapshot.
    pub const fn observed_version(&self) -> sui::types::Version {
        self.observed_version
    }
}

/// One page of durable occurrence records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccurrencePage {
    occurrences: Vec<OccurrenceSnapshot>,
    next_cursor: Option<Vec<u8>>,
}

impl OccurrencePage {
    /// Creates one occurrence page with an unchanged RPC cursor.
    pub(crate) fn new(occurrences: Vec<OccurrenceSnapshot>, next_cursor: Option<Vec<u8>>) -> Self {
        Self {
            occurrences,
            next_cursor,
        }
    }

    /// Returns the occurrence snapshots in RPC order.
    pub fn occurrences(&self) -> &[OccurrenceSnapshot] {
        &self.occurrences
    }

    /// Returns the opaque cursor for the next page.
    pub fn next_cursor(&self) -> Option<&[u8]> {
        self.next_cursor.as_deref()
    }

    /// Separates the occurrence snapshots from the opaque next page cursor.
    pub fn into_parts(self) -> (Vec<OccurrenceSnapshot>, Option<Vec<u8>>) {
        (self.occurrences, self.next_cursor)
    }
}

/// Polling policy for watching an occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchOptions {
    timeout: Duration,
    poll_interval: Duration,
}

impl WatchOptions {
    /// Creates a polling policy.
    pub const fn new(timeout: Duration, poll_interval: Duration) -> Self {
        Self {
            timeout,
            poll_interval,
        }
    }

    /// Returns the maximum observation duration.
    pub const fn timeout(self) -> Duration {
        self.timeout
    }

    /// Returns the delay between object reads.
    pub const fn poll_interval(self) -> Duration {
        self.poll_interval
    }
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self::new(DEFAULT_WATCH_TIMEOUT, DEFAULT_WATCH_POLL_INTERVAL)
    }
}

/// Current payment accounting for one occurrence execution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccurrenceCost {
    pub(crate) payment_id: sui::types::Address,
    pub(crate) max_budget_mist: u64,
    pub(crate) locked_budget_mist: u64,
    pub(crate) consumed_mist: u64,
    pub(crate) outstanding_locks: u64,
    pub(crate) accomplished: bool,
    pub(crate) refunded: bool,
}

impl OccurrenceCost {
    /// Returns the execution payment object identifier.
    pub const fn payment_id(&self) -> sui::types::Address {
        self.payment_id
    }

    /// Returns the maximum execution budget.
    pub const fn max_budget_mist(&self) -> u64 {
        self.max_budget_mist
    }

    /// Returns the budget currently locked by tool calls.
    pub const fn locked_budget_mist(&self) -> u64 {
        self.locked_budget_mist
    }

    /// Returns the amount already consumed.
    pub const fn consumed_mist(&self) -> u64 {
        self.consumed_mist
    }

    /// Returns the number of unresolved payment locks.
    pub const fn outstanding_locks(&self) -> u64 {
        self.outstanding_locks
    }

    /// Returns whether execution accounting is complete.
    pub const fn accomplished(&self) -> bool {
        self.accomplished
    }

    /// Returns whether remaining funds were refunded.
    pub const fn refunded(&self) -> bool {
        self.refunded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    #[test]
    fn occurrence_terminality_matches_scheduler_completion() {
        for status in [
            OccurrenceStatus::Pending,
            OccurrenceStatus::Advertised,
            OccurrenceStatus::Executing,
            OccurrenceStatus::Finished,
        ] {
            assert!(!status.is_terminal());
        }
        for status in [
            OccurrenceStatus::Settled { succeeded: true },
            OccurrenceStatus::Missed { missed_at_ms: 10 },
            OccurrenceStatus::Withdrawn {
                reason: WithdrawalReason::TaskCanceled,
            },
        ] {
            assert!(status.is_terminal());
        }
    }

    #[test]
    fn execution_snapshots_distinguish_missing_and_observed_runtime_state() {
        let execution_id = address("0x10");
        let unavailable = ExecutionSnapshot::unavailable(execution_id);
        assert_eq!(unavailable.execution_id(), execution_id);
        assert_eq!(unavailable.created_at_ms(), None);
        assert_eq!(unavailable.active_walks(), None);
        assert_eq!(unavailable.pending_abort_walks(), None);
        assert_eq!(unavailable.pending_settlement_walks(), None);
        assert_eq!(unavailable.successful_walks(), None);
        assert_eq!(unavailable.failed_walks(), None);
        assert_eq!(unavailable.aborted_walks(), None);

        let observed = ExecutionSnapshot::observed(
            execution_id,
            ExecutionObservation {
                created_at_ms: 1,
                active_walks: 2,
                pending_abort_walks: 3,
                pending_settlement_walks: 4,
                successful_walks: 5,
                failed_walks: 6,
                aborted_walks: 7,
            },
        );
        assert_eq!(observed.execution_id(), execution_id);
        assert_eq!(observed.created_at_ms(), Some(1));
        assert_eq!(observed.active_walks(), Some(2));
        assert_eq!(observed.pending_abort_walks(), Some(3));
        assert_eq!(observed.pending_settlement_walks(), Some(4));
        assert_eq!(observed.successful_walks(), Some(5));
        assert_eq!(observed.failed_walks(), Some(6));
        assert_eq!(observed.aborted_walks(), Some(7));
    }

    #[test]
    fn occurrence_and_task_snapshots_expose_one_coherent_object_view() {
        let task_id = address("0x11");
        let execution_id = address("0x12");
        let reference = OccurrenceRef::new(task_id, 8);
        let execution = ExecutionSnapshot::observed(
            execution_id,
            ExecutionObservation {
                created_at_ms: 10,
                active_walks: 0,
                pending_abort_walks: 0,
                pending_settlement_walks: 0,
                successful_walks: 1,
                failed_walks: 0,
                aborted_walks: 0,
            },
        );
        let occurrence = OccurrenceSnapshot {
            reference,
            source: OccurrenceSource::Recurring { iteration: 2 },
            requested_start_time_ms: 20,
            effective_start_time_ms: Some(21),
            deadline_ms: Some(30),
            priority_fee_percentage: 40,
            dispatched_at_ms: Some(22),
            settled_at_ms: Some(29),
            status: OccurrenceStatus::Settled { succeeded: true },
            execution: Some(execution),
            observed_task_version: 7,
        };

        assert_eq!(occurrence.reference(), reference);
        assert_eq!(
            occurrence.source(),
            OccurrenceSource::Recurring { iteration: 2 }
        );
        assert_eq!(occurrence.requested_start_time_ms(), 20);
        assert_eq!(occurrence.effective_start_time_ms(), Some(21));
        assert_eq!(occurrence.deadline_ms(), Some(30));
        assert_eq!(occurrence.priority_fee_percentage(), 40);
        assert_eq!(occurrence.dispatched_at_ms(), Some(22));
        assert_eq!(occurrence.settled_at_ms(), Some(29));
        assert_eq!(
            occurrence.status(),
            OccurrenceStatus::Settled { succeeded: true }
        );
        assert_eq!(
            occurrence.execution().map(ExecutionSnapshot::execution_id),
            Some(execution_id)
        );
        assert_eq!(occurrence.observed_task_version(), 7);

        let task = TaskSnapshot {
            task_id,
            controller: TaskController::Address {
                address: address("0x13"),
            },
            status: TaskStatus::Paused,
            failure_policy: FailurePolicy::Pause,
            advertised: Some(reference),
            allocated_occurrences: 9,
            pending_occurrences: 3,
            dispatched_occurrences: 6,
            in_flight_occurrences: 2,
            observed_version: 8,
        };
        assert_eq!(task.task_id(), task_id);
        assert_eq!(
            task.controller(),
            TaskController::Address {
                address: address("0x13")
            }
        );
        assert_eq!(task.status(), TaskStatus::Paused);
        assert_eq!(task.failure_policy(), FailurePolicy::Pause);
        assert_eq!(task.advertised(), Some(reference));
        assert_eq!(task.allocated_occurrences(), 9);
        assert_eq!(task.pending_occurrences(), 3);
        assert_eq!(task.dispatched_occurrences(), 6);
        assert_eq!(task.in_flight_occurrences(), 2);
        assert_eq!(task.observed_version(), 8);
    }

    #[test]
    fn occurrence_pages_preserve_records_and_opaque_cursors() {
        let page = OccurrencePage::new(Vec::new(), Some(vec![1, 2, 3]));
        assert!(page.occurrences().is_empty());
        assert_eq!(page.next_cursor(), Some([1, 2, 3].as_slice()));

        let (occurrences, cursor) = page.into_parts();
        assert!(occurrences.is_empty());
        assert_eq!(cursor, Some(vec![1, 2, 3]));
    }

    #[test]
    fn watch_options_and_costs_expose_their_complete_policy() {
        let options = WatchOptions::new(Duration::from_secs(4), Duration::from_millis(250));
        assert_eq!(options.timeout(), Duration::from_secs(4));
        assert_eq!(options.poll_interval(), Duration::from_millis(250));

        let defaults = WatchOptions::default();
        assert_eq!(defaults.timeout(), DEFAULT_WATCH_TIMEOUT);
        assert_eq!(defaults.poll_interval(), DEFAULT_WATCH_POLL_INTERVAL);

        let cost = OccurrenceCost {
            payment_id: address("0x14"),
            max_budget_mist: 100,
            locked_budget_mist: 30,
            consumed_mist: 40,
            outstanding_locks: 2,
            accomplished: true,
            refunded: false,
        };
        assert_eq!(cost.payment_id(), address("0x14"));
        assert_eq!(cost.max_budget_mist(), 100);
        assert_eq!(cost.locked_budget_mist(), 30);
        assert_eq!(cost.consumed_mist(), 40);
        assert_eq!(cost.outstanding_locks(), 2);
        assert!(cost.accomplished());
        assert!(!cost.refunded());
    }
}
