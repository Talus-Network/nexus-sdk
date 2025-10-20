use crate::{
    idents::{sui_framework, workflow},
    sui,
    types::NexusObjects,
};

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
    let string_type = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *sui::MOVE_STDLIB_PACKAGE_ID,
        module: sui::move_ident_str!("string").into(),
        name: sui::move_ident_str!("String").into(),
        type_params: vec![],
    }));

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
            vec![metadata.clone(), key, value],
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
        workflow::DefaultTAP::REGISTER_BEGIN_EXECUTION.module.into(),
        workflow::DefaultTAP::REGISTER_BEGIN_EXECUTION.name.into(),
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
        workflow::DefaultTAP::DAG_BEGIN_EXECUTION_FROM_SCHEDULER
            .module
            .into(),
        workflow::DefaultTAP::DAG_BEGIN_EXECUTION_FROM_SCHEDULER
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
