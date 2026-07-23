use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    serde_json::json,
};

/// Clears future recurring work from a Task.
pub(crate) async fn clear_recurrence(
    task_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Clearing recurrence for Task '{task_id}'",
        task_id = task_id
    );

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    let result = nexus_client
        .scheduler()
        .clear_recurrence(task_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Task recurrence cleared");

    json_output(&json!({
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "task_id": task_id,
        "advertised": result.advertised,
    }))
}
