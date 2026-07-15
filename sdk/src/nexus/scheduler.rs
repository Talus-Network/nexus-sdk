//! Scheduler-oriented actions exposed through [`NexusClient`].

use {
    crate::{
        events::NexusEventKind,
        move_bindings::{
            interface::{agent as agent_move, authorization::AgentVertexAuthorizationTemplate},
            move_std::type_name::TypeName,
            primitives::{
                automaton as automaton_move,
                data::NexusData,
                policy::Symbol as PolicySymbol,
            },
            scheduler::{scheduler as scheduler_move, scheduler::Task},
            workflow::execution_entries as execution_entries_move,
        },
        nexus::{
            client::NexusClient,
            crawler::{Crawler, DynamicFieldReference, Response},
            error::NexusError,
            signer::ExecutedTransaction,
        },
        sui,
        transactions::scheduler as scheduler_tx,
        types::{AgentId, NexusObjects, SkillId},
    },
    anyhow::{anyhow, bail},
    std::collections::HashMap,
    sui_move::MoveStruct,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ScheduledAgentExecutionConfig {
    Default(agent_move::AgentExecutionConfig),
    Registered(agent_move::AgentExecutionConfig),
}

impl ScheduledAgentExecutionConfig {
    pub fn dag(&self) -> Option<sui::types::Address> {
        match self {
            Self::Default(config) | Self::Registered(config) => config.selection.dag_id(),
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

type SchedulerConfigKey = automaton_move::TransitionConfigKey<u64, PolicySymbol>;

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
    pub input_data: HashMap<String, HashMap<String, NexusData>>,
    pub metadata: Vec<(String, String)>,
    pub execution_priority_fee_percentage: Option<u64>,
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
        prepay_amount_mist: u64,
        refund_recipient: Option<sui::types::Address>,
        occurrence_budget_mist: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
    /// Reserve funded from the selected agent's vault.
    AgentFunded {
        prepay_amount_mist: u64,
        occurrence_budget_mist: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
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
    pub prepay_amount_mist: u64,
    pub occurrence_budget_mist: u64,
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
    pub priority_fee_percentage: Option<u64>,
}

impl OccurrenceRequest {
    pub fn new(
        start_ms: Option<u64>,
        deadline_ms: Option<u64>,
        start_offset_ms: Option<u64>,
        deadline_offset_ms: Option<u64>,
        priority_fee_percentage: Option<u64>,
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
            priority_fee_percentage,
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
    pub priority_fee_percentage: Option<u64>,
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
            prepay_amount_mist,
            occurrence_budget_mist,
            ..
        } => {
            let (agent_id, skill_id) = agent_binding.unwrap_or((
                objects.default_dag_executor.agent_id,
                objects.default_dag_executor.skill_id,
            ));

            Ok(Some(CreateTaskTapPaymentResult {
                agent_id,
                skill_id,
                prepay_amount_mist: *prepay_amount_mist,
                occurrence_budget_mist: *occurrence_budget_mist,
            }))
        }
        CreateTaskTapPayment::AgentFunded {
            prepay_amount_mist,
            occurrence_budget_mist,
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
                prepay_amount_mist: *prepay_amount_mist,
                occurrence_budget_mist: *occurrence_budget_mist,
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
            execution_priority_fee_percentage,
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
        if agent_binding.is_none() && tap_payment.is_none() {
            return Err(NexusError::Configuration(
                "default-agent scheduled task creation requires user-funded tap_payment".into(),
            ));
        }
        if let Some((agent_id, skill_id)) = agent_binding {
            let payment = match tap_payment {
                Some(CreateTaskTapPayment::UserFunded {
                    prepay_amount_mist,
                    refund_recipient,
                    occurrence_budget_mist,
                    selected_dag,
                    authorization_templates,
                }) => crate::nexus::tap::AgentTaskPayment::UserFunded {
                    prepay_amount_mist,
                    refund_recipient,
                    occurrence_budget_mist,
                    selected_dag,
                    authorization_templates,
                },
                Some(CreateTaskTapPayment::AgentFunded {
                    prepay_amount_mist,
                    occurrence_budget_mist,
                    selected_dag,
                    authorization_templates,
                }) => crate::nexus::tap::AgentTaskPayment::AgentVault {
                    prepay_amount_mist,
                    occurrence_budget_mist,
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
                    execution_priority_fee_percentage,
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

        let tap_payment_result =
            create_task_payment_result(&tap_payment, agent_binding, &self.client.nexus_objects)?;

        let Some(CreateTaskTapPayment::UserFunded {
            prepay_amount_mist,
            occurrence_budget_mist,
            refund_recipient,
            selected_dag,
            authorization_templates,
        }) = &tap_payment
        else {
            return Err(NexusError::TransactionBuilding(anyhow!(
                "default-agent scheduled task creation requires user-funded tap_payment"
            )));
        };
        if refund_recipient.is_some()
            || selected_dag.is_some()
            || !authorization_templates.is_empty()
        {
            return Err(NexusError::TransactionBuilding(anyhow!(
                "default-agent scheduled task user funding does not accept refund_recipient, selected_dag, or authorization_templates"
            )));
        }

        let tx = scheduler_tx::create_default_agent_task_ptb(
            objects,
            dag_id,
            entry_group.as_str(),
            &input_data,
            &metadata,
            generator.into(),
            execution_priority_fee_percentage,
            *prepay_amount_mist,
            *occurrence_budget_mist,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

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
        let objects = &self.client.nexus_objects;

        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        let tx = scheduler_tx::update_metadata_ptb(objects, &task_ref, metadata.clone())
            .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

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
        let objects = &self.client.nexus_objects;

        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        let tx = match request {
            TaskStateAction::Pause => scheduler_tx::pause_task_for_self_ptb(objects, &task_ref),
            TaskStateAction::Resume => scheduler_tx::resume_task_for_self_ptb(objects, &task_ref),
            TaskStateAction::Cancel => scheduler_tx::cancel_task_for_self_ptb(objects, &task_ref),
        }
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

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
        let objects = &self.client.nexus_objects;
        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        let tx = scheduler_tx::configure_periodic_for_task_for_self_ptb(
            objects,
            &task_ref,
            scheduler_tx::PeriodicScheduleInputs {
                first_start_ms: config.first_start_ms,
                period_ms: config.period_ms,
                deadline_offset_ms: config.deadline_offset_ms,
                max_iterations: config.max_iterations,
                priority_fee_percentage: config.priority_fee_percentage,
            },
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

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
        let objects = &self.client.nexus_objects;
        let task = self.fetch_task(task_id).await?;
        let task_ref = task.object_ref();

        let tx = scheduler_tx::disable_periodic_for_task_for_self_ptb(objects, &task_ref)
            .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

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
        let objects = &self.client.nexus_objects;
        let task_ref = task.object_ref();

        let tx = if let Some(start_ms) = request.start_ms {
            let deadline_offset = request
                .deadline_offset_ms
                .or_else(|| request.deadline_ms.map(|deadline| deadline - start_ms));

            scheduler_tx::add_occurrence_absolute_for_task_for_self_ptb(
                objects,
                &task_ref,
                start_ms,
                deadline_offset,
                request.priority_fee_percentage,
            )
        } else {
            scheduler_tx::add_occurrence_relative_for_task_for_self_ptb(
                objects,
                &task_ref,
                request.start_offset_ms.expect("validated start offset"),
                request.deadline_offset_ms,
                request.priority_fee_percentage,
            )
        }
        .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;

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

pub async fn fetch_begin_default_agent_execution_config(
    crawler: &Crawler,
    objects: &NexusObjects,
    configured_automaton_id: &sui::types::Address,
) -> anyhow::Result<agent_move::AgentExecutionConfig> {
    let configs = fetch_scheduler_config_fields(crawler, *configured_automaton_id).await?;
    let Some(field) = find_execution_config_field::<
        execution_entries_move::AdvanceForDefaultAgentExecution,
    >(&configs, objects) else {
        bail!(
            "Missing execution policy config for AdvanceForDefaultAgentExecution; {}",
            describe_transition_config_keys(&configs)
        );
    };

    let config = decode_agent_execution_config_field(
        crawler,
        field,
        "AdvanceForDefaultAgentExecution config",
    )
    .await?;
    ensure_default_agent_execution_config(config, "AdvanceForDefaultAgentExecution config")
}

pub async fn fetch_scheduled_agent_execution_config(
    crawler: &Crawler,
    objects: &NexusObjects,
    configured_automaton_id: &sui::types::Address,
) -> anyhow::Result<ScheduledAgentExecutionConfig> {
    let configs = fetch_scheduler_config_fields(crawler, *configured_automaton_id).await?;
    if let Some(field) = find_execution_config_field::<
        execution_entries_move::AdvanceForDefaultAgentExecution,
    >(&configs, objects)
    {
        let config = decode_agent_execution_config_field(
            crawler,
            field,
            "AdvanceForDefaultAgentExecution config",
        )
        .await?;
        return ensure_default_agent_execution_config(
            config,
            "AdvanceForDefaultAgentExecution config",
        )
        .map(ScheduledAgentExecutionConfig::Default);
    }

    let Some(field) = find_execution_config_field::<execution_entries_move::AdvanceForAgentExecution>(
        &configs, objects,
    ) else {
        bail!(
            "Missing execution policy config for AdvanceForDefaultAgentExecution or AdvanceForAgentExecution; {}",
            describe_transition_config_keys(&configs)
        );
    };

    let config =
        decode_agent_execution_config_field(crawler, field, "AdvanceForAgentExecution config")
            .await?;
    ensure_agent_skill_execution_config(config, "AdvanceForAgentExecution config")
        .map(ScheduledAgentExecutionConfig::Registered)
}

pub async fn fetch_active_scheduler_start_ms(
    crawler: &Crawler,
    objects: &NexusObjects,
    configured_automaton_id: &sui::types::Address,
    generator: GeneratorKind,
) -> anyhow::Result<Option<u64>> {
    let configs = fetch_scheduler_config_fields(crawler, *configured_automaton_id).await?;

    match generator {
        GeneratorKind::Queue => {
            let Some(field) = find_scheduler_generator_state_field::<
                scheduler_move::QueueGeneratorWitness,
                scheduler_move::QueueGeneratorState,
            >(&configs, objects) else {
                bail!(
                    "Missing scheduler queue generator state config; {}",
                    describe_transition_config_keys(&configs)
                );
            };

            let state = crawler
                .get_dynamic_field_value_by_id::<
                    SchedulerConfigKey,
                    scheduler_move::QueueGeneratorState,
                >(field.field_id)
                .await
                .map_err(|error| {
                    anyhow!("decode QueueGeneratorState as generated BCS Move layout: {error}")
                })?;

            Ok(state
                .active
                .as_option()
                .map(|entry| entry.occurrence.start_time_ms))
        }
        GeneratorKind::Periodic => {
            let Some(field) = find_scheduler_generator_state_field::<
                scheduler_move::PeriodicGeneratorWitness,
                scheduler_move::PeriodicGeneratorState,
            >(&configs, objects) else {
                bail!(
                    "Missing scheduler periodic generator state config; {}",
                    describe_transition_config_keys(&configs)
                );
            };

            let state = crawler
                .get_dynamic_field_value_by_id::<
                    SchedulerConfigKey,
                    scheduler_move::PeriodicGeneratorState,
                >(field.field_id)
                .await
                .map_err(|error| {
                    anyhow!("decode PeriodicGeneratorState as generated BCS Move layout: {error}")
                })?;

            Ok(state
                .active
                .as_option()
                .map(|occurrence| occurrence.start_time_ms))
        }
    }
}

async fn fetch_scheduler_config_fields(
    crawler: &Crawler,
    configured_automaton_id: sui::types::Address,
) -> anyhow::Result<Vec<DynamicFieldReference<SchedulerConfigKey>>> {
    crawler
        .get_dynamic_field_refs_matching_key::<SchedulerConfigKey>(configured_automaton_id)
        .await
}

async fn decode_agent_execution_config_field(
    crawler: &Crawler,
    field: &DynamicFieldReference<SchedulerConfigKey>,
    label: &str,
) -> anyhow::Result<agent_move::AgentExecutionConfig> {
    crawler
        .get_dynamic_field_value_by_id::<SchedulerConfigKey, agent_move::AgentExecutionConfig>(
            field.field_id,
        )
        .await
        .map_err(|error| anyhow!("decode {label} as generated BCS Move layout: {error}"))
}

fn ensure_default_agent_execution_config(
    config: agent_move::AgentExecutionConfig,
    label: &str,
) -> anyhow::Result<agent_move::AgentExecutionConfig> {
    if !config.selection.is_default_agent() {
        bail!("{label} is not a default-agent selection");
    }
    Ok(config)
}

fn ensure_agent_skill_execution_config(
    config: agent_move::AgentExecutionConfig,
    label: &str,
) -> anyhow::Result<agent_move::AgentExecutionConfig> {
    if !config.selection.is_agent_skill() {
        bail!("{label} is not an agent-skill selection");
    }
    Ok(config)
}

fn find_execution_config_field<'a, Symbol>(
    configs: &'a [DynamicFieldReference<SchedulerConfigKey>],
    objects: &NexusObjects,
) -> Option<&'a DynamicFieldReference<SchedulerConfigKey>>
where
    Symbol: MoveStruct,
{
    configs.iter().find(|field| {
        let key = &field.name;
        let matches_config =
            type_name_matches_move_struct::<agent_move::AgentExecutionConfig>(&key.config, objects);
        let matches_symbol =
            policy_symbol_matches_move_struct::<Symbol>(&key.transition.symbol, objects);

        matches_config && key.transition.state.as_option().is_none() && matches_symbol
    })
}

fn find_scheduler_generator_state_field<'a, Symbol, State>(
    configs: &'a [DynamicFieldReference<SchedulerConfigKey>],
    objects: &NexusObjects,
) -> Option<&'a DynamicFieldReference<SchedulerConfigKey>>
where
    Symbol: MoveStruct,
    State: MoveStruct,
{
    configs.iter().find(|field| {
        let key = &field.name;
        let matches_config = type_name_matches_move_struct::<State>(&key.config, objects);
        let matches_symbol =
            policy_symbol_matches_move_struct::<Symbol>(&key.transition.symbol, objects);

        matches_config && key.transition.state.as_option().is_none() && matches_symbol
    })
}

fn describe_transition_config_keys(
    configs: &[DynamicFieldReference<SchedulerConfigKey>],
) -> String {
    let keys = configs
        .iter()
        .take(5)
        .map(|field| {
            let key = &field.name;
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
        PolicySymbol::Witness { pos0 } => format!("witness({})", pos0.name),
        PolicySymbol::Uid { pos0 } => format!("uid({})", pos0.bytes),
    }
}

fn policy_symbol_matches_move_struct<T>(symbol: &PolicySymbol, objects: &NexusObjects) -> bool
where
    T: MoveStruct,
{
    let PolicySymbol::Witness { pos0: name } = symbol else {
        return false;
    };

    type_name_matches_move_struct::<T>(name, objects)
}

fn type_name_matches_move_struct<T>(name: &TypeName, objects: &NexusObjects) -> bool
where
    T: MoveStruct,
{
    let tag = crate::move_bindings::struct_tag::<T>(objects);
    let expected = crate::move_bindings::struct_type_name::<T>(objects);
    if name.matches_qualified_name(&expected) {
        return true;
    }

    let expected_suffix = format!("::{}::{}", tag.module(), tag.name());
    let actual = name.as_str().trim_start_matches("0x");
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
            NexusEventKind::ScheduledSkillExecutionCreated(e) => Some(e.task.clone().into()),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow!(
                "ScheduledSkillExecutionCreatedEvent not found in response"
            ))
        })
}

pub(crate) fn extract_occurrence_event(response: &ExecutedTransaction) -> Option<NexusEventKind> {
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
            events::{NexusEvent, NexusEventKind},
            move_bindings::{
                interface::{
                    agent::{AgentExecutionConfig, ExecutionSelection},
                    graph as graph_move,
                    scheduled_request,
                },
                move_std::{ascii::String as MoveString, option::Option as MoveOption},
                primitives::{
                    automaton::{
                        ConfiguredAutomaton,
                        DeterministicAutomaton,
                        TransitionConfigKey,
                        TransitionKey,
                    },
                    data::NexusData,
                    event as event_move,
                    policy::{Policy, Symbol as PolicySymbol},
                },
                scheduler::scheduler::{
                    Constraints,
                    Execution,
                    Metadata,
                    OccurrenceScheduledEvent,
                    ScheduledSkillExecutionCreatedEvent,
                    State as TaskState,
                    Task,
                },
                sui_framework::vec_map,
            },
            nexus::{client::NexusClient, error::NexusError, signer::ExecutedTransaction},
            sui,
            test_utils::{nexus_mocks, sui_mocks},
            types::NexusObjects,
        },
        rand::thread_rng,
        serde::Serialize,
        std::{marker::PhantomData, sync::Arc},
        tokio::sync::Mutex,
    };

    type RequestScheduledOccurrenceEvent =
        scheduled_request::RequestScheduledExecution<OccurrenceScheduledEvent>;

    fn inline_bytes(value: &'static [u8]) -> NexusData {
        NexusData::inline_one(value.to_vec())
    }

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
    fn sample_input_data() -> HashMap<String, HashMap<String, NexusData>> {
        HashMap::from([(
            "entry_vertex".to_string(),
            HashMap::from([("entry_port".to_string(), inline_bytes(b"payload"))]),
        )])
    }

    fn mock_task_object(task_id: sui::types::Address, owner: sui::types::Address) -> Task {
        fn string(value: &str) -> crate::move_bindings::move_std::string::String {
            crate::move_bindings::move_std::string::String {
                bytes: value.as_bytes().to_vec(),
            }
        }

        let metadata = Metadata {
            values: vec_map::VecMap {
                contents: vec![vec_map::Entry {
                    key: string("initial"),
                    value: string("value"),
                }],
            },
        };

        let dfa = DeterministicAutomaton {
            states: crate::move_bindings::sui_framework::table_vec::TableVec::new(
                sui::types::Address::from_static("0x100"),
                1,
            ),
            alphabet: crate::move_bindings::sui_framework::table_vec::TableVec::new(
                sui::types::Address::from_static("0x101"),
                0,
            ),
            transition: crate::move_bindings::sui_framework::table_vec::TableVec::new(
                sui::types::Address::from_static("0x102"),
                1,
            ),
            accepting: crate::move_bindings::sui_framework::table_vec::TableVec::new(
                sui::types::Address::from_static("0x103"),
                1,
            ),
            start: 0,
            phantom_t0: PhantomData,
            phantom_t1: PhantomData,
        };

        let constraints = Policy {
            id: crate::move_bindings::sui_framework::object::UID::new(
                sui::types::Address::from_static("0x400"),
            ),
            dfa: ConfiguredAutomaton {
                id: crate::move_bindings::sui_framework::object::UID::new(
                    sui::types::Address::from_static("0x401"),
                ),
                dfa: dfa.clone(),
                phantom_t0: PhantomData,
                phantom_t1: PhantomData,
            },
            state_index: 0,
            data: Constraints { dummy_field: false },
        };

        let execution = Policy {
            id: crate::move_bindings::sui_framework::object::UID::new(
                sui::types::Address::from_static("0x500"),
            ),
            dfa: ConfiguredAutomaton {
                id: crate::move_bindings::sui_framework::object::UID::new(
                    sui::types::Address::from_static("0x600"),
                ),
                dfa,
                phantom_t0: PhantomData,
                phantom_t1: PhantomData,
            },
            state_index: 0,
            data: Execution { dummy_field: false },
        };

        Task {
            id: crate::move_bindings::sui_framework::object::UID::new(task_id),
            owner,
            agent_id: object_id(sui::types::Address::from_static("0xa11ce")),
            skill_id: 7,
            interface_version: crate::move_bindings::interface::version::InterfaceVersion::new(1),
            metadata,
            constraints,
            execution,
            state: TaskState::Active,
            data: crate::move_bindings::sui_framework::bag::Bag::new(
                sui::types::Address::from_static("0x700"),
                0,
            ),
            objects: crate::move_bindings::sui_framework::object_bag::ObjectBag::new(
                sui::types::Address::from_static("0x701"),
                0,
            ),
        }
    }

    fn mock_get_task_object(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        task_ref: sui::types::ObjectReference,
        owner: sui::types::Address,
        task: Task,
    ) {
        sui_mocks::grpc::mock_get_object_bcs(
            ledger_service_mock,
            task_ref,
            sui::types::Owner::Address(owner),
            bcs::to_bytes(&task).expect("generated task serializes"),
        );
    }

    fn generator_symbol(scheduler_pkg_id: sui::types::Address, name: &str) -> PolicySymbol {
        PolicySymbol::witness(TypeName::new(&format!(
            "{scheduler_pkg_id}::scheduler::{name}"
        )))
    }

    fn queue_generator_symbol(scheduler_pkg_id: sui::types::Address) -> PolicySymbol {
        generator_symbol(scheduler_pkg_id, "QueueGeneratorWitness")
    }

    fn generated_type_name<T>(objects: &NexusObjects) -> String
    where
        T: MoveStruct,
    {
        crate::move_bindings::struct_type_name::<T>(objects)
    }

    fn generated_type_name_with_package<T>(
        objects: &NexusObjects,
        package: sui::types::Address,
    ) -> String
    where
        T: MoveStruct,
    {
        crate::move_bindings::struct_type_name_with_package::<T>(objects, package)
    }

    fn object_id(bytes: sui::types::Address) -> crate::move_bindings::sui_framework::object::ID {
        crate::move_bindings::sui_framework::object::ID::new(bytes)
    }

    fn execution_config_field(
        config_name: &str,
        symbol: PolicySymbol,
        field_id: sui::types::Address,
    ) -> DynamicFieldReference<SchedulerConfigKey> {
        DynamicFieldReference {
            name: TransitionConfigKey {
                transition: TransitionKey {
                    state: MoveOption::from_option(None),
                    symbol,
                },
                config: TypeName::from(config_name.to_string()),
            },
            field_id,
        }
    }

    fn default_agent_execution_config(
        objects: &NexusObjects,
        dag_id: sui::types::Address,
    ) -> AgentExecutionConfig {
        AgentExecutionConfig {
            selection: ExecutionSelection::DefaultAgent {
                dag_id: object_id(dag_id),
            },
            network: object_id(objects.network_id),
            entry_group: graph_move::EntryGroup::new("entry"),
            inputs: vec_map::VecMap { contents: vec![] },
            invoker: sui::types::Address::ZERO,
            priority_fee_percentage: 0,
            authorization_templates: vec![],
        }
    }

    fn event_bcs(
        objects: &NexusObjects,
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

        let wrapper = crate::move_bindings::struct_tag::<
            event_move::EventWrapper<OccurrenceScheduledEvent>,
        >(objects);
        let wrapper_tag = sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(event_type))],
        );

        let bcs = match &kind {
            NexusEventKind::RequestScheduledOccurrence(ev) => {
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

        sui_mocks::mock_sui_event(objects.primitives_pkg_id, wrapper_tag, bcs)
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

    #[test]
    fn scheduled_execution_config_matches_execution_entries_symbol_by_module_name() {
        let objects = sui_mocks::mock_nexus_objects();
        let dag_id = sui::types::Address::from_static("0xd");
        let defining_pkg_not_in_objects = sui::types::Address::from_static("0x999");
        let config_name = generated_type_name_with_package::<AgentExecutionConfig>(
            &objects,
            defining_pkg_not_in_objects,
        );
        let symbol = PolicySymbol::witness(TypeName::new(&generated_type_name_with_package::<
            execution_entries_move::AdvanceForDefaultAgentExecution,
        >(
            &objects, defining_pkg_not_in_objects
        )));

        let fields = vec![execution_config_field(
            &config_name,
            symbol,
            sui::types::Address::from_static("0x1"),
        )];
        assert!(find_execution_config_field::<
            execution_entries_move::AdvanceForDefaultAgentExecution,
        >(&fields, &objects,)
        .is_some());

        let config = ensure_default_agent_execution_config(
            default_agent_execution_config(&objects, dag_id),
            "AdvanceForDefaultAgentExecution config",
        )
        .expect("default config should validate");
        assert_eq!(config.selection.dag_id(), Some(dag_id));
    }

    #[test]
    fn scheduled_execution_config_matches_raw_move_type_names_without_0x_prefix() {
        let objects = sui_mocks::mock_nexus_objects();
        let interface_pkg = "82da904e4d6040729d0b16ee49aae067bed36aec31e167b24e1e072221c1eb16";
        let workflow_pkg = "deafc2c9ef3914a7d4945572d07fd961fef7ff1f3e1d329f4057c28538598776";
        let config_name = format!("{interface_pkg}::agent::AgentExecutionConfig");
        let symbol = PolicySymbol::witness(TypeName::new(&format!(
            "{workflow_pkg}::execution_entries::AdvanceForDefaultAgentExecution"
        )));

        let fields = vec![execution_config_field(
            &config_name,
            symbol,
            sui::types::Address::from_static("0x1"),
        )];
        assert!(find_execution_config_field::<
            execution_entries_move::AdvanceForDefaultAgentExecution,
        >(&fields, &objects,)
        .is_some());
    }

    #[tokio::test]
    async fn scheduled_execution_config_fetches_selected_bcs_config_field() {
        #[derive(Clone, Serialize)]
        struct DynamicFieldValueBcs<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        let mut rng = rand::thread_rng();
        let objects = sui_mocks::mock_nexus_objects();
        let configured_automaton_id = sui::types::Address::from_static("0xca");
        let field_id = sui::types::Address::from_static("0xcf");
        let dag_id = sui::types::Address::from_static("0xd");
        let invoker = sui::types::Address::from_static("0x1");
        let config_name = generated_type_name::<AgentExecutionConfig>(&objects);
        let symbol = PolicySymbol::witness(TypeName::new(&generated_type_name::<
            execution_entries_move::AdvanceForDefaultAgentExecution,
        >(&objects)));
        let key = TransitionConfigKey {
            transition: TransitionKey {
                state: MoveOption::<u64>::from_option(None),
                symbol,
            },
            config: TypeName::from(config_name),
        };
        let input_value = inline_bytes(b"amount=7");
        let move_config = agent_move::AgentExecutionConfig {
            selection: agent_move::ExecutionSelection::DefaultAgent {
                dag_id: crate::move_bindings::sui_framework::object::ID::new(dag_id),
            },
            network: crate::move_bindings::sui_framework::object::ID::new(objects.network_id),
            entry_group: graph_move::EntryGroup {
                name: MoveString::from("default"),
            },
            inputs: vec_map::VecMap {
                contents: vec![vec_map::Entry {
                    key: graph_move::Vertex {
                        name: MoveString::from("source"),
                    },
                    value: vec_map::VecMap {
                        contents: vec![vec_map::Entry {
                            key: graph_move::InputPort {
                                name: MoveString::from("amount"),
                            },
                            value: input_value.clone(),
                        }],
                    },
                }],
            },
            invoker,
            priority_fee_percentage: 1000,
            authorization_templates: vec![],
        };

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(key.clone(), field_id)],
        );

        let field_value = DynamicFieldValueBcs {
            id: field_id,
            name: key,
            value: move_config,
        };
        let field_ref =
            sui::types::ObjectReference::new(field_id, 1, sui::types::Digest::generate(&mut rng));
        let object_type =
            crate::move_bindings::struct_tag::<agent_move::AgentExecutionConfig>(&objects);
        sui_mocks::grpc::mock_get_object_bcs_for(
            &mut ledger_service_mock,
            field_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&field_value).expect("dynamic field BCS should serialize"),
            object_type,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let config =
            fetch_scheduled_agent_execution_config(&crawler, &objects, &configured_automaton_id)
                .await
                .expect("generated config should decode from BCS");

        let ScheduledAgentExecutionConfig::Default(config) = config else {
            panic!("expected default scheduled config");
        };
        assert_eq!(config.selection.dag_id(), Some(dag_id));
        assert_eq!(config.network_address(), objects.network_id);
        assert_eq!(config.entry_group.name.as_str(), "default");
        assert_eq!(config.invoker, invoker);
        let vertex_inputs = config
            .inputs
            .get(&graph_move::Vertex::new("source"))
            .expect("source inputs converted");
        assert_eq!(
            vertex_inputs.get(&graph_move::InputPort::new("amount")),
            Some(&input_value)
        );
    }

    #[test]
    fn scheduled_execution_config_does_not_swallow_default_selection_mismatch() {
        let objects = sui_mocks::mock_nexus_objects();
        let config = AgentExecutionConfig {
            selection: ExecutionSelection::AgentSkill {
                agent_id: object_id(sui::types::Address::from_static("0xa")),
                skill_id: 1,
                selected_dag: MoveOption::from_option(Some(object_id(
                    sui::types::Address::from_static("0xd"),
                ))),
            },
            network: object_id(objects.network_id),
            entry_group: graph_move::EntryGroup::new("entry"),
            inputs: vec_map::VecMap { contents: vec![] },
            invoker: sui::types::Address::ZERO,
            priority_fee_percentage: 0,
            authorization_templates: vec![],
        };

        let err =
            ensure_default_agent_execution_config(config, "AdvanceForDefaultAgentExecution config")
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
            &nexus_objects,
            nexus_objects.scheduler_pkg_id,
            NexusEventKind::ScheduledSkillExecutionCreated(ScheduledSkillExecutionCreatedEvent {
                task: object_id(task_id),
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
            execution_priority_fee_percentage: Some(10),
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: None,
            skill_id: None,
            tap_payment: Some(CreateTaskTapPayment::UserFunded {
                prepay_amount_mist: 1_000,
                refund_recipient: None,
                occurrence_budget_mist: 100,
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
                prepay_amount_mist: 1_000,
                occurrence_budget_mist: 100,
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
            &nexus_objects,
            nexus_objects.scheduler_pkg_id,
            NexusEventKind::ScheduledSkillExecutionCreated(ScheduledSkillExecutionCreatedEvent {
                task: object_id(task_id),
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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);

        // Scheduled occurrence event
        let queue_generator = queue_generator_symbol(nexus_objects.scheduler_pkg_id);
        let scheduled_event =
            NexusEventKind::RequestScheduledOccurrence(RequestScheduledOccurrenceEvent {
                request: OccurrenceScheduledEvent {
                    task: object_id(task_id),
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
                &nexus_objects,
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
            OccurrenceRequest::new(Some(1_000), Some(2_000), None, None, Some(50), true)
                .expect("valid request");

        let params = CreateTaskParams {
            dag_id,
            entry_group: "entry".into(),
            input_data: sample_input_data(),
            metadata: vec![],
            execution_priority_fee_percentage: Some(50),
            initial_schedule: Some(initial_schedule),
            generator: GeneratorKind::Queue,
            agent_id: None,
            skill_id: None,
            tap_payment: Some(CreateTaskTapPayment::UserFunded {
                prepay_amount_mist: 2_000,
                refund_recipient: None,
                occurrence_budget_mist: 200,
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
                if env.request.task == object_id(task_id) && env.priority == 10 && env.start_ms == 200
        ));
        assert_eq!(
            result.tap_payment,
            Some(CreateTaskTapPaymentResult {
                agent_id: nexus_objects.default_dag_executor.agent_id,
                skill_id: nexus_objects.default_dag_executor.skill_id,
                prepay_amount_mist: 2_000,
                occurrence_budget_mist: 200,
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
            execution_priority_fee_percentage: None,
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
        // Agent-bound task creation delegates to the TAP action, which owns the
        // agent/skill/payment creation semantics.
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
            &nexus_objects,
            nexus_objects.scheduler_pkg_id,
            NexusEventKind::ScheduledSkillExecutionCreated(ScheduledSkillExecutionCreatedEvent {
                task: object_id(task_id),
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
            execution_priority_fee_percentage: Some(70),
            initial_schedule: None,
            generator: GeneratorKind::Queue,
            agent_id: Some(agent_id),
            skill_id: Some(3),
            tap_payment: Some(CreateTaskTapPayment::UserFunded {
                prepay_amount_mist: 100,
                refund_recipient: None,
                occurrence_budget_mist: 25,
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
                prepay_amount_mist: 100,
                occurrence_budget_mist: 25,
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
            execution_priority_fee_percentage: None,
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
            execution_priority_fee_percentage: None,
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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);
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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);

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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);

        let generator = queue_generator_symbol(nexus_objects.scheduler_pkg_id);
        let scheduled_event = NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
            task: object_id(task_id),
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
                &nexus_objects,
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
            OccurrenceRequest::new(Some(2_000), Some(2_500), None, None, Some(70), true).unwrap();

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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);

        let generator = queue_generator_symbol(nexus_objects.scheduler_pkg_id);
        let scheduled_event = NexusEventKind::OccurrenceScheduled(OccurrenceScheduledEvent {
            task: object_id(task_id),
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
                &nexus_objects,
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

        let request = OccurrenceRequest::new(None, None, Some(500), Some(900), Some(40), true)
            .expect("valid");

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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);

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
            priority_fee_percentage: Some(20),
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
        let task_object = mock_task_object(task_id, owner);
        let task_ref =
            sui::types::ObjectReference::new(task_id, 1, sui::types::Digest::generate(&mut rng));
        mock_get_task_object(&mut ledger_service_mock, task_ref, owner, task_object);

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
        assert!(OccurrenceRequest::new(Some(10), Some(20), None, None, None, true).is_ok());
        assert!(OccurrenceRequest::new(None, None, Some(5), Some(15), None, true).is_ok());

        let err = OccurrenceRequest::new(None, None, None, None, None, true).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Provide either")));

        let err = OccurrenceRequest::new(Some(50), Some(40), None, None, None, true).unwrap_err();
        assert!(matches!(err, NexusError::Configuration(msg) if msg.contains("Deadline")));

        let err = OccurrenceRequest::new(None, None, None, Some(10), None, false).unwrap_err();
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
                task: object_id(sui::types::Address::generate(&mut rng)),
                generator: PolicySymbol::uid(workflow_pkg),
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
                    task: object_id(task_id),
                    generator: PolicySymbol::uid(workflow_pkg),
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
                task: object_id(task_id),
                generator: PolicySymbol::uid(workflow_pkg),
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
