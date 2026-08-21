use {
    super::{mutation_receipt, occurrence, resolve},
    crate::{
        move_bindings::scheduler::task::{
            FailureMode as MoveFailureMode,
            OccurrenceRecord,
            OccurrenceRecordKey,
            TaskController as MoveTaskController,
            TaskInnerV1 as MoveTaskInnerV1,
            TaskStatus as MoveTaskStatus,
        },
        nexus::client::NexusClient,
        scheduler::{
            FailurePolicy,
            Occurrence,
            OccurrencePage,
            OccurrenceRef,
            Recurrence,
            SchedulerError,
            TaskController,
            TaskMutationReceipt,
            TaskSnapshot,
            TaskStatus,
        },
        sui,
        transactions::scheduler::{
            compile_add_occurrence_ptb,
            compile_cancel_task_ptb,
            compile_clear_recurrence_ptb,
            compile_close_task_ptb,
            compile_pause_task_ptb,
            compile_refill_task_ptb,
            compile_resume_task_ptb,
            compile_set_recurrence_ptb,
        },
    },
};

/// Stateful operations for one Task.
#[derive(Clone)]
pub struct TaskHandle {
    client: NexusClient,
    task_id: sui::types::Address,
}

impl TaskHandle {
    pub(super) const fn new(client: NexusClient, task_id: sui::types::Address) -> Self {
        Self { client, task_id }
    }

    /// Returns the Task identifier.
    pub const fn id(&self) -> sui::types::Address {
        self.task_id
    }

    /// Returns a handle for one occurrence owned by this Task.
    pub fn occurrence(&self, occurrence_id: u64) -> super::OccurrenceHandle {
        super::OccurrenceHandle::new(
            self.client.clone(),
            OccurrenceRef::new(self.task_id, occurrence_id),
        )
    }

    /// Reads the current Task object into a public snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError::TaskNotFound`] when the Task is absent, or a
    /// transport error when the object cannot be read.
    pub async fn snapshot(&self) -> Result<TaskSnapshot, SchedulerError> {
        let task = resolve::fetch_task(&self.client, self.task_id).await?;
        task_snapshot(&task.object)
    }

    /// Reads one RPC page of permanent occurrence records.
    ///
    /// The cursor is opaque and may be passed unchanged from
    /// [`OccurrencePage::next_cursor`].
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the Task is absent, `limit` is zero, or
    /// the page cannot be decoded.
    pub async fn occurrences(
        &self,
        cursor: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<OccurrencePage, SchedulerError> {
        if limit == 0 {
            return Err(SchedulerError::InvalidRequest {
                message: "occurrence page limit must be greater than zero".to_owned(),
            });
        }
        let task = resolve::fetch_task(&self.client, self.task_id).await?;
        let page = self
            .client
            .crawler()
            .get_dynamic_field_page_matching_types::<OccurrenceRecordKey, OccurrenceRecord>(
                self.task_id,
                cursor,
                limit,
                "::task::OccurrenceRecord",
            )
            .await
            .map_err(SchedulerError::transport)?;
        let (records, next_cursor) = page.into_parts();
        let execution_ids = records
            .iter()
            .filter_map(|(_, record)| occurrence::state_execution_id(&record.state))
            .collect::<Vec<_>>();
        let mut executions = Vec::with_capacity(execution_ids.len());
        for execution_id in execution_ids {
            executions
                .push(resolve::fetch_execution(&self.client, &task.context, execution_id).await?);
        }
        let mut executions = executions.into_iter();
        let mut occurrences = Vec::with_capacity(records.len());
        for (key, record) in records {
            let execution = if occurrence::state_execution_id(&record.state).is_some() {
                executions
                    .next()
                    .ok_or_else(|| SchedulerError::InconsistentChainState {
                        message: "runtime object results did not align with occurrence records"
                            .to_owned(),
                    })?
            } else {
                None
            };
            occurrences.push(occurrence::snapshot_from_record(
                self.task_id,
                &task.object.data,
                key.pos0,
                &record,
                execution.as_ref().map(|response| &response.data),
                task.object.version,
            )?);
        }
        if executions.next().is_some() {
            return Err(SchedulerError::InconsistentChainState {
                message: "runtime object results exceeded occurrence records".to_owned(),
            });
        }
        Ok(OccurrencePage::new(occurrences, next_cursor))
    }

    /// Adds one standalone occurrence.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when time resolution, authority resolution,
    /// transaction construction, or submission fails.
    pub async fn add_occurrence(
        &self,
        occurrence: Occurrence,
    ) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let occurrence = resolve::prepare_occurrence(&self.client, &occurrence).await?;
        let transaction = compile_add_occurrence_ptb(
            &task.context,
            &task.object.object_ref(),
            &task.authority,
            &occurrence,
        )?;
        self.submit(&self.client, transaction).await
    }

    /// Replaces the Task recurrence.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when time resolution, authority resolution,
    /// transaction construction, or submission fails.
    pub async fn set_recurrence(
        &self,
        recurrence: Recurrence,
    ) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let recurrence = resolve::prepare_recurrence(&self.client, &recurrence).await?;
        let transaction = compile_set_recurrence_ptb(
            &task.context,
            &task.object.object_ref(),
            &task.authority,
            &recurrence,
        )?;
        self.submit(&self.client, transaction).await
    }

    /// Clears the Task recurrence and retains its withdrawn record.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when authority resolution, transaction
    /// construction, or submission fails.
    pub async fn clear_recurrence(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let transaction = compile_clear_recurrence_ptb(
            &task.context,
            &task.object.object_ref(),
            &task.authority,
        )?;
        self.submit(&self.client, transaction).await
    }

    /// Pauses future dispatch while retaining scheduled work.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when authority resolution, transaction
    /// construction, or submission fails.
    pub async fn pause(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let transaction =
            compile_pause_task_ptb(&task.context, &task.object.object_ref(), &task.authority)?;
        self.submit(&self.client, transaction).await
    }

    /// Resumes dispatch for retained scheduled work.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when authority resolution, transaction
    /// construction, or submission fails.
    pub async fn resume(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let transaction =
            compile_resume_task_ptb(&task.context, &task.object.object_ref(), &task.authority)?;
        self.submit(&self.client, transaction).await
    }

    /// Cancels future work and retains all occurrence records.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when authority resolution, transaction
    /// construction, or submission fails.
    pub async fn cancel(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let transaction =
            compile_cancel_task_ptb(&task.context, &task.object.object_ref(), &task.authority)?;
        self.submit(&self.client, transaction).await
    }

    /// Adds MIST to the Task payment reserve.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when authority resolution, transaction
    /// construction, or submission fails.
    pub async fn refill(&self, amount_mist: u64) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let transaction = compile_refill_task_ptb(
            &task.context,
            &task.object.object_ref(),
            &task.authority,
            amount_mist,
        )?;
        self.submit(&self.client, transaction).await
    }

    /// Releases live resources while retaining the Task and its records.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when authority resolution, transaction
    /// construction, or submission fails.
    pub async fn close(&self) -> Result<TaskMutationReceipt, SchedulerError> {
        let task = resolve::resolve_task(&self.client, self.task_id, &[]).await?;
        let transaction =
            compile_close_task_ptb(&task.context, &task.object.object_ref(), &task.authority)?;
        self.submit(&self.client, transaction).await
    }

    async fn submit(
        &self,
        client: &NexusClient,
        transaction: sui::types::ProgrammableTransaction,
    ) -> Result<TaskMutationReceipt, SchedulerError> {
        let sender = client.owner().map_err(SchedulerError::from)?;
        let executed = client
            .submit_transaction(transaction, sender)
            .await
            .map_err(SchedulerError::from)?;
        mutation_receipt(executed, self.task_id)
    }
}

fn task_snapshot(
    task: &crate::nexus::crawler::Response<MoveTaskInnerV1>,
) -> Result<TaskSnapshot, SchedulerError> {
    let task_id = task.object_id;
    let controller = match task.data.controller {
        MoveTaskController::Address { pos0 } => TaskController::Address { address: pos0 },
        MoveTaskController::Agent { pos0 } => TaskController::Agent {
            agent_id: pos0.bytes,
        },
    };
    let status = match task.data.status {
        MoveTaskStatus::Active => TaskStatus::Active,
        MoveTaskStatus::Paused => TaskStatus::Paused,
        MoveTaskStatus::Canceled => TaskStatus::Canceled,
        MoveTaskStatus::Rejected { reason } => TaskStatus::Rejected {
            reason: match reason {
                crate::move_bindings::scheduler::task::TaskRejectionReason::UnsupportedWorkAdmission => {
                    crate::scheduler::TaskRejectionReason::UnsupportedWorkAdmission
                }
                crate::move_bindings::scheduler::task::TaskRejectionReason::DisabledWorkAdmission => {
                    crate::scheduler::TaskRejectionReason::DisabledWorkAdmission
                }
                crate::move_bindings::scheduler::task::TaskRejectionReason::StaleSkillContract => {
                    crate::scheduler::TaskRejectionReason::StaleSkillContract
                }
                crate::move_bindings::scheduler::task::TaskRejectionReason::MutableDAG => {
                    crate::scheduler::TaskRejectionReason::MutableDAG
                }
            },
        },
        MoveTaskStatus::Finalized => TaskStatus::Finalized,
    };
    let failure_policy = match task.data.failure_mode {
        MoveFailureMode::Continue => FailurePolicy::Continue,
        MoveFailureMode::Pause => FailurePolicy::Pause,
    };
    let advertised = task
        .data
        .schedule
        .advertised_occurrence_id
        .copied_option()
        .map(|occurrence_id| OccurrenceRef::new(task_id, occurrence_id));
    let recurrence_pending = u64::from(task.data.schedule.recurrence.as_option().is_some());

    Ok(TaskSnapshot {
        task_id,
        controller,
        status,
        failure_policy,
        advertised,
        allocated_occurrences: task.data.schedule.next_occurrence_id,
        pending_occurrences: task.data.schedule.pending.len() as u64 + recurrence_pending,
        dispatched_occurrences: task.data.schedule.dispatched_count,
        in_flight_occurrences: task.data.in_flight.size_u64(),
        observed_version: task.version,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            nexus::client::AddressBalanceGas,
            test_utils::{nexus_mocks, sui_mocks},
        },
    };

    #[tokio::test]
    async fn handles_preserve_identity_and_reject_zero_page_limit() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let private_key = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let client = NexusClient::builder()
            .with_private_key(private_key)
            .with_rpc_url(&rpc_url)
            .with_address_balance_gas_config(AddressBalanceGas::new(1_000))
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client builds");
        let task_id = sui::types::Address::from_static("0x42");
        let task = TaskHandle::new(client, task_id);

        assert_eq!(task.id(), task_id);
        assert_eq!(
            task.occurrence(7).reference(),
            OccurrenceRef::new(task_id, 7)
        );
        assert!(matches!(
            task.occurrences(None, 0).await,
            Err(SchedulerError::InvalidRequest { message })
                if message == "occurrence page limit must be greater than zero"
        ));
    }

    #[tokio::test]
    async fn scheduler_reads_reach_rpc_without_owned_coins() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::from_static("0x42");
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_get_object()
            .times(3)
            .returning(|_| Err(tonic::Status::not_found("Task not present")));
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await;
        let task = client.scheduler().task(task_id);

        let snapshot = task.snapshot().await;
        let occurrences = task.occurrences(None, 10).await;
        let occurrence = task.occurrence(7).snapshot().await;

        for result in [
            snapshot.map(|_| ()),
            occurrences.map(|_| ()),
            occurrence.map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(SchedulerError::TaskNotFound {
                    task_id: missing_task
                }) if missing_task == task_id
            ));
        }
    }
}
