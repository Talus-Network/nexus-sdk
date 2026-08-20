//! Client scoped programmable transaction composition.

use crate::{
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
};

/// One programmable transaction scoped to a [`NexusClient`].
///
/// Dropping an unfinished value has no external effect.
#[must_use = "dropping a Nexus transaction discards its commands"]
pub struct NexusTransaction {
    client: NexusClient,
    transaction: NexusPtbBuilder,
}

impl NexusTransaction {
    pub(super) fn new(client: NexusClient) -> Self {
        let objects = client.get_nexus_objects();
        Self {
            client,
            transaction: NexusPtbBuilder::new(objects),
        }
    }

    /// Borrows the scheduler command composer.
    pub fn scheduler(&mut self) -> SchedulerTransaction<'_> {
        SchedulerTransaction {
            client: &self.client,
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
        let sender = self.client.owner()?;
        let transaction = self.transaction.finish();
        self.client.submit_transaction(transaction, sender).await
    }
}

/// Scheduler commands appended to one [`NexusTransaction`].
pub struct SchedulerTransaction<'transaction> {
    client: &'transaction NexusClient,
    transaction: &'transaction mut NexusPtbBuilder,
}

impl SchedulerTransaction<'_> {
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
    ) -> Result<TaskDraft<'draft>, SchedulerError> {
        let prepared = resolve::prepare_task(self.client, task).await?;
        let compiler = TaskDraftCompiler::create(self.transaction, &prepared)?;
        Ok(TaskDraft {
            client: self.client,
            compiler,
        })
    }

    /// Appends dispatch of one advertised occurrence with an explicit leader submission gas charge.
    /// Pass zero when the leader waives reimbursement.
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
        gas_charge: u64,
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
            gas_charge,
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
pub struct TaskDraft<'builder> {
    client: &'builder NexusClient,
    compiler: TaskDraftCompiler<'builder>,
}

impl TaskDraft<'_> {
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
        let owner = self.client.owner().map_err(SchedulerError::transport)?;
        self.compiler.share(owner)
    }
}
