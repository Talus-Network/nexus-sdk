use {
    super::{
        args::{ScheduleArgs, TaskArgs},
        output,
    },
    crate::{
        command_title,
        display::{human_output, json_output},
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
};

pub(crate) async fn run(
    task: TaskArgs,
    schedule: ScheduleArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating and scheduling Task");
    let task = task.into_preparation().await?;
    let schedule = schedule.into_schedule().await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let task = task.materialize(&client).await?;
    let progress = loading!("Submitting atomic Task schedule transaction...");
    let receipt = client.scheduler().schedule_task(task, schedule).await?;
    progress.success();

    notify_success!(
        "Task scheduled: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    notify_success!(
        "Allocated occurrences: {count}",
        count = receipt.delta().scheduled().len()
    );
    human_output(&output::render_task_receipt(&receipt, None));
    json_output(&receipt)
}
