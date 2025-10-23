use {
    crate::{
        idents::{move_std, primitives, sui_framework, workflow},
        sui,
        types::{NexusObjects, DEFAULT_ENTRY_GROUP},
    },
    serde_json::Value,
    std::collections::HashMap,
};

// Shared helper for turning a scheduler task object ref into a mutable shared argument.
fn shared_task_arg(
    tx: &mut sui::ProgrammableTransactionBuilder,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    tx.obj(sui::ObjectArg::SharedObject {
        id: task.object_id,
        initial_shared_version: task.version,
        mutable: true,
    })
}

// == Metadata ==

/// PTB template to build task metadata from key/value pairs.
pub fn new_metadata<K, V>(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    key_values: impl IntoIterator<Item = (K, V)>,
) -> anyhow::Result<sui::Argument>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let string_type = move_std::StdString::type_tag();

    let metadata = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::VecMap::EMPTY.module.into(),
        sui_framework::VecMap::EMPTY.name.into(),
        vec![string_type.clone(), string_type.clone()],
        vec![],
    );

    for (key, value) in key_values.into_iter() {
        let key = tx.pure(key.as_ref().to_owned())?;
        let value = tx.pure(value.as_ref().to_owned())?;

        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::INSERT.module.into(),
            sui_framework::VecMap::INSERT.name.into(),
            vec![string_type.clone(), string_type.clone()],
            vec![metadata, key, value],
        );
    }

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::NEW_METADATA.module.into(),
        workflow::Scheduler::NEW_METADATA.name.into(),
        vec![],
        vec![metadata],
    ))
}

// == Task lifecycle ==

/// PTB template to create a new scheduler task.
pub fn new_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    metadata: sui::Argument,
    constraints: sui::Argument,
    execution: sui::Argument,
) -> anyhow::Result<sui::Argument> {
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::NEW.module.into(),
        workflow::Scheduler::NEW.name.into(),
        vec![],
        vec![metadata, constraints, execution, clock],
    ))
}

/// PTB template to update an existing task metadata bag.
pub fn update_metadata(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    metadata: sui::Argument,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::UPDATE_METADATA.module.into(),
        workflow::Scheduler::UPDATE_METADATA.name.into(),
        vec![],
        vec![task, metadata],
    ))
}

/// PTB template to register the time constraint configuration.
pub fn register_time_constraint(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    policy: sui::Argument,
    config: sui::Argument,
) -> anyhow::Result<sui::Argument> {
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::REGISTER_TIME_CONSTRAINT.module.into(),
        workflow::Scheduler::REGISTER_TIME_CONSTRAINT.name.into(),
        vec![],
        vec![policy, config],
    ))
}

/// PTB template to construct a new time constraint configuration value.
pub fn new_time_constraint_config(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::Argument> {
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::NEW_TIME_CONSTRAINT_CONFIG
            .module
            .into(),
        workflow::Scheduler::NEW_TIME_CONSTRAINT_CONFIG.name.into(),
        vec![],
        vec![],
    ))
}

/// PTB template to construct and register the default constraints policy.
pub fn new_constraints_policy(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
) -> anyhow::Result<sui::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let time_constraint_tag = workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::Scheduler::TIME_CONSTRAINT,
    );

    let constraint_symbol = tx.programmable_move_call(
        objects.primitives_pkg_id,
        primitives::Policy::WITNESS_SYMBOL.module.into(),
        primitives::Policy::WITNESS_SYMBOL.name.into(),
        vec![time_constraint_tag],
        vec![],
    );

    let constraint_sequence = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_std::Vector::SINGLETON.module.into(),
        move_std::Vector::SINGLETON.name.into(),
        vec![symbol_type.clone()],
        vec![constraint_symbol],
    );

    let constraints = tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::NEW_CONSTRAINTS_POLICY.module.into(),
        workflow::Scheduler::NEW_CONSTRAINTS_POLICY.name.into(),
        vec![],
        vec![constraint_sequence],
    );

    let config = new_time_constraint_config(tx, objects)?;
    register_time_constraint(tx, objects, constraints.clone(), config)?;

    Ok(constraints)
}

/// PTB template to construct and register the default execution policy.
#[allow(clippy::too_many_arguments)]
pub fn new_execution_policy(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    dag_id: sui::ObjectID,
    gas_price: u64,
    inputs: &Value,
    entry_group: Option<&str>,
    encrypt_handles: Option<&HashMap<String, Vec<String>>>,
) -> anyhow::Result<sui::Argument> {
    let symbol_type =
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Policy::SYMBOL);
    let witness_tag = workflow::into_type_tag(
        objects.workflow_pkg_id,
        workflow::DefaultTap::BEGIN_DAG_EXECUTION_WITNESS,
    );

    let execution_symbol = tx.programmable_move_call(
        objects.primitives_pkg_id,
        primitives::Policy::WITNESS_SYMBOL.module.into(),
        primitives::Policy::WITNESS_SYMBOL.name.into(),
        vec![witness_tag],
        vec![],
    );

    let execution_sequence = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        move_std::Vector::SINGLETON.module.into(),
        move_std::Vector::SINGLETON.name.into(),
        vec![symbol_type.clone()],
        vec![execution_symbol],
    );

    let execution = tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::NEW_EXECUTION_POLICY.module.into(),
        workflow::Scheduler::NEW_EXECUTION_POLICY.name.into(),
        vec![],
        vec![execution_sequence],
    );

    let dag_id_arg = sui_framework::Object::id_from_object_id(tx, dag_id)?;
    let network_id_arg = sui_framework::Object::id_from_object_id(tx, objects.network_id)?;
    let gas_price_arg = tx.pure(gas_price)?;

    let entry_group = workflow::Dag::entry_group_from_str(
        tx,
        objects.workflow_pkg_id,
        entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
    )?;

    let with_vertex_inputs = build_inputs_vec_map(tx, objects, inputs, encrypt_handles)?;

    let config = tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Dag::NEW_DAG_EXECUTION_CONFIG.module.into(),
        workflow::Dag::NEW_DAG_EXECUTION_CONFIG.name.into(),
        vec![],
        vec![
            dag_id_arg,
            network_id_arg,
            gas_price_arg,
            entry_group,
            with_vertex_inputs,
        ],
    );

    register_begin_execution(tx, objects, execution.clone(), config)?;

    Ok(execution)
}

fn build_inputs_vec_map(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    inputs: &Value,
    encrypt_handles: Option<&HashMap<String, Vec<String>>>,
) -> anyhow::Result<sui::Argument> {
    let inner_vec_map_type = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::INPUT_PORT),
        primitives::into_type_tag(objects.primitives_pkg_id, primitives::Data::NEXUS_DATA),
    ];

    let outer_vec_map_type = vec![
        workflow::into_type_tag(objects.workflow_pkg_id, workflow::Dag::VERTEX),
        sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::FRAMEWORK_PACKAGE_ID,
            module: sui_framework::VecMap::VEC_MAP.module.into(),
            name: sui_framework::VecMap::VEC_MAP.name.into(),
            type_params: inner_vec_map_type.clone(),
        })),
    ];

    let with_vertex_inputs = tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        sui_framework::VecMap::EMPTY.module.into(),
        sui_framework::VecMap::EMPTY.name.into(),
        outer_vec_map_type.clone(),
        vec![],
    );

    let data = inputs.as_object().ok_or_else(|| {
        anyhow::anyhow!("Input JSON must map vertex names to objects of port -> value")
    })?;

    for (vertex_name, value) in data {
        let vertex_inputs = value.as_object().ok_or_else(|| {
            anyhow::anyhow!("Vertex '{vertex_name}' value must be an object mapping ports to data")
        })?;

        let vertex = workflow::Dag::vertex_from_str(tx, objects.workflow_pkg_id, vertex_name)?;

        let inner_map = tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::EMPTY.module.into(),
            sui_framework::VecMap::EMPTY.name.into(),
            inner_vec_map_type.clone(),
            vec![],
        );

        for (port_name, port_value) in vertex_inputs {
            let encrypted = encrypt_handles.map_or(false, |handles| {
                handles
                    .get(vertex_name)
                    .map_or(false, |ports| ports.iter().any(|p| p == port_name))
            });

            let port = if encrypted {
                workflow::Dag::encrypted_input_port_from_str(
                    tx,
                    objects.workflow_pkg_id,
                    port_name,
                )?
            } else {
                workflow::Dag::input_port_from_str(tx, objects.workflow_pkg_id, port_name)?
            };

            let nexus_data = primitives::Data::nexus_data_from_json(
                tx,
                objects.primitives_pkg_id,
                port_value,
                encrypted,
            )?;

            tx.programmable_move_call(
                sui::FRAMEWORK_PACKAGE_ID,
                sui_framework::VecMap::INSERT.module.into(),
                sui_framework::VecMap::INSERT.name.into(),
                inner_vec_map_type.clone(),
                vec![inner_map.clone(), port, nexus_data],
            );
        }

        tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            sui_framework::VecMap::INSERT.module.into(),
            sui_framework::VecMap::INSERT.name.into(),
            outer_vec_map_type.clone(),
            vec![with_vertex_inputs.clone(), vertex, inner_map],
        );
    }

    Ok(with_vertex_inputs)
}

/// PTB template to obtain the execution witness for a task.
pub fn execute(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::EXECUTE.module.into(),
        workflow::Scheduler::EXECUTE.name.into(),
        vec![],
        vec![task],
    ))
}

/// PTB template to finalize a task execution.
pub fn finish(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    proof: sui::Argument,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::FINISH.module.into(),
        workflow::Scheduler::FINISH.name.into(),
        vec![],
        vec![task, proof],
    ))
}

// == Occurrence scheduling ==

/// PTB template to enqueue a new occurrence with absolute deadline.
pub fn add_occurrence_absolute_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    start_time_ms: u64,
    deadline_ms: Option<u64>,
    gas_price: u64,
) -> anyhow::Result<sui::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_time_ms: u64`
    let start_time_ms = tx.pure(start_time_ms)?;

    // `deadline_ms: option::Option<u64>`
    let deadline_ms = tx.pure(deadline_ms)?;

    // `gas_price: u64`
    let gas_price = tx.pure(gas_price)?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::ADD_OCCURRENCE_ABSOLUTE_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::ADD_OCCURRENCE_ABSOLUTE_FOR_TASK
            .name
            .into(),
        vec![],
        vec![task, start_time_ms, deadline_ms, gas_price, clock],
    ))
}

/// PTB template to enqueue a new occurrence with deadline offset.
pub fn add_occurrence_with_offset_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    start_time_ms: u64,
    deadline_offset_ms: Option<u64>,
    gas_price: u64,
) -> anyhow::Result<sui::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_time_ms: u64`
    let start_time_ms = tx.pure(start_time_ms)?;

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.pure(deadline_offset_ms)?;

    // `gas_price: u64`
    let gas_price = tx.pure(gas_price)?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::ADD_OCCURRENCE_WITH_OFFSET_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::ADD_OCCURRENCE_WITH_OFFSET_FOR_TASK
            .name
            .into(),
        vec![],
        vec![task, start_time_ms, deadline_offset_ms, gas_price, clock],
    ))
}

/// PTB template to enqueue a new occurrence relative to the current time.
pub fn add_occurrence_with_offsets_from_now_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    start_offset_ms: u64,
    deadline_offset_ms: Option<u64>,
    gas_price: u64,
) -> anyhow::Result<sui::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_offset_ms: u64`
    let start_offset_ms = tx.pure(start_offset_ms)?;

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.pure(deadline_offset_ms)?;

    // `gas_price: u64`
    let gas_price = tx.pure(gas_price)?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::ADD_OCCURRENCE_WITH_OFFSETS_FROM_NOW_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::ADD_OCCURRENCE_WITH_OFFSETS_FROM_NOW_FOR_TASK
            .name
            .into(),
        vec![],
        vec![task, start_offset_ms, deadline_offset_ms, gas_price, clock],
    ))
}

// == Periodic scheduling ==

/// PTB template to configure or update periodic scheduling.
pub fn new_or_modify_periodic_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    period_ms: u64,
    deadline_offset_ms: Option<u64>,
    max_iterations: Option<u64>,
    gas_price: u64,
) -> anyhow::Result<sui::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `period_ms: u64`
    let period_ms = tx.pure(period_ms)?;

    // `deadline_offset_ms: option::Option<u64>`
    let deadline_offset_ms = tx.pure(deadline_offset_ms)?;

    // `max_iterations: option::Option<u64>`
    let max_iterations = tx.pure(max_iterations)?;

    // `gas_price: u64`
    let gas_price = tx.pure(gas_price)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::NEW_OR_MODIFY_PERIODIC_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::NEW_OR_MODIFY_PERIODIC_FOR_TASK
            .name
            .into(),
        vec![],
        vec![
            task,
            period_ms,
            deadline_offset_ms,
            max_iterations,
            gas_price,
        ],
    ))
}

/// PTB template to disable periodic scheduling for a task.
pub fn disable_periodic_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::DISABLE_PERIODIC_FOR_TASK.module.into(),
        workflow::Scheduler::DISABLE_PERIODIC_FOR_TASK.name.into(),
        vec![],
        vec![task],
    ))
}

// == Constraint state management ==

/// PTB template to pause scheduling for a task.
pub fn pause_time_constraint_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::PAUSE_TIME_CONSTRAINT_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::PAUSE_TIME_CONSTRAINT_FOR_TASK
            .name
            .into(),
        vec![],
        vec![task],
    ))
}

/// PTB template to resume scheduling for a task.
pub fn resume_time_constraint_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::RESUME_TIME_CONSTRAINT_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::RESUME_TIME_CONSTRAINT_FOR_TASK
            .name
            .into(),
        vec![],
        vec![task],
    ))
}

/// PTB template to cancel scheduling for a task.
pub fn cancel_time_constraint_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::CANCEL_TIME_CONSTRAINT_FOR_TASK
            .module
            .into(),
        workflow::Scheduler::CANCEL_TIME_CONSTRAINT_FOR_TASK
            .name
            .into(),
        vec![],
        vec![task],
    ))
}

// == Execution flow ==

/// PTB template to evaluate the scheduler and consume the next occurrence.
pub fn check_time_constraint(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
) -> anyhow::Result<sui::Argument> {
    let task = shared_task_arg(tx, task)?;
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::Scheduler::CHECK_TIME_CONSTRAINT.module.into(),
        workflow::Scheduler::CHECK_TIME_CONSTRAINT.name.into(),
        vec![],
        vec![task, clock],
    ))
}

/// PTB template to register DAG execution config on the execution policy.
pub fn register_begin_execution(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    policy: sui::Argument,
    config: sui::Argument,
) -> anyhow::Result<sui::Argument> {
    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultTap::REGISTER_BEGIN_EXECUTION.module.into(),
        workflow::DefaultTap::REGISTER_BEGIN_EXECUTION.name.into(),
        vec![],
        vec![policy, config],
    ))
}

/// PTB template to invoke DAG execution from the scheduler via the Default TAP.
pub fn dag_begin_execution_from_scheduler(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    dag: &sui::ObjectRef,
    leader_cap: sui::Argument,
    claim_coin: sui::Argument,
    amount_execution: u64,
    amount_priority: u64,
) -> anyhow::Result<sui::Argument> {
    // `self: &mut DefaultTAP`
    let tap = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.default_tap.object_id,
        initial_shared_version: objects.default_tap.version,
        mutable: true,
    })?;

    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `dag: &DAG`
    let dag = tx.obj(sui::ObjectArg::SharedObject {
        id: dag.object_id,
        initial_shared_version: dag.version,
        mutable: false,
    })?;

    // `gas_service: &mut GasService`
    let gas_service = tx.obj(sui::ObjectArg::SharedObject {
        id: objects.gas_service.object_id,
        initial_shared_version: objects.gas_service.version,
        mutable: true,
    })?;

    // `amount_execution: u64`
    let amount_execution_arg = tx.pure(amount_execution)?;

    // `amount_priority: u64`
    let amount_priority_arg = tx.pure(amount_priority)?;

    // `clock: &Clock`
    let clock = tx.obj(sui::CLOCK_OBJ_ARG)?;

    Ok(tx.programmable_move_call(
        objects.workflow_pkg_id,
        workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER
            .module
            .into(),
        workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER
            .name
            .into(),
        vec![],
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
pub fn execute_scheduled_occurrence(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    dag: &sui::ObjectRef,
    leader_cap: sui::Argument,
    claim_coin: sui::Argument,
    amount_execution: u64,
    amount_priority: u64,
) -> anyhow::Result<()> {
    check_time_constraint(tx, objects, task)?;
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
        sui_sdk::types::transaction::ProgrammableMoveCall,
    };

    struct TxInspector {
        tx: sui::ProgrammableTransaction,
    }

    impl TxInspector {
        fn new(tx: sui::ProgrammableTransaction) -> Self {
            Self { tx }
        }

        fn commands_len(&self) -> usize {
            self.tx.commands.len()
        }

        fn move_call(&self, index: usize) -> &ProgrammableMoveCall {
            match self.tx.commands.get(index) {
                Some(sui::Command::MoveCall(call)) => call,
                Some(other) => panic!("expected MoveCall at index {index}, got {other:?}"),
                None => panic!("missing command at index {index}"),
            }
        }

        fn call_arg(&self, argument: &sui::Argument) -> &sui::CallArg {
            let sui::Argument::Input(index) = argument else {
                panic!("expected Argument::Input, got {argument:?}");
            };

            self.tx
                .inputs
                .get(*index as usize)
                .unwrap_or_else(|| panic!("missing input for index {index}"))
        }

        fn expect_shared_object(
            &self,
            argument: &sui::Argument,
            expected: &sui::ObjectRef,
            mutable: bool,
        ) {
            let sui::CallArg::Object(sui::ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable: actual_mutable,
            }) = self.call_arg(argument)
            else {
                panic!(
                    "expected shared object argument, got {:?}",
                    self.call_arg(argument)
                );
            };

            assert_eq!(*id, expected.object_id);
            assert_eq!(*initial_shared_version, expected.version);
            assert_eq!(*actual_mutable, mutable);
        }

        fn expect_owned_object(
            &self,
            argument: &sui::Argument,
            expected: &(sui::ObjectID, sui::SequenceNumber, sui::ObjectDigest),
        ) {
            let sui::CallArg::Object(sui::ObjectArg::ImmOrOwnedObject(object)) =
                self.call_arg(argument)
            else {
                panic!(
                    "expected owned object argument, got {:?}",
                    self.call_arg(argument)
                );
            };

            assert_eq!(object, expected);
        }

        fn expect_clock(&self, argument: &sui::Argument) {
            let sui::CallArg::Object(sui::ObjectArg::SharedObject {
                id,
                initial_shared_version,
                mutable,
            }) = self.call_arg(argument)
            else {
                panic!(
                    "expected clock shared object, got {:?}",
                    self.call_arg(argument)
                );
            };

            assert_eq!(*id, sui::CLOCK_OBJECT_ID);
            assert_eq!(*initial_shared_version, sui::CLOCK_OBJECT_SHARED_VERSION);
            assert!(!*mutable, "clock object must be immutable");
        }

        fn expect_pure_bytes(&self, argument: &sui::Argument, expected: &[u8]) {
            let sui::CallArg::Pure(bytes) = self.call_arg(argument) else {
                panic!("expected pure argument, got {:?}", self.call_arg(argument));
            };

            assert_eq!(bytes.as_slice(), expected);
        }

        fn expect_u64(&self, argument: &sui::Argument, value: u64) {
            self.expect_pure_bytes(argument, &value.to_le_bytes());
        }

        fn expect_option_u64(&self, argument: &sui::Argument, value: Option<u64>) {
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
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let scheduler_arg =
            new_metadata(&mut tx, &objects, [("foo", "bar")]).expect("ptb construction succeeds");

        assert_matches!(scheduler_arg, sui::Argument::Result(2));

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 3);

        let empty_call = inspector.move_call(0);
        assert_eq!(empty_call.package, sui::FRAMEWORK_PACKAGE_ID);
        assert_eq!(
            empty_call.module,
            sui_framework::VecMap::EMPTY.module.to_string()
        );
        assert_eq!(
            empty_call.function,
            sui_framework::VecMap::EMPTY.name.to_string()
        );
        assert_eq!(empty_call.type_arguments.len(), 2);
        assert!(empty_call.arguments.is_empty());

        let insert_call = inspector.move_call(1);
        assert_eq!(insert_call.package, sui::FRAMEWORK_PACKAGE_ID);
        assert_eq!(
            insert_call.module,
            sui_framework::VecMap::INSERT.module.to_string()
        );
        assert_eq!(
            insert_call.function,
            sui_framework::VecMap::INSERT.name.to_string()
        );
        assert_eq!(insert_call.type_arguments.len(), 2);
        assert_eq!(insert_call.arguments.len(), 3);

        let final_call = inspector.move_call(2);
        assert_eq!(final_call.package, objects.workflow_pkg_id);
        assert_eq!(
            final_call.module,
            workflow::Scheduler::NEW_METADATA.module.to_string()
        );
        assert_eq!(
            final_call.function,
            workflow::Scheduler::NEW_METADATA.name.to_string()
        );
        assert!(final_call.type_arguments.is_empty());
        assert_eq!(final_call.arguments.len(), 1);
        assert_matches!(final_call.arguments[0], sui::Argument::Result(0));
    }

    #[test]
    fn new_metadata_handles_empty_iterators() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let result = new_metadata(
            &mut tx,
            &objects,
            std::iter::empty::<(&'static str, &'static str)>(),
        )
        .expect("ptb construction succeeds");

        assert_matches!(result, sui::Argument::Result(1));

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 2);
    }

    #[test]
    fn new_task_adds_clock_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let metadata = tx.pure(1u8).expect("metadata input");
        let constraints = tx.pure(2u8).expect("constraints input");
        let execution = tx.pure(3u8).expect("execution input");

        let result = new_task(&mut tx, &objects, metadata, constraints, execution)
            .expect("ptb construction succeeds");

        assert_matches!(result, sui::Argument::Result(0));

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);

        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(call.module, workflow::Scheduler::NEW.module.to_string());
        assert_eq!(call.function, workflow::Scheduler::NEW.name.to_string());
        assert_eq!(call.arguments.len(), 4);
        inspector.expect_pure_bytes(&call.arguments[0], &[1]);
        inspector.expect_pure_bytes(&call.arguments[1], &[2]);
        inspector.expect_pure_bytes(&call.arguments[2], &[3]);
        inspector.expect_clock(&call.arguments[3]);
    }

    #[test]
    fn update_metadata_uses_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();

        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let metadata = tx.pure(9u8).expect("metadata input");

        update_metadata(&mut tx, &objects, &task, metadata).expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(
            call.module,
            workflow::Scheduler::UPDATE_METADATA.module.to_string()
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::UPDATE_METADATA.name.to_string()
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_pure_bytes(&call.arguments[1], &[9]);
    }

    #[test]
    fn register_time_constraint_invokes_scheduler() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let policy = tx.pure(11u8).expect("policy input");
        let config = tx.pure(12u8).expect("config input");

        register_time_constraint(&mut tx, &objects, policy, config)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(
            call.module,
            workflow::Scheduler::REGISTER_TIME_CONSTRAINT
                .module
                .to_string()
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::REGISTER_TIME_CONSTRAINT
                .name
                .to_string()
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_pure_bytes(&call.arguments[0], &[11]);
        inspector.expect_pure_bytes(&call.arguments[1], &[12]);
    }

    #[test]
    fn new_time_constraint_config_has_no_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let arg = new_time_constraint_config(&mut tx, &objects).expect("ptb construction succeeds");
        assert_matches!(arg, sui::Argument::Result(0));

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert!(call.arguments.is_empty());
        assert_eq!(
            call.module,
            workflow::Scheduler::NEW_TIME_CONSTRAINT_CONFIG
                .module
                .to_string()
        );
        assert_eq!(
            call.function,
            workflow::Scheduler::NEW_TIME_CONSTRAINT_CONFIG
                .name
                .to_string()
        );
    }

    #[test]
    fn execute_fetches_execution_witness() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let witness = execute(&mut tx, &objects, &task).expect("ptb construction succeeds");
        assert_matches!(witness, sui::Argument::Result(0));

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        assert_eq!(call.module, workflow::Scheduler::EXECUTE.module.to_string());
        assert_eq!(call.function, workflow::Scheduler::EXECUTE.name.to_string());
    }

    #[test]
    fn finish_finalizes_execution_with_proof() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let proof = tx.pure(5u8).expect("proof input");

        finish(&mut tx, &objects, &task, proof).expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.module, workflow::Scheduler::FINISH.module.to_string());
        assert_eq!(call.function, workflow::Scheduler::FINISH.name.to_string());
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_pure_bytes(&call.arguments[1], &[5]);
    }

    #[test]
    fn add_occurrence_absolute_for_task_encodes_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let start_time = 10;
        let deadline = Some(20);
        let gas_price = 30;

        add_occurrence_absolute_for_task(&mut tx, &objects, &task, start_time, deadline, gas_price)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_time);
        inspector.expect_option_u64(&call.arguments[2], deadline);
        inspector.expect_u64(&call.arguments[3], gas_price);
        inspector.expect_clock(&call.arguments[4]);
    }

    #[test]
    fn add_occurrence_with_offset_for_task_encodes_optional_deadline() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let start_time = 100;
        let gas_price = 55;

        add_occurrence_with_offset_for_task(&mut tx, &objects, &task, start_time, None, gas_price)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_time);
        inspector.expect_option_u64(&call.arguments[2], None);
        inspector.expect_u64(&call.arguments[3], gas_price);
        inspector.expect_clock(&call.arguments[4]);
    }

    #[test]
    fn add_occurrence_with_offsets_from_now_for_task_encodes_offsets() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let start_offset = 5;
        let deadline_offset = Some(15);
        let gas_price = 25;

        add_occurrence_with_offsets_from_now_for_task(
            &mut tx,
            &objects,
            &task,
            start_offset,
            deadline_offset,
            gas_price,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], start_offset);
        inspector.expect_option_u64(&call.arguments[2], deadline_offset);
        inspector.expect_u64(&call.arguments[3], gas_price);
        inspector.expect_clock(&call.arguments[4]);
    }

    #[test]
    fn new_or_modify_periodic_for_task_sets_all_arguments() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let period = 1_000;
        let deadline_offset = Some(500);
        let max_iterations = Some(3);
        let gas_price = 75;

        new_or_modify_periodic_for_task(
            &mut tx,
            &objects,
            &task,
            period,
            deadline_offset,
            max_iterations,
            gas_price,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 5);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_u64(&call.arguments[1], period);
        inspector.expect_option_u64(&call.arguments[2], deadline_offset);
        inspector.expect_option_u64(&call.arguments[3], max_iterations);
        inspector.expect_u64(&call.arguments[4], gas_price);
    }

    #[test]
    fn disable_periodic_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        disable_periodic_for_task(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
    }

    #[test]
    fn pause_time_constraint_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        pause_time_constraint_for_task(&mut tx, &objects, &task)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
    }

    #[test]
    fn resume_time_constraint_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        resume_time_constraint_for_task(&mut tx, &objects, &task)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
    }

    #[test]
    fn cancel_time_constraint_for_task_uses_shared_argument() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        cancel_time_constraint_for_task(&mut tx, &objects, &task)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 1);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
    }

    #[test]
    fn check_time_constraint_uses_clock_and_shared_task() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let mut tx = sui::ProgrammableTransactionBuilder::new();

        check_time_constraint(&mut tx, &objects, &task).expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_shared_object(&call.arguments[0], &task, true);
        inspector.expect_clock(&call.arguments[1]);
    }

    #[test]
    fn register_begin_execution_routes_through_default_tap() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let policy = tx.pure(13u8).expect("policy input");
        let config = tx.pure(14u8).expect("config input");

        register_begin_execution(&mut tx, &objects, policy, config)
            .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        let call = inspector.move_call(0);
        assert_eq!(call.package, objects.workflow_pkg_id);
        assert_eq!(
            call.module,
            workflow::DefaultTap::REGISTER_BEGIN_EXECUTION
                .module
                .to_string()
        );
        assert_eq!(
            call.function,
            workflow::DefaultTap::REGISTER_BEGIN_EXECUTION
                .name
                .to_string()
        );
        assert_eq!(call.arguments.len(), 2);
        inspector.expect_pure_bytes(&call.arguments[0], &[13]);
        inspector.expect_pure_bytes(&call.arguments[1], &[14]);
    }

    #[test]
    fn dag_begin_execution_from_scheduler_builds_full_call() {
        let objects = sui_mocks::mock_nexus_objects();
        let task = sui_mocks::mock_sui_object_ref();
        let dag = sui_mocks::mock_sui_object_ref();
        let leader_cap_tuple = {
            let object = sui_mocks::mock_sui_object_ref();
            (object.object_id, object.version, object.digest)
        };
        let claim_coin_tuple = {
            let object = sui_mocks::mock_sui_object_ref();
            (object.object_id, object.version, object.digest)
        };
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let leader_cap = tx
            .obj(sui::ObjectArg::ImmOrOwnedObject(leader_cap_tuple.clone()))
            .expect("leader cap input");
        let claim_coin = tx
            .obj(sui::ObjectArg::ImmOrOwnedObject(claim_coin_tuple.clone()))
            .expect("claim coin input");

        let amount_execution = 44;
        let amount_priority = 55;

        dag_begin_execution_from_scheduler(
            &mut tx,
            &objects,
            &task,
            &dag,
            leader_cap,
            claim_coin,
            amount_execution,
            amount_priority,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 1);
        let call = inspector.move_call(0);
        assert_eq!(call.arguments.len(), 9);

        inspector.expect_shared_object(&call.arguments[0], &objects.default_tap, true);
        inspector.expect_shared_object(&call.arguments[1], &task, true);
        let sui::CallArg::Object(sui::ObjectArg::SharedObject {
            id,
            initial_shared_version,
            mutable,
        }) = inspector.call_arg(&call.arguments[2])
        else {
            panic!(
                "expected shared object argument for DAG, got {:?}",
                inspector.call_arg(&call.arguments[2])
            );
        };
        assert_eq!(*id, dag.object_id);
        assert_eq!(*initial_shared_version, dag.version);
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
        let leader_cap_tuple = {
            let object = sui_mocks::mock_sui_object_ref();
            (object.object_id, object.version, object.digest)
        };
        let claim_coin_tuple = {
            let object = sui_mocks::mock_sui_object_ref();
            (object.object_id, object.version, object.digest)
        };
        let mut tx = sui::ProgrammableTransactionBuilder::new();
        let leader_cap = tx
            .obj(sui::ObjectArg::ImmOrOwnedObject(leader_cap_tuple.clone()))
            .expect("leader cap input");
        let claim_coin = tx
            .obj(sui::ObjectArg::ImmOrOwnedObject(claim_coin_tuple.clone()))
            .expect("claim coin input");

        execute_scheduled_occurrence(
            &mut tx, &objects, &task, &dag, leader_cap, claim_coin, 100, 200,
        )
        .expect("ptb construction succeeds");

        let inspector = TxInspector::new(tx.finish());
        assert_eq!(inspector.commands_len(), 2);

        let scheduler_call = inspector.move_call(0);
        assert_eq!(
            scheduler_call.module,
            workflow::Scheduler::CHECK_TIME_CONSTRAINT
                .module
                .to_string()
        );
        assert_eq!(
            scheduler_call.function,
            workflow::Scheduler::CHECK_TIME_CONSTRAINT.name.to_string()
        );
        assert_eq!(scheduler_call.arguments.len(), 2);
        inspector.expect_shared_object(&scheduler_call.arguments[0], &task, true);
        inspector.expect_clock(&scheduler_call.arguments[1]);

        let tap_call = inspector.move_call(1);
        assert_eq!(
            tap_call.module,
            workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER
                .module
                .to_string()
        );
        assert_eq!(
            tap_call.function,
            workflow::DefaultTap::DAG_BEGIN_EXECUTION_FROM_SCHEDULER
                .name
                .to_string()
        );
        assert_eq!(tap_call.arguments.len(), 9);
        inspector.expect_shared_object(&tap_call.arguments[0], &objects.default_tap, true);
        inspector.expect_shared_object(&tap_call.arguments[1], &task, true);
        let sui::CallArg::Object(sui::ObjectArg::SharedObject {
            id,
            initial_shared_version,
            mutable,
        }) = inspector.call_arg(&tap_call.arguments[2])
        else {
            panic!(
                "expected shared DAG object, got {:?}",
                inspector.call_arg(&tap_call.arguments[2])
            );
        };
        assert_eq!(*id, dag.object_id);
        assert_eq!(*initial_shared_version, dag.version);
        assert!(!*mutable);
        inspector.expect_shared_object(&tap_call.arguments[3], &objects.gas_service, true);
        inspector.expect_owned_object(&tap_call.arguments[4], &leader_cap_tuple);
        inspector.expect_owned_object(&tap_call.arguments[5], &claim_coin_tuple);
        inspector.expect_u64(&tap_call.arguments[6], 100);
        inspector.expect_u64(&tap_call.arguments[7], 200);
        inspector.expect_clock(&tap_call.arguments[8]);
    }
}
