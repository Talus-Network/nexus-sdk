use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::nexus::scheduler::OccurrenceRequest,
    serde_json::json,
};

/// Schedule a one-off occurrence for a scheduler task.
pub(crate) async fn add_occurrence_to_task(
    task_id: sui::ObjectID,
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    gas_price: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scheduling occurrence for task '{task_id}'",
        task_id = task_id
    );

    let schedule = OccurrenceRequest::new(
        start_ms,
        deadline_ms,
        start_offset_ms,
        deadline_offset_ms,
        gas_price,
        true,
    )
    .map_err(NexusCliError::Nexus)?;

    let (nexus_client, _) = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    let result = nexus_client
        .scheduler()
        .add_occurrence(task_id, schedule)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Occurrence scheduled");

    json_output(&json!({
        "digest": result.tx_digest,
        "task_id": task_id,
        "start_ms": start_ms,
        "deadline_ms": deadline_ms,
        "start_offset_ms": start_offset_ms,
        "deadline_offset_ms": deadline_offset_ms,
        "gas_price": gas_price,
    }))?;

    Ok(())
}
