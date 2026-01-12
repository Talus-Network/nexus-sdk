use {
    crate::{
        idents::{move_std, primitives, pure_arg, sui_framework, workflow},
        sui,
        types::{DataStorage, NexusObjects, Storable, StorageKind},
    },
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
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

// Shared helper for turning a scheduler task object ref into a mutable shared argument.
fn shared_task_arg(
    tx: &mut sui::tx::TransactionBuilder,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.input(sui::tx::Input::shared(
        *task.object_id(),
        task.version(),
        true,
    )))
}

// == Metadata ==

/// PTB template to build task metadata from key/value pairs.
pub fn new_metadata<K, V>(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> anyhow::Result<sui::types::Argument>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let string_type = move_std::StdString::type_tag();

    let metadata = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module,
            sui_framework::VecMap::EMPTY.name,
            vec![string_type.clone(), string_type.clone()],
        ),
        vec![],
    );

    for (key, value) in key_values.into_iter() {
        let key = tx.input(pure_arg(&key.as_ref().to_string())?);

        let value = tx.input(pure_arg(&value.as_ref().to_string())?);

        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
                vec![string_type.clone(), string_type.clone()],
            ),
            vec![metadata, key, value],
        );
    }

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW_METADATA.module,
            workflow::Scheduler::NEW_METADATA.name,
            vec![],
        ),
        vec![metadata],
    ))
}

// == Task lifecycle ==

/// PTB template to create a new scheduler task.
pub fn new_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    metadata: sui::types::Argument,
    constraints: sui::types::Argument,
    execution: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW.module,
            workflow::Scheduler::NEW.name,
            vec![],
        ),
        vec![metadata, constraints, execution],
    ))
}

/// PTB template to update an existing task metadata bag.
pub fn update_metadata(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    metadata: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::UPDATE_METADATA.module,
            workflow::Scheduler::UPDATE_METADATA.name,
            vec![],
        ),
        vec![task, metadata],
    ))
}

/// PTB template to construct and register the default constraints policy.
pub fn new_constraints_policy(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    generator: OccurrenceGenerator,
) -> anyhow::Result<sui::types::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let constraint_symbol = match generator {
        OccurrenceGenerator::Queue => {
            let witness_tag = workflow::into_type_tag(
                objects.workflow_pkg_id,
                workflow::Scheduler::QUEUE_GENERATOR_WITNESS,
            );
            tx.move_call(
                sui::tx::Function::new(
                    objects.primitives_pkg_id,
                    primitives::Policy::WITNESS_SYMBOL.module,
                    primitives::Policy::WITNESS_SYMBOL.name,
                    vec![witness_tag],
                ),
                vec![],
            )
        }
        OccurrenceGenerator::Periodic => {
            let witness_tag = workflow::into_type_tag(
                objects.workflow_pkg_id,
                workflow::Scheduler::PERIODIC_GENERATOR_WITNESS,
            );
            tx.move_call(
                sui::tx::Function::new(
                    objects.primitives_pkg_id,
                    primitives::Policy::WITNESS_SYMBOL.module,
                    primitives::Policy::WITNESS_SYMBOL.name,
                    vec![witness_tag],
                ),
                vec![],
            )
        }
    };

    let constraint_sequence = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::EMPTY.module,
            sui_framework::TableVec::EMPTY.name,
            vec![symbol_type.clone()],
        ),
        vec![],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::PUSH_BACK.module,
            sui_framework::TableVec::PUSH_BACK.name,
            vec![symbol_type.clone()],
        ),
        vec![constraint_sequence, constraint_symbol],
    );

    let constraints = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW_CONSTRAINTS_POLICY.module,
            workflow::Scheduler::NEW_CONSTRAINTS_POLICY.name,
            vec![],
        ),
        vec![constraint_sequence],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::DROP.module,
            sui_framework::TableVec::DROP.name,
            vec![symbol_type.clone()],
        ),
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
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW_QUEUE_GENERATOR_STATE.module,
            workflow::Scheduler::NEW_QUEUE_GENERATOR_STATE.name,
            vec![],
        ),
        vec![],
    ))
}

/// PTB template to register the queue generator state.
pub fn register_queue_generator(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    constraints: sui::types::Argument,
    queue_state: sui::types::Argument,
) -> anyhow::Result<()> {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::REGISTER_QUEUE_GENERATOR.module,
            workflow::Scheduler::REGISTER_QUEUE_GENERATOR.name,
            vec![],
        ),
        vec![constraints, queue_state],
    );

    Ok(())
}

/// PTB template to construct a periodic generator state.
pub fn new_periodic_generator_state(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW_PERIODIC_GENERATOR_STATE.module,
            workflow::Scheduler::NEW_PERIODIC_GENERATOR_STATE.name,
            vec![],
        ),
        vec![],
    ))
}

/// PTB template to register the periodic generator state.
pub fn register_periodic_generator(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    constraints: sui::types::Argument,
    periodic_state: sui::types::Argument,
) -> anyhow::Result<()> {
    tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::REGISTER_PERIODIC_GENERATOR.module,
            workflow::Scheduler::REGISTER_PERIODIC_GENERATOR.name,
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let witness_tag = workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::DefaultTap::BEGIN_DAG_EXECUTION_WITNESS,
    );

    let execution_symbol = tx.move_call(
        sui::tx::Function::new(
            objects.primitives_pkg_id,
            primitives::Policy::WITNESS_SYMBOL.module,
            primitives::Policy::WITNESS_SYMBOL.name,
            vec![witness_tag],
        ),
        vec![],
    );

    let execution_sequence = tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::EMPTY.module,
            sui_framework::TableVec::EMPTY.name,
            vec![symbol_type.clone()],
        ),
        vec![],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::PUSH_BACK.module,
            sui_framework::TableVec::PUSH_BACK.name,
            vec![symbol_type.clone()],
        ),
        vec![execution_sequence, execution_symbol],
    );

    let execution = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW_EXECUTION_POLICY.module,
            workflow::Scheduler::NEW_EXECUTION_POLICY.name,
            vec![],
        ),
        vec![execution_sequence],
    );

    tx.move_call(
        sui::tx::Function::new(
            sui_framework::PACKAGE_ID,
            sui_framework::TableVec::DROP.module,
            sui_framework::TableVec::DROP.name,
            vec![symbol_type.clone()],
        ),
        vec![execution_sequence],
    );

    let dag_id_arg = sui_framework::Object::id_from_object_id(tx, dag_id)?;
    let network_id_arg = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;
    let priority_fee_per_gas_unit = tx.input(pure_arg(&priority_fee_per_gas_unit)?);

    let entry_group =
        workflow::Dag::entry_group_from_str(tx, objects.workflow_pkg_id, entry_group)?;

    let with_vertex_inputs = build_inputs_vec_map(tx, objects, input_data)?;

    let config = tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Dag::NEW_DAG_EXECUTION_CONFIG.module,
            workflow::Dag::NEW_DAG_EXECUTION_CONFIG.name,
            vec![],
        ),
        vec![
            dag_id_arg,
            network_id_arg,
            entry_group,
            with_vertex_inputs,
            priority_fee_per_gas_unit,
        ],
    );

    register_begin_execution(tx, objects, execution, config)?;

    Ok(execution)
}

fn build_inputs_vec_map(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    input_data: &HashMap<String, HashMap<String, DataStorage>>,
) -> anyhow::Result<sui::types::Argument> {
    let inner_vec_map_type = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::VERTEX),
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
            outer_vec_map_type.clone(),
        ),
        vec![],
    );

    for (vertex_name, data) in input_data {
        // `vertex: Vertex`
        let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex_name)?;

        // `with_vertex_input: VecMap<InputPort, NexusData>`
        let with_vertex_input = tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::EMPTY.module,
                sui_framework::VecMap::EMPTY.name,
                inner_vec_map_type.clone(),
            ),
            vec![],
        );

        for (port_name, value) in data {
            // `port: InputPort`
            let port = match value.is_encrypted() {
                true => workflow::Dag::encrypted_input_port_from_str(
                    tx,
                    objects.workflow_pkg_id,
                    port_name.as_str(),
                )?,
                false => workflow::Dag::input_port_from_str(
                    tx,
                    objects.workflow_pkg_id,
                    port_name.as_str(),
                )?,
            };

            // `value: NexusData`
            let value = match value.storage_kind() {
                StorageKind::Inline => primitives::Data::nexus_data_inline_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                    value.is_encrypted(),
                )?,
                StorageKind::Walrus => primitives::Data::nexus_data_walrus_from_json(
                    tx,
                    objects.primitives_pkg_id,
                    value.as_json(),
                    value.is_encrypted(),
                )?,
            };

            // `with_vertex_input.insert(port, value)`
            tx.move_call(
                sui::tx::Function::new(
                    sui_framework::PACKAGE_ID,
                    sui_framework::VecMap::INSERT.module,
                    sui_framework::VecMap::INSERT.name,
                    inner_vec_map_type.clone(),
                ),
                vec![with_vertex_input, port, value],
            );
        }

        // `with_vertex_inputs.insert(vertex, with_vertex_input)`
        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::VecMap::INSERT.module,
                sui_framework::VecMap::INSERT.name,
                outer_vec_map_type.clone(),
            ),
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
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::EXECUTE.module,
            workflow::Scheduler::EXECUTE.name,
            vec![],
        ),
        vec![task],
    ))
}

/// PTB template to finalize a task execution.
pub fn finish(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    proof: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::FINISH.module,
            workflow::Scheduler::FINISH.name,
            vec![],
        ),
        vec![task, proof],
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
) -> anyhow::Result<sui::types::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_time_ms: u64`
    let start_time_ms = tx.input(pure_arg(&start_time_ms)?);

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.input(match deadline_offset_ms {
        Some(n) => pure_arg(&n)?,
        None => pure_arg(&0u8)?,
    });

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.input(pure_arg(&priority_fee_per_gas_unit)?);

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::ADD_OCCURRENCE_ABSOLUTE_FOR_TASK.module,
            workflow::Scheduler::ADD_OCCURRENCE_ABSOLUTE_FOR_TASK.name,
            vec![],
        ),
        vec![
            task,
            start_time_ms,
            deadline_offset_ms,
            priority_fee_per_gas_unit,
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
) -> anyhow::Result<sui::types::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_offset_ms: u64`
    let start_offset_ms = tx.input(pure_arg(&start_offset_ms)?);

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.input(match deadline_offset_ms {
        Some(n) => pure_arg(&n)?,
        None => pure_arg(&0u8)?,
    });

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.input(pure_arg(&priority_fee_per_gas_unit)?);

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::ADD_OCCURRENCE_RELATIVE_FOR_TASK.module,
            workflow::Scheduler::ADD_OCCURRENCE_RELATIVE_FOR_TASK.name,
            vec![],
        ),
        vec![
            task,
            start_offset_ms,
            deadline_offset_ms,
            priority_fee_per_gas_unit,
            clock,
        ],
    ))
}

// == Periodic scheduling ==

/// PTB template to configure or update periodic scheduling.
#[warn(clippy::too_many_arguments)]
pub fn new_or_modify_periodic_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    schedule: PeriodicScheduleInputs,
) -> anyhow::Result<sui::types::Argument> {
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
    let first_start_ms = tx.input(pure_arg(&first_start_ms)?);

    // `period_ms: u64`
    let period_ms = tx.input(pure_arg(&period_ms)?);

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.input(match deadline_offset_ms {
        Some(n) => pure_arg(&n)?,
        None => pure_arg(&0u8)?,
    });

    // `max_iterations: option::Option<u64>`
    let max_iterations = tx.input(match max_iterations {
        Some(n) => pure_arg(&n)?,
        None => pure_arg(&0u8)?,
    });

    // `priority_fee_per_gas_unit: u64`
    let priority_fee_per_gas_unit = tx.input(pure_arg(&priority_fee_per_gas_unit)?);

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::NEW_OR_MODIFY_PERIODIC_FOR_TASK.module,
            workflow::Scheduler::NEW_OR_MODIFY_PERIODIC_FOR_TASK.name,
            vec![],
        ),
        vec![
            task,
            first_start_ms,
            period_ms,
            deadline_offset_ms,
            max_iterations,
            priority_fee_per_gas_unit,
        ],
    ))
}

/// PTB template to disable periodic scheduling for a task.
pub fn disable_periodic_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::DISABLE_PERIODIC_FOR_TASK.module,
            workflow::Scheduler::DISABLE_PERIODIC_FOR_TASK.name,
            vec![],
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
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::PAUSE_TIME_CONSTRAINT_FOR_TASK.module,
            workflow::Scheduler::PAUSE_TIME_CONSTRAINT_FOR_TASK.name,
            vec![],
        ),
        vec![task],
    ))
}

/// PTB template to resume scheduling for a task.
pub fn resume_time_constraint_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::RESUME_TIME_CONSTRAINT_FOR_TASK.module,
            workflow::Scheduler::RESUME_TIME_CONSTRAINT_FOR_TASK.name,
            vec![],
        ),
        vec![task],
    ))
}

/// PTB template to cancel scheduling for a task.
pub fn cancel_time_constraint_for_task(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::CANCEL_TIME_CONSTRAINT_FOR_TASK.module,
            workflow::Scheduler::CANCEL_TIME_CONSTRAINT_FOR_TASK.name,
            vec![],
        ),
        vec![task],
    ))
}

// == Execution flow ==

/// PTB template to consume the next queued occurrence.
pub fn check_queue_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::CHECK_QUEUE_OCCURRENCE.module,
            workflow::Scheduler::CHECK_QUEUE_OCCURRENCE.name,
            vec![],
        ),
        vec![task, clock],
    ))
}

/// PTB template to consume the next periodic occurrence.
pub fn check_periodic_occurrence(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
) -> anyhow::Result<sui::types::Argument> {
    let task = shared_task_arg(tx, task)?;
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::Scheduler::CHECK_PERIODIC_OCCURRENCE.module,
            workflow::Scheduler::CHECK_PERIODIC_OCCURRENCE.name,
            vec![],
        ),
        vec![task, clock],
    ))
}

/// PTB template to register DAG execution config on the execution policy.
pub fn register_begin_execution(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    policy: sui::types::Argument,
    config: sui::types::Argument,
) -> anyhow::Result<sui::types::Argument> {
    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::DefaultTap::REGISTER_BEGIN_EXECUTION.module,
            workflow::DefaultTap::REGISTER_BEGIN_EXECUTION.name,
            vec![],
        ),
        vec![policy, config],
    ))
}

/// PTB template to invoke DAG execution from the scheduler via the Default TAP.
#[allow(clippy::too_many_arguments)]
pub fn dag_begin_execution_from_scheduler(
    tx: &mut sui::tx::TransactionBuilder,
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    claim_coin: &sui::types::ObjectReference,
    amount_execution: u64,
    amount_priority: u64,
) -> anyhow::Result<sui::types::Argument> {
    // `self: &mut DefaultTAP`
    let tap = tx.input(sui::tx::Input::shared(
        *objects.default_tap.object_id(),
        objects.default_tap.version(),
        true,
    ));

    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `dag: &DAG`
    let dag = tx.input(sui::tx::Input::shared(
        *dag.object_id(),
        dag.version(),
        false,
    ));

    // `gas_service: &mut GasService`
    let gas_service = tx.input(sui::tx::Input::shared(
        *objects.gas_service.object_id(),
        objects.gas_service.version(),
        true,
    ));

    // `amount_execution: u64`
    let amount_execution_arg = tx.input(pure_arg(&amount_execution)?);

    // `amount_priority: u64`
    let amount_priority_arg = tx.input(pure_arg(&amount_priority)?);

    // `clock: &Clock`
    let clock = tx.input(sui::tx::Input::shared(
        sui_framework::CLOCK_OBJECT_ID,
        1,
        false,
    ));

    // `leader_cap: &CloneableOwnerCap<OverNetwork>`
    let leader_cap = tx.input(sui::tx::Input::shared(
        *leader_cap.object_id(),
        leader_cap.version(),
        false,
    ));

    // `claim_coin: Coin<SUI>`
    let claim_coin = tx.input(sui::tx::Input::owned(
        *claim_coin.object_id(),
        claim_coin.version(),
        *claim_coin.digest(),
    ));

    Ok(tx.move_call(
        sui::tx::Function::new(
            objects.workflow_pkg_id,
            workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER.module,
            workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER.name,
            vec![],
        ),
        vec![
            tap,
            task,
            dag,
            gas_service,
            leader_cap,
            claim_coin,
            amount_execution_arg,
            amount_priority_arg,
            clock,
        ],
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
    claim_coin: &sui::types::ObjectReference,
    amount_execution: u64,
    amount_priority: u64,
    generator: OccurrenceGenerator,
) -> anyhow::Result<()> {
    match generator {
        OccurrenceGenerator::Queue => check_queue_occurrence(tx, objects, task)?,
        OccurrenceGenerator::Periodic => check_periodic_occurrence(tx, objects, task)?,
    };
    dag_begin_execution_from_scheduler(
        tx,
        objects,
        task,
        dag,
        leader_cap,
        claim_coin,
        amount_execution,
        amount_priority,
    )?;

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
            let sui::types::Input::Shared {
                object_id,
                initial_shared_version,
                mutable: actual_mutable,
            } = self.input(argument)
            else {
                panic!(
                    "expected shared object argument, got {:?}",
                    self.input(argument)
                );
            };

            assert_eq!(object_id, expected.object_id());
            assert_eq!(*initial_shared_version, expected.version());
            assert_eq!(*actual_mutable, mutable);
        }

        fn expect_owned_object(
            &self,
            argument: &sui::types::Argument,
            expected: &(sui::types::Address, sui::types::Version, sui::types::Digest),
        ) {
            let sui::types::Input::ImmutableOrOwned(object) = self.input(argument) else {
                panic!(
                    "expected owned object argument, got {:?}",
                    self.input(argument)
                );
            };

            assert_eq!(object.object_id(), &expected.0);
            assert_eq!(object.version(), expected.1);
            assert_eq!(object.digest(), &expected.2);
        }

        fn expect_clock(&self, argument: &sui::types::Argument) {
            let sui::types::Input::Shared {
                object_id,
                initial_shared_version,
                mutable,
            } = self.input(argument)
            else {
                panic!(
                    "expected clock shared object argument, got {:?}",
                    self.input(argument)
                );
            };

            assert_eq!(*object_id, sui_framework::CLOCK_OBJECT_ID);
            assert_eq!(*initial_shared_version, 1);
            assert!(!*mutable, "clock object must be immutable");
        }

        fn expect_pure_bytes(&self, argument: &sui::types::Argument, expected: &[u8]) {
            let sui::types::Input::Pure { value } = self.input(argument) else {
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
                    let mut bytes = vec![];
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

        let scheduler_arg =
            new_metadata(&mut tx, &objects, [("foo", "bar")]).expect("ptb construction succeeds");

        assert_matches!(scheduler_arg, sui::types::Argument::Result(2));

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
        assert_eq!(final_call.package, objects.workflow_pkg_id);
        assert_eq!(final_call.module, workflow::Scheduler::NEW_METADATA.module);
        assert_eq!(final_call.function, workflow::Scheduler::NEW_METADATA.name);
        assert!(final_call.type_arguments.is_empty());
        assert_eq!(final_call.arguments.len(), 1);
        assert_matches!(final_call.arguments[0], sui::types::Argument::Result(0));
    }

    #[test]
    fn new_metadata_handles_empty_iterators() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let result = new_metadata(
            &mut tx,
            &objects,
            std::iter::empty::<(&'static str, &'static str)>(),
        )
        .expect("ptb construction succeeds");

        assert_matches!(result, sui::types::Argument::Result(1));

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 2);
    }

    #[test]
    fn new_task_builds_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata = tx.input(pure_arg(&1_u64).unwrap());
        let constraints = tx.input(pure_arg(&2_u64).unwrap());
        let execution = tx.input(pure_arg(&3_u64).unwrap());

        let result = new_task(&mut tx, &objects, metadata, constraints, execution)
            .expect("ptb construction succeeds");

        assert_matches!(result, sui::types::Argument::Result(0));

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);

        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Scheduler::NEW.module);
        assert_eq!(call.function, workflow::Scheduler::NEW.name);
        assert_eq!(call.arguments.len(), 3);
        inspector.expect_u64(&call.arguments[0], 1);
        inspector.expect_u64(&call.arguments[1], 2);
        inspector.expect_u64(&call.arguments[2], 3);
    }

    #[test]
    fn update_metadata_uses_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata = tx.input(pure_arg(&9_u64).unwrap());

        update_metadata(&mut tx, &objects, &task, metadata).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.module, workflow::Scheduler::UPDATE_METADATA.module);
        assert_eq!(call.function, workflow::Scheduler::UPDATE_METADATA.name);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], 9);
    }

    #[test]
    fn new_queue_generator_state_has_no_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let arg = new_queue_generator_state(&mut tx, &objects).expect("ptb construction succeeds");
        assert_matches!(arg, sui::types::Argument::Result(0));

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert!(call.arguments.is_empty());
        assert_eq!(
            call.module,
            workflow::Scheduler::NEW_QUEUE_GENERATOR_STATE.module
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::NEW_QUEUE_GENERATOR_STATE.name
        );
    }

    #[test]
    fn register_queue_generator_invokes_scheduler() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let constraints = tx.input(pure_arg(&11_u64).unwrap());
        let queue_state = tx.input(pure_arg(&12_u64).unwrap());

        register_queue_generator(&mut tx, &objects, constraints, queue_state)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(
            call.module,
            workflow::Scheduler::REGISTER_QUEUE_GENERATOR.module
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::REGISTER_QUEUE_GENERATOR.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[0], 11);
        inspector.expect_u64(&call.arguments[1], 12);
    }

    #[test]
    fn new_periodic_generator_state_has_no_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();

        let arg =
            new_periodic_generator_state(&mut tx, &objects).expect("ptb construction succeeds");
        assert_matches!(arg, sui::types::Argument::Result(0));

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert!(call.arguments.is_empty());
        assert_eq!(
            call.module,
            workflow::Scheduler::NEW_PERIODIC_GENERATOR_STATE.module
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::NEW_PERIODIC_GENERATOR_STATE.name
        );
    }

    #[test]
    fn register_periodic_generator_invokes_scheduler() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let constraints = tx.input(pure_arg(&21_u64).unwrap());
        let periodic_state = tx.input(pure_arg(&22_u64).unwrap());

        register_periodic_generator(&mut tx, &objects, constraints, periodic_state)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(
            call.module,
            workflow::Scheduler::REGISTER_PERIODIC_GENERATOR.module
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::REGISTER_PERIODIC_GENERATOR.name
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

        let witness = execute(&mut tx, &objects, &task).expect("ptb construction succeeds");
        assert_matches!(witness, sui::types::Argument::Result(0));

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        assert_eq!(call.module, workflow::Scheduler::EXECUTE.module);
        assert_eq!(call.function, workflow::Scheduler::EXECUTE.name);
    }

    #[test]
    fn finish_finalizes_execution_with_proof() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let proof = tx.input(pure_arg(&5_u64).unwrap());
        finish(&mut tx, &objects, &task, proof).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.module, workflow::Scheduler::FINISH.module);
        assert_eq!(call.function, workflow::Scheduler::FINISH.name);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], 5);
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
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_time);
        inspector.expect_option_u64(&call.arguments[2], deadline);
        inspector.expect_u64(&call.arguments[3], priority_fee_per_gas_unit);
        inspector.expect_clock(&call.arguments[4]);
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
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_offset);
        inspector.expect_option_u64(&call.arguments[2], deadline_offset);
        inspector.expect_u64(&call.arguments[3], priority_fee_per_gas_unit);
        inspector.expect_clock(&call.arguments[4]);
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
        assert_eq!(call.arguments.len(), 6);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], first_start);
        inspector.expect_u64(&call.arguments[2], period);
        inspector.expect_option_u64(&call.arguments[3], deadline_offset);
        inspector.expect_option_u64(&call.arguments[4], max_iterations);
        inspector.expect_u64(&call.arguments[5], priority_fee_per_gas_unit);
    }

    #[test]
    fn disable_periodic_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        disable_periodic_for_task(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
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
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
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
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
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
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
    }

    #[test]
    fn check_queue_occurrence_uses_clock_and_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        check_queue_occurrence(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(
            call.function,
            workflow::Scheduler::CHECK_QUEUE_OCCURRENCE.name
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
        assert_eq!(
            call.function,
            workflow::Scheduler::CHECK_PERIODIC_OCCURRENCE.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_clock(&call.arguments[1]);
    }

    #[test]
    fn register_begin_execution_routes_through_default_tap() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::tx::TransactionBuilder::new();
        let policy = tx.input(pure_arg(&13_u64).unwrap());
        let config = tx.input(pure_arg(&14_u64).unwrap());
        register_begin_execution(&mut tx, &objects, policy, config)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultTap::REGISTER_BEGIN_EXECUTION.module
        );
        assert_eq!(
            call.function,
            workflow::DefaultTap::REGISTER_BEGIN_EXECUTION.name
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_u64(&call.arguments[0], 13);
        inspector.expect_u64(&call.arguments[1], 14);
    }

    #[test]
    fn dag_begin_execution_from_scheduler_builds_full_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let claim_coin = sui_mocks::mock_sui_object_ref();
        let leader_cap_tuple = leader_cap.clone().into_parts();
        let claim_coin_tuple = claim_coin.clone().into_parts();
        let mut tx = sui::tx::TransactionBuilder::new();

        let amount_execution = 44;
        let amount_priority = 55;

        dag_begin_execution_from_scheduler(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            &claim_coin,
            amount_execution,
            amount_priority,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 9);

        inspector.expect_shared_object(&call.arguments[0], &objects.default_tap, true);
        inspector.expect_shared_object(&call.arguments[1], &task, true);
        let sui::types::Input::Shared {
            object_id,
            initial_shared_version,
            mutable,
        } = inspector.input(&call.arguments[2])
        else {
            panic!(
                "expected shared object argument for DAG, got {:?}",
                inspector.input(&call.arguments[2])
            );
        };
        assert_eq!(object_id, dag.object_id());
        assert_eq!(*initial_shared_version, dag.version());
        assert!(!*mutable);

        inspector.expect_shared_object(&call.arguments[3], &objects.gas_service, true);
        inspector.expect_owned_object(&call.arguments[4], &leader_cap_tuple);
        inspector.expect_owned_object(&call.arguments[5], &claim_coin_tuple);
        inspector.expect_u64(&call.arguments[6], amount_execution);
        inspector.expect_u64(&call.arguments[7], amount_priority);
        inspector.expect_clock(&call.arguments[8]);
    }

    #[test]
    fn execute_scheduled_occurrence_chains_scheduler_and_tap_calls() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let claim_coin = sui_mocks::mock_sui_object_ref();
        let leader_cap_tuple = leader_cap.clone().into_parts();
        let claim_coin_tuple = claim_coin.clone().into_parts();
        let mut tx = sui::tx::TransactionBuilder::new();

        execute_scheduled_occurrence(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            &claim_coin,
            100,
            200,
            OccurrenceGenerator::Queue,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        assert_eq!(inspector.commands().len(), 2);

        let scheduler_call = inspector.move_call(0);
        assert_eq!(
            scheduler_call.module,
            workflow::Scheduler::CHECK_QUEUE_OCCURRENCE.module
        );
        assert_eq!(
            scheduler_call.function,
            workflow::Scheduler::CHECK_QUEUE_OCCURRENCE.name
        );
        assert_eq!(scheduler_call.arguments.len(), 2);
        inspector.expect_shared_object(&scheduler_call.arguments[0], &task, true);
        inspector.expect_clock(&scheduler_call.arguments[1]);

        let tap_call = inspector.move_call(1);
        assert_eq!(
            tap_call.module,
            workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER.module
        );
        assert_eq!(
            tap_call.function,
            workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER.name
        );
        assert_eq!(tap_call.arguments.len(), 9);
        inspector.expect_shared_object(&tap_call.arguments[0], &objects.default_tap, true);
        inspector.expect_shared_object(&tap_call.arguments[1], &task, true);
        let sui::types::Input::Shared {
            object_id,
            initial_shared_version,
            mutable,
        } = inspector.input(&tap_call.arguments[2])
        else {
            panic!(
                "expected shared DAG object, got {:?}",
                inspector.input(&tap_call.arguments[2])
            );
        };
        assert_eq!(object_id, dag.object_id());
        assert_eq!(*initial_shared_version, dag.version());
        assert!(!*mutable);
        inspector.expect_shared_object(&tap_call.arguments[3], &objects.gas_service, true);
        inspector.expect_owned_object(&tap_call.arguments[4], &leader_cap_tuple);
        inspector.expect_owned_object(&tap_call.arguments[5], &claim_coin_tuple);
        inspector.expect_u64(&tap_call.arguments[6], 100);
        inspector.expect_u64(&tap_call.arguments[7], 200);
        inspector.expect_clock(&tap_call.arguments[8]);
    }

    #[test]
    fn execute_scheduled_occurrence_supports_periodic_generators() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap = sui_mocks::mock_sui_object_ref();
        let claim_coin = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();

        execute_scheduled_occurrence(
            &mut tx,
            &objects,
            &task,
            &dag,
            &leader_cap,
            &claim_coin,
            10,
            20,
            OccurrenceGenerator::Periodic,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(sui_mocks::mock_finish_transaction(tx));
        let scheduler_call = inspector.move_call(0);
        assert_eq!(
            scheduler_call.function,
            workflow::Scheduler::CHECK_PERIODIC_OCCURRENCE.name
        );
    }
}
