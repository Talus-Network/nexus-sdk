use crate::{
    idents::workflow,
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

/// PTB template to enqueue a new occurrence with absolute deadline.
pub fn add_occurrence_absolute_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    start_time: u64,
    deadline: Option<u64>,
    gas_price: u64,
) -> anyhow::Result<sui::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_time: u64`
    let start_time = tx.pure(start_time)?;

    // `deadline: option::Option<u64>`
    let deadline = tx.pure(deadline)?;

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
        vec![task, start_time, deadline, gas_price, clock],
    ))
}

/// PTB template to enqueue a new occurrence with deadline offset.
pub fn add_occurrence_with_offset_for_task(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    start_time: u64,
    deadline_offset_ms: Option<u64>,
    gas_price: u64,
) -> anyhow::Result<sui::Argument> {
    // `task: &mut Task`
    let task = shared_task_arg(tx, task)?;

    // `start_time: u64`
    let start_time = tx.pure(start_time)?;

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
        vec![task, start_time, deadline_offset_ms, gas_price, clock],
    ))
}

/// PTB template to update periodic schedule parameters.
pub fn modify_periodic_for_task(
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
        workflow::Scheduler::MODIFY_PERIODIC_FOR_TASK.module.into(),
        workflow::Scheduler::MODIFY_PERIODIC_FOR_TASK.name.into(),
        vec![],
        vec![task, period_ms, deadline_offset_ms, max_iterations, gas_price],
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

/// PTB template to invoke DAG execution from the scheduler via the Default TAP.
pub fn dag_begin_execution_from_scheduler(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    dag: &sui::ObjectRef,
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
        vec![tap, task, dag, gas_service, clock],
    ))
}

/// PTB helper that consumes the next scheduled occurrence and invokes the TAP.
pub fn execute_scheduled_occurrence(
    tx: &mut sui::ProgrammableTransactionBuilder,
    objects: &NexusObjects,
    task: &sui::ObjectRef,
    dag: &sui::ObjectRef,
) -> anyhow::Result<()> {
    check_time_constraint(tx, objects, task)?;
    dag_begin_execution_from_scheduler(tx, objects, task, dag)?;

    Ok(())
}
