use {
    super::args::TaskArgs,
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
};

pub(crate) async fn run(task: TaskArgs, gas: GasArgs) -> AnyResult<(), NexusCliError> {
    command_title!("Creating empty Task");
    let task = task.into_spec().await?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting Task creation transaction...");
    let receipt = client.scheduler().create_task(task).await?;
    progress.success();

    notify_success!(
        "Task created: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    json_output(&receipt)
}
