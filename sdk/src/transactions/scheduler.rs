use {
    crate::{
        move_bindings::{
            interface::{
                agent::{self as agent_binding, AgentExecutionConfig, ExecutionSelection},
                authorization::{self as authorization_binding, AgentVertexAuthorizationTemplate},
                graph::{InputPort, Vertex},
            },
            move_std::option::Option as MoveOption,
            primitives::data::NexusData,
            scheduler::scheduler as scheduler_binding,
            sui_framework::{
                object::ID as MoveObjectId,
                vec_map::{self as vec_map_binding, VecMap},
            },
            workflow::{execution_entries as execution_entries_binding, gas as gas_binding},
        },
        move_boundary,
        sui,
        transactions::agent_input::AgentInput,
        types::{effective_priority_fee_percentage, NexusObjects},
    },
    std::collections::HashSet,
    sui::types::ProgrammableTransaction,
};

/// Funding and controller authority for a new scheduled [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub enum TaskFunding {
    /// Funds a Task from the sender address balance.
    ///
    /// `agent` is required for an agent skill and absent for the default
    /// executor.
    User {
        agent: Option<AgentInput>,
        prepay_amount_mist: u64,
        refund_recipient: sui::types::Address,
    },
    /// Funds and controls a Task through an [`Agent`] payment vault.
    ///
    /// [`Agent`]: crate::move_bindings::interface::agent::Agent
    Agent {
        agent: AgentInput,
        prepay_amount_mist: u64,
    },
}

/// Failure behavior applied after terminal occurrence settlement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskFailureMode {
    /// Continue advertising eligible work after failure.
    #[default]
    Continue,
    /// Pause the Task after failure.
    Pause,
}

/// One manually scheduled occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OccurrenceSpec {
    pub start_time_ms: u64,
    pub deadline_ms: Option<u64>,
    pub priority_fee_percentage: u64,
}

impl OccurrenceSpec {
    /// Validates time ordering and the priority fee range.
    pub fn validate(self) -> anyhow::Result<Self> {
        if self
            .deadline_ms
            .is_some_and(|deadline| deadline < self.start_time_ms)
        {
            anyhow::bail!("occurrence deadline must not precede its start");
        }
        effective_priority_fee_percentage(Some(self.priority_fee_percentage))?;
        Ok(self)
    }
}

/// One lazy recurrence whose first candidate uses [`Self::first`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecurrenceSpec {
    pub first: OccurrenceSpec,
    pub interval_ms: u64,
    pub occurrences: Option<u64>,
}

impl RecurrenceSpec {
    /// Validates recurrence bounds that are independent from skill policy.
    pub fn validate(self) -> anyhow::Result<Self> {
        self.first.validate()?;
        if self.interval_ms == 0 {
            anyhow::bail!("recurrence interval must be greater than zero");
        }
        if self.occurrences == Some(0) {
            anyhow::bail!("recurrence occurrences must be greater than zero when present");
        }
        Ok(self)
    }
}

/// Complete input for one composable Task creation transaction.
#[derive(Clone, Debug)]
pub struct CreateTaskParams {
    pub execution: AgentExecutionConfig,
    pub funding: TaskFunding,
    pub occurrence_budget_mist: u64,
    pub failure_mode: TaskFailureMode,
    pub occurrences: Vec<OccurrenceSpec>,
    pub recurrence: Option<RecurrenceSpec>,
}

/// Authority used to mutate an existing [`Task`].
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
#[derive(Clone, Debug)]
pub enum TaskAuthority {
    Address,
    Agent(AgentInput),
}

fn shared_task_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.shared_object(task, true)?)
}

fn execution_config_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    execution: &AgentExecutionConfig,
) -> anyhow::Result<sui::types::Argument> {
    let network = tx.object_id(execution.network.bytes)?;
    let entry_group = tx.graph_entry_group(execution.entry_group.as_str())?;
    let inputs = execution_inputs_arg(tx, &execution.inputs)?;

    match &execution.selection {
        ExecutionSelection::DefaultAgent { dag_id } => {
            if !execution.authorization_templates.is_empty() {
                anyhow::bail!("default execution cannot include authorization templates");
            }
            let dag_id = tx.object_id(dag_id.bytes)?;
            tx.call_target(
                agent_binding::new_default_agent_execution_config_target,
                vec![dag_id, network, entry_group, inputs],
            )
        }
        ExecutionSelection::AgentSkill {
            agent_id,
            skill_id,
            selected_dag,
        } => {
            let agent_id = tx.object_id(agent_id.bytes)?;
            let skill_id = tx.arg(skill_id)?;
            let selected_dag =
                optional_object_id_arg(tx, selected_dag.as_option().map(|dag_id| dag_id.bytes))?;
            let authorization_templates =
                authorization_templates_arg(tx, &execution.authorization_templates)?;
            tx.call_target(
                agent_binding::new_agent_execution_config_target,
                vec![
                    agent_id,
                    network,
                    entry_group,
                    inputs,
                    skill_id,
                    selected_dag,
                    authorization_templates,
                ],
            )
        }
    }
}

type ExecutionInputs = VecMap<Vertex, VecMap<InputPort, NexusData>>;
type VertexInputs = VecMap<InputPort, NexusData>;

fn execution_inputs_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    inputs: &ExecutionInputs,
) -> anyhow::Result<sui::types::Argument> {
    let execution_inputs = tx.call_target(
        vec_map_binding::empty_target::<Vertex, VertexInputs>,
        vec![],
    )?;

    for vertex_inputs in &inputs.contents {
        let vertex = tx.graph_vertex(vertex_inputs.key.as_str())?;
        let inputs_for_vertex = tx.call_target(
            vec_map_binding::empty_target::<InputPort, NexusData>,
            vec![],
        )?;

        for input in &vertex_inputs.value.contents {
            let input_port = tx.graph_input_port(input.key.as_str())?;
            let value = tx.nexus_data(&input.value)?;
            tx.call_target(
                vec_map_binding::insert_target::<InputPort, NexusData>,
                vec![inputs_for_vertex, input_port, value],
            )?;
        }

        tx.call_target(
            vec_map_binding::insert_target::<Vertex, VertexInputs>,
            vec![execution_inputs, vertex, inputs_for_vertex],
        )?;
    }

    Ok(execution_inputs)
}

fn optional_object_id_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    value: Option<sui::types::Address>,
) -> anyhow::Result<sui::types::Argument> {
    let value = value.map(|value| tx.object_id(value)).transpose()?;
    Ok(tx.option::<MoveObjectId>(value)?)
}

fn authorization_template_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    template: &AgentVertexAuthorizationTemplate,
) -> anyhow::Result<sui::types::Argument> {
    let skill_id = tx.arg(&template.skill_id)?;
    let vertex = tx.ascii_string(template.vertex.as_str())?;
    let recipient_id = tx.object_id(template.recipient_id.bytes)?;
    tx.call_target(
        authorization_binding::agent_vertex_authorization_template_target,
        vec![skill_id, vertex, recipient_id],
    )
}

fn authorization_templates_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    templates: &[AgentVertexAuthorizationTemplate],
) -> anyhow::Result<sui::types::Argument> {
    let templates = templates
        .iter()
        .map(|template| authorization_template_arg(tx, template))
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(tx.move_vector::<AgentVertexAuthorizationTemplate>(templates)?)
}

fn failure_mode_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    failure_mode: TaskFailureMode,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        match failure_mode {
            TaskFailureMode::Continue => scheduler_binding::continue_on_failure_target,
            TaskFailureMode::Pause => scheduler_binding::pause_on_failure_target,
        },
        vec![],
    )
}

fn occurrence_args(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    occurrence: OccurrenceSpec,
) -> anyhow::Result<(
    sui::types::Argument,
    sui::types::Argument,
    sui::types::Argument,
)> {
    let occurrence = occurrence.validate()?;
    Ok((
        tx.arg(&occurrence.start_time_ms)?,
        tx.arg(&MoveOption::from_option(occurrence.deadline_ms))?,
        tx.arg(&occurrence.priority_fee_percentage)?,
    ))
}

fn append_schedule(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: sui::types::Argument,
    authority: &TaskAuthority,
    occurrence: OccurrenceSpec,
) -> anyhow::Result<()> {
    let (start_time_ms, deadline_ms, priority_fee_percentage) = occurrence_args(tx, occurrence)?;
    let leader_registry_ref = tx.objects().leader_registry.clone();
    let leader_registry = tx.shared_object(&leader_registry_ref, false)?;
    let clock = tx.clock()?;

    match authority {
        TaskAuthority::Address => {
            tx.call_target(
                scheduler_binding::schedule_target,
                vec![
                    task,
                    start_time_ms,
                    deadline_ms,
                    priority_fee_percentage,
                    leader_registry,
                    clock,
                ],
            )?;
        }
        TaskAuthority::Agent(agent) => {
            let agent = agent.clone().immutable_ptb_argument(tx)?;
            tx.call_target(
                scheduler_binding::schedule_as_agent_target,
                vec![
                    task,
                    agent,
                    start_time_ms,
                    deadline_ms,
                    priority_fee_percentage,
                    leader_registry,
                    clock,
                ],
            )?;
        }
    }
    Ok(())
}

fn append_set_recurrence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: sui::types::Argument,
    authority: &TaskAuthority,
    recurrence: RecurrenceSpec,
) -> anyhow::Result<()> {
    let recurrence = recurrence.validate()?;
    let (start_time_ms, deadline_ms, priority_fee_percentage) =
        occurrence_args(tx, recurrence.first)?;
    let interval_ms = tx.arg(&recurrence.interval_ms)?;
    let occurrences = tx.arg(&MoveOption::from_option(recurrence.occurrences))?;
    let leader_registry_ref = tx.objects().leader_registry.clone();
    let leader_registry = tx.shared_object(&leader_registry_ref, false)?;
    let clock = tx.clock()?;

    match authority {
        TaskAuthority::Address => {
            tx.call_target(
                scheduler_binding::set_recurrence_target,
                vec![
                    task,
                    start_time_ms,
                    deadline_ms,
                    interval_ms,
                    occurrences,
                    priority_fee_percentage,
                    leader_registry,
                    clock,
                ],
            )?;
        }
        TaskAuthority::Agent(agent) => {
            let agent = agent.clone().immutable_ptb_argument(tx)?;
            tx.call_target(
                scheduler_binding::set_recurrence_as_agent_target,
                vec![
                    task,
                    agent,
                    start_time_ms,
                    deadline_ms,
                    interval_ms,
                    occurrences,
                    priority_fee_percentage,
                    leader_registry,
                    clock,
                ],
            )?;
        }
    }
    Ok(())
}

/// Appends Task creation, scheduling, recurrence, and sharing to one PTB.
pub fn append_create_task(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    params: &CreateTaskParams,
) -> anyhow::Result<sui::types::Argument> {
    if params.occurrence_budget_mist == 0 {
        anyhow::bail!("occurrence budget must be greater than zero");
    }
    params
        .occurrences
        .iter()
        .copied()
        .try_for_each(|occurrence| occurrence.validate().map(|_| ()))?;
    if let Some(recurrence) = params.recurrence {
        recurrence.validate()?;
    }

    let selection = &params.execution.selection;
    let registry_ref = tx.objects().agent_registry.clone();
    let registry = tx.shared_object(&registry_ref, true)?;
    let config = execution_config_arg(tx, &params.execution)?;
    let occurrence_budget_mist = tx.arg(&params.occurrence_budget_mist)?;
    let failure_mode = failure_mode_arg(tx, params.failure_mode)?;

    let (task, authority) = match (&params.funding, selection) {
        (
            TaskFunding::User {
                agent: None,
                prepay_amount_mist,
                refund_recipient,
            },
            ExecutionSelection::DefaultAgent { .. },
        ) => {
            let prepayment = tx.withdraw_sui_coin(*prepay_amount_mist)?;
            let refund_recipient = tx.arg(refund_recipient)?;
            (
                tx.call_target(
                    scheduler_binding::new_default_task_target,
                    vec![
                        registry,
                        config,
                        prepayment,
                        refund_recipient,
                        occurrence_budget_mist,
                        failure_mode,
                    ],
                )?,
                TaskAuthority::Address,
            )
        }
        (
            TaskFunding::User {
                agent: Some(agent),
                prepay_amount_mist,
                refund_recipient,
            },
            ExecutionSelection::AgentSkill { agent_id, .. },
        ) => {
            if agent.object_id() != agent_id.bytes {
                anyhow::bail!("funding agent does not match execution selection");
            }
            let agent_arg = agent.clone().immutable_ptb_argument(tx)?;
            let prepayment = tx.withdraw_sui_coin(*prepay_amount_mist)?;
            let refund_recipient = tx.arg(refund_recipient)?;
            (
                tx.call_target(
                    scheduler_binding::new_user_task_target,
                    vec![
                        registry,
                        agent_arg,
                        config,
                        prepayment,
                        refund_recipient,
                        occurrence_budget_mist,
                        failure_mode,
                    ],
                )?,
                TaskAuthority::Address,
            )
        }
        (
            TaskFunding::Agent {
                agent,
                prepay_amount_mist,
            },
            ExecutionSelection::AgentSkill { agent_id, .. },
        ) => {
            if agent.object_id() != agent_id.bytes {
                anyhow::bail!("funding agent does not match execution selection");
            }
            let agent_arg = agent.clone().mutable_ptb_argument(tx)?;
            let prepay_amount_mist = tx.arg(prepay_amount_mist)?;
            (
                tx.call_target(
                    scheduler_binding::new_agent_task_target,
                    vec![
                        registry,
                        agent_arg,
                        config,
                        prepay_amount_mist,
                        occurrence_budget_mist,
                        failure_mode,
                    ],
                )?,
                TaskAuthority::Agent(agent.clone()),
            )
        }
        (TaskFunding::User { agent: None, .. }, ExecutionSelection::AgentSkill { .. }) => {
            anyhow::bail!("an agent skill Task requires its Agent object")
        }
        (TaskFunding::User { agent: Some(_), .. }, ExecutionSelection::DefaultAgent { .. }) => {
            anyhow::bail!("a default executor Task must not include an Agent object")
        }
        (TaskFunding::Agent { .. }, ExecutionSelection::DefaultAgent { .. }) => {
            anyhow::bail!("a default executor Task cannot use Agent vault funding")
        }
    };

    params
        .occurrences
        .iter()
        .copied()
        .try_for_each(|occurrence| append_schedule(tx, task, &authority, occurrence))?;
    if let Some(recurrence) = params.recurrence {
        append_set_recurrence(tx, task, &authority, recurrence)?;
    }
    tx.call_target(scheduler_binding::share_target, vec![task])?;
    Ok(task)
}

/// Builds one PTB for a complete [`Task`] composition.
///
/// [`Task`]: crate::move_bindings::scheduler::task::Task
pub fn create_task_ptb(
    objects: &NexusObjects,
    params: &CreateTaskParams,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        append_create_task(tx, params)?;
        Ok(())
    })
}

/// Builds a PTB that adds one manual occurrence.
pub fn schedule_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: TaskAuthority,
    occurrence: OccurrenceSpec,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        append_schedule(tx, task, &authority, occurrence)
    })
}

/// Builds a PTB that sets the Task recurrence.
pub fn set_recurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: TaskAuthority,
    recurrence: RecurrenceSpec,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        append_set_recurrence(tx, task, &authority, recurrence)
    })
}

/// Builds a PTB that clears the Task recurrence.
pub fn clear_recurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: TaskAuthority,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        match authority {
            TaskAuthority::Address => {
                tx.call_target(
                    scheduler_binding::clear_recurrence_target,
                    vec![task, leader_registry, clock],
                )?;
            }
            TaskAuthority::Agent(agent) => {
                let agent = agent.immutable_ptb_argument(tx)?;
                tx.call_target(
                    scheduler_binding::clear_recurrence_as_agent_target,
                    vec![task, agent, leader_registry, clock],
                )?;
            }
        }
        Ok(())
    })
}

/// Builds a PTB that pauses, resumes, or cancels a Task.
pub fn set_task_state_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: TaskAuthority,
    action: TaskStateAction,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let leader_registry = matches!(action, TaskStateAction::Resume)
            .then(|| tx.shared_object(&objects.leader_registry, false))
            .transpose()?;
        let clock = matches!(action, TaskStateAction::Resume)
            .then(|| tx.clock())
            .transpose()?;

        match (authority, action) {
            (TaskAuthority::Address, TaskStateAction::Pause) => {
                tx.call_target(scheduler_binding::pause_target, vec![task])?;
            }
            (TaskAuthority::Address, TaskStateAction::Resume) => {
                tx.call_target(
                    scheduler_binding::resume_target,
                    vec![task, leader_registry.unwrap(), clock.unwrap()],
                )?;
            }
            (TaskAuthority::Address, TaskStateAction::Cancel) => {
                tx.call_target(scheduler_binding::cancel_target, vec![task])?;
            }
            (TaskAuthority::Agent(agent), TaskStateAction::Pause) => {
                let agent = agent.immutable_ptb_argument(tx)?;
                tx.call_target(scheduler_binding::pause_as_agent_target, vec![task, agent])?;
            }
            (TaskAuthority::Agent(agent), TaskStateAction::Resume) => {
                let agent = agent.immutable_ptb_argument(tx)?;
                tx.call_target(
                    scheduler_binding::resume_as_agent_target,
                    vec![task, agent, leader_registry.unwrap(), clock.unwrap()],
                )?;
            }
            (TaskAuthority::Agent(agent), TaskStateAction::Cancel) => {
                let agent = agent.immutable_ptb_argument(tx)?;
                tx.call_target(scheduler_binding::cancel_as_agent_target, vec![task, agent])?;
            }
        }
        Ok(())
    })
}

/// State transition requested for a Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStateAction {
    Pause,
    Resume,
    Cancel,
}

/// Builds a PTB that refills an address controlled Task.
pub fn refill_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    amount_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let funds = tx.withdraw_sui_coin(amount_mist)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::refill_target,
            vec![task, funds, leader_registry, clock],
        )?;
        Ok(())
    })
}

/// Builds a PTB that refills an Agent controlled Task from its vault.
pub fn refill_task_from_agent_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
    amount_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent = agent.mutable_ptb_argument(tx)?;
        let amount_mist = tx.arg(&amount_mist)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::refill_from_agent_target,
            vec![task, agent, amount_mist, leader_registry, clock],
        )?;
        Ok(())
    })
}

/// Builds a PTB that expires one stale advertised occurrence.
pub fn expire_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    occurrence_id: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let occurrence_id = tx.arg(&occurrence_id)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::expire_target,
            vec![task, occurrence_id, leader_registry, clock],
        )?;
        Ok(())
    })
}

/// Builds a PTB that dispatches a scheduled occurrence as a [`DAGExecution`].
///
/// The occurrence identity is validated by `dispatch_next` before gas
/// preparation and controlled sharing.
///
/// [`DAGExecution`]: crate::move_bindings::workflow::execution::DAGExecution
pub fn dispatch_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    occurrence_id: u64,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let dag = tx.shared_object(dag, false)?;
        let agent_registry = tx.shared_object(&objects.agent_registry, false)?;
        let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
        let leader_cap = tx.shared_object(leader_cap, false)?;
        let occurrence_id = tx.arg(&occurrence_id)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        let execution = tx.call_target(
            scheduler_binding::dispatch_next_target,
            vec![
                task,
                dag,
                agent_registry,
                tool_registry,
                leader_cap,
                occurrence_id,
                leader_registry,
                clock,
            ],
        )?;

        let gas_service = tx.shared_object(&objects.gas_service, false)?;
        tx.call_target(
            gas_binding::snapshot_dag_tool_costs_target,
            vec![gas_service, execution, dag],
        )?;
        for (address, version) in tools_gas {
            let tool_gas = tx.shared_object_by_id(*address, *version, true)?;
            tx.call_target(
                gas_binding::lock_payment_state_for_tool_target,
                vec![tool_gas, dag, execution],
            )?;
        }
        tx.call_target(
            execution_entries_binding::start_and_share_target,
            vec![dag, execution, leader_registry, clock],
        )?;
        Ok(())
    })
}

/// Builds a PTB that settles one finished occurrence.
pub fn settle_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let execution = tx.shared_object(execution, true)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        append_settle_occurrence(tx, task, execution, leader_registry, clock)
    })
}

pub(crate) fn append_settle_occurrence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: &sui::types::ObjectReference,
    execution: sui::types::Argument,
    leader_registry: sui::types::Argument,
    clock: sui::types::Argument,
) -> anyhow::Result<()> {
    let task = shared_task_arg(tx, task)?;
    tx.call_target(
        scheduler_binding::settle_target,
        vec![task, execution, leader_registry, clock],
    )?;
    Ok(())
}

/// Builds a PTB that closes a Task and refunds its remaining reserve.
pub fn close_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: TaskAuthority,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let registry = tx.shared_object(&objects.agent_registry, true)?;
        match authority {
            TaskAuthority::Address => {
                tx.call_target(scheduler_binding::close_target, vec![task, registry])?;
            }
            TaskAuthority::Agent(agent) => {
                let agent = agent.mutable_ptb_argument(tx)?;
                tx.call_target(
                    scheduler_binding::close_as_agent_target,
                    vec![task, registry, agent],
                )?;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                interface::{
                    agent::ExecutionSelection,
                    authorization::AgentVertexAuthorizationTemplate,
                    graph::{EntryGroup, InputPort, Vertex},
                },
                primitives::data::NexusData,
                sui_framework::{
                    object::ID,
                    vec_map::{Entry as VecMapEntry, VecMap},
                },
            },
            test_utils::sui_mocks::{mock_nexus_objects, mock_sui_object_ref},
        },
        sui::types::{Command, MoveCall},
    };

    fn execution(sender: sui::types::Address) -> AgentExecutionConfig {
        AgentExecutionConfig::new(
            ExecutionSelection::DefaultAgent {
                dag_id: ID::new(sui::types::Address::from_static("0x42")),
            },
            ID::new(sui::types::Address::from_static("0x43")),
            EntryGroup::new("default"),
            VecMap::new(vec![]),
            sender,
            vec![],
        )
    }

    fn agent_skill_execution(sender: sui::types::Address) -> AgentExecutionConfig {
        let inputs = VecMap::new(vec![VecMapEntry::new(
            Vertex::new("vertex"),
            VecMap::new(vec![VecMapEntry::new(
                InputPort::new("input"),
                NexusData::new(b"inline".to_vec(), b"value".to_vec(), vec![]),
            )]),
        )]);
        AgentExecutionConfig::new(
            ExecutionSelection::AgentSkill {
                agent_id: ID::new(sui::types::Address::from_static("0x45")),
                skill_id: 7,
                selected_dag: MoveOption::from_option(Some(ID::new(
                    sui::types::Address::from_static("0x46"),
                ))),
            },
            ID::new(sui::types::Address::from_static("0x43")),
            EntryGroup::new("default"),
            inputs,
            sender,
            vec![AgentVertexAuthorizationTemplate::new(
                7,
                "vertex",
                ID::new(sui::types::Address::from_static("0x47")),
            )],
        )
    }

    fn move_calls(ptb: &ProgrammableTransaction) -> Vec<&MoveCall> {
        ptb.commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn creation_scheduling_recurrence_and_share_use_one_ptb() {
        let objects = mock_nexus_objects();
        let sender = sui::types::Address::from_static("0x44");
        let params = CreateTaskParams {
            execution: execution(sender),
            funding: TaskFunding::User {
                agent: None,
                prepay_amount_mist: 10_000,
                refund_recipient: sender,
            },
            occurrence_budget_mist: 1_000,
            failure_mode: TaskFailureMode::Continue,
            occurrences: vec![OccurrenceSpec {
                start_time_ms: 100,
                deadline_ms: None,
                priority_fee_percentage: 20,
            }],
            recurrence: Some(RecurrenceSpec {
                first: OccurrenceSpec {
                    start_time_ms: 200,
                    deadline_ms: Some(300),
                    priority_fee_percentage: 30,
                },
                interval_ms: 100,
                occurrences: Some(2),
            }),
        };

        let ptb = create_task_ptb(&objects, &params).expect("Task PTB builds");
        let functions = move_calls(&ptb)
            .into_iter()
            .map(|call| (call.module.as_str(), call.function.as_str()))
            .collect::<Vec<_>>();

        assert!(functions.contains(&("agent", "new_default_agent_execution_config")));
        assert!(functions.contains(&("scheduler", "new_default_task")));
        assert!(functions.contains(&("scheduler", "schedule")));
        assert!(functions.contains(&("scheduler", "set_recurrence")));
        assert_eq!(functions.last(), Some(&("scheduler", "share")));
    }

    #[test]
    fn agent_skill_execution_config_is_composed_from_move_values() {
        let objects = mock_nexus_objects();
        let execution = agent_skill_execution(sui::types::Address::from_static("0x44"));
        let ptb = move_boundary::ptb(&objects, |tx| {
            execution_config_arg(tx, &execution)?;
            Ok(())
        })
        .expect("agent skill execution config builds");
        let functions = move_calls(&ptb)
            .into_iter()
            .map(|call| (call.module.as_str(), call.function.as_str()))
            .collect::<Vec<_>>();

        assert!(functions.contains(&("graph", "entry_group_from_string")));
        assert!(functions.contains(&("graph", "vertex_from_string")));
        assert!(functions.contains(&("graph", "input_port_from_string")));
        assert!(functions.contains(&("data", "inline_one")));
        assert!(functions.contains(&("authorization", "agent_vertex_authorization_template")));
        assert!(functions.contains(&("agent", "new_agent_execution_config")));
    }

    #[test]
    fn now_and_future_occurrences_use_the_same_schedule_call() {
        let objects = mock_nexus_objects();
        for start_time_ms in [0, 10_000] {
            let ptb = schedule_task_ptb(
                &objects,
                &mock_sui_object_ref(),
                TaskAuthority::Address,
                OccurrenceSpec {
                    start_time_ms,
                    deadline_ms: None,
                    priority_fee_percentage: 20,
                },
            )
            .expect("schedule PTB builds");
            assert!(move_calls(&ptb).iter().any(|call| {
                call.module.as_str() == "scheduler" && call.function.as_str() == "schedule"
            }));
        }
    }

    #[test]
    fn refill_rechecks_and_advertises_dispatchable_work() {
        let ptb = refill_task_ptb(&mock_nexus_objects(), &mock_sui_object_ref(), 1_000)
            .expect("refill PTB builds");
        let refill = move_calls(&ptb)
            .into_iter()
            .find(|call| call.function.as_str() == "refill")
            .expect("refill call");

        assert_eq!(refill.arguments.len(), 4);
    }

    #[test]
    fn settlement_rechecks_and_advertises_dispatchable_work() {
        let ptb = settle_occurrence_ptb(
            &mock_nexus_objects(),
            &mock_sui_object_ref(),
            &mock_sui_object_ref(),
        )
        .expect("settlement PTB builds");
        let settle = move_calls(&ptb)
            .into_iter()
            .find(|call| call.function.as_str() == "settle")
            .expect("settle call");

        assert_eq!(settle.arguments.len(), 4);
    }

    #[test]
    fn dispatch_uses_scheduler_then_controlled_workflow_share() {
        let ptb = dispatch_occurrence_ptb(
            &mock_nexus_objects(),
            &mock_sui_object_ref(),
            &mock_sui_object_ref(),
            &mock_sui_object_ref(),
            7,
            &HashSet::new(),
        )
        .expect("dispatch PTB builds");
        let calls = move_calls(&ptb);
        let dispatch = calls
            .iter()
            .position(|call| call.function.as_str() == "dispatch_next")
            .expect("dispatch call");
        let start = calls
            .iter()
            .position(|call| call.function.as_str() == "start_and_share")
            .expect("controlled share call");
        assert!(dispatch < start);
        assert!(!calls
            .iter()
            .any(|call| call.function.as_str().contains("begin_")));
    }
}
