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
        sui::{get_nexus_client, resolve_creator_package},
    },
    nexus_sdk::types::PackageRole,
};

pub(crate) async fn run(
    scheduler_package: Option<sui::types::Address>,
    task: TaskArgs,
    schedule: ScheduleArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating and scheduling Task");
    let task = task.into_preparation().await?;
    let schedule = schedule.into_schedule().await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let scheduler_package =
        resolve_creator_package(&client, scheduler_package, PackageRole::Scheduler).await?;
    let task = task.materialize(&client, scheduler_package).await?;
    let progress = loading!("Submitting atomic Task schedule transaction...");
    let receipt = client
        .scheduler()
        .schedule_task(scheduler_package, task, schedule)
        .await?;
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
