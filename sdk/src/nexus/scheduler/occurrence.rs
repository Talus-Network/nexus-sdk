use {
    super::{mutation_receipt, occurrence_source, resolve, settlement_receipt, withdrawal_reason},
    crate::{
        move_bindings::{
            derive_task_execution_id,
            scheduler::task::{
                OccurrenceRecord,
                OccurrenceRecordKey,
                OccurrenceState,
                TaskInnerV1 as MoveTaskInnerV1,
            },
            workflow::execution::DAGExecutionInnerV1,
        },
        nexus::client::NexusClient,
        scheduler::{
            AbortReceipt,
            ExecutionObservation,
            ExecutionSnapshot,
            OccurrenceCost,
            OccurrenceRef,
            OccurrenceSnapshot,
            OccurrenceStatus,
            SchedulerError,
            TaskMutationReceipt,
            TransactionReference,
            WatchOptions,
        },
        sui,
        transactions::scheduler::{compile_expire_occurrence_ptb, compile_settle_occurrence_ptb},
        types::NexusContext,
    },
    tokio::time::Instant,
};

/// Stateful operations and object inspection for one occurrence.
#[derive(Clone)]
pub struct OccurrenceHandle {
    client: NexusClient,
    reference: OccurrenceRef,
}

impl OccurrenceHandle {
    pub(super) const fn new(client: NexusClient, reference: OccurrenceRef) -> Self {
        Self { client, reference }
    }

    /// Returns the stable occurrence identity.
    pub const fn reference(&self) -> OccurrenceRef {
        self.reference
    }

    /// Reads the permanent occurrence record and optional runtime object.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::TaskNotFound`] when the Task is absent,
    /// [`SchedulerError::OccurrenceNotFound`] when its record is absent, or an
    /// invariant error when stored identities disagree.
    pub async fn snapshot(&self) -> Result<OccurrenceSnapshot, SchedulerError> {
        self.snapshot_with(&self.client).await
    }

    async fn snapshot_with(
        &self,
        client: &NexusClient,
    ) -> Result<OccurrenceSnapshot, SchedulerError> {
        let task = resolve::fetch_task(client, self.reference.task_id()).await?;
        self.snapshot_from_task(client, &task).await
    }

    async fn snapshot_from_task(
        &self,
        client: &NexusClient,
        task: &resolve::FetchedTask,
    ) -> Result<OccurrenceSnapshot, SchedulerError> {
        let record = self.record_with(client, &task.context).await?;
        let execution_id = state_execution_id(&record.state);
        let execution = match execution_id {
            Some(execution_id) => {
                resolve::fetch_execution(client, &task.context, execution_id).await?
            }
            None => None,
        };

        snapshot_from_record(
            self.reference.task_id(),
            &task.object.data,
            self.reference.occurrence_id(),
            &record,
            execution.as_ref().map(|response| &response.data),
            task.object.version,
        )
    }

    /// Polls object state until scheduler processing is terminal.
    ///
    /// Terminal states are settlement, a missed deadline, and withdrawal.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::WatchTimedOut`] with the latest snapshot when
    /// the timeout elapses. Invalid polling options and object read failures
    /// are returned directly.
    pub async fn watch(&self, options: WatchOptions) -> Result<OccurrenceSnapshot, SchedulerError> {
        self.watch_until(options, |snapshot| snapshot.status().is_terminal())
            .await
    }

    /// Polls object state until `stop` accepts a snapshot.
    ///
    /// This permits callers to stop at a state that requires their action,
    /// such as [`OccurrenceStatus::Finished`], without changing the terminal
    /// scheduler lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::WatchTimedOut`] with the latest snapshot when
    /// the timeout elapses. Invalid polling options and object read failures
    /// are returned directly.
    pub async fn watch_until<F>(
        &self,
        options: WatchOptions,
        mut stop: F,
    ) -> Result<OccurrenceSnapshot, SchedulerError>
    where
        F: FnMut(&OccurrenceSnapshot) -> bool,
    {
        if options.poll_interval().is_zero() {
            return Err(SchedulerError::InvalidRequest {
                message: "watch poll interval must be greater than zero".to_owned(),
            });
        }
        let deadline = Instant::now()
            .checked_add(options.timeout())
            .ok_or_else(|| SchedulerError::InvalidRequest {
                message: "watch timeout exceeds the supported duration".to_owned(),
            })?;
        let mut snapshot = self.snapshot_with(&self.client).await?;

        loop {
            if stop(&snapshot) {
                return Ok(snapshot);
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(SchedulerError::WatchTimedOut {
                    last_snapshot: Box::new(snapshot),
                });
            }
            let wake = now
                .checked_add(options.poll_interval())
                .map_or(deadline, |wake| wake.min(deadline));
            tokio::time::sleep_until(wake).await;
            if Instant::now() >= deadline {
                return Err(SchedulerError::WatchTimedOut {
                    last_snapshot: Box::new(snapshot),
                });
            }
            snapshot = self.snapshot_with(&self.client).await?;
        }
    }

    /// Records an advertised occurrence as missed after its deadline.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the Task cannot be read or the
    /// transaction cannot be built, submitted, or confirmed.
    pub async fn expire(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task =
            resolve::fetch_task_with_roots(&self.client, self.reference.task_id(), &[]).await?;
        let runtime = self
            .client
            .runtime_context(&[])
            .await
            .map_err(SchedulerError::from)?;
        let transaction = compile_expire_occurrence_ptb(
            &runtime,
            &task.object.object_ref(),
            self.reference.occurrence_id(),
        )?;
        let sender = self.client.owner().map_err(SchedulerError::from)?;
        let executed = self
            .client
            .submit_transaction(transaction, sender)
            .await
            .map_err(SchedulerError::from)?;
        mutation_receipt(executed, self.reference.task_id())
    }

    /// Settles a finished runtime object into its Task record.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::OccurrenceNotReadyForSettlement`] unless the
    /// occurrence is finished, or another [`SchedulerError`] when object reads
    /// or submission fail.
    pub async fn settle(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task =
            resolve::fetch_task_with_roots(&self.client, self.reference.task_id(), &[]).await?;
        let snapshot = self.snapshot_from_task(&self.client, &task).await?;
        let execution_id = settlement_execution_id(&snapshot)?;
        let execution = resolve::fetch_execution(&self.client, &task.context, execution_id)
            .await?
            .ok_or_else(|| SchedulerError::InconsistentChainState {
                message: format!(
                    "dispatched occurrence '{}:{}' has no runtime object '{}'",
                    self.reference.task_id(),
                    self.reference.occurrence_id(),
                    execution_id
                ),
            })?;
        let runtime = self
            .client
            .runtime_context(&[])
            .await
            .map_err(SchedulerError::from)?;
        let transaction = compile_settle_occurrence_ptb(
            &runtime,
            &task.object.object_ref(),
            &execution.object_ref(),
        )?;
        let sender = self.client.owner().map_err(SchedulerError::from)?;
        let executed = self
            .client
            .submit_transaction(transaction, sender)
            .await
            .map_err(SchedulerError::from)?;
        settlement_receipt(executed, self.reference, execution_id)
    }

    /// Reads payment accounting for the dispatched runtime object.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::OccurrenceNotDispatched`] before dispatch,
    /// or a transport error when payment state cannot be read.
    pub async fn cost(&self) -> Result<OccurrenceCost, SchedulerError> {
        let execution_id = dispatched_execution_id(&self.snapshot_with(&self.client).await?)?;
        let cost = self
            .client
            .workflow()
            .execution_cost(execution_id)
            .await
            .map_err(SchedulerError::from)?;
        Ok(OccurrenceCost {
            payment_id: cost.payment_id,
            max_budget_mist: cost.max_budget_mist,
            locked_budget_mist: cost.locked_budget_mist,
            consumed_mist: cost.consumed,
            outstanding_locks: cost.outstanding_locks,
            accomplished: cost.accomplished,
            refunded: cost.refunded,
        })
    }

    /// Aborts expired runtime work.
    ///
    /// Supplying `invocation_id` refunds that exact Invocation before abort.
    /// Without one, the unlocked workflow abort path is used.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::OccurrenceNotDispatched`] before dispatch,
    /// or another scheduler error when candidate resolution or submission
    /// fails.
    pub async fn abort_expired(
        &self,
        invocation_id: Option<sui::types::Address>,
    ) -> Result<AbortReceipt, SchedulerError> {
        let client = &self.client;
        let execution_id = dispatched_execution_id(&self.snapshot_with(client).await?)?;
        let transaction = if let Some(invocation_id) = invocation_id {
            let result = client
                .workflow()
                .abort_expired_execution_with_invocation(execution_id, Some(invocation_id))
                .await
                .map_err(SchedulerError::from)?;
            TransactionReference::new(result.tx_digest, result.tx_checkpoint)
        } else {
            let result = client
                .workflow()
                .abort_expired_execution(execution_id)
                .await
                .map_err(SchedulerError::from)?;
            TransactionReference::new(result.tx_digest, result.tx_checkpoint)
        };
        Ok(AbortReceipt::new(transaction, self.reference, execution_id))
    }

    async fn record_with(
        &self,
        client: &NexusClient,
        context: &NexusContext,
    ) -> Result<OccurrenceRecord, SchedulerError> {
        let key = OccurrenceRecordKey::new(self.reference.occurrence_id());
        let key_type = crate::move_bindings::type_tag::<OccurrenceRecordKey>(context);
        client
            .crawler()
            .get_dynamic_field_by_key::<OccurrenceRecordKey, OccurrenceRecord>(
                self.reference.task_id(),
                key,
                &key_type,
            )
            .await
            .map_err(SchedulerError::transport)?
            .ok_or(SchedulerError::OccurrenceNotFound {
                task_id: self.reference.task_id(),
                occurrence_id: self.reference.occurrence_id(),
            })
    }
}

pub(super) fn snapshot_from_record(
    task_id: sui::types::Address,
    task: &MoveTaskInnerV1,
    occurrence_id: u64,
    record: &OccurrenceRecord,
    execution: Option<&DAGExecutionInnerV1>,
    task_version: sui::types::Version,
) -> Result<OccurrenceSnapshot, SchedulerError> {
    if record.occurrence.id != occurrence_id {
        return Err(SchedulerError::InconsistentChainState {
            message: format!(
                "occurrence record key '{occurrence_id}' contains occurrence '{}'",
                record.occurrence.id
            ),
        });
    }

    let expected_execution_id =
        derive_task_execution_id(task_id, occurrence_id).map_err(|error| {
            SchedulerError::InconsistentChainState {
                message: format!(
                    "could not derive the runtime identity for occurrence \
                     '{task_id}:{occurrence_id}': {error}"
                ),
            }
        })?;
    if let Some(stored_execution_id) = state_execution_id(&record.state) {
        if stored_execution_id != expected_execution_id {
            return Err(SchedulerError::InconsistentChainState {
                message: format!(
                    "occurrence '{task_id}:{occurrence_id}' stores runtime identity \
                     '{stored_execution_id}', expected '{expected_execution_id}'"
                ),
            });
        }
    }
    if let Some(execution) = execution {
        if execution.task_id.bytes != task_id || execution.occurrence_id != occurrence_id {
            return Err(SchedulerError::InconsistentChainState {
                message: format!(
                    "runtime object '{expected_execution_id}' does not identify occurrence \
                     '{task_id}:{occurrence_id}'"
                ),
            });
        }
    }

    let execution_snapshot = state_execution_id(&record.state).map(|execution_id| {
        execution.map_or_else(
            || ExecutionSnapshot::unavailable(execution_id),
            |execution| execution_snapshot(execution_id, execution),
        )
    });
    let advertised = task.schedule.advertised_occurrence_id.copied_option() == Some(occurrence_id);
    let (status, dispatched_at_ms, settled_at_ms) = project_lifecycle(
        &record.state,
        advertised,
        execution.is_some_and(execution_finished),
    );

    Ok(OccurrenceSnapshot {
        reference: OccurrenceRef::new(task_id, occurrence_id),
        source: occurrence_source(record.occurrence.source),
        requested_start_time_ms: record.occurrence.start_time_ms,
        effective_start_time_ms: record.last_effective_start_time_ms.copied_option(),
        deadline_ms: record.occurrence.deadline_ms.copied_option(),
        priority_fee_percentage: record.occurrence.priority_fee_percentage,
        dispatched_at_ms,
        settled_at_ms,
        status,
        execution: execution_snapshot,
        observed_task_version: task_version,
    })
}

fn project_lifecycle(
    state: &OccurrenceState,
    advertised: bool,
    execution_finished: bool,
) -> (OccurrenceStatus, Option<u64>, Option<u64>) {
    match state {
        OccurrenceState::Scheduled => (
            if advertised {
                OccurrenceStatus::Advertised
            } else {
                OccurrenceStatus::Pending
            },
            None,
            None,
        ),
        OccurrenceState::Dispatched {
            dispatched_at_ms, ..
        } => (
            if execution_finished {
                OccurrenceStatus::Finished
            } else {
                OccurrenceStatus::Executing
            },
            Some(*dispatched_at_ms),
            None,
        ),
        OccurrenceState::Missed { missed_at_ms } => (
            OccurrenceStatus::Missed {
                missed_at_ms: *missed_at_ms,
            },
            None,
            None,
        ),
        OccurrenceState::Withdrawn { reason } => (
            OccurrenceStatus::Withdrawn {
                reason: withdrawal_reason(*reason),
            },
            None,
            None,
        ),
        OccurrenceState::Settled {
            dispatched_at_ms,
            settled_at_ms,
            succeeded,
            ..
        } => (
            OccurrenceStatus::Settled {
                succeeded: *succeeded,
            },
            Some(*dispatched_at_ms),
            Some(*settled_at_ms),
        ),
    }
}

pub(super) fn state_execution_id(state: &OccurrenceState) -> Option<sui::types::Address> {
    match state {
        OccurrenceState::Dispatched { execution_id, .. }
        | OccurrenceState::Settled { execution_id, .. } => Some(execution_id.bytes),
        OccurrenceState::Scheduled
        | OccurrenceState::Missed { .. }
        | OccurrenceState::Withdrawn { .. } => None,
    }
}

fn execution_snapshot(
    execution_id: sui::types::Address,
    execution: &DAGExecutionInnerV1,
) -> ExecutionSnapshot {
    ExecutionSnapshot::observed(
        execution_id,
        ExecutionObservation {
            created_at_ms: execution.created_at,
            active_walks: execution.active_walks,
            pending_abort_walks: execution.pending_abort_walks,
            pending_settlement_walks: execution.pending_settlement_walks,
            successful_walks: execution.successful_walks,
            failed_walks: execution.failed_walks,
            aborted_walks: execution.aborted_walks,
        },
    )
}

fn execution_finished(execution: &DAGExecutionInnerV1) -> bool {
    execution.active_walks == 0
        && execution.pending_abort_walks == 0
        && execution.pending_settlement_walks == 0
}

fn dispatched_execution_id(
    snapshot: &OccurrenceSnapshot,
) -> Result<sui::types::Address, SchedulerError> {
    snapshot
        .execution()
        .map(ExecutionSnapshot::execution_id)
        .ok_or(SchedulerError::OccurrenceNotDispatched {
            task_id: snapshot.reference().task_id(),
            occurrence_id: snapshot.reference().occurrence_id(),
        })
}

/// Returns the runtime identity for [`OccurrenceStatus::Finished`].
///
/// The identity survives settlement, so status remains authoritative.
fn settlement_execution_id(
    snapshot: &OccurrenceSnapshot,
) -> Result<sui::types::Address, SchedulerError> {
    if snapshot.status() != OccurrenceStatus::Finished {
        return Err(SchedulerError::OccurrenceNotReadyForSettlement {
            task_id: snapshot.reference().task_id(),
            occurrence_id: snapshot.reference().occurrence_id(),
            observed: snapshot.status(),
        });
    }
    dispatched_execution_id(snapshot)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                scheduler::schedule::OccurrenceWithdrawalReason,
                sui_framework::object::ID,
            },
            scheduler::{OccurrenceSource, WithdrawalReason},
        },
    };

    #[test]
    fn lifecycle_projection_covers_every_stored_state() {
        let execution_id = ID::new(sui::types::Address::from_static("0x42"));
        assert_eq!(
            project_lifecycle(&OccurrenceState::Scheduled, false, false),
            (OccurrenceStatus::Pending, None, None)
        );
        assert_eq!(
            project_lifecycle(&OccurrenceState::Scheduled, true, false),
            (OccurrenceStatus::Advertised, None, None)
        );
        let dispatched = OccurrenceState::Dispatched {
            execution_id,
            dispatched_at_ms: 10,
        };
        assert_eq!(
            project_lifecycle(&dispatched, false, false),
            (OccurrenceStatus::Executing, Some(10), None)
        );
        assert_eq!(
            project_lifecycle(&dispatched, false, true),
            (OccurrenceStatus::Finished, Some(10), None)
        );
        assert_eq!(
            project_lifecycle(&OccurrenceState::Missed { missed_at_ms: 20 }, false, false,),
            (OccurrenceStatus::Missed { missed_at_ms: 20 }, None, None,)
        );
        assert_eq!(
            project_lifecycle(
                &OccurrenceState::Withdrawn {
                    reason: OccurrenceWithdrawalReason::TaskCanceled,
                },
                false,
                false,
            ),
            (
                OccurrenceStatus::Withdrawn {
                    reason: crate::scheduler::WithdrawalReason::TaskCanceled,
                },
                None,
                None,
            )
        );
        assert_eq!(
            project_lifecycle(
                &OccurrenceState::Settled {
                    execution_id,
                    dispatched_at_ms: 10,
                    settled_at_ms: 30,
                    succeeded: true,
                },
                false,
                false,
            ),
            (
                OccurrenceStatus::Settled { succeeded: true },
                Some(10),
                Some(30),
            )
        );
    }

    #[test]
    fn stored_runtime_identity_is_present_only_after_dispatch() {
        let execution_id = ID::new(sui::types::Address::from_static("0x43"));
        for state in [
            OccurrenceState::Scheduled,
            OccurrenceState::Missed { missed_at_ms: 1 },
            OccurrenceState::Withdrawn {
                reason: OccurrenceWithdrawalReason::TaskCanceled,
            },
        ] {
            assert_eq!(state_execution_id(&state), None);
        }
        assert_eq!(
            state_execution_id(&OccurrenceState::Dispatched {
                execution_id,
                dispatched_at_ms: 2,
            }),
            Some(execution_id.bytes)
        );
        assert_eq!(
            state_execution_id(&OccurrenceState::Settled {
                execution_id,
                dispatched_at_ms: 2,
                settled_at_ms: 3,
                succeeded: true,
            }),
            Some(execution_id.bytes)
        );
    }

    #[test]
    fn dispatched_identity_requires_runtime_observation() {
        let task_id = sui::types::Address::from_static("0x44");
        let execution_id = sui::types::Address::from_static("0x45");
        let reference = OccurrenceRef::new(task_id, 6);
        let snapshot = |execution: Option<ExecutionSnapshot>| OccurrenceSnapshot {
            reference,
            source: OccurrenceSource::Standalone,
            requested_start_time_ms: 1,
            effective_start_time_ms: Some(1),
            deadline_ms: None,
            priority_fee_percentage: 20,
            dispatched_at_ms: execution.as_ref().map(|_| 2),
            settled_at_ms: None,
            status: if execution.is_some() {
                OccurrenceStatus::Executing
            } else {
                OccurrenceStatus::Pending
            },
            execution,
            observed_task_version: 1,
        };

        let unavailable = snapshot(None);
        assert!(matches!(
            dispatched_execution_id(&unavailable),
            Err(SchedulerError::OccurrenceNotDispatched {
                task_id: observed_task,
                occurrence_id: 6,
            }) if observed_task == task_id
        ));

        let dispatched = snapshot(Some(ExecutionSnapshot::unavailable(execution_id)));
        assert_eq!(
            dispatched_execution_id(&dispatched).expect("runtime identity is present"),
            execution_id
        );
    }

    #[test]
    fn settlement_requires_a_finished_occurrence() {
        let task_id = sui::types::Address::from_static("0x46");
        let execution_id = sui::types::Address::from_static("0x47");
        let reference = OccurrenceRef::new(task_id, 7);
        let snapshot = |status| OccurrenceSnapshot {
            reference,
            source: OccurrenceSource::Standalone,
            requested_start_time_ms: 1,
            effective_start_time_ms: Some(1),
            deadline_ms: None,
            priority_fee_percentage: 20,
            dispatched_at_ms: Some(2),
            settled_at_ms: None,
            status,
            execution: Some(ExecutionSnapshot::unavailable(execution_id)),
            observed_task_version: 1,
        };

        assert_eq!(
            settlement_execution_id(&snapshot(OccurrenceStatus::Finished))
                .expect("finished occurrence is ready"),
            execution_id
        );
        for status in [
            OccurrenceStatus::Executing,
            OccurrenceStatus::Settled { succeeded: true },
        ] {
            assert!(matches!(
                settlement_execution_id(&snapshot(status)),
                Err(SchedulerError::OccurrenceNotReadyForSettlement {
                    task_id: observed_task,
                    occurrence_id: 7,
                    observed,
                }) if observed_task == task_id && observed == status
            ));
        }
    }

    #[test]
    fn withdrawal_projection_maps_every_protocol_reason() {
        for (stored, projected) in [
            (
                OccurrenceWithdrawalReason::RecurrenceReplaced,
                WithdrawalReason::RecurrenceReplaced,
            ),
            (
                OccurrenceWithdrawalReason::RecurrenceCleared,
                WithdrawalReason::RecurrenceCleared,
            ),
            (
                OccurrenceWithdrawalReason::TaskCanceled,
                WithdrawalReason::TaskCanceled,
            ),
        ] {
            assert_eq!(
                project_lifecycle(&OccurrenceState::Withdrawn { reason: stored }, false, false,),
                (
                    OccurrenceStatus::Withdrawn { reason: projected },
                    None,
                    None,
                )
            );
        }
    }
}
