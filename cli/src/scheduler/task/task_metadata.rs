use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        scheduler::helpers,
        sui::get_nexus_client,
    },
    serde_json::json,
};

/// Update all metadata entries for a scheduler task.
pub(crate) async fn update_task_metadata(
    task_id: sui::types::Address,
    metadata: Vec<String>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Updating scheduler metadata for task '{task_id}'",
        task_id = task_id
    );

    let metadata_pairs = helpers::parse_metadata(&metadata)?;

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    let result = nexus_client
        .scheduler()
        .update_metadata(task_id, metadata_pairs.clone())
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Metadata updated for task {task_id}",
        task_id = task_id.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({
        "digest": result.tx_digest,
        "task_id": task_id,
        "metadata_entries": metadata_pairs.len(),
    }))?;

    Ok(())
}
