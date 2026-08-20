use {
    super::{args::TaskArgs, output},
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
    scheduler_package: sui::types::Address,
    task: TaskArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating empty Task");
    let task = task.into_preparation().await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let task = task.materialize(&client, scheduler_package).await?;
    let progress = loading!("Submitting Task creation transaction...");
    let receipt = client
        .scheduler()
        .create_task(scheduler_package, task)
        .await?;
    progress.success();

    notify_success!(
        "Task created: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    human_output(&output::render_task_receipt(&receipt, None));
    json_output(&receipt)
}
