//! Scheduler actions exposed through [`NexusClient`].

pub use scheduler_tx::{OccurrenceSpec, RecurrenceSpec, TaskFailureMode, TaskStateAction};
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
                scheduler::OccurrenceAdvertised,
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
    std::collections::HashMap,
};

/// High level scheduler operations.
#[derive(Clone)]
pub struct SchedulerActions {
    pub(super) client: NexusClient,
}

/// Execution selected for every occurrence of a [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub enum TaskExecution {
    /// Uses the configured default Agent to execute a published DAG.
    Default { dag_id: sui::types::Address },
    /// Uses one registered Agent skill.
    AgentSkill {
        agent_id: AgentId,
        skill_id: SkillId,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
}

impl TaskExecution {
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
        prepay_amount_mist: u64,
        refund_recipient: Option<sui::types::Address>,
    },
    /// Uses an Agent vault and Agent control.
    Agent { prepay_amount_mist: u64 },
}

/// Complete input for one [`Task`] creation transaction.
///
/// Manual occurrences and recurrence are composed before the Task is shared.
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub struct CreateTaskParams {
    pub execution: TaskExecution,
    pub entry_group: String,
    pub input_data: HashMap<String, HashMap<String, NexusData>>,
    pub funding: TaskFunding,
    pub occurrence_budget_mist: u64,
    pub failure_mode: TaskFailureMode,
    pub occurrences: Vec<OccurrenceSpec>,
    pub recurrence: Option<RecurrenceSpec>,
}

/// Result of creating and composing one [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub struct CreateTaskResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub task_id: sui::types::Address,
    pub advertised: Option<OccurrenceAdvertised>,
}

/// Result of one scheduler mutation.
#[derive(Clone, Debug)]
pub struct SchedulerMutationResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
}

/// Result of a mutation that may advertise the next occurrence.
#[derive(Clone, Debug)]
pub struct ScheduleMutationResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub advertised: Option<OccurrenceAdvertised>,
}

impl SchedulerActions {
    /// Creates and fully composes one [`Task`] in a single transaction.
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn create_task(
        &self,
        params: CreateTaskParams,
    ) -> Result<CreateTaskResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let agent = match params.execution.agent_id() {
            Some(agent_id) => Some(self.agent_input(agent_id).await?),
            None => None,
        };
        let execution = build_execution_config(
            &self.client.nexus_objects,
            sender,
            &params.execution,
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
                    "default execution cannot use Agent vault funding".into(),
                ));
            }
        };
        let tx = scheduler_tx::create_task_ptb(
            &self.client.nexus_objects,
            &scheduler_tx::CreateTaskParams {
                execution,
                funding,
                occurrence_budget_mist: params.occurrence_budget_mist,
                failure_mode: params.failure_mode,
                occurrences: params.occurrences,
                recurrence: params.recurrence,
            },
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;

        Ok(CreateTaskResult {
            task_id: extract_task_id(&response)?,
            advertised: extract_advertisement(&response),
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
        })
    }

    /// Adds one manual occurrence to a [`Task`].
    ///
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn schedule(
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
        Ok(schedule_result(response))
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
        Ok(schedule_result(response))
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
        Ok(schedule_result(response))
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
        Ok(schedule_result(response))
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
        Ok(schedule_result(response))
    }

    /// Closes a [`Task`] after all work and settlement are complete.
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
        task_id: sui::types::Address,
        occurrence_id: u64,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let tx = scheduler_tx::expire_occurrence_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            occurrence_id,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        Ok(schedule_result(response))
    }

    /// Settles a finished [`DAGExecution`] into its owning [`Task`].
    ///
    /// [`DAGExecution`]: crate::move_bindings::workflow::execution::DAGExecution
    /// [`Task`]: crate::move_bindings::scheduler::task::Task
    pub async fn settle(
        &self,
        task_id: sui::types::Address,
        execution_id: sui::types::Address,
    ) -> Result<ScheduleMutationResult, NexusError> {
        let sender = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;
        let execution = self
            .client
            .crawler()
            .get_object::<DAGExecution>(execution_id)
            .await
            .map_err(NexusError::Rpc)?;
        let tx = scheduler_tx::settle_occurrence_ptb(
            &self.client.nexus_objects,
            &task.object_ref(),
            &execution.object_ref(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, sender).await?;
        Ok(schedule_result(response))
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

fn build_execution_config(
    objects: &crate::types::NexusObjects,
    sender: sui::types::Address,
    execution: &TaskExecution,
    entry_group: String,
    input_data: HashMap<String, HashMap<String, NexusData>>,
) -> AgentExecutionConfig {
    AgentExecutionConfig::new(
        execution.selection(),
        ID::new(objects.network_id),
        EntryGroup::new(entry_group),
        execution_inputs(input_data),
        sender,
        execution.authorization_templates(),
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

fn extract_advertisement(response: &ExecutedTransaction) -> Option<OccurrenceAdvertised> {
    response.events.iter().find_map(|event| match &event.data {
        NexusEventKind::OccurrenceAdvertised(event) => Some(event.clone()),
        _ => None,
    })
}

fn mutation_result(response: ExecutedTransaction) -> SchedulerMutationResult {
    SchedulerMutationResult {
        tx_digest: response.digest,
        tx_checkpoint: response.checkpoint,
    }
}

fn schedule_result(response: ExecutedTransaction) -> ScheduleMutationResult {
    ScheduleMutationResult {
        advertised: extract_advertisement(&response),
        tx_digest: response.digest,
        tx_checkpoint: response.checkpoint,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::primitives::data::NexusData,
            test_utils::sui_mocks::mock_nexus_objects,
        },
    };

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
        let config = build_execution_config(
            &objects,
            sender,
            &TaskExecution::Default {
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
