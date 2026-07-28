use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::{get_nexus_client, get_read_only_nexus_client},
    },
    nexus_sdk::scheduler::TaskMutationReceipt,
};

pub(crate) async fn inspect(task_id: sui::types::Address) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Task");
    let client = get_read_only_nexus_client().await?;
    let progress = loading!("Reading Task object...");
    let snapshot = client.scheduler().task(task_id).snapshot().await?;
    progress.success();
    json_output(&snapshot)
}

pub(crate) async fn pause(
    task_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Pausing Task");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting Task pause transaction...");
    let receipt = client.scheduler().task(task_id).pause().await?;
    progress.success();
    finish("Task paused", &receipt)
}

pub(crate) async fn resume(
    task_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Resuming Task");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting Task resume transaction...");
    let receipt = client.scheduler().task(task_id).resume().await?;
    progress.success();
    finish("Task resumed", &receipt)
}

pub(crate) async fn cancel(
    task_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Canceling Task");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting Task cancel transaction...");
    let receipt = client.scheduler().task(task_id).cancel().await?;
    progress.success();
    finish("Task canceled", &receipt)
}

pub(crate) async fn refill(
    task_id: sui::types::Address,
    amount_mist: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Refilling Task");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting Task refill transaction...");
    let receipt = client.scheduler().task(task_id).refill(amount_mist).await?;
    progress.success();
    finish("Task refilled", &receipt)
}

pub(crate) async fn close(
    task_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Closing Task");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting Task close transaction...");
    let receipt = client.scheduler().task(task_id).close().await?;
    progress.success();
    finish("Task closed", &receipt)
}

fn finish(message: &str, receipt: &TaskMutationReceipt) -> AnyResult<(), NexusCliError> {
    notify_success!(
        "{message}: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    json_output(receipt)
}
