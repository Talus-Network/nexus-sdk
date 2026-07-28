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
        let sender = self.client.owner().map_err(SchedulerError::transport)?;
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
        let sender = self.client.owner().map_err(SchedulerError::transport)?;
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEvent,
            move_bindings::{
                move_std::option::Option as MoveOption,
                scheduler::{
                    schedule::{
                        OccurrenceSource as MoveOccurrenceSource,
                        OccurrenceWithdrawalReason,
                    },
                    scheduler as scheduler_binding,
                    task::TaskController as MoveTaskController,
                },
                sui_framework::object::ID,
            },
        },
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn event(index: u64, data: NexusEventKind) -> NexusEvent {
        NexusEvent {
            id: (sui::types::Digest::new([9; 32]), index),
            generics: Vec::new(),
            data,
            distribution: None,
        }
    }

    fn executed(events: Vec<NexusEventKind>) -> ExecutedTransaction {
        let digest = sui::types::Digest::new([9; 32]);
        ExecutedTransaction {
            effects: sui::types::TransactionEffectsV2 {
                status: sui::types::ExecutionStatus::Success,
                epoch: 1,
                gas_used: sui::types::GasCostSummary {
                    computation_cost: 0,
                    storage_cost: 0,
                    storage_rebate: 0,
                    non_refundable_storage_fee: 0,
                },
                transaction_digest: digest,
                gas_object_index: None,
                events_digest: None,
                dependencies: Vec::new(),
                lamport_version: 1,
                changed_objects: Vec::new(),
                unchanged_consensus_objects: Vec::new(),
                auxiliary_data_digest: None,
            },
            events: events
                .into_iter()
                .enumerate()
                .map(|(index, data)| event(index as u64, data))
                .collect(),
            objects: Vec::new(),
            digest,
            checkpoint: 14,
        }
    }

    fn task_created(task_id: sui::types::Address) -> NexusEventKind {
        NexusEventKind::TaskCreated(scheduler_binding::TaskCreated::new(
            ID::new(task_id),
            MoveTaskController::Address {
                pos0: address("0x52"),
            },
            ID::new(address("0x53")),
            0,
        ))
    }

    #[test]
    fn mutation_receipt_collects_the_confirmed_schedule_delta() {
        let task_id = address("0x51");
        let executed = executed(vec![
            task_created(task_id),
            NexusEventKind::OccurrenceScheduled(scheduler_binding::OccurrenceScheduled::new(
                ID::new(task_id),
                1,
                100,
                MoveOption::from_option(Some(150)),
                20,
                MoveOccurrenceSource::Standalone,
            )),
            NexusEventKind::OccurrenceScheduled(scheduler_binding::OccurrenceScheduled::new(
                ID::new(task_id),
                2,
                200,
                MoveOption::from_option(None),
                30,
                MoveOccurrenceSource::Recurring { iteration: 3 },
            )),
            NexusEventKind::OccurrenceWithdrawn(scheduler_binding::OccurrenceWithdrawn::new(
                ID::new(task_id),
                1,
                OccurrenceWithdrawalReason::RecurrenceCleared,
            )),
            NexusEventKind::OccurrenceAdvertised(scheduler_binding::OccurrenceAdvertised::new(
                ID::new(task_id),
                2,
                200,
                MoveOption::from_option(None),
                30,
                MoveOccurrenceSource::Recurring { iteration: 3 },
            )),
        ]);

        assert_eq!(
            created_task_id(&executed).expect("one Task was created"),
            task_id
        );
        let receipt = mutation_receipt(executed, task_id).expect("events agree on Task identity");

        assert_eq!(receipt.task_id(), task_id);
        assert_eq!(receipt.transaction().checkpoint(), 14);
        assert_eq!(receipt.delta().scheduled().len(), 2);
        assert_eq!(
            receipt.delta().scheduled()[0].source(),
            OccurrenceSource::Standalone
        );
        assert_eq!(
            receipt.delta().scheduled()[1].source(),
            OccurrenceSource::Recurring { iteration: 3 }
        );
        assert_eq!(
            receipt.delta().withdrawn()[0].reason(),
            WithdrawalReason::RecurrenceCleared
        );
        assert_eq!(
            receipt.delta().advertised(),
            Some(OccurrenceRef::new(task_id, 2))
        );
    }

    #[test]
    fn confirmation_rejects_missing_duplicate_and_mismatched_task_identity() {
        assert!(matches!(
            created_task_id(&executed(Vec::new())),
            Err(SchedulerError::Confirmation { .. })
        ));

        let task_id = address("0x54");
        assert!(matches!(
            created_task_id(&executed(vec![
                task_created(task_id),
                task_created(task_id),
            ])),
            Err(SchedulerError::Confirmation { .. })
        ));

        let other_task = address("0x55");
        assert!(matches!(
            mutation_receipt(
                executed(vec![NexusEventKind::TaskPaused(
                    scheduler_binding::TaskPaused::new(ID::new(other_task)),
                )]),
                task_id,
            ),
            Err(SchedulerError::Confirmation { .. })
        ));
    }

    #[test]
    fn scheduler_event_identity_covers_every_task_event() {
        let task_id = address("0x56");
        let execution_id = ID::new(address("0x57"));
        let events = [
            NexusEventKind::OccurrenceAdvertised(scheduler_binding::OccurrenceAdvertised::new(
                ID::new(task_id),
                1,
                10,
                MoveOption::from_option(None),
                20,
                MoveOccurrenceSource::Standalone,
            )),
            NexusEventKind::OccurrenceDispatched(scheduler_binding::OccurrenceDispatched::new(
                ID::new(task_id),
                1,
                execution_id,
                11,
            )),
            NexusEventKind::OccurrenceMissed(scheduler_binding::OccurrenceMissed::new(
                ID::new(task_id),
                1,
                12,
            )),
            NexusEventKind::OccurrenceScheduled(scheduler_binding::OccurrenceScheduled::new(
                ID::new(task_id),
                1,
                10,
                MoveOption::from_option(None),
                20,
                MoveOccurrenceSource::Standalone,
            )),
            NexusEventKind::OccurrenceSettled(scheduler_binding::OccurrenceSettled::new(
                ID::new(task_id),
                1,
                execution_id,
                true,
            )),
            NexusEventKind::OccurrenceWithdrawn(scheduler_binding::OccurrenceWithdrawn::new(
                ID::new(task_id),
                1,
                OccurrenceWithdrawalReason::TaskCanceled,
            )),
            NexusEventKind::TaskCanceled(scheduler_binding::TaskCanceled::new(ID::new(task_id))),
            NexusEventKind::TaskClosed(scheduler_binding::TaskClosed::new(ID::new(task_id))),
            task_created(task_id),
            NexusEventKind::TaskPaused(scheduler_binding::TaskPaused::new(ID::new(task_id))),
            NexusEventKind::TaskResumed(scheduler_binding::TaskResumed::new(ID::new(task_id))),
        ];

        for event in events {
            assert_eq!(scheduler_event_task_id(&event), Some(task_id));
        }
    }

    #[test]
    fn source_and_withdrawal_projections_cover_every_protocol_variant() {
        assert_eq!(
            occurrence_source(MoveOccurrenceSource::Standalone),
            OccurrenceSource::Standalone
        );
        assert_eq!(
            occurrence_source(MoveOccurrenceSource::Recurring { iteration: 4 }),
            OccurrenceSource::Recurring { iteration: 4 }
        );
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
            assert_eq!(withdrawal_reason(stored), projected);
        }
    }
}
