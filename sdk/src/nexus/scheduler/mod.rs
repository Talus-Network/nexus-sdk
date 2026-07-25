//! Stateful scheduling operations exposed through [`NexusClient`].
//!
//! A [`Scheduler`] creates Tasks and returns handles for later mutations and
//! object inspection. Scheduling is the only request model. Runtime execution
//! objects appear only after an occurrence is dispatched.
//!
//! [`NexusClient`]: crate::nexus::client::NexusClient

mod occurrence;
pub(crate) mod resolve;
mod task;

use crate::{
    events::NexusEventKind,
    nexus::{client::NexusClient, signer::ExecutedTransaction},
    scheduler::{
        OccurrenceRef,
        OccurrenceSource,
        Schedule,
        ScheduleDelta,
        ScheduledOccurrence,
        SchedulerError,
        TaskMutationReceipt,
        TaskSpec,
        TransactionReference,
        WithdrawalReason,
        WithdrawnOccurrence,
    },
    sui,
    transactions::scheduler::{compile_create_task_ptb, compile_schedule_task_ptb},
};
pub use {occurrence::OccurrenceHandle, task::TaskHandle};

/// Scheduling facade owned by one configured [`NexusClient`].
///
/// Cloning this value is inexpensive. Every method retains the deployment,
/// signer, gas, and transport configuration of its client.
#[derive(Clone)]
pub struct Scheduler {
    pub(super) client: NexusClient,
}

impl Scheduler {
    /// Creates and shares an empty Task.
    ///
    /// Use [`Self::schedule_task`] when the creation transaction must also add
    /// one or more occurrences.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the Task is invalid, request
    /// preparation fails, or the transaction is not confirmed.
    pub async fn create_task(&self, task: TaskSpec) -> Result<TaskMutationReceipt, SchedulerError> {
        let sender = self.client.signer.get_active_address();
        let prepared = resolve::prepare_task(&self.client, &task).await?;
        let transaction = compile_create_task_ptb(&self.client.nexus_objects, &prepared)?;
        let executed = self
            .client
            .submit_transaction(transaction, sender)
            .await
            .map_err(SchedulerError::transport)?;
        let task_id = created_task_id(&executed)?;
        mutation_receipt(executed, task_id)
    }

    /// Creates, schedules, and shares one Task atomically.
    ///
    /// The Schedule may combine standalone occurrences and one recurrence.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerError`] when the Task or Schedule is invalid,
    /// including when the Schedule is empty, or when submission fails.
    pub async fn schedule_task(
        &self,
        task: TaskSpec,
        schedule: Schedule,
    ) -> Result<TaskMutationReceipt, SchedulerError> {
        schedule.validate_for_task_creation()?;
        let sender = self.client.signer.get_active_address();
        let prepared_task = resolve::prepare_task(&self.client, &task).await?;
        let prepared_schedule = resolve::prepare_schedule(&self.client, &schedule).await?;
        let transaction = compile_schedule_task_ptb(
            &self.client.nexus_objects,
            &prepared_task,
            &prepared_schedule,
        )?;
        let executed = self
            .client
            .submit_transaction(transaction, sender)
            .await
            .map_err(SchedulerError::transport)?;
        let task_id = created_task_id(&executed)?;
        mutation_receipt(executed, task_id)
    }

    /// Returns a stateful handle for one Task identifier.
    pub fn task(&self, task_id: sui::types::Address) -> TaskHandle {
        TaskHandle::new(self.client.clone(), task_id)
    }
}

pub(super) fn mutation_receipt(
    executed: ExecutedTransaction,
    task_id: sui::types::Address,
) -> Result<TaskMutationReceipt, SchedulerError> {
    let mut scheduled = Vec::new();
    let mut withdrawn = Vec::new();
    let mut advertised = None;

    for event in &executed.events {
        if let Some(observed_task_id) = scheduler_event_task_id(&event.data) {
            if observed_task_id != task_id {
                return Err(SchedulerError::Confirmation {
                    message: format!(
                        "transaction for Task '{task_id}' contained a scheduler event for Task \
                         '{observed_task_id}'"
                    ),
                });
            }
        }

        match &event.data {
            NexusEventKind::OccurrenceScheduled(event) => {
                scheduled.push(ScheduledOccurrence::new(
                    OccurrenceRef::new(task_id, event.occurrence_id),
                    event.start_time_ms,
                    event.deadline_ms.copied_option(),
                    event.priority_fee_percentage,
                    occurrence_source(event.source),
                ));
            }
            NexusEventKind::OccurrenceWithdrawn(event) => {
                withdrawn.push(WithdrawnOccurrence::new(
                    OccurrenceRef::new(task_id, event.occurrence_id),
                    withdrawal_reason(event.reason),
                ));
            }
            NexusEventKind::OccurrenceAdvertised(event) => {
                advertised = Some(OccurrenceRef::new(task_id, event.occurrence_id));
            }
            _ => {}
        }
    }

    Ok(TaskMutationReceipt::new(
        TransactionReference::new(executed.digest, executed.checkpoint),
        task_id,
        ScheduleDelta::new(scheduled, withdrawn, advertised),
    ))
}

fn created_task_id(executed: &ExecutedTransaction) -> Result<sui::types::Address, SchedulerError> {
    let mut task_ids = executed
        .events
        .iter()
        .filter_map(|event| match &event.data {
            NexusEventKind::TaskCreated(event) => Some(event.task_id.bytes),
            _ => None,
        });
    let Some(task_id) = task_ids.next() else {
        return Err(SchedulerError::Confirmation {
            message: "Task creation did not emit TaskCreated".to_owned(),
        });
    };
    if task_ids.next().is_some() {
        return Err(SchedulerError::Confirmation {
            message: "Task creation emitted more than one Task identifier".to_owned(),
        });
    }
    Ok(task_id)
}

fn scheduler_event_task_id(event: &NexusEventKind) -> Option<sui::types::Address> {
    match event {
        NexusEventKind::OccurrenceAdvertised(event) => Some(event.task_id.bytes),
        NexusEventKind::OccurrenceDispatched(event) => Some(event.task_id.bytes),
        NexusEventKind::OccurrenceMissed(event) => Some(event.task_id.bytes),
        NexusEventKind::OccurrenceScheduled(event) => Some(event.task_id.bytes),
        NexusEventKind::OccurrenceSettled(event) => Some(event.task_id.bytes),
        NexusEventKind::OccurrenceWithdrawn(event) => Some(event.task_id.bytes),
        NexusEventKind::TaskCanceled(event) => Some(event.task_id.bytes),
        NexusEventKind::TaskClosed(event) => Some(event.task_id.bytes),
        NexusEventKind::TaskCreated(event) => Some(event.task_id.bytes),
        NexusEventKind::TaskPaused(event) => Some(event.task_id.bytes),
        NexusEventKind::TaskResumed(event) => Some(event.task_id.bytes),
        _ => None,
    }
}

pub(super) const fn occurrence_source(
    source: crate::move_bindings::scheduler::schedule::OccurrenceSource,
) -> OccurrenceSource {
    match source {
        crate::move_bindings::scheduler::schedule::OccurrenceSource::Standalone => {
            OccurrenceSource::Standalone
        }
        crate::move_bindings::scheduler::schedule::OccurrenceSource::Recurring { iteration } => {
            OccurrenceSource::Recurring { iteration }
        }
    }
}

pub(super) const fn withdrawal_reason(
    reason: crate::move_bindings::scheduler::schedule::OccurrenceWithdrawalReason,
) -> WithdrawalReason {
    match reason {
        crate::move_bindings::scheduler::schedule::OccurrenceWithdrawalReason::RecurrenceReplaced => {
            WithdrawalReason::RecurrenceReplaced
        }
        crate::move_bindings::scheduler::schedule::OccurrenceWithdrawalReason::RecurrenceCleared => {
            WithdrawalReason::RecurrenceCleared
        }
        crate::move_bindings::scheduler::schedule::OccurrenceWithdrawalReason::TaskCanceled => {
            WithdrawalReason::TaskCanceled
        }
    }
}
