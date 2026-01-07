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

/// Disable the periodic schedule for a scheduler task.
pub(crate) async fn disable_periodic_task(
    task_id: sui::types::Address,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Disabling periodic schedule for task '{task_id}'",
        task_id = task_id
    );

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    let result = nexus_client
        .scheduler()
        .disable_periodic(task_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Periodic schedule disabled");

    json_output(&json!({
        "digest": result.tx_digest,
        "task_id": task_id,
    }))?;

    Ok(())
}
