//! Scheduler-oriented actions exposed through [`NexusClient`].

use {
    crate::{
        events::NexusEventKind,
        idents::{scheduler, sui_framework, workflow, ModuleAndNameIdent},
        nexus::{
            client::NexusClient,
            crawler::{Crawler, DynamicMap, Response},
            error::NexusError,
            signer::ExecutedTransaction,
        },
        sui,
        transactions::scheduler as scheduler_tx,
        types::{
            deserialize_sui_u64,
            AgentExecutionConfig,
            AgentId,
            AgentVertexAuthorizationTemplate,
            DataStorage,
            ExecutionSelection,
            MoveFields,
            MoveOption,
            NexusObjects,
            PolicySymbol,
            SkillId,
            Task,
            TransitionConfigKey,
            TypeName,
        },
    },
    anyhow::{anyhow, bail},
    std::collections::HashMap,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ScheduledAgentExecutionConfig {
    Default(AgentExecutionConfig),
    Registered(AgentExecutionConfig),
}

impl ScheduledAgentExecutionConfig {
    pub fn dag(&self) -> Option<sui::types::Address> {
        match self {
            Self::Default(config) | Self::Registered(config) => config.selection.dag_id(),
        }
    }

    /// Mirror of Move's `agent_registry::resolve_agent_execution_config_dag`.
    /// For default-executor and runtime-selected skills the dag is encoded
    /// directly in the config; for pinned skills it comes from the skill
    /// record's `dag_binding`.
    pub async fn resolve_dag(
        &self,
        crawler: &Crawler,
        objects: &NexusObjects,
    ) -> anyhow::Result<sui::types::Address> {
        if let Some(dag) = self.dag() {
            return Ok(dag);
        }
        let (agent_id, skill_id) = match self {
            Self::Default(_) => bail!("default-agent scheduled config is missing its DAG id"),
            Self::Registered(config) => match &config.selection {
                ExecutionSelection::AgentSkill {
                    agent_id, skill_id, ..
                } => (*agent_id, *skill_id),
                ExecutionSelection::DefaultAgent { .. } => {
                    bail!("registered scheduled config carries a default-agent selection",)
                }
            },
        };
        let target = crate::nexus::tap::fetch_configured_active_tap_skill_execution_target(
            crawler, objects, agent_id, skill_id,
        )
        .await?;
        match target.data.skill.dag_binding {
            crate::types::SkillDagBinding::Pinned { dag_id } => Ok(dag_id),
            crate::types::SkillDagBinding::RuntimeSelected => bail!(
                "scheduled agent execution config for runtime-selected skill {skill_id} \
                 of agent {agent_id} is missing the selected dag id"
            ),
        }
    }
}

/// High-level interface for scheduler operations.
#[derive(Clone)]
pub struct SchedulerActions {
    pub(super) client: NexusClient,
}

/// Supported generator types for a scheduled task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorKind {
    Queue,
    Periodic,
}

impl From<GeneratorKind> for scheduler_tx::OccurrenceGenerator {
    fn from(value: GeneratorKind) -> Self {
        match value {
            GeneratorKind::Queue => scheduler_tx::OccurrenceGenerator::Queue,
            GeneratorKind::Periodic => scheduler_tx::OccurrenceGenerator::Periodic,
        }
    }
}

/// Parameters required to create a scheduled task.
pub struct CreateTaskParams {
    pub dag_id: sui::types::Address,
    pub entry_group: String,
    pub input_data: HashMap<String, HashMap<String, DataStorage>>,
    pub metadata: Vec<(String, String)>,
    pub execution_priority_fee_per_gas_unit: u64,
    pub initial_schedule: Option<OccurrenceRequest>,
    pub generator: GeneratorKind,
    /// When both `agent_id` and `skill_id` are supplied, the task is built
    /// with the agent-bound execution policy (`AdvanceForAgentExecution`)
    /// so the workflow dispatches walks against `(agent, skill)` rather
    /// than the default DAG-execution policy. Either both must be set or
    /// neither; supplying just one is rejected with `NexusError::Configuration`.
    pub agent_id: Option<AgentId>,
    pub skill_id: Option<SkillId>,
    pub tap_payment: Option<CreateTaskTapPayment>,
}

#[derive(Clone, Debug)]
pub enum CreateTaskTapPayment {
    /// Reserve funded from the transaction sender's SUI balance.
    UserFunded {
        prepay_amount: u64,
        refund_recipient: Option<sui::types::Address>,
        occurrence_budget: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
    /// Reserve funded from the selected agent's vault.
    AgentFunded {
        prepay_amount: u64,
        occurrence_budget: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
}

impl CreateTaskTapPayment {
    /// Caller-supplied `selected_dag`. `None` is the on-chain shape for pinned
    /// skills (the stored `AgentExecutionConfig` carries no dag; the resolver
    /// reads it from the skill's `dag_binding`). `Some(_)` is required for
    /// runtime-selected skills.
    pub fn selected_dag(&self) -> Option<sui::types::Address> {
        match self {
            Self::UserFunded { selected_dag, .. } | Self::AgentFunded { selected_dag, .. } => {
                *selected_dag
            }
        }
    }
}

/// Result returned after creating a scheduled task.
pub struct CreateTaskResult {
    pub tx_digest: sui::types::Digest,
    pub task_id: sui::types::Address,
    pub initial_schedule: Option<ScheduleExecutionResult>,
    pub tap_payment: Option<CreateTaskTapPaymentResult>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateTaskTapPaymentResult {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub prepay_amount: u64,
    pub occurrence_budget: u64,
}

/// Result returned after enqueuing an occurrence.
pub struct ScheduleExecutionResult {
    pub tx_digest: sui::types::Digest,
    pub event: Option<NexusEventKind>,
}

/// Parameters for a sporadic occurrence (start/deadline offsets).
#[derive(Clone, Debug)]
pub struct OccurrenceRequest {
    pub start_ms: Option<u64>,
    pub deadline_ms: Option<u64>,
    pub start_offset_ms: Option<u64>,
    pub deadline_offset_ms: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
}

impl OccurrenceRequest {
    pub fn new(
        start_ms: Option<u64>,
        deadline_ms: Option<u64>,
        start_offset_ms: Option<u64>,
        deadline_offset_ms: Option<u64>,
        priority_fee_per_gas_unit: u64,
        require_start: bool,
    ) -> Result<Self, NexusError> {
        validate_schedule_options(
            start_ms,
            deadline_ms,
            start_offset_ms,
            deadline_offset_ms,
            require_start,
        )?;

        Ok(Self {
            start_ms,
            deadline_ms,
            start_offset_ms,
            deadline_offset_ms,
            priority_fee_per_gas_unit,
        })
    }
}

/// Actions supported when mutating task state.
#[derive(Clone, Copy, Debug)]
pub enum TaskStateAction {
    Pause,
    Resume,
    Cancel,
}

/// Configuration for periodic scheduling.
pub struct PeriodicScheduleConfig {
    pub first_start_ms: u64,
    pub period_ms: u64,
    pub deadline_offset_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
}

pub struct PeriodicScheduleResult {
    pub tx_digest: sui::types::Digest,
    pub event: Option<NexusEventKind>,
}

pub struct DisablePeriodicResult {
    pub tx_digest: sui::types::Digest,
}

pub struct UpdateMetadataResult {
    pub tx_digest: sui::types::Digest,
    pub entries: usize,
}

pub struct TaskStateResult {
    pub tx_digest: sui::types::Digest,
    pub state: TaskStateAction,
}

fn create_task_payment_result(
    tap_payment: &Option<CreateTaskTapPayment>,
    agent_binding: Option<(AgentId, SkillId)>,
    objects: &NexusObjects,
) -> Result<Option<CreateTaskTapPaymentResult>, NexusError> {
    let Some(tap_payment) = tap_payment else {
        return Ok(None);
    };

    match tap_payment {
        CreateTaskTapPayment::UserFunded {
            prepay_amount,
            occurrence_budget,
            ..
        } => {
            let (agent_id, skill_id) = agent_binding.unwrap_or((
                objects.default_dag_executor.agent_id,
                objects.default_dag_executor.skill_id,
            ));

            Ok(Some(CreateTaskTapPaymentResult {
                agent_id,
                skill_id,
                prepay_amount: *prepay_amount,
                occurrence_budget: *occurrence_budget,
            }))
        }
        CreateTaskTapPayment::AgentFunded {
            prepay_amount,
            occurrence_budget,
            ..
        } => {
            let Some((agent_id, skill_id)) = agent_binding else {
                return Err(NexusError::Configuration(
                    "agent-funded scheduled payment requires both scheduled task agent_id and skill_id"
                        .into(),
                ));
            };

            Ok(Some(CreateTaskTapPaymentResult {
                agent_id,
                skill_id,
                prepay_amount: *prepay_amount,
                occurrence_budget: *occurrence_budget,
            }))
        }
    }
}

impl SchedulerActions {
    /// Create a scheduled task and optionally enqueue its first occurrence.
    pub async fn create_task(
        &self,
        params: CreateTaskParams,
    ) -> Result<CreateTaskResult, NexusError> {
        let CreateTaskParams {
            dag_id,
            entry_group,
            input_data,
            metadata,
            execution_priority_fee_per_gas_unit,
            initial_schedule: initial_schedule_request,
            generator,
            agent_id,
            skill_id,
            tap_payment,
        } = params;
        let agent_binding = match (agent_id, skill_id) {
            (Some(agent_id), Some(skill_id)) => Some((agent_id, skill_id)),
            (None, None) => None,
            _ => {
                return Err(NexusError::Configuration(
                    "Scheduled task agent_id and skill_id must both be set or both be unset".into(),
                ));
            }
        };
        if let Some((agent_id, skill_id)) = agent_binding {
            let payment = match tap_payment {
                Some(CreateTaskTapPayment::UserFunded {
                    prepay_amount,
                    refund_recipient,
                    occurrence_budget,
                    selected_dag,
                    authorization_templates,
                }) => crate::nexus::tap::AgentTaskPayment::AddressFunded {
                    prepay_amount,
                    refund_recipient,
                    occurrence_budget,
                    selected_dag,
                    authorization_templates,
                },
                Some(CreateTaskTapPayment::AgentFunded {
                    prepay_amount,
                    occurrence_budget,
                    selected_dag,
                    authorization_templates,
                }) => crate::nexus::tap::AgentTaskPayment::AgentVault {
                    prepay_amount,
                    occurrence_budget,
                    selected_dag,
                    authorization_templates,
                },
                None => {
                    return Err(NexusError::Configuration(
                        "agent-bound scheduled tasks require an invoker-funded or agent-funded scheduled payment"
                            .into(),
                    ));
                }
            };

            return self
                .client
                .tap()
                .create_agent_task(crate::nexus::tap::CreateAgentTaskParams {
                    entry_group,
                    input_data,
                    metadata,
                    execution_priority_fee_per_gas_unit,
                    initial_schedule: initial_schedule_request,
                    generator,
                    agent_id,
                    skill_id,
                    payment,
                })
                .await;
        }
        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;

        let mut tx = sui::tx::TransactionBuilder::new();
        let tap_payment_result =
            create_task_payment_result(&tap_payment, agent_binding, &self.client.nexus_objects)?;

        let metadata_arg = scheduler_tx::new_metadata(&mut tx, objects, metadata.iter().cloned())
            .map_err(NexusError::TransactionBuilding)?;

        let constraints_arg =
            scheduler_tx::new_constraints_policy(&mut tx, objects, generator.into())
                .map_err(NexusError::TransactionBuilding)?;

        let execution_arg = scheduler_tx::new_execution_policy(
            &mut tx,
            objects,
            dag_id,
            execution_priority_fee_per_gas_unit,
            entry_group.as_str(),
            &input_data,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let registry = tx.object(sui::tx::ObjectInput::shared(
            *objects.agent_registry.object_id(),
            objects.agent_registry.version(),
            true,
        ));
        let task = match &tap_payment {
            Some(CreateTaskTapPayment::UserFunded {
                prepay_amount,
                occurrence_budget,
                refund_recipient,
                selected_dag,
                authorization_templates,
            }) => {
                if refund_recipient.is_some()
                    || selected_dag.is_some()
                    || !authorization_templates.is_empty()
                {
                    return Err(NexusError::Configuration(
                        "default-agent scheduled task user funding does not accept refund_recipient, selected_dag, or authorization_templates"
                            .into(),
                    ));
                }
                let prepay_amount = tx.pure(prepay_amount);
                let gas = tx.gas();
                let prepayment_coin = tx
                    .split_coins(gas, vec![prepay_amount])
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        NexusError::TransactionBuilding(anyhow!(
                            "failed to split default scheduled prepayment coin"
                        ))
                    })?;
                scheduler_tx::new_default_agent_task(
                    &mut tx,
                    objects,
                    metadata_arg,
                    constraints_arg,
                    execution_arg,
                    registry,
                    prepayment_coin,
                    *occurrence_budget,
                )
            }
            Some(CreateTaskTapPayment::AgentFunded { .. }) => {
                return Err(NexusError::Configuration(
                    "agent-funded scheduled payment requires both scheduled task agent_id and skill_id"
                        .into(),
                ));
            }
            None => {
                return Err(NexusError::Configuration(
                    "default-agent scheduled task creation requires user-funded tap_payment".into(),
                ));
            }
        }
        .map_err(NexusError::TransactionBuilding)?;

        let task_type =
            scheduler::into_type_tag(objects.scheduler_pkg_id, scheduler::Scheduler::TASK);

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            )
            .with_type_args(vec![task_type]),
            vec![task],
        );

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        let task_id = extract_task_id(&response)?;

        let mut initial_schedule_result = None;
        if initial_schedule_request.is_some() && generator != GeneratorKind::Queue {
            return Err(NexusError::Configuration(
                "Initial queue schedule can only be used with the queue generator".into(),
            ));
        }

        if let Some(schedule) = initial_schedule_request {
            let task_object = self.fetch_task(task_id).await?;
            initial_schedule_result = Some(
                self.enqueue_occurrence(&task_object, schedule, address)
                    .await?,
            );
        }

        Ok(CreateTaskResult {
            tx_digest: response.digest,
            task_id,
            initial_schedule: initial_schedule_result,
            tap_payment: tap_payment_result,
        })
    }

    /// Update metadata entries associated with a task.
    pub async fn update_metadata(
        &self,
        task_id: sui::types::Address,
        metadata: Vec<(String, String)>,
    ) -> Result<UpdateMetadataResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;

        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        let metadata_arg = scheduler_tx::new_metadata(&mut tx, objects, metadata.clone())
            .map_err(NexusError::TransactionBuilding)?;

        scheduler_tx::update_metadata(&mut tx, objects, &task_ref, metadata_arg)
            .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(UpdateMetadataResult {
            tx_digest: response.digest,
            entries: metadata.len(),
        })
    }

    /// Set the scheduler state for a task (pause/resume/cancel).
    pub async fn set_task_state(
        &self,
        task_id: sui::types::Address,
        request: TaskStateAction,
    ) -> Result<TaskStateResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;

        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        match request {
            TaskStateAction::Pause => {
                scheduler_tx::pause_time_constraint_for_task(&mut tx, objects, &task_ref)
            }
            TaskStateAction::Resume => {
                scheduler_tx::resume_time_constraint_for_task(&mut tx, objects, &task_ref)
            }
            TaskStateAction::Cancel => {
                scheduler_tx::cancel_time_constraint_for_task(&mut tx, objects, &task_ref)
            }
        }
        .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(TaskStateResult {
            tx_digest: response.digest,
            state: request,
        })
    }

    /// Add a sporadic occurrence for an existing task.
    pub async fn add_occurrence(
        &self,
        task_id: sui::types::Address,
        request: OccurrenceRequest,
    ) -> Result<ScheduleExecutionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let task = self.fetch_task(task_id).await?;

        self.enqueue_occurrence(&task, request, address).await
    }

    /// Configure or update periodic scheduling for a task.
    pub async fn configure_periodic(
        &self,
        task_id: sui::types::Address,
        config: PeriodicScheduleConfig,
    ) -> Result<PeriodicScheduleResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;
        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        scheduler_tx::new_or_modify_periodic_for_task(
            &mut tx,
            objects,
            &task_ref,
            scheduler_tx::PeriodicScheduleInputs {
                first_start_ms: config.first_start_ms,
                period_ms: config.period_ms,
                deadline_offset_ms: config.deadline_offset_ms,
                max_iterations: config.max_iterations,
                priority_fee_per_gas_unit: config.priority_fee_per_gas_unit,
            },
        )
        .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(PeriodicScheduleResult {
            tx_digest: response.digest,
            event: extract_occurrence_event(&response),
        })
    }

    /// Disable periodic scheduling for a task.
    pub async fn disable_periodic(
        &self,
        task_id: sui::types::Address,
    ) -> Result<DisablePeriodicResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;
        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        scheduler_tx::disable_periodic_for_task(&mut tx, objects, &task_ref)
            .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(DisablePeriodicResult {
            tx_digest: response.digest,
        })
    }

    pub(crate) async fn enqueue_occurrence(
        &self,
        task: &Response<Task>,
        request: OccurrenceRequest,
        address: sui::types::Address,
    ) -> Result<ScheduleExecutionResult, NexusError> {
        let mut tx = sui::tx::TransactionBuilder::new();
        let objects = &self.client.nexus_objects;
        let task_ref = task.object_ref();

        // Agent-funded tasks (created via `new_agent_funded_task`) are owned
        // by the agent's id rather than the sender. The Move scheduler enforces
        // `for_task` against `task.owner`, so the regular sender-keyed helpers
        // fail; route through the `*_for_agent_funded_task` variants and pass
        // the agent as an input.
        let agent_arg = if task.data.owner == task.data.agent_id {
            let agent_ref = self
                .client
                .crawler()
                .get_object_metadata(task.data.agent_id)
                .await
                .map_err(NexusError::Rpc)?;
            Some(
                crate::nexus::tap::agent_argument_from_metadata(&mut tx, &agent_ref, false)
                    .map_err(NexusError::TransactionBuilding)?,
            )
        } else {
            None
        };

        let build_result = if let Some(start_ms) = request.start_ms {
            let deadline_offset = request
                .deadline_offset_ms
                .or_else(|| request.deadline_ms.map(|deadline| deadline - start_ms));

            if let Some(agent) = agent_arg {
                scheduler_tx::add_occurrence_absolute_for_agent_funded_task(
                    &mut tx,
                    objects,
                    &task_ref,
                    agent,
                    start_ms,
                    deadline_offset,
                    request.priority_fee_per_gas_unit,
                )
            } else {
                scheduler_tx::add_occurrence_absolute_for_task(
                    &mut tx,
                    objects,
                    &task_ref,
                    start_ms,
                    deadline_offset,
                    request.priority_fee_per_gas_unit,
                )
            }
        } else if let Some(agent) = agent_arg {
            scheduler_tx::add_occurrence_relative_for_agent_funded_task(
                &mut tx,
                objects,
                &task_ref,
                agent,
                request.start_offset_ms.expect("validated start offset"),
                request.deadline_offset_ms,
                request.priority_fee_per_gas_unit,
            )
        } else {
            scheduler_tx::add_occurrence_relative_for_task(
                &mut tx,
                objects,
                &task_ref,
                request.start_offset_ms.expect("validated start offset"),
                request.deadline_offset_ms,
                request.priority_fee_per_gas_unit,
            )
        };
        build_result.map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(ScheduleExecutionResult {
            tx_digest: response.digest,
            event: extract_occurrence_event(&response),
        })
    }

    pub(crate) async fn fetch_task(
        &self,
        task_id: sui::types::Address,
    ) -> Result<Response<Task>, NexusError> {
        self.client
            .crawler()
            .get_object::<Task>(task_id)
            .await
            .map_err(NexusError::Rpc)
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SchedulerOccurrence {
    #[serde(deserialize_with = "deserialize_sui_u64")]
    start_time_ms: u64,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SchedulerQueueEntry {
    occurrence: SchedulerOccurrence,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SchedulerQueueGeneratorState {
    active: MoveOption<SchedulerQueueEntry>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SchedulerPeriodicGeneratorState {
    active: MoveOption<SchedulerOccurrence>,
}

pub fn active_scheduler_start_ms<F>(
    configs: impl IntoIterator<Item = (TransitionConfigKey<u64, PolicySymbol>, serde_json::Value)>,
    expected_config: &str,
    generator: GeneratorKind,
    symbol_matches_generator: F,
) -> anyhow::Result<Option<u64>>
where
    F: Fn(&PolicySymbol) -> bool,
{
    let mut candidates = configs.into_iter().filter(|(key, _)| {
        key.config.matches_qualified_name(expected_config)
            && key.transition.state.is_none()
            && symbol_matches_generator(&key.transition.symbol)
    });

    let Some((_key, value)) = candidates.next() else {
        bail!("Missing scheduler generator state config for {expected_config}");
    };

    let active_start_ms = match generator {
        GeneratorKind::Queue => {
            let MoveFields(state): MoveFields<SchedulerQueueGeneratorState> =
                serde_json::from_value(value)?;
            state.active.0.map(|entry| entry.occurrence.start_time_ms)
        }
        GeneratorKind::Periodic => {
            let MoveFields(state): MoveFields<SchedulerPeriodicGeneratorState> =
                serde_json::from_value(value)?;
            state.active.0.map(|occ| occ.start_time_ms)
        }
    };

    Ok(active_start_ms)
}

pub async fn fetch_begin_default_agent_execution_config(
    crawler: &Crawler,
    objects: &NexusObjects,
    configured_automaton_id: &sui::types::Address,
) -> anyhow::Result<AgentExecutionConfig> {
    let parent: DynamicMap<TransitionConfigKey<u64, PolicySymbol>, serde_json::Value> =
        DynamicMap::new(*configured_automaton_id, 0);

    let configs = crawler.get_dynamic_fields(&parent).await?;

    extract_begin_default_agent_execution_config(configs, objects)
}

pub async fn fetch_scheduled_agent_execution_config(
    crawler: &Crawler,
    objects: &NexusObjects,
    configured_automaton_id: &sui::types::Address,
) -> anyhow::Result<ScheduledAgentExecutionConfig> {
    let parent: DynamicMap<TransitionConfigKey<u64, PolicySymbol>, serde_json::Value> =
        DynamicMap::new(*configured_automaton_id, 0);

    let configs = crawler.get_dynamic_fields(&parent).await?;

    extract_scheduled_agent_execution_config(configs, objects)
}

fn extract_begin_default_agent_execution_config(
    configs: impl IntoIterator<Item = (TransitionConfigKey<u64, PolicySymbol>, serde_json::Value)>,
    objects: &NexusObjects,
) -> anyhow::Result<AgentExecutionConfig> {
    let configs = configs.into_iter().collect::<Vec<_>>();
    let expected_symbol = &workflow::ExecutionEntries::ADVANCE_FOR_DEFAULT_AGENT_EXECUTION_TYPE;

    let Some(value) = find_execution_config_value(&configs, expected_symbol, objects) else {
        bail!(
            "Missing execution policy config for AdvanceForDefaultAgentExecution; {}",
            describe_transition_config_keys(&configs)
        );
    };

    let config = decode_agent_execution_config(value, "AdvanceForDefaultAgentExecution config")?;
    if !matches!(config.selection, ExecutionSelection::DefaultAgent { .. }) {
        bail!("AdvanceForDefaultAgentExecution config is not a default-agent selection");
    }
    Ok(config)
}

fn extract_scheduled_agent_execution_config(
    configs: impl IntoIterator<Item = (TransitionConfigKey<u64, PolicySymbol>, serde_json::Value)>,
    objects: &NexusObjects,
) -> anyhow::Result<ScheduledAgentExecutionConfig> {
    let configs = configs.into_iter().collect::<Vec<_>>();
    let default_symbol = &workflow::ExecutionEntries::ADVANCE_FOR_DEFAULT_AGENT_EXECUTION_TYPE;
    if let Some(value) = find_execution_config_value(&configs, default_symbol, objects) {
        let config =
            decode_agent_execution_config(value, "AdvanceForDefaultAgentExecution config")?;
        if !matches!(config.selection, ExecutionSelection::DefaultAgent { .. }) {
            bail!("AdvanceForDefaultAgentExecution config is not a default-agent selection");
        }
        return Ok(ScheduledAgentExecutionConfig::Default(config));
    }

    let expected_symbol = &workflow::ExecutionEntries::ADVANCE_FOR_AGENT_EXECUTION_TYPE;

    let Some(value) = find_execution_config_value(&configs, expected_symbol, objects) else {
        bail!(
            "Missing execution policy config for AdvanceForDefaultAgentExecution or AdvanceForAgentExecution; {}",
            describe_transition_config_keys(&configs)
        );
    };

    let config = decode_agent_execution_config(value, "AdvanceForAgentExecution config")?;
    if !matches!(config.selection, ExecutionSelection::AgentSkill { .. }) {
        bail!("AdvanceForAgentExecution config is not an agent-skill selection");
    }
    Ok(ScheduledAgentExecutionConfig::Registered(config))
}

fn find_execution_config_value<'a>(
    configs: &'a [(TransitionConfigKey<u64, PolicySymbol>, serde_json::Value)],
    expected_symbol: &ModuleAndNameIdent,
    objects: &NexusObjects,
) -> Option<&'a serde_json::Value> {
    let expected_config = &crate::idents::interface::Agent::AGENT_EXECUTION_CONFIG;

    configs.iter().find_map(|(key, value)| {
        let matches_config =
            type_name_matches_ident(&key.config, expected_config, &[objects.interface_pkg_id]);
        let matches_symbol = policy_symbol_matches_ident(
            &key.transition.symbol,
            expected_symbol,
            &[
                objects.workflow_type_origin_pkg_id(),
                objects.workflow_pkg_id,
            ],
        );

        (matches_config && key.transition.state.is_none() && matches_symbol).then_some(value)
    })
}

fn decode_agent_execution_config(
    value: &serde_json::Value,
    label: &str,
) -> anyhow::Result<AgentExecutionConfig> {
    let MoveFields(config): MoveFields<AgentExecutionConfig> =
        serde_json::from_value(value.clone()).map_err(|err| anyhow!("decode {label}: {err}"))?;
    Ok(config)
}

fn describe_transition_config_keys(
    configs: &[(TransitionConfigKey<u64, PolicySymbol>, serde_json::Value)],
) -> String {
    let keys = configs
        .iter()
        .take(5)
        .map(|(key, _)| {
            format!(
                "state={:?}, config={}, symbol={}",
                key.transition.state,
                key.config.name,
                policy_symbol_display(&key.transition.symbol)
            )
        })
        .collect::<Vec<_>>();
    format!("fetched {} config(s): [{}]", configs.len(), keys.join(", "))
}

fn policy_symbol_display(symbol: &PolicySymbol) -> String {
    match symbol {
        PolicySymbol::Witness(name) => format!("witness({})", name.name),
        PolicySymbol::Uid(uid) => format!("uid({uid})"),
    }
}

fn policy_symbol_matches_ident(
    symbol: &PolicySymbol,
    ident: &ModuleAndNameIdent,
    package_ids: &[sui::types::Address],
) -> bool {
    let PolicySymbol::Witness(name) = symbol else {
        return false;
    };

    type_name_matches_ident(name, ident, package_ids)
}

fn type_name_matches_ident(
    name: &TypeName,
    ident: &ModuleAndNameIdent,
    package_ids: &[sui::types::Address],
) -> bool {
    if package_ids
        .iter()
        .any(|package_id| name.matches_qualified_name(&ident.qualified_name(*package_id)))
    {
        return true;
    }

    let expected_suffix = format!("::{}::{}", ident.module, ident.name);
    let actual = name.name.trim_start_matches("0x");
    actual
        .get(actual.len().saturating_sub(expected_suffix.len())..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(&expected_suffix))
}

pub(crate) fn extract_task_id(
    response: &ExecutedTransaction,
) -> Result<sui::types::Address, NexusError> {
    response
        .events
        .iter()
        .find_map(|event| match &event.data {
            NexusEventKind::ScheduledSkillExecutionCreated(e) => Some(e.task),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow!(
                "ScheduledSkillExecutionCreatedEvent not found in response"
            ))
        })
}

fn extract_occurrence_event(response: &ExecutedTransaction) -> Option<NexusEventKind> {
    response.events.iter().find_map(|event| match &event.data {
        NexusEventKind::RequestScheduledOccurrence(_) => Some(event.data.clone()),
        NexusEventKind::OccurrenceScheduled(_) => Some(event.data.clone()),
        _ => None,
    })
}

fn validate_schedule_options(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    require_start: bool,
) -> Result<(), NexusError> {
    if require_start && start_ms.is_none() && start_offset_ms.is_none() {
        return Err(NexusError::Configuration(
            "Provide either an absolute start or a start offset".into(),
        ));
    }

    if deadline_ms.is_some() && start_ms.is_none() {
        return Err(NexusError::Configuration(
            "Absolute deadlines require an absolute start time".into(),
        ));
    }

    if start_ms.is_none()
        && start_offset_ms.is_none()
        && (deadline_ms.is_some() || deadline_offset_ms.is_some())
    {
        return Err(NexusError::Configuration(
            "Deadline flags require a corresponding start flag".into(),
        ));
    }

    if let Some(start) = start_ms {
        ensure_start_before_deadline(Some(start), deadline_ms)?;
    }

    ensure_offset_deadline_valid(start_ms, start_offset_ms, deadline_offset_ms)?;

    Ok(())
}

fn ensure_start_before_deadline(
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
) -> Result<(), NexusError> {
    if let (Some(start), Some(deadline)) = (start_ms, deadline_ms) {
        if deadline < start {
            return Err(NexusError::Configuration(format!(
                "Deadline ({deadline}) cannot be earlier than start ({start})"
            )));
        }
    }
    Ok(())
}

fn ensure_offset_deadline_valid(
    start_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
) -> Result<(), NexusError> {
    if deadline_offset_ms.is_some() && start_offset_ms.is_none() && start_ms.is_none() {
        return Err(NexusError::Configuration(
            "Deadline offset requires either an absolute start or a start offset".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::{
                NexusEvent,
                NexusEventKind,
                OccurrenceScheduledEvent,
                RequestScheduledOccurrenceEvent,
                ScheduledSkillExecutionCreatedEvent,
            },
            nexus::{
                client::NexusClient,
                crawler::{Bag, Map, ObjectBag, TableVec},
                error::NexusError,
                signer::ExecutedTransaction,
            },
            sui,
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                ConfiguredAutomaton,
                ConstraintsData,
                DataStorage,
                DeterministicAutomaton,
                ExecutionData,
                Metadata,
                NexusData,
                NexusObjects,
                Policy,
                PolicySymbol,
                TaskState,
                TransitionConfigKey,
                TransitionKey,
                TypeName,
            },
        },
        rand::thread_rng,
        serde::Serialize,
        serde_json::json,
    };

    fn dummy_executed_transaction(events: Vec<NexusEvent>) -> ExecutedTransaction {
        let mut rng = thread_rng();

        ExecutedTransaction {
            effects: sui::types::TransactionEffectsV2 {
                status: sui::types::ExecutionStatus::Success,
                epoch: 0,
                gas_used: sui::types::GasCostSummary::new(0, 0, 0, 0),
                transaction_digest: sui::types::Digest::generate(&mut rng),
                gas_object_index: None,
                events_digest: None,
                dependencies: vec![],
                lamport_version: 0,
                changed_objects: vec![],
                unchanged_consensus_objects: vec![],
                auxiliary_data_digest: None,
            },
            events,
            objects: vec![],
            digest: sui::types::Digest::generate(&mut rng),
            checkpoint: 0,
        }
    }
    fn sample_input_data() -> HashMap<String, HashMap<String, DataStorage>> {
        HashMap::from([(
            "entry_vertex".to_string(),
            HashMap::from([(
                "entry_port".to_string(),
                NexusData::new_inline(json!("payload")).commit_inline_plain(),
            )]),
        )])
    }

    fn mock_task_object(
        task_id: sui::types::Address,
        owner: sui::types::Address,
    ) -> serde_json::Value {
        let metadata = Metadata {
            values: Map::from_iter([("initial".to_string(), "value".to_string())]),
        };

        let dfa = DeterministicAutomaton {
            states: TableVec::new(sui::types::Address::from_static("0x100"), 1),
            alphabet: TableVec::new(sui::types::Address::from_static("0x101"), 0),
            transition: TableVec::new(sui::types::Address::from_static("0x102"), 1),
            accepting: TableVec::new(sui::types::Address::from_static("0x103"), 1),
            start: 0,
        };

        let constraints = Policy {
            id: sui::types::Address::from_static("0x400"),
            dfa: ConfiguredAutomaton {
                id: sui::types::Address::from_static("0x401"),
                dfa: dfa.clone(),
            },
            state_index: 0,
            data: ConstraintsData::default(),
        };

        let execution = Policy {
            id: sui::types::Address::from_static("0x500"),
            dfa: ConfiguredAutomaton {
                id: sui::types::Address::from_static("0x600"),
                dfa,
            },
            state_index: 0,
            data: ExecutionData::default(),
        };

        let task = Task {
            id: task_id,
            owner,
            agent_id: sui::types::Address::from_static("0xa11ce"),
            skill_id: 7,
            interface_version: crate::types::InterfaceRevision(1),
            metadata,
            constraints,
            execution,
            state: TaskState::Active,
            data: Bag::new(sui::types::Address::from_static("0x700"), 0),
            objects: ObjectBag::new(sui::types::Address::from_static("0x701"), 0),
        };

        serde_json::to_value(task).expect("serialize task")
    }

    fn generator_symbol(scheduler_pkg_id: sui::types::Address, name: &str) -> PolicySymbol {
        PolicySymbol::Witness(TypeName::new(&format!(
            "{scheduler_pkg_id}::scheduler::{name}"
        )))
    }

    fn queue_generator_symbol(scheduler_pkg_id: sui::types::Address) -> PolicySymbol {
        generator_symbol(scheduler_pkg_id, "QueueGeneratorWitness")
    }

    fn scheduler_config(
        config_name: &str,
        start_time_ms: u64,
        symbol: PolicySymbol,
        generator: GeneratorKind,
        wrapped: bool,
    ) -> (TransitionConfigKey<u64, PolicySymbol>, serde_json::Value) {
        let inner = match generator {
            GeneratorKind::Queue => json!({
                "active": {
                    "vec": [{
                        "fields": {
                            "occurrence": {
                                "start_time_ms": start_time_ms.to_string()
                            }
                        }
                    }]
                }
            }),
            GeneratorKind::Periodic => json!({
                "active": {
                    "vec": [{
                        "fields": {
                            "start_time_ms": start_time_ms.to_string()
                        }
                    }]
                }
            }),
        };

        let value = if wrapped {
            json!({ "fields": inner })
        } else {
            inner
        };

        (
            TransitionConfigKey {
                transition: TransitionKey {
                    state: None,
                    symbol,
                },
                config: TypeName::from(config_name.to_string()),
            },
            value,
        )
    }

    fn execution_config(
        config_name: &str,
        symbol: PolicySymbol,
        config: AgentExecutionConfig,
    ) -> (TransitionConfigKey<u64, PolicySymbol>, serde_json::Value) {
        (
            TransitionConfigKey {
                transition: TransitionKey {
                    state: None,
                    symbol,
                },
                config: TypeName::from(config_name.to_string()),
            },
            json!({ "fields": serde_json::to_value(config).expect("serialize config") }),
        )
    }

    fn default_agent_execution_config(
        objects: &NexusObjects,
        dag_id: sui::types::Address,
    ) -> AgentExecutionConfig {
        AgentExecutionConfig {
            selection: ExecutionSelection::DefaultAgent { dag_id },
            network: objects.network_id,
            entry_group: crate::types::SchedulerEntryGroup {
                name: "entry".into(),
            },
            inputs: crate::nexus::crawler::Map::new(),
            invoker: sui::types::Address::ZERO,
            priority_fee_per_gas_unit: 0,
            authorization_templates: vec![],
        }
    }

    fn agent_skill_execution_config(
        objects: &NexusObjects,
        agent_id: sui::types::Address,
        skill_id: SkillId,
        selected_dag: Option<sui::types::Address>,
    ) -> AgentExecutionConfig {
        AgentExecutionConfig {
            selection: ExecutionSelection::AgentSkill {
                agent_id,
                skill_id,
                selected_dag: crate::types::MoveOption(selected_dag),
            },
            network: objects.network_id,
            entry_group: crate::types::SchedulerEntryGroup {
                name: "entry".into(),
            },
            inputs: crate::nexus::crawler::Map::new(),
            invoker: sui::types::Address::ZERO,
            priority_fee_per_gas_unit: 0,
            authorization_templates: vec![],
        }
    }

    /// Build a `Crawler` that points at an empty mock server. Safe to use when
    /// the test should *not* hit the network (e.g. asserting short-circuit
    /// branches of `resolve_dag`).
    async fn dummy_crawler() -> crate::nexus::crawler::Crawler {
        let url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks::default());
        let client = sui::grpc::Client::new(url).expect("mock client");
        crate::nexus::crawler::Crawler::new(std::sync::Arc::new(tokio::sync::Mutex::new(client)))
    }

    fn event_bcs(
        primitives_pkg: sui::types::Address,
        event_pkg: sui::types::Address,
        kind: NexusEventKind,
    ) -> sui::types::Event {
        #[derive(Serialize)]
        struct Wrapper<'a, T: Serialize + ?Sized> {
            event: &'a T,
        }

        let event_type = sui::types::StructTag::new(
            event_pkg,
            sui::types::Identifier::from_static("scheduler"),
            sui::types::Identifier::new(kind.name()).unwrap(),
            vec![],
        );

        let wrapper_tag = sui::types::StructTag::new(
            primitives_pkg,
            crate::idents::primitives::Event::EVENT_WRAPPER.module,
            crate::idents::primitives::Event::EVENT_WRAPPER.name,
            vec![sui::types::TypeTag::Struct(Box::new(event_type))],
        );

        let bcs = match &kind {
            NexusEventKind::RequestScheduledOccurrence(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::RequestScheduledWalk(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::OccurrenceScheduled(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::ScheduledSkillExecutionCreated(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::ScheduledSkillExecutionPaused(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::ScheduledSkillExecutionResumed(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            NexusEventKind::ScheduledSkillExecutionCanceled(ev) => {
                bcs::to_bytes(&Wrapper { event: ev }).unwrap()
            }
            _ => bcs::to_bytes(&Wrapper { event: &kind }).unwrap(),
        };

        sui_mocks::mock_sui_event(primitives_pkg, wrapper_tag, bcs)
    }

    async fn mock_nexus_client_with_server(
        ledger_service_mock: sui_mocks::grpc::MockLedgerService,
        execution_service_mock: sui_mocks::grpc::MockTransactionExecutionService,
        subscription_service_mock: sui_mocks::grpc::MockSubscriptionService,
        nexus_objects: NexusObjects,
    ) -> (String, NexusClient) {
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(execution_service_mock),
            subscription_service_mock: Some(subscription_service_mock),
            ..Default::default()
        });

        let nexus_client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        (rpc_url, nexus_client)
    }

    #[tokio::test]
    async fn resolve_dag_returns_default_executor_dag_without_crawler_hit() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Address::from_static("0xd");
        let config =
            ScheduledAgentExecutionConfig::Default(default_agent_execution_config(&objects, dag));
        let crawler = dummy_crawler().await;

        let resolved = config
            .resolve_dag(&crawler, &objects)
            .await
            .expect("default-agent config short-circuits via dag()");

        assert_eq!(resolved, dag);
    }

    #[tokio::test]
    async fn resolve_dag_returns_runtime_selected_dag_without_crawler_hit() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag = sui::types::Address::from_static("0xd");
        let config = ScheduledAgentExecutionConfig::Registered(agent_skill_execution_config(
            &objects,
            sui::types::Address::from_static("0xa"),
            7,
            Some(dag),
        ));
        let crawler = dummy_crawler().await;

        let resolved = config
            .resolve_dag(&crawler, &objects)
            .await
            .expect("runtime-selected config short-circuits via dag()");

        assert_eq!(resolved, dag);
    }

    #[tokio::test]
    async fn resolve_dag_rejects_default_variant_carrying_agent_skill_selection() {
        // Inconsistent shape: `Default(_)` is only correct for `DefaultAgent`
        // selections; if it carries an `AgentSkill` selection with no dag we
        // should refuse to silently fall back to the skill record lookup.
        let objects = sui_mocks::mock_nexus_objects();
        let config = ScheduledAgentExecutionConfig::Default(agent_skill_execution_config(
            &objects,
            sui::types::Address::from_static("0xa"),
            7,
            None,
        ));
        let crawler = dummy_crawler().await;

        let err = config
            .resolve_dag(&crawler, &objects)
            .await
            .expect_err("inconsistent Default(_) config must be rejected");

        assert!(
            err.to_string().contains("default-agent scheduled config"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn active_scheduler_start_ms_reads_wrapped_queue_state() {
        let objects = sui_mocks::mock_nexus_objects();
        let expected = scheduler::Scheduler::QUEUE_GENERATOR_STATE
            .qualified_name(objects.scheduler_type_origin_pkg_id());

        let active = active_scheduler_start_ms(
            vec![scheduler_config(
                &expected,
                42,
                objects.scheduler_queue_generator_symbol(),
                GeneratorKind::Queue,
                true,
            )],
            &expected,
            GeneratorKind::Queue,
            |symbol| objects.scheduler_matches_queue_generator(symbol),
        )
        .unwrap();

        assert_eq!(active, Some(42));
    }

    #[test]
    fn active_scheduler_start_ms_reads_plain_periodic_state() {
        let objects = sui_mocks::mock_nexus_objects();
        let expected = scheduler::Scheduler::PERIODIC_GENERATOR_STATE
            .qualified_name(objects.scheduler_type_origin_pkg_id());

        let active = active_scheduler_start_ms(
            vec![scheduler_config(
                &expected,
                77,
                objects.scheduler_periodic_generator_symbol(),
                GeneratorKind::Periodic,
                false,
            )],
            &expected,
            GeneratorKind::Periodic,
            |symbol| objects.scheduler_matches_periodic_generator(symbol),
        )
        .unwrap();

        assert_eq!(active, Some(77));
    }

    #[test]
    fn active_scheduler_start_ms_errors_when_config_is_missing() {
        let objects = sui_mocks::mock_nexus_objects();
        let expected = scheduler::Scheduler::QUEUE_GENERATOR_STATE
            .qualified_name(objects.scheduler_type_origin_pkg_id());

        let err = active_scheduler_start_ms(vec![], &expected, GeneratorKind::Queue, |symbol| {
            objects.scheduler_matches_queue_generator(symbol)
        })
        .expect_err("missing generator config must error");

        assert!(err
            .to_string()
            .contains("Missing scheduler generator state config"));
    }

    #[test]
    fn scheduled_execution_config_matches_execution_entries_symbol_by_module_name() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag_id = sui::types::Address::from_static("0xd");
        let defining_pkg_not_in_objects = sui::types::Address::from_static("0x999");
        let config_name = crate::idents::interface::Agent::AGENT_EXECUTION_CONFIG
            .qualified_name(defining_pkg_not_in_objects);
        let symbol = PolicySymbol::Witness(TypeName::new(
            &workflow::ExecutionEntries::ADVANCE_FOR_DEFAULT_AGENT_EXECUTION_TYPE
                .qualified_name(defining_pkg_not_in_objects),
        ));

        let config = extract_scheduled_agent_execution_config(
            vec![execution_config(
                &config_name,
                symbol,
                default_agent_execution_config(&objects, dag_id),
            )],
            &objects,
        )
        .expect("config should match by execution_entries module/name");

        assert_eq!(config.dag(), Some(dag_id));
        assert!(matches!(config, ScheduledAgentExecutionConfig::Default(_)));
    }

    #[test]
    fn scheduled_execution_config_matches_raw_move_type_names_without_0x_prefix() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag_id = sui::types::Address::from_static("0xd");
        let interface_pkg = "82da904e4d6040729d0b16ee49aae067bed36aec31e167b24e1e072221c1eb16";
        let workflow_pkg = "deafc2c9ef3914a7d4945572d07fd961fef7ff1f3e1d329f4057c28538598776";
        let config_name = format!("{interface_pkg}::agent::AgentExecutionConfig");
        let symbol = PolicySymbol::Witness(TypeName::new(&format!(
            "{workflow_pkg}::execution_entries::AdvanceForDefaultAgentExecution"
        )));

        let config = extract_scheduled_agent_execution_config(
            vec![execution_config(
                &config_name,
                symbol,
                default_agent_execution_config(&objects, dag_id),
            )],
            &objects,
        )
        .expect("config should match raw dynamic field type names by module/name");

        assert_eq!(config.dag(), Some(dag_id));
        assert!(matches!(config, ScheduledAgentExecutionConfig::Default(_)));
    }

    #[test]
    fn scheduled_execution_config_does_not_swallow_default_selection_mismatch() {
        let objects = sui_mocks::mock_nexus_objects();
        let config_name = crate::idents::interface::Agent::AGENT_EXECUTION_CONFIG
            .qualified_name(objects.interface_pkg_id);
        let symbol = PolicySymbol::Witness(TypeName::new(
            &workflow::ExecutionEntries::ADVANCE_FOR_DEFAULT_AGENT_EXECUTION_TYPE
                .qualified_name(objects.workflow_pkg_id),
        ));
        let config = AgentExecutionConfig {
            selection: ExecutionSelection::AgentSkill {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 1,
                selected_dag: MoveOption(Some(sui::types::Address::from_static("0xd"))),
            },
            network: objects.network_id,
            entry_group: crate::types::SchedulerEntryGroup {
                name: "entry".into(),
            },
            inputs: crate::nexus::crawler::Map::new(),
            invoker: sui::types::Address::ZERO,
            priority_fee_per_gas_unit: 0,
            authorization_templates: vec![],
        };

        let err = extract_scheduled_agent_execution_config(
            vec![execution_config(&config_name, symbol, config)],
            &objects,
        )
        .expect_err("default witness with agent selection should not fall through");

        assert!(err
            .to_string()
            .contains("AdvanceForDefaultAgentExecution config is not a default-agent selection"));
    }

    #[tokio::test]
    async fn test_scheduler_create_task_without_initial_schedule() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let owner = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);

        let events = vec![event_bcs(
            nexus_objects.primitives_pkg_id,
            nexus_objects.scheduler_pkg_id,
            NexusEventKind::ScheduledSkillExecutionCreated(ScheduledSkillExecutionCreatedEvent {
                task: task_id,
                owner,
            }),
        )];

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            events,
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let params = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: sample_input_data(),
            metadata: vec![("team".into(), "sdk".into())],
            execution_priority_fee_per_gas_unit: 1,
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: None,
            skill_id: None,
            tap_payment: Some(CreateTaskTapPayment::UserFunded {
                prepay_amount: 1_000,
                refund_recipient: None,
                occurrence_budget: 100,
                selected_dag: None,
                authorization_templates: vec![],
            }),
        };

        let result = nexus_client
            .scheduler()
            .create_task(params)
            .await
            .expect("task created");

        assert_eq!(result.task_id, task_id);
        assert_eq!(result.tx_digest, digest);
        assert!(result.initial_schedule.is_none());
        assert_eq!(
            result.tap_payment,
            Some(CreateTaskTapPaymentResult {
                agent_id: nexus_objects.default_dag_executor.agent_id,
                skill_id: nexus_objects.default_dag_executor.skill_id,
                prepay_amount: 1_000,
                occurrence_budget: 100,
            })
        );
    }

    #[tokio::test]
    async fn test_scheduler_create_task_with_initial_schedule() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let creation_digest = sui::types::Digest::generate(&mut rng);
        let schedule_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let owner = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);

        let creation_events = vec![event_bcs(
            nexus_objects.primitives_pkg_id,
            nexus_objects.scheduler_pkg_id,
            NexusEventKind::ScheduledSkillExecutionCreated(ScheduledSkillExecutionCreatedEvent {
                task: task_id,
                owner,
            }),
        )];

        // Creation TX
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            creation_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            creation_events,
        );

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        // Scheduled occurrence event
        let queue_generator = queue_generator_symbol(nexus_objects.scheduler_pkg_id);
        let scheduled_event =
            NexusEventKind::RequestScheduledOccurrence(RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: task_id,
                    generator: queue_generator.clone(),
                },
                priority: 10,
                request_ms: 100,
                start_ms: 200,
                deadline_ms: 300,
            });

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            schedule_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![event_bcs(
                nexus_objects.primitives_pkg_id,
                nexus_objects.scheduler_pkg_id,
                scheduled_event.clone(),
            )],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let initial_schedule =
            OccurrenceRequest::new(Some(1_000), Some(2_000), None, None, 5, true)
                .expect("valid request");

        let params = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: sample_input_data(),
            metadata: vec![],
            execution_priority_fee_per_gas_unit: 5,
            initial_schedule: Some(initial_schedule),
            generator: GeneratorKind::Queue,
            agent_id: None,
            skill_id: None,
            tap_payment: Some(CreateTaskTapPayment::UserFunded {
                prepay_amount: 2_000,
                refund_recipient: None,
                occurrence_budget: 200,
                selected_dag: None,
                authorization_templates: vec![],
            }),
        };

        let result = nexus_client
            .scheduler()
            .create_task(params)
            .await
            .expect("task created");

        assert_eq!(result.task_id, task_id);
        let schedule = result.initial_schedule.expect("schedule created");
        assert_eq!(schedule.tx_digest, schedule_digest);
        assert!(matches!(
            schedule.event,
            Some(NexusEventKind::RequestScheduledOccurrence(env))
                if env.request.task == task_id && env.priority == 10 && env.start_ms == 200
        ));
        assert_eq!(
            result.tap_payment,
            Some(CreateTaskTapPaymentResult {
                agent_id: nexus_objects.default_dag_executor.agent_id,
                skill_id: nexus_objects.default_dag_executor.skill_id,
                prepay_amount: 2_000,
                occurrence_budget: 200,
            })
        );
    }

    #[tokio::test]
    async fn create_task_rejects_default_without_tap_payment_before_rpc() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);
        let execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects,
        )
        .await;

        let mut rng = rand::thread_rng();
        let params = CreateTaskParams {
            dag_id: sui::types::Address::generate(&mut rng),
            entry_group: "entry".into(),
            input_data: HashMap::new(),
            metadata: vec![],
            execution_priority_fee_per_gas_unit: 0,
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: None,
            skill_id: None,
            tap_payment: None,
        };

        let result = nexus_client.scheduler().create_task(params).await;
        let Err(err) = result else {
            panic!("default scheduled task without payment must error");
        };
        assert!(matches!(err, NexusError::Configuration(_)));
        assert!(err.to_string().contains("requires user-funded tap_payment"));
    }

    #[tokio::test]
    async fn create_task_agent_bound_branch_delegates_to_tap_action() {
        // Compatibility path for agent-bound task creation. The scheduler
        // action delegates explicit-agent requests to the TAP action, which
        // owns the agent/skill/payment creation semantics.
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let owner = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);
        let dag_id = sui::types::Address::generate(&mut rng);
        let agent_id = sui::types::Address::generate(&mut rng);
        let agent_ref =
            sui::types::ObjectReference::new(agent_id, 2, sui::types::Digest::generate(&mut rng));

        let events = vec![event_bcs(
            nexus_objects.primitives_pkg_id,
            nexus_objects.scheduler_pkg_id,
            NexusEventKind::ScheduledSkillExecutionCreated(ScheduledSkillExecutionCreatedEvent {
                task: task_id,
                owner,
            }),
        )];

        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref,
            sui::types::Owner::Shared(2),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            events,
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let params = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: sample_input_data(),
            metadata: vec![],
            execution_priority_fee_per_gas_unit: 7,
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: Some(agent_id),
            skill_id: Some(3),
            tap_payment: Some(CreateTaskTapPayment::UserFunded {
                prepay_amount: 100,
                refund_recipient: None,
                occurrence_budget: 25,
                selected_dag: None,
                authorization_templates: vec![],
            }),
        };

        let result = nexus_client
            .scheduler()
            .create_task(params)
            .await
            .expect("agent-bound task created");

        assert_eq!(result.task_id, task_id);
        assert_eq!(result.tx_digest, digest);
        assert!(result.initial_schedule.is_none());
        assert_eq!(
            result.tap_payment,
            Some(CreateTaskTapPaymentResult {
                agent_id,
                skill_id: 3,
                prepay_amount: 100,
                occurrence_budget: 25,
            })
        );
    }

    #[tokio::test]
    async fn create_task_rejects_half_agent_binding_before_rpc() {
        // Half-supplied agent binding (agent_id without skill_id, or vice
        // versa) must be caught locally before any gRPC round-trip. The
        // client itself probes `reference_gas_price` at construction time —
        // we mock that, but expect no other RPC traffic. The test passes
        // only when `create_task` errors out before the first PTB build.
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);
        let execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let mut rng = rand::thread_rng();
        let dag_id = sui::types::Address::generate(&mut rng);

        let agent_only = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: HashMap::new(),
            metadata: vec![],
            execution_priority_fee_per_gas_unit: 0,
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: Some(sui::types::Address::generate(&mut rng)),
            skill_id: None,
            tap_payment: None,
        };
        let result = nexus_client.scheduler().create_task(agent_only).await;
        let Err(err) = result else {
            panic!("agent-only binding must error");
        };
        assert!(matches!(err, NexusError::Configuration(_)));
        assert!(err.to_string().contains("agent_id and skill_id must both"));

        let skill_only = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: HashMap::new(),
            metadata: vec![],
            execution_priority_fee_per_gas_unit: 0,
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: None,
            skill_id: Some(0),
            tap_payment: None,
        };
        let result = nexus_client.scheduler().create_task(skill_only).await;
        let Err(err) = result else {
            panic!("skill-only binding must error");
        };
        assert!(matches!(err, NexusError::Configuration(_)));
    }

    #[tokio::test]
    async fn test_scheduler_update_metadata() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let metadata = vec![
            ("region".into(), "us".into()),
            ("tier".into(), "gold".into()),
        ];
        let result = nexus_client
            .scheduler()
            .update_metadata(task_id, metadata.clone())
            .await
            .expect("metadata updated");

        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.entries, metadata.len());
    }

    #[tokio::test]
    async fn test_scheduler_set_task_state_pause() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let result = nexus_client
            .scheduler()
            .set_task_state(task_id, TaskStateAction::Pause)
            .await
            .expect("state updated");

        assert_eq!(result.tx_digest, digest);
        assert!(matches!(result.state, TaskStateAction::Pause));
    }

    #[tokio::test]
    async fn test_scheduler_add_occurrence_absolute() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        let generator = queue_generator_symbol(nexus_objects.scheduler_pkg_id);
        let scheduled_event = NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
            task: task_id,
            generator: generator.clone(),
        });

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![event_bcs(
                nexus_objects.primitives_pkg_id,
                nexus_objects.scheduler_pkg_id,
                scheduled_event.clone(),
            )],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let request =
            OccurrenceRequest::new(Some(2_000), Some(2_500), None, None, 7, true).unwrap();

        let result = nexus_client
            .scheduler()
            .add_occurrence(task_id, request)
            .await
            .expect("occurrence enqueued");

        assert_eq!(result.tx_digest, digest);
        assert!(matches!(
            result.event,
            Some(NexusEventKind::OccurrenceScheduled(_))
        ));
    }

    #[tokio::test]
    async fn test_scheduler_add_occurrence_with_offsets() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        let generator = queue_generator_symbol(nexus_objects.scheduler_pkg_id);
        let scheduled_event = NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
            task: task_id,
            generator: generator.clone(),
        });

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![event_bcs(
                nexus_objects.primitives_pkg_id,
                nexus_objects.scheduler_pkg_id,
                scheduled_event.clone(),
            )],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let request =
            OccurrenceRequest::new(None, None, Some(500), Some(900), 4, true).expect("valid");

        let result = nexus_client
            .scheduler()
            .add_occurrence(task_id, request)
            .await
            .expect("occurrence enqueued");

        assert!(matches!(
            result.event,
            Some(NexusEventKind::OccurrenceScheduled(_))
        ));
    }

    #[tokio::test]
    async fn test_scheduler_configure_periodic() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let config = PeriodicScheduleConfig {
            first_start_ms: 10_000,
            period_ms: 5_000,
            deadline_offset_ms: Some(1_000),
            max_iterations: Some(5),
            priority_fee_per_gas_unit: 20,
        };

        let result = nexus_client
            .scheduler()
            .configure_periodic(task_id, config)
            .await
            .expect("periodic configured");

        assert_eq!(result.tx_digest, digest);
        assert!(result.event.is_none());
    }

    #[tokio::test]
    async fn test_scheduler_disable_periodic() {
        let mut rng = rand::thread_rng();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut execution_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1_000);

        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let nexus_objects = sui_mocks::mock_nexus_objects();
        let task_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Address::generate(&mut rng);

        // Task fetch
        let task_object_json = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            task_object_json,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut execution_service_mock,
            &mut subscription_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let (_url, nexus_client) = mock_nexus_client_with_server(
            ledger_service_mock,
            execution_service_mock,
            subscription_service_mock,
            nexus_objects.clone(),
        )
        .await;

        let result = nexus_client
            .scheduler()
            .disable_periodic(task_id)
            .await
            .expect("periodic disabled");

        assert_eq!(result.tx_digest, digest);
    }

    #[test]
    fn test_occurrence_request_validation_rules() {
        assert!(OccurrenceRequest::new(Some(10), Some(20), None, None, 1, true).is_ok());
        assert!(OccurrenceRequest::new(None, None, Some(5), Some(15), 1, true).is_ok());

        let err = OccurrenceRequest::new(None, None, None, None, 1, true).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Provide either")));

        let err = OccurrenceRequest::new(Some(50), Some(40), None, None, 1, true).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline")));

        let err = OccurrenceRequest::new(None, None, None, Some(10), 1, false).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline flags")));
    }

    #[test]
    fn test_extract_task_id_errors_without_event() {
        let mut rng = thread_rng();
        let workflow_pkg = sui::types::Address::generate(&mut rng);
        let response = dummy_executed_transaction(vec![NexusEvent {
            id: (sui::types::Digest::generate(&mut rng), 0),
            generics: vec![],
            data: NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
                task: sui::types::Address::generate(&mut rng),
                generator: PolicySymbol::Uid(workflow_pkg),
            }),
            distribution: None,
        }]);

        let err = extract_task_id(&response).expect_err("missing event");
        assert!(matches!(err, NexusError::Parsing(_)));
    }

    #[test]
    fn test_extract_occurrence_event_variants() {
        let mut rng = thread_rng();
        let workflow_pkg = sui::types::Address::generate(&mut rng);
        let task_id = sui::types::Address::generate(&mut rng);

        let scheduled =
            NexusEventKind::RequestScheduledOccurrence(RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: task_id,
                    generator: PolicySymbol::Uid(workflow_pkg),
                },
                priority: 1,
                request_ms: 10,
                start_ms: 11,
                deadline_ms: 12,
            });

        let response = dummy_executed_transaction(vec![NexusEvent {
            id: (sui::types::Digest::generate(&mut rng), 0),
            generics: vec![],
            data: scheduled.clone(),
            distribution: None,
        }]);
        assert!(matches!(
            extract_occurrence_event(&response),
            Some(NexusEventKind::RequestScheduledOccurrence(_))
        ));

        let direct = dummy_executed_transaction(vec![NexusEvent {
            id: (sui::types::Digest::generate(&mut rng), 0),
            generics: vec![],
            data: NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
                task: task_id,
                generator: PolicySymbol::Uid(workflow_pkg),
            }),
            distribution: None,
        }]);
        assert!(matches!(
            extract_occurrence_event(&direct),
            Some(NexusEventKind::OccurrenceScheduled(_))
        ));

        let empty = dummy_executed_transaction(vec![]);
        assert!(extract_occurrence_event(&empty).is_none());
    }
}
