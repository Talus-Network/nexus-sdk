use {
    crate::{
        idents::{interface, move_std, primitives, scheduler, sui_framework, workflow},
        sui,
        transactions::{self, agent_input::AgentInput},
        types::{AgentId, DataStorage, NexusObjects, SkillId, Storable, StorageKind},
    },
    serde::{Deserialize, Serialize},
    std::collections::{HashMap, HashSet},
};

/// Generator variants supported by the scheduler when executing occurrences.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OccurrenceGenerator {
    Queue,
    Periodic,
}

/// Arguments required to configure periodic scheduling in the PTB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeriodicScheduleInputs {
    pub first_start_ms: u64,
    pub period_ms: u64,
    pub deadline_offset_ms: Option<u64>,
    pub max_iterations: Option<u64>,
    pub priority_fee_per_gas_unit: u64,
}

// Shared helper for turning a scheduled task object ref into a mutable shared argument.
fn shared_task_arg(
    tx: &mut sui::tx::TransactionBuilder,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    shared_mutable_object_arg(tx, task)
}

fn shared_mutable_object_arg(
    tx: &mut sui::tx::TransactionBuilder,
    object: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.object(sui::tx::ObjectInput::shared(
        *object.object_id(),
        object.version(),
        true,
    )))
}

// == Metadata ==

/// PTB template to build task metadata from key/value pairs.
pub fn new_metadata<K, V>(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> anyhow::Result<sui::tx::Argument>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let string_type = move_std::String::type_tag();

    let metadata = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module,
            sui_framework::VecMap::EMPTY.name,
        )
        .with_type_args(vec![string_type.clone(), string_type.clone()]),
        vec![],
    );

    for (key, value) in key_values.into_iter() {
        let key = tx.pure(&key.as_ref().to_string());

        let value = tx.pure(&value.as_ref().to_string());

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
            )
            .with_type_args(vec![string_type.clone(), string_type.clone()]),
            vec![metadata, key, value],
        );
    }

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_METADATA.module,
            scheduler::Scheduler::NEW_METADATA.name,
        ),
        vec![metadata],
    ))
}

// == Task lifecycle ==

/// PTB template to create a funded scheduled task for the registry-owned default agent.
#[allow(clippy::too_many_arguments)]
pub fn new_default_agent_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    metadata: sui::tx::Argument,
    constraints: sui::tx::Argument,
    execution: sui::tx::Argument,
    registry: sui::tx::Argument,
    prepayment_coin: sui::tx::Argument,
    occurrence_budget: u64,
) -> anyhow::Result<sui::tx::Argument> {
    let occurrence_budget = tx.pure(&occurrence_budget);
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_DEFAULT_AGENT_TASK.module,
            scheduler::Scheduler::NEW_DEFAULT_AGENT_TASK.name,
        ),
        vec![
            metadata,
            constraints,
            execution,
            registry,
            prepayment_coin,
            occurrence_budget,
        ],
    ))
}

/// PTB template to update an existing task metadata bag.
pub fn update_metadata(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    metadata: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::UPDATE_METADATA.module,
            scheduler::Scheduler::UPDATE_METADATA.name,
        ),
        vec![task, metadata],
    ))
}

/// PTB template to construct and register the default constraints policy.
pub fn new_constraints_policy(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    generator: OccurrenceGenerator,
) -> anyhow::Result<sui::tx::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let constraint_symbol = match generator {
        OccurrenceGenerator::Queue => {
            let witness_tag = scheduler::into_type_tag(
                objects.scheduler_pkg_id,
                scheduler::Scheduler::QUEUE_GENERATOR_WITNESS,
            );
            tx.move_call(
                sui::tx::Function::new(
                    objects.primitives_pkg_id,
                    primitives::Policy::WITNESS_SYMBOL.module,
                    primitives::Policy::WITNESS_SYMBOL.name,
                )
                .with_type_args(vec![witness_tag]),
                vec![],
            )
        }
        OccurrenceGenerator::Periodic => {
            let witness_tag = scheduler::into_type_tag(
                objects.scheduler_pkg_id,
                scheduler::Scheduler::PERIODIC_GENERATOR_WITNESS,
            );
            tx.move_call(
                sui::tx::Function::new(
                    objects.primitives_pkg_id,
                    primitives::Policy::WITNESS_SYMBOL.module,
                    primitives::Policy::WITNESS_SYMBOL.name,
                )
                .with_type_args(vec![witness_tag]),
                vec![],
            )
        }
    };

    let constraint_sequence = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::EMPTY.module,
            sui_framework::TableVec::EMPTY.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::PUSH_BACK.module,
            sui_framework::TableVec::PUSH_BACK.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![constraint_sequence, constraint_symbol],
    );

    let constraints = tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_CONSTRAINTS_POLICY.module,
            scheduler::Scheduler::NEW_CONSTRAINTS_POLICY.name,
        ),
        vec![constraint_sequence],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::DROP.module,
            sui_framework::TableVec::DROP.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![constraint_sequence],
    );

    match generator {
        OccurrenceGenerator::Queue => {
            let queue_state = new_queue_generator_state(tx, objects)?;
            register_queue_generator(tx, objects, constraints, queue_state)?;
        }
        OccurrenceGenerator::Periodic => {
            let periodic_state = new_periodic_generator_state(tx, objects)?;
            register_periodic_generator(tx, objects, constraints, periodic_state)?;
        }
    };

    Ok(constraints)
}

/// PTB template to construct a queue generator state.
pub fn new_queue_generator_state(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_QUEUE_GENERATOR_STATE.module,
            scheduler::Scheduler::NEW_QUEUE_GENERATOR_STATE.name,
        ),
        vec![],
    ))
}

/// PTB template to register the queue generator state.
pub fn register_queue_generator(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    constraints: sui::tx::Argument,
    queue_state: sui::tx::Argument,
) -> anyhow::Result<()> {
    tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::REGISTER_QUEUE_GENERATOR.module,
            scheduler::Scheduler::REGISTER_QUEUE_GENERATOR.name,
        ),
        vec![constraints, queue_state],
    );

    Ok(())
}

/// PTB template to construct a periodic generator state.
pub fn new_periodic_generator_state(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_PERIODIC_GENERATOR_STATE.module,
            scheduler::Scheduler::NEW_PERIODIC_GENERATOR_STATE.name,
        ),
        vec![],
    ))
}

/// PTB template to register the periodic generator state.
pub fn register_periodic_generator(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    constraints: sui::tx::Argument,
    periodic_state: sui::tx::Argument,
) -> anyhow::Result<()> {
    tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::REGISTER_PERIODIC_GENERATOR.module,
            scheduler::Scheduler::REGISTER_PERIODIC_GENERATOR.name,
        ),
        vec![constraints, periodic_state],
    );

    Ok(())
}

/// PTB template to construct and register the default execution policy.
#[allow(clippy::too_many_arguments)]
pub fn new_execution_policy(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag_id: sui::types::Address,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
) -> anyhow::Result<sui::tx::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let witness_tag = workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::ExecutionEntries::ADVANCE_FOR_DEFAULT_AGENT_EXECUTION_TYPE,
    );

    let execution_symbol = tx.move_call(
        sui::tx::Function::new(
            objects.primitives_pkg_id,
            primitives::Policy::WITNESS_SYMBOL.module,
            primitives::Policy::WITNESS_SYMBOL.name,
        )
        .with_type_args(vec![witness_tag]),
        vec![],
    );

    let execution_sequence = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::EMPTY.module,
            sui_framework::TableVec::EMPTY.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::PUSH_BACK.module,
            sui_framework::TableVec::PUSH_BACK.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![execution_sequence, execution_symbol],
    );

    let execution = tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_EXECUTION_POLICY.module,
            scheduler::Scheduler::NEW_EXECUTION_POLICY.name,
        ),
        vec![execution_sequence],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::DROP.module,
            sui_framework::TableVec::DROP.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![execution_sequence],
    );

    let dag_id_arg = sui_framework::Object::id_from_object_id(tx, dag_id)?;
    let network_id_arg = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;
    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);

    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;

    let with_vertex_inputs = build_inputs_vec_map(tx, objects, input_data)?;

    let config = transactions::tap::default_agent_execution_config_arg(
        tx,
        objects,
        dag_id_arg,
        network_id_arg,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
    )?;

    register_begin_default_agent_execution(tx, objects, execution, config)?;

    Ok(execution)
}

/// PTB template to construct and register a registered TAP agent execution policy.
#[allow(clippy::too_many_arguments)]
pub fn new_agent_execution_policy(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    dag_id: sui::types::Address,
    priority_fee_per_gas_unit: u64,
    entry_group: &str,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<sui::tx::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let witness_tag = workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::ExecutionEntries::ADVANCE_FOR_AGENT_EXECUTION_TYPE,
    );

    let execution_symbol = tx.move_call(
        sui::tx::Function::new(
            objects.primitives_pkg_id,
            primitives::Policy::WITNESS_SYMBOL.module,
            primitives::Policy::WITNESS_SYMBOL.name,
        )
        .with_type_args(vec![witness_tag]),
        vec![],
    );

    let execution_sequence = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::EMPTY.module,
            sui_framework::TableVec::EMPTY.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::PUSH_BACK.module,
            sui_framework::TableVec::PUSH_BACK.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![execution_sequence, execution_symbol],
    );

    let execution = tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_EXECUTION_POLICY.module,
            scheduler::Scheduler::NEW_EXECUTION_POLICY.name,
        ),
        vec![execution_sequence],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::DROP.module,
            sui_framework::TableVec::DROP.name,
        )
        .with_type_args(vec![symbol_type.clone()]),
        vec![execution_sequence],
    );

    let agent_id_arg = sui_framework::Object::id_from_object_id(tx, agent_id)?;
    let network_id_arg = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;
    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);
    let entry_group =
        interface::Graph::entry_group_from_str(tx, objects.interface_pkg_id, entry_group)?;

    let with_vertex_inputs = build_inputs_vec_map(tx, objects, input_data)?;

    let config = transactions::tap::agent_execution_config_arg(
        tx,
        objects,
        agent_id_arg,
        network_id_arg,
        entry_group,
        with_vertex_inputs,
        priority_fee_per_gas_unit,
        skill_id,
        Some(dag_id),
        &[],
    )?;

    register_begin_agent_execution(tx, objects, execution, config)?;

    Ok(execution)
}

pub(crate) fn build_inputs_vec_map(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
) -> anyhow::Result<sui::tx::Argument> {
    let inner_vec_map_type = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        interface::into_type_tag(objects.interface_pkg_id, interface::Graph::VERTEX),
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::VEC_MAP.module,
            sui_framework::VecMap::VEC_MAP.name,
            inner_vec_map_type.clone(),
        ))),
    ];

    let with_vertex_inputs = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module,
            sui_framework::VecMap::EMPTY.name,
        )
        .with_type_args(outer_vec_map_type.clone()),
        vec![],
    );

    for (vertex_name, data) in input_data {
        // `vertex: Vertex`
        let vertex = interface::Graph::vertex_from_str(tx, objects.interface_pkg_id, vertex_name)?;

        // `with_vertex_input: VecMap<InputPort, NexusData>`
        let with_vertex_input = tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::EMPTY.module,
                sui_framework::VecMap::EMPTY.name,
            )
            .with_type_args(inner_vec_map_type.clone()),
            vec![],
        );

        for (port_name, value) in data {
            // `port: InputPort`
            let port = interface::Graph::input_port_from_str(
                tx,
                objects.interface_pkg_id,
                port_name.as_str(),
            )?;

            // `value: NexusData`
            let value = match value.storage_kind() {
                StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                )?,
                StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                )?,
            };

            // `with_vertex_input.insert(port, value)`
            tx.move_call(
                sui::tx::Function::new(
                    sui_framework::PACKAGE_ID,
                    sui_framework::VecMap::INSERT.module,
                    sui_framework::VecMap::INSERT.name,
                )
                .with_type_args(inner_vec_map_type.clone()),
                vec![with_vertex_input, port, value],
            );
        }

        // `with_vertex_inputs.insert(vertex, with_vertex_input)`
        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
            )
            .with_type_args(outer_vec_map_type.clone()),
            vec![with_vertex_inputs, vertex, with_vertex_input],
        );
    }

    Ok(with_vertex_inputs)
}

/// PTB template to obtain the execution witness for a task.
pub fn execute(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::EXECUTE.module,
            scheduler::Scheduler::EXECUTE.name,
        ),
        vec![task],
    ))
}

/// PTB template to finalize a task execution.
pub fn finish(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    proof: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::FINISH.module,
            scheduler::Scheduler::FINISH.name,
        ),
        vec![task, proof],
    ))
}

/// PTB template to settle an address-funded scheduled execution payment after completion.
pub fn settle_finished_scheduled_tap_execution_payment_if_ready(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let execution = shared_mutable_object_arg(tx, execution)?;

    settle_finished_scheduled_tap_execution_payment_if_ready_with_args(tx, objects, task, execution)
}

fn settle_finished_scheduled_tap_execution_payment_if_ready_with_args(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: sui::tx::Argument,
    execution: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::SETTLE_FINISHED_SCHEDULED_EXECUTION_PAYMENT_IF_READY.module,
            scheduler::Scheduler::SETTLE_FINISHED_SCHEDULED_EXECUTION_PAYMENT_IF_READY.name,
        ),
        vec![task, execution],
    ))
}

/// PTB template to collect idle agent-funded scheduled task reserve funds back to the agent vault.
pub fn collect_idle_agent_funded_scheduled_payment_reserve_to_vault(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent = agent.mutable_argument(tx)?;
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        true,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::COLLECT_IDLE_AGENT_FUNDED_SCHEDULED_PAYMENT_RESERVE_TO_VAULT
                .module,
            scheduler::Scheduler::COLLECT_IDLE_AGENT_FUNDED_SCHEDULED_PAYMENT_RESERVE_TO_VAULT.name,
        ),
        vec![task, agent, agent_registry],
    ))
}

// == Occurrence scheduling ==

/// PTB template to enqueue a new occurrence with absolute start time.
pub fn add_occurrence_absolute_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    start_time_ms: u64,
    deadline_offset_ms: Option<u64>,
    priority_fee_per_gas_unit: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_time_ms: u64`
    let start_time_ms = tx.pure(&start_time_ms);

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.pure(&deadline_offset_ms);

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);

    // `leader_registry: &LeaderRegistry`
    let leader_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    // `clock: &Clock`
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::ADD_OCCURRENCE_ABSOLUTE_FOR_TASK.module,
            scheduler::Scheduler::ADD_OCCURRENCE_ABSOLUTE_FOR_TASK.name,
        ),
        vec![
            task,
            start_time_ms,
            deadline_offset_ms,
            priority_fee_per_gas_unit,
            leader_registry,
            clock,
        ],
    ))
}

/// PTB template to enqueue a new occurrence relative to the current clock time.
pub fn add_occurrence_relative_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    start_offset_ms: u64,
    deadline_offset_ms: Option<u64>,
    priority_fee_per_gas_unit: u64,
) -> anyhow::Result<sui::tx::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_offset_ms: u64`
    let start_offset_ms = tx.pure(&start_offset_ms);

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.pure(&deadline_offset_ms);

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);

    // `leader_registry: &LeaderRegistry`
    let leader_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    // `clock: &Clock`
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::ADD_OCCURRENCE_RELATIVE_FOR_TASK.module,
            scheduler::Scheduler::ADD_OCCURRENCE_RELATIVE_FOR_TASK.name,
        ),
        vec![
            task,
            start_offset_ms,
            deadline_offset_ms,
            priority_fee_per_gas_unit,
            leader_registry,
            clock,
        ],
    ))
}

// == Periodic scheduling ==

/// PTB template to configure or update periodic scheduling.
#[allow(clippy::too_many_arguments)]
pub fn new_or_modify_periodic_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    schedule: PeriodicScheduleInputs,
) -> anyhow::Result<sui::tx::Argument> {
    let PeriodicScheduleInputs {
        first_start_ms,
        period_ms,
        deadline_offset_ms,
        max_iterations,
        priority_fee_per_gas_unit,
    } = schedule;

    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `first_start_ms: u64`
    let first_start_ms = tx.pure(&first_start_ms);

    // `period_ms: u64`
    let period_ms = tx.pure(&period_ms);

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.pure(&deadline_offset_ms);

    // `max_iterations: option::Option<u64>`
    let max_iterations = tx.pure(&max_iterations);

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.pure(&priority_fee_per_gas_unit);

    // `leader_registry: &LeaderRegistry`
    let leader_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    // `clock: &Clock`
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::NEW_OR_MODIFY_PERIODIC_FOR_TASK.module,
            scheduler::Scheduler::NEW_OR_MODIFY_PERIODIC_FOR_TASK.name,
        ),
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
    ))
}

/// PTB template to disable periodic scheduling for a task.
pub fn disable_periodic_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::DISABLE_PERIODIC_FOR_TASK.module,
            scheduler::Scheduler::DISABLE_PERIODIC_FOR_TASK.name,
        ),
        vec![task],
    ))
}

// == Constraint state management ==

/// PTB template to pause scheduling for a task.
pub fn pause_time_constraint_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::PAUSE.module,
            scheduler::Scheduler::PAUSE.name,
        ),
        vec![task, agent_registry],
    ))
}

/// PTB template to pause scheduling for an explicit-agent task.
pub fn pause_time_constraint_for_agent_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent = agent.immutable_argument(tx)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::PAUSE_WITH_AGENT.module,
            scheduler::Scheduler::PAUSE_WITH_AGENT.name,
        ),
        vec![task, agent],
    ))
}

/// PTB template to resume scheduling for a task.
pub fn resume_time_constraint_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::RESUME.module,
            scheduler::Scheduler::RESUME.name,
        ),
        vec![task, agent_registry],
    ))
}

/// PTB template to resume scheduling for an explicit-agent task.
pub fn resume_time_constraint_for_agent_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent = agent.immutable_argument(tx)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::RESUME_WITH_AGENT.module,
            scheduler::Scheduler::RESUME_WITH_AGENT.name,
        ),
        vec![task, agent],
    ))
}

/// PTB template to cancel scheduling for a task.
pub fn cancel_time_constraint_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        true,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::CANCEL.module,
            scheduler::Scheduler::CANCEL.name,
        ),
        vec![task, agent_registry],
    ))
}

/// PTB template to cancel scheduling for an explicit-agent task.
pub fn cancel_time_constraint_for_agent_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    agent: AgentInput,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let agent = agent.immutable_argument(tx)?;
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        true,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::CANCEL_WITH_AGENT.module,
            scheduler::Scheduler::CANCEL_WITH_AGENT.name,
        ),
        vec![task, agent, agent_registry],
    ))
}

// == Execution flow ==

/// PTB template to consume the next queued occurrence.
pub fn check_queue_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module,
            scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name,
        ),
        vec![task, clock],
    ))
}

/// PTB template to consume the next periodic occurrence.
pub fn check_periodic_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::tx::Argument> {
    let task = shared_task_arg(tx, task)?;
    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.module,
            scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.name,
        ),
        vec![task, clock],
    ))
}

/// PTB template to register default agent DAG execution config on the execution policy.
pub fn register_begin_default_agent_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    policy: sui::tx::Argument,
    config: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::REGISTER_BEGIN_DEFAULT_AGENT_EXECUTION.module,
            scheduler::Scheduler::REGISTER_BEGIN_DEFAULT_AGENT_EXECUTION.name,
        ),
        vec![policy, config],
    ))
}

/// PTB template to register registered agent DAG execution config on the execution policy.
pub fn register_begin_agent_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    policy: sui::tx::Argument,
    config: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::REGISTER_BEGIN_AGENT_EXECUTION.module,
            scheduler::Scheduler::REGISTER_BEGIN_AGENT_EXECUTION.name,
        ),
        vec![policy, config],
    ))
}

/// PTB template to invoke DAG execution from a durable TAP scheduled payment.
#[allow(clippy::too_many_arguments)]
pub fn prepare_execution_from_scheduled_payment(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    tool_registry: sui::tx::Argument,
    agent_registry: sui::tx::Argument,
    task: sui::tx::Argument,
    dag: sui::tx::Argument,
    leader_cap: sui::tx::Argument,
    clock: sui::tx::Argument,
) -> anyhow::Result<sui::tx::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.module,
            scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.name,
        ),
        vec![dag, agent_registry, tool_registry, task, leader_cap, clock],
    ))
}

/// PTB helper that consumes the next scheduled occurrence and invokes the TAP.
#[allow(clippy::too_many_arguments)]
pub fn execute_scheduled_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    _amount_priority: u64,
    generator: OccurrenceGenerator,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    // Create shared inputs once so subsequent commands reuse the same arguments.
    let task = shared_task_arg(tx, task)?;

    // `leader_registry: &LeaderRegistry`
    let leader_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // Consume the occurrence and obtain the proof-of-UID hot potato.
    let proof = match generator {
        OccurrenceGenerator::Queue => tx.move_call(
            sui::tx::Function::new(
                objects.scheduler_pkg_id,
                scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module,
                scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name,
            ),
            vec![task, leader_registry, clock],
        ),
        OccurrenceGenerator::Periodic => tx.move_call(
            sui::tx::Function::new(
                objects.scheduler_pkg_id,
                scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.module,
                scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.name,
            ),
            vec![task, leader_registry, clock],
        ),
    };

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `agent_registry: &AgentRegistry`
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));

    // `dag: &DAG`
    let dag = tx.object(sui::tx::ObjectInput::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));
    let leader_cap = tx.object(sui::tx::ObjectInput::shared(
        *leader_cap.object_id(),
        leader_cap.version(),
        false,
    ));

    let results = prepare_execution_from_scheduled_payment(
        tx,
        objects,
        tool_registry,
        agent_registry,
        task,
        dag,
        leader_cap,
        clock,
    )?;

    let execution = results;

    let gas_service = tx.object(sui::tx::ObjectInput::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        false,
    ));
    transactions::gas::snapshot_dag_tool_costs(tx, objects, gas_service, execution, dag);

    // `tools_gas: Vec<&mut ToolGas>`
    let tools_gas = tools_gas
        .iter()
        .map(|(address, version)| tx.object(sui::tx::ObjectInput::shared(*address, *version, true)))
        .collect();

    transactions::dag::lock_payment_state_for_tools(tx, objects, tools_gas, dag, execution);

    // `nexus_workflow::execution_entries::start_execution()`
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionEntries::START_EXECUTION.module,
            workflow::ExecutionEntries::START_EXECUTION.name,
        ),
        vec![dag, execution, leader_registry, clock],
    );

    // `DAGExecution`
    let execution_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Execution::DAG_EXECUTION);

    // `sui::transfer::public_share_object<DAGExecution>`
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
        )
        .with_type_args(vec![execution_type]),
        vec![execution],
    );

    // Consume the proof to satisfy Move's non-drop requirement and reset task policies.
    tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::FINISH.module,
            scheduler::Scheduler::FINISH.name,
        ),
        vec![task, proof],
    );

    Ok(())
}

/// PTB helper that consumes the next scheduled occurrence for a registered TAP skill.
#[allow(clippy::too_many_arguments)]
pub fn execute_registered_scheduled_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    _amount_priority: u64,
    generator: OccurrenceGenerator,
    tools_gas: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> anyhow::Result<()> {
    // Create shared inputs once so subsequent commands reuse the same arguments.
    let task = shared_task_arg(tx, task)?;

    // `leader_registry: &LeaderRegistry`
    let leader_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.leader_registry.object_id(),
        objects.leader_registry.version(),
        false,
    ));

    let clock = tx.object(sui::tx::ObjectInput::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // Consume the occurrence and obtain the proof-of-UID hot potato.
    let proof = match generator {
        OccurrenceGenerator::Queue => tx.move_call(
            sui::tx::Function::new(
                objects.scheduler_pkg_id,
                scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module,
                scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name,
            ),
            vec![task, leader_registry, clock],
        ),
        OccurrenceGenerator::Periodic => tx.move_call(
            sui::tx::Function::new(
                objects.scheduler_pkg_id,
                scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.module,
                scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.name,
            ),
            vec![task, leader_registry, clock],
        ),
    };

    // `tool_registry: &ToolRegistry`
    let tool_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.tool_registry.object_id(),
        objects.tool_registry.version(),
        false,
    ));

    // `agent_registry: &AgentRegistry`
    let agent_registry = tx.object(sui::tx::ObjectInput::shared(
        *objects.agent_registry.object_id(),
        objects.agent_registry.version(),
        false,
    ));

    // `dag: &DAG`
    let dag = tx.object(sui::tx::ObjectInput::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));
    let leader_cap = tx.object(sui::tx::ObjectInput::shared(
        *leader_cap.object_id(),
        leader_cap.version(),
        false,
    ));

    let results = prepare_execution_from_scheduled_payment(
        tx,
        objects,
        tool_registry,
        agent_registry,
        task,
        dag,
        leader_cap,
        clock,
    )?;

    let execution = results;

    let gas_service = tx.object(sui::tx::ObjectInput::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        false,
    ));
    transactions::gas::snapshot_dag_tool_costs(tx, objects, gas_service, execution, dag);

    // `tools_gas: Vec<&mut ToolGas>`
    let tools_gas = tools_gas
        .iter()
        .map(|(address, version)| tx.object(sui::tx::ObjectInput::shared(*address, *version, true)))
        .collect();

    transactions::dag::lock_payment_state_for_tools(tx, objects, tools_gas, dag, execution);

    // `nexus_workflow::execution_entries::start_execution()`
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::ExecutionEntries::START_EXECUTION.module,
            workflow::ExecutionEntries::START_EXECUTION.name,
        ),
        vec![dag, execution, leader_registry, clock],
    );

    // `DAGExecution`
    let execution_type =
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Execution::DAG_EXECUTION);

    // `sui::transfer::public_share_object<DAGExecution>`
    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
            sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
        )
        .with_type_args(vec![execution_type]),
        vec![execution],
    );

    // Consume the proof to satisfy Move's non-drop requirement and reset task policies.
    tx.move_call(
        sui::tx::Function::new(
            objects.scheduler_pkg_id,
            scheduler::Scheduler::FINISH.module,
            scheduler::Scheduler::FINISH.name,
        ),
        vec![task, proof],
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{sui, test_utils::sui_mocks},
        assert_matches::assert_matches,
    };

    struct TxInspector {
        tx: sui::types::Transaction,
    }

    impl TxInspector {
        fn new(tx: sui::types::Transaction) -> Self {
            Self { tx }
        }

        fn commands(&self) -> &Vec<sui::types::Command> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { commands, .. },
            ) = &self.tx.kind
            else {
                panic!("expected PTB transaction kind, got {:?}", self.tx.kind);
            };

            commands
        }

        fn inputs(&self) -> &Vec<sui::types::Input> {
            let sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction { inputs, .. },
            ) = &self.tx.kind
            else {
                panic!("expected PTB transaction kind, got {:?}", self.tx.kind);
            };

            inputs
        }

        fn move_call(&self, index: usize) -> &sui::types::MoveCall {
            match self.commands().get(index) {
                Some(sui::types::Command::MoveCall(call)) => call,
                Some(other) => panic!("expected MoveCall at index {index}, got {other:?}"),
                None => panic!("missing command at index {index}"),
            }
        }

        fn input(&self, argument: &sui::types::Argument) -> &sui::types::Input {
            let sui::types::Argument::Input(index) = argument else {
                panic!("expected Argument::Input, got {argument:?}");
            };

            self.inputs()
                .get(*index as usize)
                .unwrap_or_else(|| panic!("missing input for index {index}"))
        }

        fn expect_shared_object(
            &self,
            argument: &sui::types::Argument,
            expected: &sui::types::ObjectReference,
            mutable: bool,
        ) {
            let sui::types::Input::Shared(shared) = self.input(argument) else {
                panic!(
                    "expected shared object argument, got {:?}",
                    self.input(argument)
                );
            };

            assert_eq!(shared.object_id(), *expected.object_id());
            assert_eq!(shared.version(), expected.version());
            assert_eq!(shared.mutability().is_mutable(), mutable);
        }

        fn expect_clock(&self, argument: &sui::types::Argument) {
            let sui::types::Input::Shared(shared) = self.input(argument) else {
                panic!(
                    "expected clock shared object argument, got {:?}",
                    self.input(argument)
                );
            };

            assert_eq!(shared.object_id(), sui_framework::CLOCK_OBJECT_ID);
            assert_eq!(shared.version(), 1);
            assert!(
                !shared.mutability().is_mutable(),
                "clock object must be immutable"
            );
        }

        fn expect_pure_bytes(&self, argument: &sui::types::Argument, expected: &[u8]) {
            let sui::types::Input::Pure(value) = self.input(argument) else {
                panic!("expected pure argument, got {:?}", self.input(argument));
            };

            assert_eq!(value.as_slice(), expected);
        }

        fn expect_u64(&self, argument: &sui::types::Argument, value: u64) {
            self.expect_pure_bytes(argument, &value.to_le_bytes());
        }

        fn expect_option_u64(&self, argument: &sui::types::Argument, value: Option<u64>) {
            match value {
                Some(inner) => {
                    let mut bytes = vec![1];
                    bytes.extend_from_slice(&inner.to_le_bytes());
                    self.expect_pure_bytes(argument, &bytes);
                }
                None => self.expect_pure_bytes(argument, &[0]),
            }
        }
    }

    #[test]
    fn new_metadata_builds_vecmap_and_scheduler_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        new_metadata(&mut tx, &objects, [("foo", "bar")]).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 3);

        let empty_call = inspector.move_call(0);
        assert_eq!(empty_call.package, sui_framework::PACKAGE_ID);
        assert_eq!(empty_call.module, sui_framework::VecMap::EMPTY.module);
        assert_eq!(empty_call.function, sui_framework::VecMap::EMPTY.name);
        assert_eq!(empty_call.type_arguments.len(), 2);
        assert!(empty_call.arguments.is_empty());

        let insert_call = inspector.move_call(1);
        assert_eq!(insert_call.package, sui_framework::PACKAGE_ID);
        assert_eq!(insert_call.module, sui_framework::VecMap::INSERT.module);
        assert_eq!(insert_call.function, sui_framework::VecMap::INSERT.name);
        assert_eq!(insert_call.type_arguments.len(), 2);
        assert_eq!(insert_call.arguments.len(), 3);

        let final_call = inspector.move_call(2);
        assert_eq!(final_call.package, objects.scheduler_pkg_id);
        assert_eq!(final_call.module, scheduler::Scheduler::NEW_METADATA.module);
        assert_eq!(final_call.function, scheduler::Scheduler::NEW_METADATA.name);
        assert!(final_call.type_arguments.is_empty());
        assert_eq!(final_call.arguments.len(), 1);
        assert_matches!(final_call.arguments[0], sui::types::Argument::Result(0));
    }

    #[test]
    fn new_metadata_handles_empty_iterators() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        new_metadata(
            &mut tx,
            &objects,
            std::iter::empty::<(&'static str, &'static str)>(),
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 2);
    }

    #[test]
    fn new_default_agent_task_builds_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata = tx.pure(&1_u64);
        let constraints = tx.pure(&2_u64);
        let execution = tx.pure(&3_u64);
        let registry = tx.pure(&4_u64);
        let coin = tx.pure(&5_u64);

        let _result = new_default_agent_task(
            &mut tx,
            &objects,
            metadata,
            constraints,
            execution,
            registry,
            coin,
            25,
        )
        .expect("ptb construction succeeds");

        // Opaque builder argument; finished transaction assertions below prove the call shape.

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::NEW_DEFAULT_AGENT_TASK.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::NEW_DEFAULT_AGENT_TASK.name
        );
        assert_eq!(call.arguments.len(), 6);
        inspector.expect_u64(&call.arguments[0], 1);
        inspector.expect_u64(&call.arguments[1], 2);
        inspector.expect_u64(&call.arguments[2], 3);
        inspector.expect_u64(&call.arguments[4], 5);
        inspector.expect_u64(&call.arguments[5], 25);
    }

    #[test]
    fn update_metadata_uses_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata = tx.pure(&9_u64);

        update_metadata(&mut tx, &objects, &task, metadata).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.module, scheduler::Scheduler::UPDATE_METADATA.module);
        assert_eq!(call.function, scheduler::Scheduler::UPDATE_METADATA.name);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], 9);
    }

    #[test]
    fn new_queue_generator_state_has_no_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        new_queue_generator_state(&mut tx, &objects).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert!(call.arguments.is_empty());
        assert_eq!(
            call.module,
            scheduler::Scheduler::NEW_QUEUE_GENERATOR_STATE.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::NEW_QUEUE_GENERATOR_STATE.name
        );
    }

    #[test]
    fn register_queue_generator_invokes_scheduler() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let constraints = tx.pure(&11_u64);
        let queue_state = tx.pure(&12_u64);

        register_queue_generator(&mut tx, &objects, constraints, queue_state)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::REGISTER_QUEUE_GENERATOR.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::REGISTER_QUEUE_GENERATOR.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[0], 11);
        inspector.expect_u64(&call.arguments[1], 12);
    }

    #[test]
    fn new_periodic_generator_state_has_no_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        new_periodic_generator_state(&mut tx, &objects).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert!(call.arguments.is_empty());
        assert_eq!(
            call.module,
            scheduler::Scheduler::NEW_PERIODIC_GENERATOR_STATE.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::NEW_PERIODIC_GENERATOR_STATE.name
        );
    }

    #[test]
    fn register_periodic_generator_invokes_scheduler() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let constraints = tx.pure(&21_u64);
        let periodic_state = tx.pure(&22_u64);

        register_periodic_generator(&mut tx, &objects, constraints, periodic_state)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::REGISTER_PERIODIC_GENERATOR.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::REGISTER_PERIODIC_GENERATOR.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[0], 21);
        inspector.expect_u64(&call.arguments[1], 22);
    }

    #[test]
    fn execute_fetches_execution_witness() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        execute(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        assert_eq!(call.module, scheduler::Scheduler::EXECUTE.module);
        assert_eq!(call.function, scheduler::Scheduler::EXECUTE.name);
    }

    #[test]
    fn finish_finalizes_execution_with_proof() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let proof = tx.pure(&5_u64);
        finish(&mut tx, &objects, &task, proof).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.module, scheduler::Scheduler::FINISH.module);
        assert_eq!(call.function, scheduler::Scheduler::FINISH.name);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], 5);
    }

    #[test]
    fn settle_finished_scheduled_tap_execution_payment_uses_scheduler_task_and_execution() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let execution = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        settle_finished_scheduled_tap_execution_payment_if_ready(
            &mut tx, &objects, &task, &execution,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::SETTLE_FINISHED_SCHEDULED_EXECUTION_PAYMENT_IF_READY.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::SETTLE_FINISHED_SCHEDULED_EXECUTION_PAYMENT_IF_READY.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &execution, true);
    }

    #[test]
    fn collect_idle_agent_funded_scheduled_payment_reserve_uses_task_and_agent() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        collect_idle_agent_funded_scheduled_payment_reserve_to_vault(
            &mut tx,
            &objects,
            &task,
            AgentInput::Shared(agent.clone()),
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::COLLECT_IDLE_AGENT_FUNDED_SCHEDULED_PAYMENT_RESERVE_TO_VAULT
                .module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::COLLECT_IDLE_AGENT_FUNDED_SCHEDULED_PAYMENT_RESERVE_TO_VAULT.name
        );
        assert_eq!(call.arguments.len(), 3);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &agent, true);
        inspector.expect_shared_object(&call.arguments[2], &objects.agent_registry, true);
    }

    #[test]
    fn add_occurrence_absolute_for_task_encodes_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        let start_time = 10;
        let deadline = Some(20);
        let priority_fee_per_gas_unit = 30;

        add_occurrence_absolute_for_task(
            &mut tx,
            &objects,
            &task,
            start_time,
            deadline,
            priority_fee_per_gas_unit,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 6);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_time);
        inspector.expect_option_u64(&call.arguments[2], deadline);
        inspector.expect_u64(&call.arguments[3], priority_fee_per_gas_unit);
        inspector.expect_shared_object(&call.arguments[4], &objects.leader_registry, false);
        inspector.expect_clock(&call.arguments[5]);
    }

    #[test]
    fn add_occurrence_relative_for_task_encodes_offsets() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        let start_offset = 5;
        let deadline_offset = Some(15);
        let priority_fee_per_gas_unit = 25;

        add_occurrence_relative_for_task(
            &mut tx,
            &objects,
            &task,
            start_offset,
            deadline_offset,
            priority_fee_per_gas_unit,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 6);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_offset);
        inspector.expect_option_u64(&call.arguments[2], deadline_offset);
        inspector.expect_u64(&call.arguments[3], priority_fee_per_gas_unit);
        inspector.expect_shared_object(&call.arguments[4], &objects.leader_registry, false);
        inspector.expect_clock(&call.arguments[5]);
    }

    #[test]
    fn new_or_modify_periodic_for_task_sets_all_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        let first_start = 10_000;
        let period = 1_000;
        let deadline_offset = Some(500);
        let max_iterations = Some(3);
        let priority_fee_per_gas_unit = 75;

        new_or_modify_periodic_for_task(
            &mut tx,
            &objects,
            &task,
            PeriodicScheduleInputs {
                first_start_ms: first_start,
                period_ms: period,
                deadline_offset_ms: deadline_offset,
                max_iterations,
                priority_fee_per_gas_unit,
            },
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 8);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], first_start);
        inspector.expect_u64(&call.arguments[2], period);
        inspector.expect_option_u64(&call.arguments[3], deadline_offset);
        inspector.expect_option_u64(&call.arguments[4], max_iterations);
        inspector.expect_u64(&call.arguments[5], priority_fee_per_gas_unit);
        inspector.expect_shared_object(&call.arguments[6], &objects.leader_registry, false);
        inspector.expect_clock(&call.arguments[7]);
    }

    #[test]
    fn disable_periodic_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        disable_periodic_for_task(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
    }

    #[test]
    fn pause_time_constraint_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        pause_time_constraint_for_task(&mut tx, &objects, &task)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &objects.agent_registry, false);
    }

    #[test]
    fn resume_time_constraint_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        resume_time_constraint_for_task(&mut tx, &objects, &task)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &objects.agent_registry, false);
    }

    #[test]
    fn cancel_time_constraint_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        cancel_time_constraint_for_task(&mut tx, &objects, &task)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &objects.agent_registry, true);
    }

    #[test]
    fn pause_time_constraint_for_agent_task_borrows_agent() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        pause_time_constraint_for_agent_task(
            &mut tx,
            &objects,
            &task,
            AgentInput::Shared(agent.clone()),
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.function, scheduler::Scheduler::PAUSE_WITH_AGENT.name);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &agent, false);
    }

    #[test]
    fn resume_time_constraint_for_agent_task_borrows_agent() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        resume_time_constraint_for_agent_task(
            &mut tx,
            &objects,
            &task,
            AgentInput::Shared(agent.clone()),
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.function, scheduler::Scheduler::RESUME_WITH_AGENT.name);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &agent, false);
    }

    #[test]
    fn cancel_time_constraint_for_agent_task_borrows_agent_and_registry() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let agent = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        cancel_time_constraint_for_agent_task(
            &mut tx,
            &objects,
            &task,
            AgentInput::Shared(agent.clone()),
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(call.function, scheduler::Scheduler::CANCEL_WITH_AGENT.name);
        assert_eq!(call.arguments.len(), 3);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_shared_object(&call.arguments[1], &agent, false);
        inspector.expect_shared_object(&call.arguments[2], &objects.agent_registry, true);
    }

    #[test]
    fn check_queue_occurrence_uses_clock_and_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        check_queue_occurrence(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.function,
            scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_clock(&call.arguments[1]);
    }

    #[test]
    fn check_periodic_occurrence_uses_clock_and_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        check_periodic_occurrence(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.function,
            scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_clock(&call.arguments[1]);
    }

    #[test]
    fn register_begin_default_agent_execution_routes_through_dag() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let policy = tx.pure(&13_u64);
        let config = tx.pure(&14_u64);
        register_begin_default_agent_execution(&mut tx, &objects, policy, config)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::REGISTER_BEGIN_DEFAULT_AGENT_EXECUTION.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::REGISTER_BEGIN_DEFAULT_AGENT_EXECUTION.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[0], 13);
        inspector.expect_u64(&call.arguments[1], 14);
    }

    #[test]
    fn register_begin_agent_execution_routes_through_dag() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let policy = tx.pure(&23_u64);
        let config = tx.pure(&24_u64);
        register_begin_agent_execution(&mut tx, &objects, policy, config)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.scheduler_pkg_id);
        assert_eq!(
            call.module,
            scheduler::Scheduler::REGISTER_BEGIN_AGENT_EXECUTION.module
        );
        assert_eq!(
            call.function,
            scheduler::Scheduler::REGISTER_BEGIN_AGENT_EXECUTION.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[0], 23);
        inspector.expect_u64(&call.arguments[1], 24);
    }

    #[test]
    fn execute_scheduled_occurrence_chains_scheduler_and_tap_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);
        let mut tx = sui::tx::TransactionBuilder::new();

        execute_scheduled_occurrence(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            0,
            OccurrenceGenerator::Queue,
            &tools_gas,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 7);

        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        let scheduler_call = calls
            .iter()
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module == scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module
                    && call.function == scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name
            })
            .expect("queue occurrence check call");
        assert_eq!(
            scheduler_call.module,
            scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module
        );
        assert_eq!(
            scheduler_call.function,
            scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name
        );
        assert_eq!(scheduler_call.arguments.len(), 3);
        inspector.expect_shared_object(&scheduler_call.arguments[0], &task, true);
        inspector.expect_shared_object(
            &scheduler_call.arguments[1],
            &objects.leader_registry,
            false,
        );
        inspector.expect_clock(&scheduler_call.arguments[2]);

        let tap_call = calls
            .iter()
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module
                        == scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.module
                    && call.function
                        == scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.name
            })
            .expect("scheduler default-agent preparation call");
        assert_eq!(
            tap_call.module,
            scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.module
        );
        assert_eq!(
            tap_call.function,
            scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.name
        );
        assert_eq!(tap_call.arguments.len(), 6);
        let sui::types::Input::Shared(shared) = inspector.input(&tap_call.arguments[0]) else {
            panic!(
                "expected shared DAG object, got {:?}",
                inspector.input(&tap_call.arguments[0])
            );
        };
        assert_eq!(shared.object_id(), *dag.object_id());
        assert_eq!(shared.version(), dag.version());
        assert!(!shared.mutability().is_mutable());
        inspector.expect_shared_object(&tap_call.arguments[1], &objects.agent_registry, false);
        inspector.expect_shared_object(&tap_call.arguments[2], &objects.tool_registry, false);
        inspector.expect_shared_object(&tap_call.arguments[3], &task, true);
        inspector.expect_shared_object(&tap_call.arguments[4], &leader_cap, false);
        inspector.expect_clock(&tap_call.arguments[5]);

        let lock_call = calls
            .iter()
            .find(|call| {
                call.package == objects.workflow_pkg_id
                    && call.module == workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.module
                    && call.function == workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.name
            })
            .expect("tool payment lock call");
        assert_eq!(lock_call.arguments.len(), 3);

        let finish_call = calls
            .iter()
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module == scheduler::Scheduler::FINISH.module
                    && call.function == scheduler::Scheduler::FINISH.name
            })
            .expect("scheduler finish call");
        assert_eq!(finish_call.module, scheduler::Scheduler::FINISH.module);
        assert_eq!(finish_call.function, scheduler::Scheduler::FINISH.name);
        assert_eq!(finish_call.arguments.len(), 2);
        inspector.expect_shared_object(&finish_call.arguments[0], &task, true);
        assert_matches!(&finish_call.arguments[1], sui::types::Argument::Result(0));
    }

    #[test]
    fn execute_registered_scheduled_occurrence_chains_scheduler_and_tap_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);
        let mut tx = sui::tx::TransactionBuilder::new();

        execute_registered_scheduled_occurrence(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            0,
            OccurrenceGenerator::Queue,
            &tools_gas,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 7);

        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        let scheduler_call = calls
            .iter()
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module == scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module
                    && call.function == scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name
            })
            .expect("queue occurrence check call");
        assert_eq!(scheduler_call.arguments.len(), 3);
        inspector.expect_shared_object(&scheduler_call.arguments[0], &task, true);
        inspector.expect_shared_object(
            &scheduler_call.arguments[1],
            &objects.leader_registry,
            false,
        );
        inspector.expect_clock(&scheduler_call.arguments[2]);

        let tap_call = calls
            .iter()
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module
                        == scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.module
                    && call.function
                        == scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.name
            })
            .expect("scheduler registered-agent preparation call");
        assert_eq!(
            tap_call.module,
            scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.module
        );
        assert_eq!(
            tap_call.function,
            scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.name
        );
        assert_eq!(tap_call.arguments.len(), 6);
        inspector.expect_shared_object(&tap_call.arguments[0], &dag, false);
        inspector.expect_shared_object(&tap_call.arguments[1], &objects.agent_registry, false);
        inspector.expect_shared_object(&tap_call.arguments[2], &objects.tool_registry, false);
        inspector.expect_shared_object(&tap_call.arguments[3], &task, true);
        inspector.expect_shared_object(&tap_call.arguments[4], &leader_cap, false);
        inspector.expect_clock(&tap_call.arguments[5]);

        let lock_call = calls
            .iter()
            .find(|call| {
                call.package == objects.workflow_pkg_id
                    && call.module == workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.module
                    && call.function == workflow::Gas::LOCK_PAYMENT_STATE_FOR_TOOL.name
            })
            .expect("tool payment lock call");
        assert_eq!(lock_call.arguments.len(), 3);

        let finish_call = calls
            .iter()
            .find(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module == scheduler::Scheduler::FINISH.module
                    && call.function == scheduler::Scheduler::FINISH.name
            })
            .expect("scheduler finish call");
        assert_eq!(finish_call.arguments.len(), 2);
        inspector.expect_shared_object(&finish_call.arguments[0], &task, true);
        assert_matches!(&finish_call.arguments[1], sui::types::Argument::Result(0));
    }

    #[test]
    fn execute_scheduled_occurrence_prepares_default_agent_after_scheduler_check() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);
        let mut tx = sui::tx::TransactionBuilder::new();

        execute_scheduled_occurrence(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            0,
            OccurrenceGenerator::Queue,
            &tools_gas,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let calls = inspector
            .commands()
            .iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect::<Vec<_>>();

        let scheduler_idx = calls
            .iter()
            .position(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module == scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.module
                    && call.function == scheduler::Scheduler::CHECK_QUEUE_OCCURRENCE.name
            })
            .expect("queue occurrence check call");
        let tap_idx = calls
            .iter()
            .position(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module
                        == scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.module
                    && call.function
                        == scheduler::Scheduler::PREPARE_EXECUTION_FROM_SCHEDULED_PAYMENT.name
            })
            .expect("scheduler default-agent preparation call");
        let finish_idx = calls
            .iter()
            .position(|call| {
                call.package == objects.scheduler_pkg_id
                    && call.module == scheduler::Scheduler::FINISH.module
                    && call.function == scheduler::Scheduler::FINISH.name
            })
            .expect("scheduler finish call");

        assert!(scheduler_idx < tap_idx);
        assert!(tap_idx < finish_idx);
    }

    #[test]
    fn execute_scheduled_occurrence_supports_periodic_generators() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let tools_gas = HashSet::from([(sui_mocks::mock_sui_address(), 0)]);
        let mut tx = sui::tx::TransactionBuilder::new();

        execute_scheduled_occurrence(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            0,
            OccurrenceGenerator::Periodic,
            &tools_gas,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let scheduler_call = inspector.move_call(0);
        assert_eq!(
            scheduler_call.function,
            scheduler::Scheduler::CHECK_PERIODIC_OCCURRENCE.name
        );
    }
}
