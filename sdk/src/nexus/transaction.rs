//! Client scoped programmable transaction composition.

use {
    crate::{
        move_boundary::NexusPtbBuilder,
        nexus::{
            client::NexusClient,
            error::NexusError,
            scheduler::resolve,
            signer::ExecutedTransaction,
        },
        scheduler::{DispatchOffer, OccurrenceRef, Schedule, SchedulerError, TaskSpec},
        sui,
        transactions::scheduler::{
            compile_append_dispatch_occurrence,
            compile_append_expire_occurrence,
            TaskDraftCompiler,
        },
    },
    std::collections::HashSet,
};

/// One programmable transaction scoped to a [`NexusClient`].
///
/// Dropping an unfinished value has no external effect.
#[must_use = "dropping a Nexus transaction discards its commands"]
pub struct NexusTransaction<'client> {
    client: &'client NexusClient,
    transaction: NexusPtbBuilder<'client>,
}

impl<'client> NexusTransaction<'client> {
    pub(super) fn new(client: &'client NexusClient) -> Self {
        Self {
            client,
            transaction: NexusPtbBuilder::new(&client.nexus_objects),
        }
    }

    /// Borrows the scheduler command composer.
    pub fn scheduler(&mut self) -> SchedulerTransaction<'_, 'client> {
        SchedulerTransaction {
            client: self.client,
            transaction: &mut self.transaction,
        }
    }

    /// Finishes the programmable transaction without submitting it.
    #[must_use]
    pub fn finish(self) -> sui::types::ProgrammableTransaction {
        self.transaction.finish()
    }

    /// Finishes and submits the transaction with this client's signer and gas.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when signing, submission, or confirmation fails.
    pub async fn submit(self) -> Result<ExecutedTransaction, NexusError> {
        let sender = self.client.signer.get_active_address();
        let client = self.client;
        let transaction = self.transaction.finish();
        client.submit_transaction(transaction, sender).await
    }
}

/// Scheduler commands appended to one [`NexusTransaction`].
pub struct SchedulerTransaction<'builder, 'client> {
    client: &'client NexusClient,
    transaction: &'builder mut NexusPtbBuilder<'client>,
}

impl<'builder, 'client> SchedulerTransaction<'builder, 'client> {
    /// Creates an unshared Task draft.
    ///
    /// Call [`TaskDraft::share`] after adding any desired Schedule.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when Task preparation or command
    /// construction fails.
    pub async fn create_task<'draft>(
        &'draft mut self,
        task: &TaskSpec,
    ) -> Result<TaskDraft<'draft, 'client>, SchedulerError> {
        let prepared = resolve::prepare_task(self.client, task).await?;
        let compiler = TaskDraftCompiler::create(self.transaction, &prepared)?;
        Ok(TaskDraft {
            client: self.client,
            compiler,
        })
    }

    /// Appends dispatch of one advertised occurrence.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the offer and Task disagree or command
    /// construction fails.
    pub fn dispatch_occurrence(
        &mut self,
        offer: &DispatchOffer,
        task: &sui::types::ObjectReference,
        dag: &sui::types::ObjectReference,
        leader_cap: &sui::types::ObjectReference,
        tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
    ) -> Result<(), SchedulerError> {
        if offer.occurrence().task_id() != *task.object_id() {
            return Err(SchedulerError::InvalidRequest {
                message: format!(
                    "dispatch offer Task '{}' differs from object '{}'",
                    offer.occurrence().task_id(),
                    task.object_id()
                ),
            });
        }
        compile_append_dispatch_occurrence(
            self.transaction,
            task,
            dag,
            leader_cap,
            offer.occurrence().occurrence_id(),
            tools_gas,
        )
    }

    /// Appends expiration of one advertised occurrence.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the occurrence and Task disagree or
    /// command construction fails.
    pub fn expire_occurrence(
        &mut self,
        occurrence: OccurrenceRef,
        task: &sui::types::ObjectReference,
    ) -> Result<(), SchedulerError> {
        if occurrence.task_id() != *task.object_id() {
            return Err(SchedulerError::InvalidRequest {
                message: format!(
                    "occurrence Task '{}' differs from object '{}'",
                    occurrence.task_id(),
                    task.object_id()
                ),
            });
        }
        compile_append_expire_occurrence(self.transaction, task, occurrence.occurrence_id())
    }
}

/// An unshared Task being composed in one transaction.
#[must_use = "a Task draft must be shared before its transaction is finished"]
pub struct TaskDraft<'builder, 'client> {
    client: &'client NexusClient,
    compiler: TaskDraftCompiler<'builder, 'client>,
}

impl<'builder, 'client> TaskDraft<'builder, 'client> {
    /// Appends a complete Schedule before the Task is shared.
    ///
    /// Empty Schedules are valid and append no commands.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when time preparation or command
    /// construction fails.
    pub async fn schedule(self, schedule: &Schedule) -> Result<Self, SchedulerError> {
        let prepared = resolve::prepare_schedule(self.client, schedule).await?;
        let compiler = self.compiler.schedule(&prepared)?;
        Ok(Self {
            client: self.client,
            compiler,
        })
    }

    /// Shares the composed Task and consumes the draft.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the share command cannot be built.
    pub fn share(self) -> Result<(), SchedulerError> {
        self.compiler.share()
    }
}
