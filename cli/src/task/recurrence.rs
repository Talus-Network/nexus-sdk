use {
    super::{args::RecurrenceArgs, RecurrenceCommand},
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
};

pub(crate) async fn handle(command: RecurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        RecurrenceCommand::Set {
            task_id,
            recurrence,
            gas,
        } => set(task_id, recurrence, gas).await,
        RecurrenceCommand::Clear { task_id, gas } => clear(task_id, gas).await,
    }
}

async fn set(
    task_id: sui::types::Address,
    recurrence: RecurrenceArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Setting recurrence");
    let recurrence = recurrence.into_recurrence()?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting recurrence transaction...");
    let receipt = client
        .scheduler()
        .task(task_id)
        .set_recurrence(recurrence)
        .await?;
    progress.success();
    notify_success!(
        "Recurrence set for Task: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    json_output(&receipt)
}

async fn clear(task_id: sui::types::Address, gas: GasArgs) -> AnyResult<(), NexusCliError> {
    command_title!("Clearing recurrence");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting recurrence clear transaction...");
    let receipt = client.scheduler().task(task_id).clear_recurrence().await?;
    progress.success();
    notify_success!(
        "Recurrence cleared for Task: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    json_output(&receipt)
}
