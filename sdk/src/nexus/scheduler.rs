//! Scheduler actions exposed through [`NexusClient`].

mod inspection;

pub use {
    crate::move_bindings::derive_task_execution_id,
    inspection::{ExecutionSnapshot, OccurrenceSnapshot, OccurrenceStatus, WatchOccurrenceOptions},
    scheduler_tx::{OccurrenceSpec, RecurrenceSpec, TaskFailureMode, TaskStateAction},
};
use {
    crate::{
        events::NexusEventKind,
        move_bindings::{
            interface::{
                agent::{AgentExecutionConfig, ExecutionSelection},
                authorization::AgentVertexAuthorizationTemplate,
                graph::{EntryGroup, InputPort, Vertex},
            },
            move_std::option::Option as MoveOption,
            primitives::data::NexusData,
            scheduler::{
                schedule::{OccurrenceSource, OccurrenceWithdrawalReason},
                scheduler::{
                    OccurrenceScheduled as OccurrenceScheduledEvent,
                    OccurrenceWithdrawn as OccurrenceWithdrawnEvent,
                },
                task::{Task, TaskController},
            },
            sui_framework::{
                clock::Clock as SuiClock,
                object::ID,
                vec_map::{Entry as VecMapEntry, VecMap},
            },
            workflow::execution::DAGExecution,
        },
        move_boundary,
        nexus::{
            client::NexusClient,
            crawler::Response,
            error::NexusError,
            signer::ExecutedTransaction,
            tap,
        },
        sui,
        transactions::{agent_input::AgentInput, scheduler as scheduler_tx},
        types::{AgentId, SkillId},
    },
    anyhow::anyhow,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// High level scheduler operations.
#[derive(Clone)]
pub struct SchedulerActions {
    pub(super) client: NexusClient,
}

/// Operation selected for every occurrence of a [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub enum TaskOperation {
    /// Uses the configured default Agent to execute a published DAG.
    Default {
        /// Identifier of the published DAG.
        dag_id: sui::types::Address,
    },
    /// Uses one registered Agent skill.
    AgentSkill {
        /// Identifier of the Agent.
        agent_id: AgentId,
        /// Identifier of the registered skill.
        skill_id: SkillId,
        /// Optional DAG selected for the skill.
        selected_dag: Option<sui::types::Address>,
        /// Authorization templates materialized for each occurrence.
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
}

impl TaskOperation {
    fn agent_id(&self) -> Option<AgentId> {
        match self {
            Self::Default { .. } => None,
            Self::AgentSkill { agent_id, .. } => Some(*agent_id),
        }
    }

    fn selection(&self) -> ExecutionSelection {
        match self {
            Self::Default { dag_id } => ExecutionSelection::DefaultAgent {
                dag_id: ID::new(*dag_id),
            },
            Self::AgentSkill {
                agent_id,
                skill_id,
                selected_dag,
                ..
            } => ExecutionSelection::AgentSkill {
                agent_id: ID::new(*agent_id),
                skill_id: *skill_id,
                selected_dag: MoveOption::from_option(selected_dag.map(ID::new)),
            },
        }
    }

    fn authorization_templates(&self) -> Vec<AgentVertexAuthorizationTemplate> {
        match self {
            Self::Default { .. } => Vec::new(),
            Self::AgentSkill {
                authorization_templates,
                ..
            } => authorization_templates.clone(),
        }
    }
}

/// Funding source and controller for a new [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Copy, Debug)]
pub enum TaskFunding {
    /// Uses sender funds and sender control.
    Address {
        /// Funds reserved for future occurrences in MIST.
        prepay_amount_mist: u64,
        /// Address that receives unused funds.
        refund_recipient: Option<sui::types::Address>,
    },
    /// Uses an Agent vault and Agent control.
    Agent {
        /// Funds reserved from the Agent vault in MIST.
        prepay_amount_mist: u64,
    },
}

/// Complete input for one [`Task`] creation transaction.
///
/// Standalone occurrences and recurrence are composed before the Task is shared.
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub struct CreateTaskParams {
    /// Operation performed by every occurrence.
    pub operation: TaskOperation,
    /// DAG entry group selected for every occurrence.
    pub entry_group: String,
    /// Input values keyed by vertex and port.
    pub input_data: HashMap<String, HashMap<String, NexusData>>,
    /// Funding source and controller.
    pub funding: TaskFunding,
    /// Maximum funds available to each occurrence in MIST.
    pub occurrence_budget_mist: u64,
    /// Behavior after an occurrence fails.
    pub failure_mode: TaskFailureMode,
    /// Standalone occurrences composed into the Task.
    pub occurrences: Vec<OccurrenceSpec>,
    /// Optional recurrence composed into the Task.
    pub recurrence: Option<RecurrenceSpec>,
}

/// Identifies one materialized occurrence of a scheduler [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OccurrenceRef {
    /// Identifier of the owning [`Task`].
    pub task_id: sui::types::Address,
    /// Identifier allocated within the owning [`Task`].
    pub occurrence_id: u64,
}

impl OccurrenceRef {
    /// Creates a reference to one materialized occurrence.
    pub const fn new(task_id: sui::types::Address, occurrence_id: u64) -> Self {
        Self {
            task_id,
            occurrence_id,
        }
    }

    /// Derives the related [`DAGExecution`] identifier.
    ///
    /// [`DAGExecution`]: crate::move_bindings::workflow::execution::DAGExecution
    pub fn execution_id(self) -> Result<sui::types::Address, NexusError> {
        derive_task_execution_id(self.task_id, self.occurrence_id).map_err(NexusError::Parsing)
    }
}

/// One occurrence allocated by a schedule mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledOccurrence {
    /// Stable occurrence identity.
    pub reference: OccurrenceRef,
    /// Requested start time in milliseconds.
    pub start_time_ms: u64,
    /// Optional deadline in milliseconds.
    pub deadline_ms: Option<u64>,
    /// Priority fee percentage selected for dispatch.
    pub priority_fee_percentage: u64,
    /// Source that allocated the occurrence.
    pub source: OccurrenceSource,
}

/// One future occurrence removed by a schedule mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawnOccurrence {
    /// Stable occurrence identity.
    pub reference: OccurrenceRef,
    /// Reason the occurrence was removed.
    pub reason: OccurrenceWithdrawalReason,
}

/// Result of one scheduler mutation.
#[derive(Clone, Debug)]
pub struct SchedulerMutationResult {
    /// Digest of the committed transaction.
    pub tx_digest: sui::types::Digest,
    /// Checkpoint containing the committed transaction.
    pub tx_checkpoint: u64,
}

/// Exact occurrence changes produced by one schedule mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleMutationResult {
    /// Digest of the committed transaction.
    pub tx_digest: sui::types::Digest,
    /// Checkpoint containing the committed transaction.
    pub tx_checkpoint: u64,
    /// Identifier of the mutated [`Task`].
    pub task_id: sui::types::Address,
    /// Every occurrence allocated by the mutation.
    pub scheduled: Vec<ScheduledOccurrence>,
    /// Every occurrence removed by the mutation.
    pub withdrawn: Vec<WithdrawnOccurrence>,
    /// Final occurrence advertised by the mutation.
    pub advertised: Option<OccurrenceRef>,
}

impl SchedulerActions {
    /// Creates and fully composes one [`Task`] in a single transaction.
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn create_task(
        &self,
        params: CreateTaskParams,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let agent = match params.operation.agent_id() {
            Some(agent_id) => Some(self.agent_input(agent_id).await?),
            None => None,
        };
        let operation = build_operation_config(
            &self.client.nexus_objects,
            sender,
            &params.operation,
            params.entry_group,
            params.input_data,
        );
        let funding = match (params.funding, agent) {
            (
                TaskFunding::Address {
                    prepay_amount_mist,
                    refund_recipient,
                },
                agent,
            ) => scheduler_tx::TaskFunding::User {
                agent,
                prepay_amount_mist,
                refund_recipient: refund_recipient.unwrap_or(sender),
            },
            (TaskFunding::Agent { prepay_amount_mist }, Some(agent)) => {
                scheduler_tx::TaskFunding::Agent {
                    agent,
                    prepay_amount_mist,
                }
            }
            (TaskFunding::Agent { .. }, None) => {
                return Err(NexusError::Configuration(
                    "default operation cannot use Agent vault funding".into(),
                ));
            }
        };
        let tx = scheduler_tx::create_task_ptb(
            &self.client.nexus_objects,
            &scheduler_tx::CreateTaskParams {
                execution: operation,
                funding,
                occurrence_budget_mist: params.occurrence_budget_mist,
                failure_mode: params.failure_mode,
                occurrences: params.occurrences,
                recurrence: params.recurrence,
            },
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        let task_id = extract_task_id(&response)?;

        schedule_result(response, task_id)
    }

    /// Adds one standalone occurrence to a [`Task`].
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn schedule_occurrence(
        &self,
        task_id: sui::types::Address,
        occurrence: OccurrenceSpec,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let authority = self.task_authority(&task).await?;
        let tx = scheduler_tx::schedule_task_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            authority,
            occurrence,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, task_id)
    }

    /// Replaces the lazy recurrence for a [`Task`].
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn set_recurrence(
        &self,
        task_id: sui::types::Address,
        recurrence: RecurrenceSpec,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let authority = self.task_authority(&task).await?;
        let tx = scheduler_tx::set_recurrence_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            authority,
            recurrence,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, task_id)
    }

    /// Clears future recurring work from a [`Task`].
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn clear_recurrence(
        &self,
        task_id: sui::types::Address,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let authority = self.task_authority(&task).await?;
        let tx = scheduler_tx::clear_recurrence_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            authority,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, task_id)
    }

    /// Applies a state transition to a [`Task`].
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn set_task_state(
        &self,
        task_id: sui::types::Address,
        action: TaskStateAction,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let authority = self.task_authority(&task).await?;
        let tx = scheduler_tx::set_task_state_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            authority,
            action,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, task_id)
    }

    /// Refills the payment reserve for a [`Task`].
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn refill(
        &self,
        task_id: sui::types::Address,
        amount_mist: u64,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let tx = match self.task_authority(&task).await? {
            scheduler_tx::TaskAuthority::Address => scheduler_tx::refill_task_ptb(
                &self.client.nexus_objects,
                &task.object_ref(),
                amount_mist,
            ),
            scheduler_tx::TaskAuthority::Agent(agent) => scheduler_tx::refill_task_from_agent_ptb(
                &self.client.nexus_objects,
                &task.object_ref(),
                agent,
                amount_mist,
            ),
        }
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, task_id)
    }

    /// Finalizes a [`Task`] after all work and settlement are complete.
    ///
    /// The Task remains available as the durable record for its occurrences and executions.
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn close(
        &self,
        task_id: sui::types::Address,
    ) -> Result<SchedulerMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let authority = self.task_authority(&task).await?;
        let tx =
            scheduler_tx::close_task_ptb(&self.client.nexus_objects, &task.object_ref(), authority)
                .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        Ok(mutation_result(response))
    }

    /// Expires the advertised occurrence when its deadline has passed.
    pub async fn expire(
        &self,
        occurrence: OccurrenceRef,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(occurrence.task_id).await?;
        let tx = scheduler_tx::expire_occurrence_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            occurrence.occurrence_id,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, occurrence.task_id)
    }

    /// Settles the finished runtime object for one occurrence into its owning [`Task`].
    ///
    /// [`DAGExecution`]: crate::move_bindings::workflow::execution::DAGExecution
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn settle(
        &self,
        occurrence: OccurrenceRef,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(occurrence.task_id).await?;
        let execution = self
            .client
            .crawler()
            .get_object::<DAGExecution>(occurrence.execution_id()?)
            .await
            .map_err(NexusError::Rpc)?;
        let tx = scheduler_tx::settle_occurrence_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            &execution.object_ref(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        schedule_result(response, occurrence.task_id)
    }

    /// Fetches a [`Task`] and its current object reference.
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn fetch_task(
        &self,
        task_id: sui::types::Address,
    ) -> Result<Response<Task>, NexusError> {
        self.client
            .crawler()
            .get_object::<Task>(task_id)
            .await
            .map_err(NexusError::Rpc)
    }

    /// Reads the current Sui Clock timestamp.
    pub async fn clock_timestamp_ms(&self) -> Result<u64, NexusError> {
        self.client
            .crawler()
            .get_object::<SuiClock>(move_boundary::CLOCK_OBJECT_ID)
            .await
            .map(|clock| clock.data.timestamp_ms)
            .map_err(NexusError::Rpc)
    }

    async fn task_authority(
        &self,
        task: &Response<Task>,
    ) -> Result<scheduler_tx::TaskAuthority, NexusError> {
        match &task.data.controller {
            TaskController::Address { pos0 } => {
                let sender = self.client.signer.get_active_address();
                if *pos0 != sender {
                    return Err(NexusError::Configuration(format!(
                        "Task '{}' is controlled by address '{}', not active address '{}'",
                        task.object_id, pos0, sender
                    )));
                }
                Ok(scheduler_tx::TaskAuthority::Address)
            }
            TaskController::Agent { pos0 } => self
                .agent_input(pos0.bytes)
                .await
                .map(scheduler_tx::TaskAuthority::Agent),
        }
    }

    async fn agent_input(&self, agent_id: AgentId) -> Result<AgentInput, NexusError> {
        let metadata = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;
        tap::agent_input_from_metadata(&metadata).map_err(NexusError::TransactionBuilding)
    }
}

fn build_operation_config(
    objects: &crate::types::NexusObjects,
    sender: sui::types::Address,
    operation: &TaskOperation,
    entry_group: String,
    input_data: HashMap<String, HashMap<String, NexusData>>,
) -> AgentExecutionConfig {
    AgentExecutionConfig::new(
        operation.selection(),
        ID::new(objects.network_id),
        EntryGroup::new(entry_group),
        execution_inputs(input_data),
        sender,
        operation.authorization_templates(),
    )
}

fn execution_inputs(
    input_data: HashMap<String, HashMap<String, NexusData>>,
) -> VecMap<Vertex, VecMap<InputPort, NexusData>> {
    let mut vertices = input_data.into_iter().collect::<Vec<_>>();
    vertices.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    VecMap::new(
        vertices
            .into_iter()
            .map(|(vertex, ports)| {
                let mut ports = ports.into_iter().collect::<Vec<_>>();
                ports.sort_unstable_by(|left, right| left.0.cmp(&right.0));
                VecMapEntry::new(
                    Vertex::new(vertex),
                    VecMap::new(
                        ports
                            .into_iter()
                            .map(|(port, value)| VecMapEntry::new(InputPort::new(port), value))
                            .collect(),
                    ),
                )
            })
            .collect(),
    )
}

fn extract_task_id(response: &ExecutedTransaction) -> Result<sui::types::Address, NexusError> {
    response
        .events
        .iter()
        .find_map(|event| match &event.data {
            NexusEventKind::TaskCreated(event) => Some(event.task_id.bytes),
            _ => None,
        })
        .ok_or_else(|| NexusError::Parsing(anyhow!("TaskCreated event missing from transaction")))
}

#[derive(Debug, Default)]
struct ScheduleChanges {
    scheduled: Vec<ScheduledOccurrence>,
    withdrawn: Vec<WithdrawnOccurrence>,
    advertised: Option<OccurrenceRef>,
}

fn extract_schedule_changes(
    task_id: sui::types::Address,
    events: &[crate::events::NexusEvent],
) -> Result<ScheduleChanges, NexusError> {
    let mut changes = ScheduleChanges::default();

    for event in events {
        let event_task_id = scheduler_event_task_id(&event.data);
        if let Some(event_task_id) = event_task_id {
            if event_task_id != task_id {
                return Err(NexusError::Parsing(anyhow!(
                    "Scheduler transaction for Task '{task_id}' contains an event for Task '{event_task_id}'"
                )));
            }
        }

        match &event.data {
            NexusEventKind::OccurrenceScheduled(event) => {
                changes.scheduled.push(scheduled_occurrence(event));
            }
            NexusEventKind::OccurrenceWithdrawn(event) => {
                changes.withdrawn.push(withdrawn_occurrence(event));
            }
            NexusEventKind::OccurrenceAdvertised(event) => {
                changes.advertised = Some(OccurrenceRef::new(task_id, event.occurrence_id));
            }
            _ => {}
        }
    }

    Ok(changes)
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

fn scheduled_occurrence(event: &OccurrenceScheduledEvent) -> ScheduledOccurrence {
    ScheduledOccurrence {
        reference: OccurrenceRef::new(event.task_id.bytes, event.occurrence_id),
        start_time_ms: event.start_time_ms,
        deadline_ms: event.deadline_ms.copied_option(),
        priority_fee_percentage: event.priority_fee_percentage,
        source: event.source,
    }
}

fn withdrawn_occurrence(event: &OccurrenceWithdrawnEvent) -> WithdrawnOccurrence {
    WithdrawnOccurrence {
        reference: OccurrenceRef::new(event.task_id.bytes, event.occurrence_id),
        reason: event.reason,
    }
}

fn mutation_result(response: ExecutedTransaction) -> SchedulerMutationResult {
    SchedulerMutationResult {
        tx_digest: response.digest,
        tx_checkpoint: response.checkpoint,
    }
}

fn schedule_result(
    response: ExecutedTransaction,
    task_id: sui::types::Address,
) -> Result<ScheduleMutationResult, NexusError> {
    let changes = extract_schedule_changes(task_id, &response.events)?;

    Ok(ScheduleMutationResult {
        tx_digest: response.digest,
        tx_checkpoint: response.checkpoint,
        task_id,
        scheduled: changes.scheduled,
        withdrawn: changes.withdrawn,
        advertised: changes.advertised,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::{NexusEvent, NexusEventKind},
            move_bindings::{
                move_std::option::Option as MoveOption,
                primitives::data::NexusData,
                scheduler::{
                    schedule::{OccurrenceSource, OccurrenceWithdrawalReason},
                    scheduler::{OccurrenceAdvertised, OccurrenceScheduled, OccurrenceWithdrawn},
                },
                sui_framework::object::ID,
            },
            test_utils::sui_mocks::mock_nexus_objects,
        },
    };

    fn scheduler_event(index: u64, data: NexusEventKind) -> NexusEvent {
        NexusEvent {
            id: (sui::types::Digest::ZERO, index),
            generics: Vec::new(),
            data,
            distribution: None,
        }
    }

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    #[test]
    fn occurrence_ref_derives_execution_id() {
        let occurrence = OccurrenceRef::new(address("0x31"), 7);

        assert_eq!(
            occurrence.execution_id().expect("identity derives"),
            derive_task_execution_id(address("0x31"), 7).expect("identity derives")
        );
    }

    #[test]
    fn mutation_collects_every_schedule_change_and_final_advertisement() {
        let task_id = address("0x41");
        let events = vec![
            scheduler_event(
                0,
                NexusEventKind::OccurrenceScheduled(OccurrenceScheduled::new(
                    ID::new(task_id),
                    1,
                    100,
                    MoveOption::from_option(Some(200)),
                    5,
                    OccurrenceSource::Standalone,
                )),
            ),
            scheduler_event(
                1,
                NexusEventKind::OccurrenceAdvertised(OccurrenceAdvertised::new(
                    ID::new(task_id),
                    1,
                    100,
                    MoveOption::from_option(Some(200)),
                    5,
                    OccurrenceSource::Standalone,
                )),
            ),
            scheduler_event(
                2,
                NexusEventKind::OccurrenceWithdrawn(OccurrenceWithdrawn::new(
                    ID::new(task_id),
                    1,
                    OccurrenceWithdrawalReason::RecurrenceReplaced,
                )),
            ),
            scheduler_event(
                3,
                NexusEventKind::OccurrenceScheduled(OccurrenceScheduled::new(
                    ID::new(task_id),
                    2,
                    300,
                    MoveOption::from_option(None),
                    10,
                    OccurrenceSource::Recurring { iteration: 0 },
                )),
            ),
            scheduler_event(
                4,
                NexusEventKind::OccurrenceAdvertised(OccurrenceAdvertised::new(
                    ID::new(task_id),
                    2,
                    300,
                    MoveOption::from_option(None),
                    10,
                    OccurrenceSource::Recurring { iteration: 0 },
                )),
            ),
        ];

        let changes = extract_schedule_changes(task_id, &events).expect("changes extract");

        assert_eq!(changes.scheduled.len(), 2);
        assert_eq!(
            changes.scheduled[0].reference,
            OccurrenceRef::new(task_id, 1)
        );
        assert_eq!(
            changes.scheduled[1].reference,
            OccurrenceRef::new(task_id, 2)
        );
        assert_eq!(changes.withdrawn.len(), 1);
        assert_eq!(
            changes.withdrawn[0].reason,
            OccurrenceWithdrawalReason::RecurrenceReplaced
        );
        assert_eq!(changes.advertised, Some(OccurrenceRef::new(task_id, 2)));
    }

    #[test]
    fn mutation_rejects_mismatched_task_events() {
        let task_id = address("0x51");
        let events = vec![scheduler_event(
            0,
            NexusEventKind::OccurrenceScheduled(OccurrenceScheduled::new(
                ID::new(address("0x52")),
                1,
                100,
                MoveOption::from_option(None),
                0,
                OccurrenceSource::Standalone,
            )),
        )];

        let error =
            extract_schedule_changes(task_id, &events).expect_err("mismatched Task must fail");

        assert!(error.to_string().contains(&address("0x52").to_string()));
        assert!(error.to_string().contains(&address("0x51").to_string()));
    }

    #[test]
    fn execution_inputs_have_stable_vertex_and_port_order() {
        let inputs = execution_inputs(HashMap::from([
            (
                "z".to_string(),
                HashMap::from([("b".to_string(), NexusData::inline_one([]))]),
            ),
            (
                "a".to_string(),
                HashMap::from([
                    ("y".to_string(), NexusData::inline_one([])),
                    ("x".to_string(), NexusData::inline_one([])),
                ]),
            ),
        ]));

        assert_eq!(inputs.contents[0].key.as_str(), "a");
        assert_eq!(inputs.contents[0].value.contents[0].key.as_str(), "x");
        assert_eq!(inputs.contents[0].value.contents[1].key.as_str(), "y");
        assert_eq!(inputs.contents[1].key.as_str(), "z");
    }

    #[test]
    fn default_execution_config_has_no_authorization_templates() {
        let objects = mock_nexus_objects();
        let sender = sui::types::Address::from_static("0x42");
        let config = build_operation_config(
            &objects,
            sender,
            &TaskOperation::Default {
                dag_id: sui::types::Address::from_static("0x43"),
            },
            "entry".to_string(),
            HashMap::new(),
        );

        assert!(matches!(
            config.selection,
            ExecutionSelection::DefaultAgent { .. }
        ));
        assert_eq!(config.invoker, sender);
        assert!(config.authorization_templates.is_empty());
    }
}
