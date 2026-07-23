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

/// Adds one manual occurrence to a Task.
pub(crate) async fn add_occurrence_to_task(
    task_id: sui::types::Address,
    start_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    priority_fee_percentage: Option<u64>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Scheduling occurrence for Task '{task_id}'");

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let clock_ms = nexus_client
        .scheduler()
        .clock_timestamp_ms()
        .await
        .map_err(NexusCliError::Nexus)?;
    let occurrence = helpers::occurrence_spec(
        clock_ms,
        start_ms,
        start_offset_ms,
        deadline_offset_ms,
        priority_fee_percentage,
    )?;
    let result = nexus_client
        .scheduler()
        .schedule(task_id, occurrence)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Occurrence scheduled");
    json_output(&json!({
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "task_id": task_id,
        "occurrence": occurrence,
        "advertised": result.advertised,
    }))
}

/// Expires one advertised occurrence after its deadline.
pub(crate) async fn expire_occurrence(
    task_id: sui::types::Address,
    occurrence_id: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Expiring occurrence '{occurrence_id}' for Task '{task_id}'");

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let result = nexus_client
        .scheduler()
        .expire(task_id, occurrence_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Occurrence expired");
    json_output(&json!({
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "task_id": task_id,
        "occurrence_id": occurrence_id,
        "advertised": result.advertised,
    }))
}
