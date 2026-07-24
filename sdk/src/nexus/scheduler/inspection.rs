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
    if dispatched.is_some() && (missed.is_some() || withdrawn.is_some()) {
        return Err(inconsistent(
            reference,
            "dispatch conflicts with a nondispatch terminal outcome",
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
                interface::{
                    agent::{ExecutionSpec, SkillSchedulePolicy},
                    graph::EntryGroup,
                    version::InterfaceVersion,
                },
                move_std::option::Option as MoveOption,
                scheduler::{
                    schedule::{
                        Occurrence,
                        OccurrenceSource,
                        OccurrenceWithdrawalReason,
                        Schedule,
                    },
                    scheduler::{
                        OccurrenceAdvertised,
                        OccurrenceDispatched,
                        OccurrenceMissed,
                        OccurrenceScheduled,
                        OccurrenceSettled,
                        OccurrenceWithdrawn,
                    },
                    task::{FailureMode, TaskController, TaskStatus},
                },
                sui_framework::{
                    object::{ID, UID},
                    object_table::ObjectTable,
                    table::Table,
                    vec_map::VecMap,
                },
            },
            nexus::{client::NexusClient, error::NexusError},
            sui,
            test_utils::sui_mocks,
        },
        serde::Serialize,
        std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
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

    fn advertised(start_time_ms: u64) -> NexusEventKind {
        NexusEventKind::OccurrenceAdvertised(OccurrenceAdvertised::new(
            ID::new(reference().task_id),
            reference().occurrence_id,
            start_time_ms,
            MoveOption::from_option(Some(200)),
            10,
            OccurrenceSource::Standalone,
        ))
    }

    fn missed() -> NexusEventKind {
        NexusEventKind::OccurrenceMissed(OccurrenceMissed::new(
            ID::new(reference().task_id),
            reference().occurrence_id,
            250,
        ))
    }

    fn withdrawn() -> NexusEventKind {
        NexusEventKind::OccurrenceWithdrawn(OccurrenceWithdrawn::new(
            ID::new(reference().task_id),
            reference().occurrence_id,
            OccurrenceWithdrawalReason::TaskCanceled,
        ))
    }

    fn settled(execution_id: sui::types::Address, succeeded: bool) -> NexusEventKind {
        NexusEventKind::OccurrenceSettled(OccurrenceSettled::new(
            ID::new(reference().task_id),
            reference().occurrence_id,
            ID::new(execution_id),
            succeeded,
        ))
    }

    fn task_with_schedule(pending: Vec<Occurrence>, advertised_occurrence_id: Option<u64>) -> Task {
        Task {
            id: UID::new(reference().task_id),
            controller: TaskController::Address {
                pos0: address("0x91"),
            },
            status: TaskStatus::Active,
            failure_mode: FailureMode::Continue,
            operation: ExecutionSpec::new(
                ID::new(address("0x92")),
                ID::new(address("0x93")),
                7,
                InterfaceVersion::new(1),
                ID::new(address("0x94")),
                EntryGroup::new("default"),
                VecMap { contents: vec![] },
                address("0x91"),
            ),
            schedule_policy: SkillSchedulePolicy::Once,
            schedule: Schedule::new(
                pending,
                MoveOption::from_option(None),
                MoveOption::from_option(advertised_occurrence_id),
                4,
                0,
                MoveOption::from_option(None),
            ),
            in_flight: Table::new(address("0x95"), 0),
            registration: MoveOption::from_option(None),
        }
    }

    fn scheduled_task_occurrence() -> Occurrence {
        Occurrence::new(
            reference().occurrence_id,
            100,
            MoveOption::from_option(Some(200)),
            10,
            OccurrenceSource::Standalone,
        )
    }

    fn empty_object_table<T0, T1>(id: sui::types::Address) -> ObjectTable<T0, T1> {
        ObjectTable {
            id: UID::new(id),
            size: 0,
            phantom_t0: std::marker::PhantomData,
            phantom_t1: std::marker::PhantomData,
        }
    }

    fn execution(
        task_id: sui::types::Address,
        occurrence_id: u64,
        active_walks: u64,
    ) -> DAGExecution {
        DAGExecution {
            id: UID::new(
                OccurrenceRef::new(task_id, occurrence_id)
                    .execution_id()
                    .expect("execution identity derives"),
            ),
            dag: ID::new(address("0x92")),
            entry_group: EntryGroup::new("default"),
            invoker: address("0x91"),
            created_at: 125,
            priority_fee_percentage: 10,
            agent_id: ID::new(address("0x93")),
            skill_id: 7,
            interface_version: InterfaceVersion::new(1),
            task_id: ID::new(task_id),
            occurrence_id,
            last_request_for_execution_emitted_at_digest: vec![],
            last_request_for_execution_leaders: vec![],
            network: ID::new(address("0x94")),
            evaluations: empty_object_table(address("0x96")),
            terminal_records: VecMap { contents: vec![] },
            submission_failure_records: VecMap { contents: vec![] },
            pending_retry_handoff_cap_ids: VecMap { contents: vec![] },
            walk_request_authorities: VecMap { contents: vec![] },
            pending_gas_settlements: VecMap { contents: vec![] },
            walks: vec![],
            active_walks,
            pending_abort_walks: 0,
            pending_settlement_walks: 0,
            successful_walks: 2,
            failed_walks: 1,
            aborted_walks: 3,
            consumed_walks: 0,
            cancelled_walks: 0,
        }
    }

    #[derive(Clone)]
    struct TaskUpdateFixture {
        object_ref: sui::types::ObjectReference,
        previous_transaction: sui::types::Digest,
        task_bcs: Vec<u8>,
        input_state: sui::types::ObjectIn,
        events: Vec<NexusEventKind>,
        checkpoint: u64,
    }

    impl TaskUpdateFixture {
        fn new(
            version: u64,
            digest_byte: u8,
            transaction_byte: u8,
            task: &Task,
            input_state: sui::types::ObjectIn,
            events: Vec<NexusEventKind>,
        ) -> Self {
            Self {
                object_ref: sui::types::ObjectReference::new(
                    reference().task_id,
                    version,
                    sui::types::Digest::from([digest_byte; 32]),
                ),
                previous_transaction: sui::types::Digest::from([transaction_byte; 32]),
                task_bcs: bcs::to_bytes(task).expect("Task serializes"),
                input_state,
                events,
                checkpoint: version + 10,
            }
        }
    }

    #[derive(Clone)]
    struct ObjectReply {
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        object_type: sui::types::StructTag,
        previous_transaction: Option<sui::types::Digest>,
        contents: Vec<u8>,
        expected_requested_version: Option<u64>,
    }

    #[derive(Serialize)]
    struct EventWrapper<T> {
        event: T,
    }

    fn scheduler_grpc_event(
        objects: &crate::types::NexusObjects,
        event: NexusEventKind,
    ) -> sui::grpc::Event {
        let event_name = event.name();
        let contents = match event {
            NexusEventKind::OccurrenceAdvertised(event) => {
                bcs::to_bytes(&EventWrapper { event }).expect("event serializes")
            }
            NexusEventKind::OccurrenceDispatched(event) => {
                bcs::to_bytes(&EventWrapper { event }).expect("event serializes")
            }
            NexusEventKind::OccurrenceMissed(event) => {
                bcs::to_bytes(&EventWrapper { event }).expect("event serializes")
            }
            NexusEventKind::OccurrenceScheduled(event) => {
                bcs::to_bytes(&EventWrapper { event }).expect("event serializes")
            }
            NexusEventKind::OccurrenceSettled(event) => {
                bcs::to_bytes(&EventWrapper { event }).expect("event serializes")
            }
            NexusEventKind::OccurrenceWithdrawn(event) => {
                bcs::to_bytes(&EventWrapper { event }).expect("event serializes")
            }
            _ => panic!("unsupported scheduler test event"),
        };
        let mut grpc_event = sui::grpc::Event::default();
        grpc_event.set_package_id(objects.scheduler_pkg_id);
        grpc_event.set_module("scheduler".to_string());
        grpc_event.set_sender(sui::types::Address::ZERO);
        grpc_event.set_event_type(format!(
            "{}::event::EventWrapper<{}::scheduler::{event_name}>",
            objects.primitives_pkg_id, objects.scheduler_pkg_id,
        ));
        grpc_event.set_contents(contents);
        grpc_event
    }

    async fn actions_for_history(
        objects: crate::types::NexusObjects,
        updates: Vec<TaskUpdateFixture>,
        execution_object: Option<(sui::types::ObjectReference, DAGExecution)>,
    ) -> SchedulerActions {
        let task_type = crate::move_bindings::struct_tag::<Task>(&objects);
        let mut object_replies = Vec::new();
        for update in &updates {
            for expected_requested_version in [None, Some(update.object_ref.version())] {
                object_replies.push(ObjectReply {
                    object_ref: update.object_ref.clone(),
                    owner: sui::types::Owner::Shared(1),
                    object_type: task_type.clone(),
                    previous_transaction: Some(update.previous_transaction),
                    contents: update.task_bcs.clone(),
                    expected_requested_version,
                });
            }
        }
        if let Some((execution_ref, execution)) = execution_object {
            object_replies.push(ObjectReply {
                object_ref: execution_ref,
                owner: sui::types::Owner::Shared(1),
                object_type: crate::move_bindings::struct_tag::<DAGExecution>(&objects),
                previous_transaction: None,
                contents: bcs::to_bytes(&execution).expect("DAGExecution serializes"),
                expected_requested_version: None,
            });
        }

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let object_replies = Arc::new(object_replies);
        let object_call = Arc::new(AtomicUsize::new(0));
        let replies_for_object = Arc::clone(&object_replies);
        let call_for_object = Arc::clone(&object_call);
        ledger_service
            .expect_get_object()
            .times(object_replies.len())
            .returning(move |request| {
                let index = call_for_object.fetch_add(1, Ordering::SeqCst);
                let reply = &replies_for_object[index];
                assert_eq!(
                    request.get_ref().object_id.as_deref(),
                    Some(reply.object_ref.object_id().to_string().as_str()),
                );
                assert_eq!(request.get_ref().version, reply.expected_requested_version);

                let mut object = sui::grpc::Object::default();
                object.set_object_id(*reply.object_ref.object_id());
                object.set_owner(sui::grpc::Owner::from(reply.owner));
                object.set_object_type(reply.object_type.to_string());
                object.set_version(reply.object_ref.version());
                object.set_digest(*reply.object_ref.digest());
                if let Some(previous_transaction) = reply.previous_transaction {
                    object.set_previous_transaction(previous_transaction.to_string());
                }
                let mut contents = sui::grpc::Bcs::default();
                contents.set_name(reply.object_type.to_string());
                contents.set_value(reply.contents.clone());
                object.contents = Some(contents);

                let mut response = sui::grpc::GetObjectResponse::default();
                response.set_object(object);
                Ok(tonic::Response::new(response))
            });

        let updates_for_transaction = Arc::new(updates);
        let objects_for_transaction = objects.clone();
        ledger_service
            .expect_get_transaction()
            .times(updates_for_transaction.len())
            .returning(move |request| {
                let requested_digest = request
                    .get_ref()
                    .digest_opt()
                    .expect("transaction digest is requested");
                let update = updates_for_transaction
                    .iter()
                    .find(|update| update.previous_transaction.to_string() == requested_digest)
                    .expect("requested Task update exists");
                let created = matches!(&update.input_state, sui::types::ObjectIn::NotExist);
                let effects = sui::types::TransactionEffects::V2(Box::new(
                    sui::types::TransactionEffectsV2 {
                        status: sui::types::ExecutionStatus::Success,
                        epoch: 1,
                        gas_used: sui::types::GasCostSummary {
                            computation_cost: 0,
                            storage_cost: 0,
                            storage_rebate: 0,
                            non_refundable_storage_fee: 0,
                        },
                        transaction_digest: update.previous_transaction,
                        gas_object_index: None,
                        events_digest: None,
                        dependencies: vec![],
                        lamport_version: update.object_ref.version(),
                        changed_objects: vec![sui::types::ChangedObject {
                            object_id: reference().task_id,
                            input_state: update.input_state.clone(),
                            output_state: sui::types::ObjectOut::ObjectWrite {
                                digest: *update.object_ref.digest(),
                                owner: sui::types::Owner::Shared(1),
                            },
                            id_operation: if created {
                                sui::types::IdOperation::Created
                            } else {
                                sui::types::IdOperation::None
                            },
                        }],
                        unchanged_consensus_objects: vec![],
                        auxiliary_data_digest: None,
                    },
                ));
                let mut grpc_effects = sui::grpc::TransactionEffects::default();
                grpc_effects.set_bcs(bcs::to_bytes(&effects).expect("effects serialize"));
                let mut grpc_events = sui::grpc::TransactionEvents::default();
                grpc_events.set_events(
                    update
                        .events
                        .clone()
                        .into_iter()
                        .map(|event| scheduler_grpc_event(&objects_for_transaction, event))
                        .collect(),
                );
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(update.previous_transaction);
                transaction.set_checkpoint(update.checkpoint);
                transaction.set_effects(grpc_effects);
                transaction.set_events(grpc_events);
                let mut response = sui::grpc::GetTransactionResponse::default();
                response.set_transaction(transaction);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let private_key = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        NexusClient::builder()
            .with_private_key(private_key)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(objects)
            .with_address_balance_gas(1_000)
            .build()
            .await
            .expect("mock Nexus client builds")
            .scheduler()
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

    #[test]
    fn occurrence_status_terminality_and_watch_defaults_are_stable() {
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
            OccurrenceStatus::Missed { missed_at_ms: 1 },
            OccurrenceStatus::Withdrawn {
                reason: OccurrenceWithdrawalReason::TaskCanceled,
            },
        ] {
            assert!(status.is_terminal());
        }

        let options = WatchOccurrenceOptions::default();
        assert_eq!(options.timeout, DEFAULT_OCCURRENCE_WATCH_TIMEOUT);
        assert_eq!(
            options.poll_interval,
            DEFAULT_OCCURRENCE_WATCH_POLL_INTERVAL
        );
    }

    #[test]
    fn advertisement_updates_effective_start_and_unrelated_events_are_ignored() {
        let unrelated = NexusEventKind::OccurrenceScheduled(OccurrenceScheduled::new(
            ID::new(address("0x99")),
            reference().occurrence_id,
            1,
            MoveOption::from_option(None),
            0,
            OccurrenceSource::Standalone,
        ));
        let reduced = reduce_occurrence(
            reference(),
            &[unrelated, scheduled(), advertised(125), advertised(150)],
        )
        .expect("history reduces");

        assert_eq!(reduced.effective_start_time_ms, Some(150));
    }

    #[test]
    fn missing_occurrence_history_is_not_found() {
        let error = reduce_occurrence(reference(), &[]).expect_err("empty history must fail");

        assert!(matches!(error, NexusError::OccurrenceNotFound { .. }));
    }

    #[test]
    fn duplicate_lifecycle_records_are_rejected() {
        let execution_id = reference().execution_id().expect("identity derives");
        let histories = [
            vec![scheduled(), scheduled()],
            vec![
                scheduled(),
                dispatched(execution_id),
                dispatched(execution_id),
            ],
            vec![scheduled(), missed(), missed()],
            vec![scheduled(), withdrawn(), withdrawn()],
            vec![
                scheduled(),
                dispatched(execution_id),
                settled(execution_id, true),
                settled(execution_id, true),
            ],
        ];

        for history in histories {
            let error =
                reduce_occurrence(reference(), &history).expect_err("duplicate record must fail");
            assert!(matches!(
                error,
                NexusError::OccurrenceLifecycleInconsistent { .. }
            ));
        }
    }

    #[test]
    fn dispatch_and_terminal_conflicts_are_rejected() {
        let execution_id = reference().execution_id().expect("identity derives");
        for history in [
            vec![scheduled(), dispatched(execution_id), missed()],
            vec![scheduled(), dispatched(execution_id), withdrawn()],
            vec![
                scheduled(),
                dispatched(execution_id),
                settled(execution_id, true),
                missed(),
            ],
        ] {
            let error = reduce_occurrence(reference(), &history).expect_err("conflict must fail");
            assert!(matches!(
                error,
                NexusError::OccurrenceLifecycleInconsistent { .. }
            ));
        }
    }

    #[test]
    fn settlement_with_mismatched_execution_is_rejected() {
        let execution_id = reference().execution_id().expect("identity derives");
        let error = reduce_occurrence(
            reference(),
            &[
                scheduled(),
                dispatched(execution_id),
                settled(address("0x82"), true),
            ],
        )
        .expect_err("mismatched settlement must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleInconsistent { .. }
        ));
    }

    #[test]
    fn transaction_not_found_detection_is_specific() {
        let missing = anyhow::Error::new(tonic::Status::not_found("missing"));
        let unavailable = anyhow::Error::new(tonic::Status::unavailable("unavailable"));
        let plain = anyhow::anyhow!("plain error");

        assert!(is_not_found(&missing));
        assert!(!is_not_found(&unavailable));
        assert!(!is_not_found(&plain));
    }

    #[tokio::test]
    async fn inspection_reconstructs_an_advertised_occurrence_from_task_updates() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = task_with_schedule(
            vec![scheduled_task_occurrence()],
            Some(reference().occurrence_id),
        );
        let update = TaskUpdateFixture::new(
            1,
            1,
            11,
            &task,
            sui::types::ObjectIn::NotExist,
            vec![scheduled(), advertised(125)],
        );
        let actions = actions_for_history(objects, vec![update], None).await;

        let snapshot = actions
            .inspect_occurrence(reference())
            .await
            .expect("occurrence inspection succeeds");

        assert_eq!(snapshot.reference, reference());
        assert_eq!(snapshot.source, OccurrenceSource::Standalone);
        assert_eq!(snapshot.requested_start_time_ms, 100);
        assert_eq!(snapshot.effective_start_time_ms, Some(125));
        assert_eq!(snapshot.deadline_ms, Some(200));
        assert_eq!(snapshot.priority_fee_percentage, 10);
        assert_eq!(snapshot.status, OccurrenceStatus::Advertised);
        assert!(snapshot.execution.is_none());
        assert_eq!(snapshot.observed_task_version, 1);
        assert_eq!(snapshot.observed_checkpoint, 11);
    }

    #[tokio::test]
    async fn watch_reuses_task_history_and_observes_a_terminal_update() {
        let objects = sui_mocks::mock_nexus_objects();
        let first_task = task_with_schedule(vec![scheduled_task_occurrence()], None);
        let first = TaskUpdateFixture::new(
            1,
            1,
            11,
            &first_task,
            sui::types::ObjectIn::NotExist,
            vec![scheduled()],
        );
        let second_task = task_with_schedule(vec![], None);
        let second = TaskUpdateFixture::new(
            2,
            2,
            12,
            &second_task,
            sui::types::ObjectIn::Exist {
                version: 1,
                digest: *first.object_ref.digest(),
                owner: sui::types::Owner::Shared(1),
            },
            vec![withdrawn()],
        );
        let actions = actions_for_history(objects, vec![first, second], None).await;

        let snapshot = actions
            .watch_occurrence(
                reference(),
                WatchOccurrenceOptions {
                    timeout: Duration::from_secs(1),
                    poll_interval: Duration::from_millis(1),
                },
            )
            .await
            .expect("watch observes the terminal update");

        assert_eq!(
            snapshot.status,
            OccurrenceStatus::Withdrawn {
                reason: OccurrenceWithdrawalReason::TaskCanceled,
            }
        );
        assert_eq!(snapshot.observed_task_version, 2);
        assert_eq!(snapshot.observed_checkpoint, 12);
    }

    async fn inspect_dispatched_occurrence(active_walks: u64) -> OccurrenceSnapshot {
        let objects = sui_mocks::mock_nexus_objects();
        let task = task_with_schedule(vec![], None);
        let execution_id = reference().execution_id().expect("identity derives");
        let update = TaskUpdateFixture::new(
            1,
            1,
            11,
            &task,
            sui::types::ObjectIn::NotExist,
            vec![scheduled(), dispatched(execution_id)],
        );
        let execution_ref =
            sui::types::ObjectReference::new(execution_id, 1, sui::types::Digest::from([7; 32]));
        let actions = actions_for_history(
            objects,
            vec![update],
            Some((
                execution_ref,
                execution(reference().task_id, reference().occurrence_id, active_walks),
            )),
        )
        .await;

        actions
            .inspect_occurrence(reference())
            .await
            .expect("dispatched occurrence inspection succeeds")
    }

    #[tokio::test]
    async fn dispatched_runtime_objects_distinguish_executing_from_finished() {
        let executing = inspect_dispatched_occurrence(1).await;
        assert_eq!(executing.status, OccurrenceStatus::Executing);
        let executing_snapshot = executing.execution.as_ref().expect("execution snapshot");
        assert_eq!(executing_snapshot.created_at_ms, Some(125));
        assert_eq!(executing_snapshot.active_walks, Some(1));
        assert_eq!(executing_snapshot.successful_walks, Some(2));
        assert_eq!(executing_snapshot.failed_walks, Some(1));
        assert_eq!(executing_snapshot.aborted_walks, Some(3));
        assert_eq!(
            dispatched_execution_id(&executing).expect("execution was dispatched"),
            reference().execution_id().expect("identity derives")
        );

        let finished = inspect_dispatched_occurrence(0).await;
        assert_eq!(finished.status, OccurrenceStatus::Finished);
        let mut undispatched = finished;
        undispatched.execution = None;
        assert!(matches!(
            dispatched_execution_id(&undispatched),
            Err(NexusError::OccurrenceNotDispatched { .. })
        ));
    }

    #[tokio::test]
    async fn inspection_rejects_a_runtime_object_owned_by_another_occurrence() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = task_with_schedule(vec![], None);
        let execution_id = reference().execution_id().expect("identity derives");
        let update = TaskUpdateFixture::new(
            1,
            1,
            11,
            &task,
            sui::types::ObjectIn::NotExist,
            vec![scheduled(), dispatched(execution_id)],
        );
        let execution_ref =
            sui::types::ObjectReference::new(execution_id, 1, sui::types::Digest::from([7; 32]));
        let actions = actions_for_history(
            objects,
            vec![update],
            Some((
                execution_ref,
                execution(address("0x98"), reference().occurrence_id, 1),
            )),
        )
        .await;

        let error = actions
            .inspect_occurrence(reference())
            .await
            .expect_err("foreign runtime object must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleInconsistent { .. }
        ));
    }

    #[tokio::test]
    async fn inspection_rejects_an_occurrence_missing_from_the_live_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = task_with_schedule(vec![], None);
        let update = TaskUpdateFixture::new(
            1,
            1,
            11,
            &task,
            sui::types::ObjectIn::NotExist,
            vec![scheduled()],
        );
        let actions = actions_for_history(objects, vec![update], None).await;

        let error = actions
            .inspect_occurrence(reference())
            .await
            .expect_err("missing live occurrence must fail");

        assert!(matches!(
            error,
            NexusError::OccurrenceLifecycleIncomplete { .. }
        ));
    }
}
