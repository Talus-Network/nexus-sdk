use {
    crate::{
        command_title, display::json_output, item, loading, notify_success, prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::sui,
    num_format::{Locale, ToFormattedString},
};

pub(crate) async fn execution_cost(
    dag_execution_id: sui::types::Address,
) -> AnyResult<(), NexusCliError> {
    command_title!("Fetching DAG execution payment '{dag_execution_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let fetch_handle = loading!("Fetching execution payment from Sui...");

    let result = match nexus_client
        .workflow()
        .execution_cost(dag_execution_id)
        .await
        .map_err(NexusCliError::Nexus)
    {
        Ok(cost) => cost,
        Err(e) => {
            fetch_handle.error();

            return Err(e);
        }
    };

    fetch_handle.success();

    item!(
        "Payment object: {}",
        result.payment_id.to_string().truecolor(100, 100, 100)
    );
    item!(
        "Consumed: {}",
        format!("{} MIST", result.consumed.to_formatted_string(&Locale::en))
            .truecolor(100, 100, 100)
    );
    item!(
        "Budget: {} locked from max {}",
        format!(
            "{} MIST",
            result.locked_budget.to_formatted_string(&Locale::en)
        )
        .truecolor(100, 100, 100),
        format!(
            "{} MIST",
            result.max_budget.to_formatted_string(&Locale::en)
        )
        .truecolor(100, 100, 100)
    );
    item!("Outstanding tool locks: {}", result.outstanding_locks);

    notify_success!(
        "Execution payment consumed: {}",
        format!("{} MIST", result.consumed.to_formatted_string(&Locale::en))
            .truecolor(100, 255, 100),
    );

    json_output(&serde_json::json!({
        "payment_id": result.payment_id,
        "max_budget": result.max_budget,
        "locked_budget": result.locked_budget,
        "consumed": result.consumed,
        "outstanding_locks": result.outstanding_locks,
        "accomplished": result.accomplished,
        "refunded": result.refunded,
    }))?;

    Ok(())
}
