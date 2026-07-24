//! Scheduler occurrence inspection.

use {
    super::{OccurrenceRef, ScheduledOccurrence, SchedulerActions},
    crate::{
        events::{NexusEventKind, NexusEventQuery},
        move_bindings::{
            scheduler::{
                schedule::{OccurrenceSource, OccurrenceWithdrawalReason},
                task::Task,
            },
            workflow::execution::DAGExecution,
        },
        nexus::{
            error::NexusError,
            object_history::{fetch_shared_object_history, ObjectHistoryRequest},
            workflow::{AbortExecutionResult, AbortExpiredExecutionResult, ExecutionCostResult},
        },
        sui,
    },
    serde::{Deserialize, Serialize},
    std::sync::Arc,
    tokio::time::{Duration, Instant},
};

const DEFAULT_OCCURRENCE_WATCH_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const DEFAULT_OCCURRENCE_WATCH_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// Public lifecycle state of one scheduled occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OccurrenceStatus {
    /// The occurrence is allocated but not offered for dispatch.
    Pending,
    /// The occurrence is currently offered for dispatch.
    Advertised,
    /// The runtime object still has active or pending work.
    Executing,
    /// Runtime work is complete but scheduler settlement is pending.
    Finished,
    /// Scheduler settlement completed.
    Settled {
        /// Whether runtime execution succeeded.
        succeeded: bool,
    },
    /// The dispatch deadline elapsed.
    Missed {
        /// Time at which the scheduler recorded the missed occurrence.
        missed_at_ms: u64,
    },
    /// The occurrence left its schedule before dispatch.
    Withdrawn {
        /// Reason the scheduler removed the occurrence.
        reason: OccurrenceWithdrawalReason,
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

/// Observable runtime data related to one occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionSnapshot {
    /// Deterministic runtime object identifier.
    pub execution_id: sui::types::Address,
    /// Runtime creation time when the object is visible.
    pub created_at_ms: Option<u64>,
    /// Count of active walks when the object is visible.
    pub active_walks: Option<u64>,
    /// Count of walks pending abort when the object is visible.
    pub pending_abort_walks: Option<u64>,
    /// Count of walks pending settlement when the object is visible.
    pub pending_settlement_walks: Option<u64>,
    /// Count of successful walks when the object is visible.
    pub successful_walks: Option<u64>,
    /// Count of failed walks when the object is visible.
    pub failed_walks: Option<u64>,
    /// Count of aborted walks when the object is visible.
    pub aborted_walks: Option<u64>,
}

impl ExecutionSnapshot {
    fn pending(execution_id: sui::types::Address) -> Self {
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

    fn from_execution(execution_id: sui::types::Address, execution: &DAGExecution) -> Self {
        Self {
            execution_id,
            created_at_ms: Some(execution.created_at),
            active_walks: Some(execution.active_walks),
            pending_abort_walks: Some(execution.pending_abort_walks),
            pending_settlement_walks: Some(execution.pending_settlement_walks),
            successful_walks: Some(execution.successful_walks),
            failed_walks: Some(execution.failed_walks),
            aborted_walks: Some(execution.aborted_walks),
        }
    }
}

/// Complete public snapshot of one occurrence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccurrenceSnapshot {
    /// Stable occurrence identity.
    pub reference: OccurrenceRef,
    /// Source that allocated the occurrence.
    pub source: OccurrenceSource,
    /// Start time requested when the occurrence was allocated.
    pub requested_start_time_ms: u64,
    /// Last effective start time advertised to leaders.
    pub effective_start_time_ms: Option<u64>,
    /// Optional dispatch deadline.
    pub deadline_ms: Option<u64>,
    /// Priority fee percentage selected for dispatch.
    pub priority_fee_percentage: u64,
    /// Current public lifecycle state.
    pub status: OccurrenceStatus,
    /// Runtime data after dispatch.
    pub execution: Option<ExecutionSnapshot>,
    /// Exact Task version used for this snapshot.
    pub observed_task_version: sui::types::Version,
    /// Latest Task update checkpoint used for this snapshot.
    pub observed_checkpoint: u64,
}

/// Polling policy for [`SchedulerActions::watch_occurrence`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchOccurrenceOptions {
    /// Maximum observation duration.
    pub timeout: Duration,
    /// Delay between object observations.
    pub poll_interval: Duration,
}

impl Default for WatchOccurrenceOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_OCCURRENCE_WATCH_TIMEOUT,
            poll_interval: DEFAULT_OCCURRENCE_WATCH_POLL_INTERVAL,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DispatchRecord {
    execution_id: sui::types::Address,
}

#[derive(Clone, Debug)]
struct ReducedOccurrence {
    scheduled: ScheduledOccurrence,
    effective_start_time_ms: Option<u64>,
    dispatched: Option<DispatchRecord>,
    terminal: Option<OccurrenceStatus>,
}

#[derive(Clone, Debug)]
struct InspectionState {
    events: Vec<NexusEventKind>,
    task_version: sui::types::Version,
    checkpoint: u64,
}

fn incomplete(reference: OccurrenceRef, message: impl Into<String>) -> NexusError {
    NexusError::OccurrenceLifecycleIncomplete {
        task_id: reference.task_id,
        occurrence_id: reference.occurrence_id,
        message: message.into(),
    }
}

fn inconsistent(reference: OccurrenceRef, message: impl Into<String>) -> NexusError {
    NexusError::OccurrenceLifecycleInconsistent {
        task_id: reference.task_id,
        occurrence_id: reference.occurrence_id,
        message: message.into(),
    }
}

fn event_reference(event: &NexusEventKind) -> Option<OccurrenceRef> {
    let (task_id, occurrence_id) = match event {
        NexusEventKind::OccurrenceAdvertised(event) => (event.task_id.bytes, event.occurrence_id),
        NexusEventKind::OccurrenceDispatched(event) => (event.task_id.bytes, event.occurrence_id),
        NexusEventKind::OccurrenceMissed(event) => (event.task_id.bytes, event.occurrence_id),
        NexusEventKind::OccurrenceScheduled(event) => (event.task_id.bytes, event.occurrence_id),
        NexusEventKind::OccurrenceSettled(event) => (event.task_id.bytes, event.occurrence_id),
        NexusEventKind::OccurrenceWithdrawn(event) => (event.task_id.bytes, event.occurrence_id),
        _ => return None,
    };

    Some(OccurrenceRef::new(task_id, occurrence_id))
}

fn reduce_occurrence(
    reference: OccurrenceRef,
    events: &[NexusEventKind],
) -> Result<ReducedOccurrence, NexusError> {
    let expected_execution_id = reference.execution_id()?;
    let mut scheduled = None;
    let mut effective_start_time_ms = None;
    let mut dispatched = None;
    let mut missed = None;
    let mut withdrawn = None;
    let mut settled = None;
    let mut saw_lifecycle_event = false;

    for event in events {
        if event_reference(event) != Some(reference) {
            continue;
        }
        saw_lifecycle_event = true;

        match event {
            NexusEventKind::OccurrenceScheduled(event) => {
                let allocation = ScheduledOccurrence {
                    reference,
                    start_time_ms: event.start_time_ms,
                    deadline_ms: event.deadline_ms.copied_option(),
                    priority_fee_percentage: event.priority_fee_percentage,
                    source: event.source,
                };
                if scheduled.replace(allocation).is_some() {
                    return Err(inconsistent(
                        reference,
                        "allocation was recorded more than once",
                    ));
                }
            }
            NexusEventKind::OccurrenceAdvertised(event) => {
                effective_start_time_ms = Some(event.start_time_ms);
            }
            NexusEventKind::OccurrenceDispatched(event) => {
                let execution_id = event.execution_id.bytes;
                if execution_id != expected_execution_id {
                    return Err(inconsistent(
                        reference,
                        format!(
                            "dispatch uses execution '{execution_id}', expected '{expected_execution_id}'"
                        ),
                    ));
                }
                if dispatched
                    .replace(DispatchRecord { execution_id })
                    .is_some()
                {
                    return Err(inconsistent(
                        reference,
                        "dispatch was recorded more than once",
                    ));
                }
            }
            NexusEventKind::OccurrenceMissed(event) => {
                if missed.replace(event.missed_at_ms).is_some() {
                    return Err(inconsistent(
                        reference,
                        "missed outcome was recorded more than once",
                    ));
                }
            }
            NexusEventKind::OccurrenceWithdrawn(event) => {
                if withdrawn.replace(event.reason).is_some() {
                    return Err(inconsistent(
                        reference,
                        "withdrawal was recorded more than once",
                    ));
                }
            }
            NexusEventKind::OccurrenceSettled(event) => {
                let execution_id = event.execution_id.bytes;
                if execution_id != expected_execution_id {
                    return Err(inconsistent(
                        reference,
                        format!(
                            "settlement uses execution '{execution_id}', expected '{expected_execution_id}'"
                        ),
                    ));
                }
                if settled.replace(event.succeeded).is_some() {
                    return Err(inconsistent(
                        reference,
                        "settlement was recorded more than once",
                    ));
                }
            }
            _ => {}
        }
    }

    if !saw_lifecycle_event {
        return Err(NexusError::OccurrenceNotFound {
            task_id: reference.task_id,
            occurrence_id: reference.occurrence_id,
        });
    }
    let scheduled =
        scheduled.ok_or_else(|| incomplete(reference, "allocation event is missing"))?;
    if settled.is_some() && dispatched.is_none() {
        return Err(inconsistent(
            reference,
            "settlement exists without dispatch",
        ));
    }
    if dispatched.is_some() && (missed.is_some() || withdrawn.is_some()) {
        return Err(inconsistent(
            reference,
            "dispatch conflicts with a nondispatch terminal outcome",
        ));
    }
    if missed.is_some() && withdrawn.is_some() {
        return Err(inconsistent(
            reference,
            "missed and withdrawn outcomes both exist",
        ));
    }
    if settled.is_some() && (missed.is_some() || withdrawn.is_some()) {
        return Err(inconsistent(
            reference,
            "settlement conflicts with another terminal outcome",
        ));
    }

    let terminal = settled
        .map(|succeeded| OccurrenceStatus::Settled { succeeded })
        .or_else(|| missed.map(|missed_at_ms| OccurrenceStatus::Missed { missed_at_ms }))
        .or_else(|| withdrawn.map(|reason| OccurrenceStatus::Withdrawn { reason }));

    Ok(ReducedOccurrence {
        scheduled,
        effective_start_time_ms,
        dispatched,
        terminal,
    })
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        source
            .downcast_ref::<tonic::Status>()
            .is_some_and(|status| status.code() == tonic::Code::NotFound)
    })
}

impl SchedulerActions {
    /// Inspects one occurrence through its durable [`Task`] history.
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn inspect_occurrence(
        &self,
        occurrence: OccurrenceRef,
    ) -> Result<OccurrenceSnapshot, NexusError> {
        let deadline = Instant::now() + DEFAULT_OCCURRENCE_WATCH_TIMEOUT;
        self.inspect_occurrence_with_state(
            occurrence,
            None,
            DEFAULT_OCCURRENCE_WATCH_POLL_INTERVAL,
            deadline,
        )
        .await
        .map(|(snapshot, _)| snapshot)
    }

    /// Watches one occurrence until scheduler lifecycle processing is complete.
    pub async fn watch_occurrence(
        &self,
        occurrence: OccurrenceRef,
        options: WatchOccurrenceOptions,
    ) -> Result<OccurrenceSnapshot, NexusError> {
        if options.poll_interval.is_zero() {
            return Err(NexusError::Configuration(
                "Occurrence watch poll interval must be greater than zero".to_string(),
            ));
        }

        let deadline = Instant::now() + options.timeout;
        let (mut snapshot, mut state) = self
            .inspect_occurrence_with_state(occurrence, None, options.poll_interval, deadline)
            .await?;

        loop {
            if snapshot.status.is_terminal() {
                return Ok(snapshot);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(NexusError::OccurrenceWatchTimedOut {
                    last_snapshot: Box::new(snapshot),
                });
            }

            let next_poll = now
                .checked_add(options.poll_interval)
                .map_or(deadline, |next| next.min(deadline));
            tokio::time::sleep_until(next_poll).await;
            if Instant::now() >= deadline {
                return Err(NexusError::OccurrenceWatchTimedOut {
                    last_snapshot: Box::new(snapshot),
                });
            }

            (snapshot, state) = self
                .inspect_occurrence_with_state(
                    occurrence,
                    Some(state),
                    options.poll_interval,
                    deadline,
                )
                .await?;
        }
    }

    /// Returns the payment cost for a dispatched occurrence.
    pub async fn occurrence_cost(
        &self,
        occurrence: OccurrenceRef,
    ) -> Result<ExecutionCostResult, NexusError> {
        let snapshot = self.inspect_occurrence(occurrence).await?;
        let execution_id = dispatched_execution_id(&snapshot)?;
        self.client.workflow().execution_cost(execution_id).await
    }

    /// Aborts expired runtime work for a dispatched occurrence.
    pub async fn abort_expired_occurrence(
        &self,
        occurrence: OccurrenceRef,
    ) -> Result<AbortExecutionResult, NexusError> {
        let snapshot = self.inspect_occurrence(occurrence).await?;
        let execution_id = dispatched_execution_id(&snapshot)?;
        self.client
            .workflow()
            .abort_expired_execution(execution_id)
            .await
    }

    /// Aborts expired runtime work with an eligible ToolGas object.
    pub async fn abort_expired_occurrence_with_tool_gas(
        &self,
        occurrence: OccurrenceRef,
        tool_gas_id: Option<sui::types::Address>,
    ) -> Result<AbortExpiredExecutionResult, NexusError> {
        let snapshot = self.inspect_occurrence(occurrence).await?;
        let execution_id = dispatched_execution_id(&snapshot)?;
        self.client
            .workflow()
            .abort_expired_execution_with_tool_gas(execution_id, tool_gas_id)
            .await
    }

    async fn inspect_occurrence_with_state(
        &self,
        occurrence: OccurrenceRef,
        previous: Option<InspectionState>,
        poll_interval: Duration,
        deadline: Instant,
    ) -> Result<(OccurrenceSnapshot, InspectionState), NexusError> {
        let task = self.fetch_task(occurrence.task_id).await?;
        let mut events = previous
            .as_ref()
            .map_or_else(Vec::new, |state| state.events.clone());
        let after_version = previous.as_ref().map(|state| state.task_version);
        let mut checkpoint = previous.as_ref().map_or(0, |state| state.checkpoint);

        if after_version.is_some_and(|version| version > task.version) {
            return Err(inconsistent(
                occurrence,
                format!(
                    "Task moved backwards from observed version {} to version {}",
                    after_version.expect("checked as some"),
                    task.version
                ),
            ));
        }

        if after_version != Some(task.version) {
            let latest = self
                .client
                .crawler()
                .get_object_update_reference(occurrence.task_id, Some(task.version))
                .await
                .map_err(NexusError::Rpc)?;
            if latest.digest != task.digest {
                return Err(inconsistent(
                    occurrence,
                    format!(
                        "Task version {} metadata digest does not match its history anchor",
                        task.version
                    ),
                ));
            }
            let updates = fetch_shared_object_history(
                self.client.crawler(),
                ObjectHistoryRequest {
                    object_name: "Task",
                    object_id: occurrence.task_id,
                    expected_type: crate::move_bindings::struct_tag::<Task>(
                        &self.client.nexus_objects,
                    ),
                    latest,
                    after_version,
                    poll_interval,
                    deadline,
                },
            )
            .await?;
            if updates.is_empty() {
                return Err(incomplete(
                    occurrence,
                    format!("Task version {} has no reconstructed update", task.version),
                ));
            }

            let event_query = NexusEventQuery::new(Arc::clone(&self.client.nexus_objects));
            for update in updates {
                checkpoint = update.checkpoint;
                for (index, event) in update.events.iter().enumerate() {
                    let decoded = event_query
                        .decode_sui_event(index as u64, update.digest, event)
                        .map_err(|error| {
                            NexusError::Parsing(anyhow::anyhow!(
                                "Could not decode event {index} from transaction '{}' while reconstructing Task '{}': {error}",
                                update.digest,
                                occurrence.task_id
                            ))
                        })?;
                    if let Some(decoded) = decoded {
                        if event_reference(&decoded.data) == Some(occurrence) {
                            events.push(decoded.data);
                        }
                    }
                }
            }
        }

        let reduced = reduce_occurrence(occurrence, &events)?;
        let (status, execution) = self
            .resolve_occurrence_status(occurrence, &task.data, &reduced)
            .await?;
        let snapshot = OccurrenceSnapshot {
            reference: occurrence,
            source: reduced.scheduled.source,
            requested_start_time_ms: reduced.scheduled.start_time_ms,
            effective_start_time_ms: reduced.effective_start_time_ms,
            deadline_ms: reduced.scheduled.deadline_ms,
            priority_fee_percentage: reduced.scheduled.priority_fee_percentage,
            status,
            execution,
            observed_task_version: task.version,
            observed_checkpoint: checkpoint,
        };
        let state = InspectionState {
            events,
            task_version: task.version,
            checkpoint,
        };

        Ok((snapshot, state))
    }

    async fn resolve_occurrence_status(
        &self,
        occurrence: OccurrenceRef,
        task: &Task,
        reduced: &ReducedOccurrence,
    ) -> Result<(OccurrenceStatus, Option<ExecutionSnapshot>), NexusError> {
        let Some(dispatch) = reduced.dispatched else {
            if let Some(terminal) = reduced.terminal {
                return Ok((terminal, None));
            }
            if task
                .scheduled_occurrence(occurrence.occurrence_id)
                .is_none()
            {
                return Err(incomplete(
                    occurrence,
                    "occurrence is absent from the current schedule without a terminal event",
                ));
            }
            let advertised = task.schedule.advertised_occurrence_id.copied_option()
                == Some(occurrence.occurrence_id);
            return Ok((
                if advertised {
                    OccurrenceStatus::Advertised
                } else {
                    OccurrenceStatus::Pending
                },
                None,
            ));
        };

        let execution = match self
            .client
            .crawler()
            .get_object::<DAGExecution>(dispatch.execution_id)
            .await
        {
            Ok(execution) => {
                if execution.data.task_id.bytes != occurrence.task_id
                    || execution.data.occurrence_id != occurrence.occurrence_id
                {
                    return Err(inconsistent(
                        occurrence,
                        "runtime object does not identify its owning occurrence",
                    ));
                }
                Some(ExecutionSnapshot::from_execution(
                    dispatch.execution_id,
                    &execution.data,
                ))
            }
            Err(error) if is_not_found(&error) => {
                Some(ExecutionSnapshot::pending(dispatch.execution_id))
            }
            Err(error) => return Err(NexusError::Rpc(error)),
        };

        if let Some(terminal) = reduced.terminal {
            return Ok((terminal, execution));
        }
        let finished = execution.as_ref().is_some_and(|execution| {
            execution.active_walks == Some(0)
                && execution.pending_abort_walks == Some(0)
                && execution.pending_settlement_walks == Some(0)
        });

        Ok((
            if finished {
                OccurrenceStatus::Finished
            } else {
                OccurrenceStatus::Executing
            },
            execution,
        ))
    }
}

fn dispatched_execution_id(
    snapshot: &OccurrenceSnapshot,
) -> Result<sui::types::Address, NexusError> {
    snapshot
        .execution
        .as_ref()
        .map(|execution| execution.execution_id)
        .ok_or(NexusError::OccurrenceNotDispatched {
            task_id: snapshot.reference.task_id,
            occurrence_id: snapshot.reference.occurrence_id,
        })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            move_bindings::{
                move_std::option::Option as MoveOption,
                scheduler::{
                    schedule::{OccurrenceSource, OccurrenceWithdrawalReason},
                    scheduler::{
                        OccurrenceDispatched,
                        OccurrenceMissed,
                        OccurrenceScheduled,
                        OccurrenceSettled,
                        OccurrenceWithdrawn,
                    },
                },
                sui_framework::object::ID,
            },
            nexus::error::NexusError,
            sui,
        },
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn reference() -> OccurrenceRef {
        OccurrenceRef::new(address("0x81"), 3)
    }

    fn scheduled() -> NexusEventKind {
        NexusEventKind::OccurrenceScheduled(OccurrenceScheduled::new(
            ID::new(reference().task_id),
            reference().occurrence_id,
            100,
            MoveOption::from_option(Some(200)),
            10,
            OccurrenceSource::Standalone,
        ))
    }

    fn dispatched(execution_id: sui::types::Address) -> NexusEventKind {
        NexusEventKind::OccurrenceDispatched(OccurrenceDispatched::new(
            ID::new(reference().task_id),
            reference().occurrence_id,
            ID::new(execution_id),
            120,
        ))
    }

    #[test]
    fn scheduled_occurrence_has_no_terminal_outcome() {
        let reduced = reduce_occurrence(reference(), &[scheduled()]).expect("history reduces");

        assert_eq!(reduced.scheduled.start_time_ms, 100);
        assert!(reduced.dispatched.is_none());
        assert!(reduced.terminal.is_none());
    }

    #[test]
    fn dispatched_occurrence_uses_the_deterministic_execution() {
        let execution_id = reference().execution_id().expect("identity derives");
        let reduced = reduce_occurrence(reference(), &[scheduled(), dispatched(execution_id)])
            .expect("history reduces");

        assert_eq!(
            reduced.dispatched.expect("dispatch").execution_id,
            execution_id
        );
    }

    #[test]
    fn successful_settlement_is_a_terminal_value() {
        let execution_id = reference().execution_id().expect("identity derives");
        let reduced = reduce_occurrence(
            reference(),
            &[
                scheduled(),
                dispatched(execution_id),
                NexusEventKind::OccurrenceSettled(OccurrenceSettled::new(
                    ID::new(reference().task_id),
                    reference().occurrence_id,
                    ID::new(execution_id),
                    true,
                )),
            ],
        )
        .expect("history reduces");

        assert!(matches!(
            reduced.terminal,
            Some(OccurrenceStatus::Settled { succeeded: true })
        ));
    }

    #[test]
    fn missed_and_withdrawn_occurrences_are_terminal_values() {
        let missed = reduce_occurrence(
            reference(),
            &[
                scheduled(),
                NexusEventKind::OccurrenceMissed(OccurrenceMissed::new(
                    ID::new(reference().task_id),
                    reference().occurrence_id,
                    250,
                )),
            ],
        )
        .expect("missed history reduces");
        assert!(matches!(
            missed.terminal,
            Some(OccurrenceStatus::Missed { missed_at_ms: 250 })
        ));

        let withdrawn = reduce_occurrence(
            reference(),
            &[
                scheduled(),
                NexusEventKind::OccurrenceWithdrawn(OccurrenceWithdrawn::new(
                    ID::new(reference().task_id),
                    reference().occurrence_id,
                    OccurrenceWithdrawalReason::TaskCanceled,
                )),
            ],
        )
        .expect("withdrawn history reduces");
        assert!(matches!(
            withdrawn.terminal,
            Some(OccurrenceStatus::Withdrawn {
                reason: OccurrenceWithdrawalReason::TaskCanceled
            })
        ));
    }

    #[test]
    fn conflicting_terminal_events_are_rejected() {
        let error = reduce_occurrence(
            reference(),
            &[
                scheduled(),
                NexusEventKind::OccurrenceMissed(OccurrenceMissed::new(
                    ID::new(reference().task_id),
                    reference().occurrence_id,
                    250,
                )),
                NexusEventKind::OccurrenceWithdrawn(OccurrenceWithdrawn::new(
                    ID::new(reference().task_id),
                    reference().occurrence_id,
                    OccurrenceWithdrawalReason::TaskCanceled,
                )),
            ],
        )
        .expect_err("conflicting history must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleInconsistent { .. }
        ));
    }

    #[test]
    fn settlement_without_dispatch_is_rejected() {
        let execution_id = reference().execution_id().expect("identity derives");
        let error = reduce_occurrence(
            reference(),
            &[
                scheduled(),
                NexusEventKind::OccurrenceSettled(OccurrenceSettled::new(
                    ID::new(reference().task_id),
                    reference().occurrence_id,
                    ID::new(execution_id),
                    true,
                )),
            ],
        )
        .expect_err("settlement without dispatch must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleInconsistent { .. }
        ));
    }

    #[test]
    fn mismatched_execution_id_is_rejected() {
        let error = reduce_occurrence(reference(), &[scheduled(), dispatched(address("0x82"))])
            .expect_err("mismatched execution must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleInconsistent { .. }
        ));
    }

    #[test]
    fn missing_scheduled_event_is_incomplete() {
        let execution_id = reference().execution_id().expect("identity derives");
        let error = reduce_occurrence(reference(), &[dispatched(execution_id)])
            .expect_err("missing allocation must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleIncomplete { .. }
        ));
    }
}
