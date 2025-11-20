use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::nexus::scheduler::PeriodicScheduleConfig,
    serde_json::json,
};

/// Configure or update the periodic schedule for a scheduler task.
pub(crate) async fn set_periodic_task(
    task_id: sui::ObjectID,
    first_start_ms: u64,
    period_ms: u64,
    deadline_offset_ms: Option<u64>,
    max_iterations: Option<u64>,
    gas_price: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Configuring periodic schedule for task '{task_id}'",
        task_id = task_id
    );

    let (nexus_client, _) = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    let result = nexus_client
        .scheduler()
        .configure_periodic(
            task_id,
            PeriodicScheduleConfig {
                first_start_ms,
                period_ms,
                deadline_offset_ms,
                max_iterations,
                gas_price,
            },
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Periodic schedule set");

    json_output(&json!({
        "digest": result.tx_digest,
        "task_id": task_id,
        "period_ms": period_ms,
        "deadline_offset_ms": deadline_offset_ms,
        "max_iterations": max_iterations,
        "gas_price": gas_price,
    }))?;

    Ok(())
}
