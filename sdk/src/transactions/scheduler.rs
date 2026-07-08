use {
    crate::{
        move_bindings::{
            move_std::option::Option as MoveOption,
            primitives::{data::NexusData, policy as policy_binding},
            scheduler::scheduler as scheduler_binding,
            sui_framework::{
                table_vec as table_vec_binding,
                transfer as transfer_binding,
                vec_map as vec_map_binding,
            },
            workflow::{
                execution as execution_binding,
                execution_entries as execution_entries_binding,
                gas as gas_binding,
            },
        },
        move_boundary,
        sui,
        transactions::{self, agent_input::AgentInput},
        types::{AgentId, NexusObjects, SkillId},
    },
    std::collections::{HashMap, HashSet},
    sui::types::ProgrammableTransaction,
};

/// Generator variants supported by the scheduler when executing occurrences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OccurrenceGenerator {
    Queue,
    Periodic,
}

/// Arguments required to configure periodic scheduling in the PTB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PeriodicScheduleInputs {
    pub first_start_ms: u64,
    pub period_ms: u64,
    pub deadline_offset_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
}

// Shared helper for turning a scheduled task object ref into a mutable shared argument.
fn shared_task_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    shared_mutable_object_arg(tx, task)
}

fn shared_mutable_object_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    object: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.shared_object(object, true)?)
}

// == Metadata ==

/// PTB template to build task metadata from key/value pairs.
pub(crate) fn new_metadata<K, V>(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> anyhow::Result<sui::types::Argument>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    type MoveString = crate::move_bindings::move_std::string::String;

    let metadata = tx.call_target(
        vec_map_binding::empty_target::<MoveString, MoveString>,
        vec![],
    )?;

    for (key, value) in key_values.into_iter() {
        let key = tx.ascii_string(key.as_ref())?;

        let value = tx.ascii_string(value.as_ref())?;

        tx.call_target(
            vec_map_binding::insert_target::<MoveString, MoveString>,
            vec![metadata, key, value],
        )?;
    }

    tx.call_target(scheduler_binding::new_metadata_target, vec![metadata])
}

// == Task lifecycle ==

/// PTB template to create a funded scheduled task for the registry-owned default agent.
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_default_agent_task(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    metadata: sui::types::Argument,
    constraints: sui::types::Argument,
    execution: sui::types::Argument,
    registry: sui::types::Argument,
    prepayment_coin: sui::types::Argument,
    occurrence_budget: u64,
) -> anyhow::Result<sui::types::Argument> {
    let occurrence_budget = tx.arg(&occurrence_budget)?;
    tx.call_target(
        scheduler_binding::new_default_agent_task_target,
        vec![
            metadata,
            constraints,
            execution,
            registry,
            prepayment_coin,
            occurrence_budget,
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn create_default_agent_task_ptb(
    objects: &NexusObjects,
    dag_id: sui::types::Address,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    metadata: &[(String, String)],
    generator: OccurrenceGenerator,
    priority_fee_per_gas_unit: u64,
    prepay_amount: u64,
    occurrence_budget: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let metadata = new_metadata(tx, metadata.iter().cloned())?;
        let constraints = new_constraints_policy(tx, generator)?;
        let execution = new_execution_policy(
            tx,
            dag_id,
            priority_fee_per_gas_unit,
            entry_group,
            input_data,
        )?;
        let registry = tx.shared_object(&objects.agent_registry, true)?;
        let prepay_amount = tx.arg(&prepay_amount)?;
        let gas = tx.gas();
        let prepayment_coin = tx.split_coins(gas, vec![prepay_amount])?;
        let task = new_default_agent_task(
            tx,
            metadata,
            constraints,
            execution,
            registry,
            prepayment_coin,
            occurrence_budget,
        )?;

        tx.call_target(
            transfer_binding::public_share_object_target::<scheduler_binding::Task>,
            vec![task],
        )?;
        Ok(())
    })
}

/// PTB template to update an existing task metadata bag.
pub(crate) fn update_metadata(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: &sui::types::ObjectReference,
    metadata: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    tx.call_target(
        scheduler_binding::update_metadata_target,
        vec![task, metadata],
    )
}

/// Build a PTB that updates metadata entries associated with a task.
pub(crate) fn update_metadata_ptb<K, V>(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    metadata: impl IntoIterator<Item = (K, V)>,
) -> anyhow::Result<ProgrammableTransaction>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    move_boundary::ptb(objects, |tx| {
        let metadata = new_metadata(tx, metadata)?;
        update_metadata(tx, task, metadata)?;
        Ok(())
    })
}

/// PTB template to construct and register the default constraints policy.
pub(crate) fn new_constraints_policy(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    generator: OccurrenceGenerator,
) -> anyhow::Result<sui::types::Argument> {
    let constraint_symbol = match generator {
        OccurrenceGenerator::Queue => tx.call_target(
            policy_binding::witness_symbol_target::<scheduler_binding::QueueGeneratorWitness>,
            vec![],
        )?,
        OccurrenceGenerator::Periodic => tx.call_target(
            policy_binding::witness_symbol_target::<scheduler_binding::PeriodicGeneratorWitness>,
            vec![],
        )?,
    };

    let constraint_sequence = tx.call_target(
        table_vec_binding::empty_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![],
    )?;

    tx.call_target(
        table_vec_binding::push_back_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![constraint_sequence, constraint_symbol],
    )?;

    let constraints = tx.call_target(
        scheduler_binding::new_constraints_policy_target,
        vec![constraint_sequence],
    )?;

    tx.call_target(
        table_vec_binding::drop_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![constraint_sequence],
    )?;

    match generator {
        OccurrenceGenerator::Queue => {
            let queue_state = new_queue_generator_state(tx)?;
            register_queue_generator(tx, constraints, queue_state)?;
        }
        OccurrenceGenerator::Periodic => {
            let periodic_state = new_periodic_generator_state(tx)?;
            register_periodic_generator(tx, constraints, periodic_state)?;
        }
    };

    Ok(constraints)
}

/// PTB template to construct a queue generator state.
fn new_queue_generator_state(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(scheduler_binding::new_queue_generator_state_target, vec![])
}

/// PTB template to register the queue generator state.
fn register_queue_generator(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    constraints: sui::types::Argument,
    queue_state: sui::types::Argument,
) -> anyhow::Result<()> {
    tx.call_target(
        scheduler_binding::register_queue_generator_target,
        vec![constraints, queue_state],
    )?;

    Ok(())
}

/// PTB template to construct a periodic generator state.
fn new_periodic_generator_state(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        scheduler_binding::new_periodic_generator_state_target,
        vec![],
    )
}

/// PTB template to register the periodic generator state.
fn register_periodic_generator(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    constraints: sui::types::Argument,
    periodic_state: sui::types::Argument,
) -> anyhow::Result<()> {
    tx.call_target(
        scheduler_binding::register_periodic_generator_target,
        vec![constraints, periodic_state],
    )?;

    Ok(())
}

/// PTB template to construct and register the default execution policy.
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_execution_policy(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag_id: sui::types::Address,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let execution_symbol = tx.call_target(
        policy_binding::witness_symbol_target::<
            execution_entries_binding::AdvanceForDefaultAgentExecution,
        >,
        vec![],
    )?;

    let execution_sequence = tx.call_target(
        table_vec_binding::empty_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![],
    )?;

    tx.call_target(
        table_vec_binding::push_back_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![execution_sequence, execution_symbol],
    )?;

    let execution = tx.call_target(
        scheduler_binding::new_execution_policy_target,
        vec![execution_sequence],
    )?;

    tx.call_target(
        table_vec_binding::drop_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![execution_sequence],
    )?;

    let dag_id_arg = tx.object_id(dag_id)?;
    let network_id_arg = tx.object_id(objects.network_id)?;
    let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;

    let entry_group = tx.graph_entry_group(entry_group)?;

    let with_vertex_inputs = build_inputs_vec_map(tx, input_data)?;

    let config = transactions::tap::default_agent_execution_config_arg(
        tx,
        dag_id_arg,
        network_id_arg,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
    )?;

    register_begin_default_agent_execution(tx, execution, config)?;

    Ok(execution)
}

/// PTB template to construct and register a registered TAP agent execution policy.
#[allow(clippy::too_many_arguments)]
pub(crate) fn new_agent_execution_policy(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
    agent_id: AgentId,
    skill_id: SkillId,
    selected_dag: Option<sui::types::Address>,
) -> anyhow::Result<sui::types::Argument> {
    let objects = tx.objects();
    let execution_symbol =
        tx.call_target(
            policy_binding::witness_symbol_target::<
                execution_entries_binding::AdvanceForAgentExecution,
            >,
            vec![],
        )?;

    let execution_sequence = tx.call_target(
        table_vec_binding::empty_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![],
    )?;

    tx.call_target(
        table_vec_binding::push_back_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![execution_sequence, execution_symbol],
    )?;

    let execution = tx.call_target(
        scheduler_binding::new_execution_policy_target,
        vec![execution_sequence],
    )?;

    tx.call_target(
        table_vec_binding::drop_target::<crate::move_bindings::primitives::policy::Symbol>,
        vec![execution_sequence],
    )?;

    let agent_id_arg = tx.object_id(agent_id)?;
    let network_id_arg = tx.object_id(objects.network_id)?;
    let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;
    let entry_group = tx.graph_entry_group(entry_group)?;

    let with_vertex_inputs = build_inputs_vec_map(tx, input_data)?;

    let config = transactions::tap::agent_execution_config_arg(
        tx,
        agent_id_arg,
        network_id_arg,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
        skill_id,
        selected_dag,
        &[],
    )?;

    register_begin_agent_execution(tx, execution, config)?;

    Ok(execution)
}

pub(crate) fn build_inputs_vec_map(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    input_data: &HashMap<String, HashMap<String, NexusData>>,
) -> anyhow::Result<sui::types::Argument> {
    type InputPort = crate::move_bindings::interface::graph::InputPort;
    type Vertex = crate::move_bindings::interface::graph::Vertex;
    type VertexInputMap =
        crate::move_bindings::sui_framework::vec_map::VecMap<InputPort, NexusData>;

    let with_vertex_inputs = tx.call_target(
        vec_map_binding::empty_target::<Vertex, VertexInputMap>,
        vec![],
    )?;

    for (vertex_name, data) in input_data {
        // `vertex: Vertex`
        let vertex = tx.graph_vertex(vertex_name)?;

        // `with_vertex_input: VecMap<InputPort, NexusData>`
        let with_vertex_input = tx.call_target(
            vec_map_binding::empty_target::<InputPort, NexusData>,
            vec![],
        )?;

        for (port_name, value) in data {
            // `port: InputPort`
            let port = tx.graph_input_port(port_name.as_str())?;

            // `value: NexusData`
            let value = tx.nexus_data(value)?;

            // `with_vertex_input.insert(port, value)`
            tx.call_target(
                vec_map_binding::insert_target::<InputPort, NexusData>,
                vec![with_vertex_input, port, value],
            )?;
        }

        // `with_vertex_inputs.insert(vertex, with_vertex_input)`
        tx.call_target(
            vec_map_binding::insert_target::<Vertex, VertexInputMap>,
            vec![with_vertex_inputs, vertex, with_vertex_input],
        )?;
    }

    Ok(with_vertex_inputs)
}

/// Build a PTB that pauses a task.
pub(crate) fn pause_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent_registry = tx.shared_object(&objects.agent_registry, false)?;
        tx.call_target(scheduler_binding::pause_target, vec![task, agent_registry])?;
        Ok(())
    })
}

/// Build a PTB that resumes a task.
pub(crate) fn resume_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent_registry = tx.shared_object(&objects.agent_registry, false)?;
        tx.call_target(scheduler_binding::resume_target, vec![task, agent_registry])?;
        Ok(())
    })
}

/// Build a PTB that cancels a task.
pub(crate) fn cancel_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent_registry = tx.shared_object(&objects.agent_registry, true)?;
        tx.call_target(scheduler_binding::cancel_target, vec![task, agent_registry])?;
        Ok(())
    })
}

/// Build a PTB that pauses an explicit agent task.
pub(crate) fn pause_agent_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent = agent.immutable_ptb_argument(tx)?;
        tx.call_target(
            scheduler_binding::pause_with_agent_target,
            vec![task, agent],
        )?;
        Ok(())
    })
}

/// Build a PTB that resumes an explicit agent task.
pub(crate) fn resume_agent_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent = agent.immutable_ptb_argument(tx)?;
        tx.call_target(
            scheduler_binding::resume_with_agent_target,
            vec![task, agent],
        )?;
        Ok(())
    })
}

/// Build a PTB that cancels an explicit agent task.
pub(crate) fn cancel_agent_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent = agent.immutable_ptb_argument(tx)?;
        let agent_registry = tx.shared_object(&objects.agent_registry, true)?;
        tx.call_target(
            scheduler_binding::cancel_with_agent_target,
            vec![task, agent, agent_registry],
        )?;
        Ok(())
    })
}

/// Build a PTB that enqueues an occurrence with an absolute start time.
pub(crate) fn add_occurrence_absolute_for_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    start_time_ms: u64,
    deadline_offset_ms: Option<u64>,
    priority_fee_per_gas_unit: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let start_time_ms = tx.arg(&start_time_ms)?;
        let deadline_offset_ms = tx.arg(&MoveOption::from_option(deadline_offset_ms))?;
        let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::add_occurrence_absolute_for_task_target,
            vec![
                task,
                start_time_ms,
                deadline_offset_ms,
                priority_fee_per_gas_unit,
                leader_registry,
                clock,
            ],
        )?;
        Ok(())
    })
}

/// Build a PTB that enqueues an occurrence with a relative start offset.
pub(crate) fn add_occurrence_relative_for_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    start_offset_ms: u64,
    deadline_offset_ms: Option<u64>,
    priority_fee_per_gas_unit: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let start_offset_ms = tx.arg(&start_offset_ms)?;
        let deadline_offset_ms = tx.arg(&MoveOption::from_option(deadline_offset_ms))?;
        let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::add_occurrence_relative_for_task_target,
            vec![
                task,
                start_offset_ms,
                deadline_offset_ms,
                priority_fee_per_gas_unit,
                leader_registry,
                clock,
            ],
        )?;
        Ok(())
    })
}

/// Build a PTB that enqueues a relative occurrence for an explicit agent task.
pub(crate) fn add_occurrence_relative_for_agent_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
    start_offset_ms: u64,
    deadline_offset_ms: Option<u64>,
    priority_fee_per_gas_unit: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let agent = agent.immutable_ptb_argument(tx)?;
        let start_offset_ms = tx.arg(&start_offset_ms)?;
        let deadline_offset_ms = tx.arg(&MoveOption::from_option(deadline_offset_ms))?;
        let priority_fee_per_gas_unit = tx.arg(&priority_fee_per_gas_unit)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::add_occurrence_relative_for_agent_task_target,
            vec![
                task,
                agent,
                start_offset_ms,
                deadline_offset_ms,
                priority_fee_per_gas_unit,
                leader_registry,
                clock,
            ],
        )?;
        Ok(())
    })
}

/// Build a PTB that configures or updates periodic scheduling for a task.
pub(crate) fn configure_periodic_for_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    schedule: PeriodicScheduleInputs,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        let first_start_ms = tx.arg(&schedule.first_start_ms)?;
        let period_ms = tx.arg(&schedule.period_ms)?;
        let deadline_offset_ms = tx.arg(&MoveOption::from_option(schedule.deadline_offset_ms))?;
        let max_iterations = tx.arg(&MoveOption::from_option(schedule.max_iterations))?;
        let priority_fee_per_gas_unit = tx.arg(&schedule.priority_fee_per_gas_unit)?;
        let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
        let clock = tx.clock()?;
        tx.call_target(
            scheduler_binding::new_or_modify_periodic_for_task_target,
            vec![
                task,
                first_start_ms,
                period_ms,
                deadline_offset_ms,
                max_iterations,
                priority_fee_per_gas_unit,
                leader_registry,
                clock,
            ],
        )?;
        Ok(())
    })
}

/// Build a PTB that disables periodic scheduling for a task.
pub(crate) fn disable_periodic_for_task_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let task = shared_task_arg(tx, task)?;
        tx.call_target(
            scheduler_binding::disable_periodic_for_task_target,
            vec![task],
        )?;
        Ok(())
    })
}

/// Build a PTB that consumes the active scheduled occurrence and starts its DAG execution.
pub fn execute_scheduled_occurrence_for_self_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    generator: OccurrenceGenerator,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        execute_scheduled_occurrence(tx, task, dag, leader_cap, generator, tools_gas)
    })
}

/// PTB template to consume the active scheduled occurrence and start its DAG execution.
fn execute_scheduled_occurrence(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    generator: OccurrenceGenerator,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    let objects = tx.objects().clone();
    let task = shared_task_arg(tx, task)?;
    let leader_registry = tx.shared_object(&objects.leader_registry, false)?;
    let clock = tx.clock()?;

    let proof = match generator {
        OccurrenceGenerator::Queue => tx.call_target(
            scheduler_binding::check_queue_occurrence_target,
            vec![task.clone(), leader_registry.clone(), clock.clone()],
        )?,
        OccurrenceGenerator::Periodic => tx.call_target(
            scheduler_binding::check_periodic_occurrence_target,
            vec![task.clone(), leader_registry.clone(), clock.clone()],
        )?,
    };

    let tool_registry = tx.shared_object(&objects.tool_registry, false)?;
    let agent_registry = tx.shared_object(&objects.agent_registry, false)?;
    let dag = tx.shared_object(dag, false)?;
    let leader_cap = tx.shared_object(leader_cap, false)?;

    let execution = tx.call_target(
        scheduler_binding::prepare_execution_from_scheduled_payment_target,
        vec![
            dag.clone(),
            agent_registry,
            tool_registry,
            task.clone(),
            leader_cap,
            clock.clone(),
        ],
    )?;

    let gas_service = tx.shared_object(&objects.gas_service, false)?;
    tx.call_target(
        gas_binding::snapshot_dag_tool_costs_target,
        vec![gas_service, execution.clone(), dag.clone()],
    )?;

    for (address, version) in tools_gas {
        let tool_gas = tx.shared_object_by_id(*address, *version, true)?;
        tx.call_target(
            gas_binding::lock_payment_state_for_tool_target,
            vec![tool_gas, dag.clone(), execution.clone()],
        )?;
    }

    tx.call_target(
        execution_entries_binding::start_execution_target,
        vec![dag.clone(), execution.clone(), leader_registry, clock],
    )?;
    tx.call_target(
        transfer_binding::public_share_object_target::<execution_binding::DAGExecution>,
        vec![execution],
    )?;
    tx.call_target(scheduler_binding::finish_target, vec![task, proof])?;

    Ok(())
}

/// PTB template to settle a completed scheduled execution payment if one is ready.
pub(crate) fn settle_finished_scheduled_execution_payment_if_ready(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    task: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;
    let execution = shared_mutable_object_arg(tx, execution)?;
    tx.call_target(
        scheduler_binding::settle_finished_scheduled_execution_payment_if_ready_target,
        vec![task, execution],
    )
}

/// PTB template to register default agent DAG execution config on the execution policy.
fn register_begin_default_agent_execution(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    policy: sui::types::Argument,
    config: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        scheduler_binding::register_begin_default_agent_execution_target,
        vec![policy, config],
    )
}

/// PTB template to register registered agent DAG execution config on the execution policy.
fn register_begin_agent_execution(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    policy: sui::types::Argument,
    config: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    tx.call_target(
        scheduler_binding::register_begin_agent_execution_target,
        vec![policy, config],
    )
}
