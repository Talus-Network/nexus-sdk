use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        scheduler::helpers,
        sui::get_nexus_client,
    },
    nexus_sdk::nexus::scheduler::RecurrenceSpec,
    serde_json::json,
};

pub(crate) struct SetRecurrenceOptions {
    pub(crate) task_id: sui::types::Address,
    pub(crate) first_start_ms: Option<u64>,
    pub(crate) first_start_offset_ms: Option<u64>,
    pub(crate) interval_ms: u64,
    pub(crate) deadline_offset_ms: Option<u64>,
    pub(crate) occurrences: Option<u64>,
    pub(crate) priority_fee_percentage: Option<u64>,
    pub(crate) gas: GasArgs,
}

/// Sets the lazy recurrence for a Task.
pub(crate) async fn set_recurrence(options: SetRecurrenceOptions) -> AnyResult<(), NexusCliError> {
    let SetRecurrenceOptions {
        task_id,
        first_start_ms,
        first_start_offset_ms,
        interval_ms,
        deadline_offset_ms,
        occurrences,
        priority_fee_percentage,
        gas,
    } = options;
    command_title!("Setting recurrence for Task '{task_id}'");

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let clock_ms = nexus_client
        .scheduler()
        .clock_timestamp_ms()
        .await
        .map_err(NexusCliError::Nexus)?;
    let first = helpers::occurrence_spec(
        clock_ms,
        first_start_ms,
        first_start_offset_ms,
        deadline_offset_ms,
        priority_fee_percentage,
    )?;
    let result = nexus_client
        .scheduler()
        .set_recurrence(
            task_id,
            RecurrenceSpec {
                first,
                interval_ms,
                occurrences,
            },
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("Task recurrence set");

    json_output(&json!({
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "task_id": task_id,
        "recurrence": {
            "first": first,
            "interval_ms": interval_ms,
            "occurrences": occurrences,
        },
        "advertised": result.advertised,
    }))
}
