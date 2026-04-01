use {
    crate::{
        command_title,
        display::json_output,
        item,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::sui,
    num_format::{Locale, ToFormattedString},
};

/// Inspect a Nexus DAG execution process based on the provided object ID and
/// execution digest.
pub(crate) async fn execution_cost(
    dag_execution_id: sui::types::Address,
) -> AnyResult<(), NexusCliError> {
    command_title!("Calculating DAG execution cost '{dag_execution_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;

    let fetch_handle = loading!("Fetching execution claims from Sui...");

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

    let mut total = 0;

    for (digest, claim) in &result.leader_claims {
        item!(
            "Claimed {} for execution and {} for priority at digest '{}'",
            format!("{} MIST", claim.execution.to_formatted_string(&Locale::en))
                .truecolor(100, 100, 100),
            format!("{} MIST", claim.priority.to_formatted_string(&Locale::en))
                .truecolor(100, 100, 100),
            sui::types::Digest::from_bytes(digest.as_slice())
                .unwrap_or(sui::types::Digest::ZERO)
                .to_string()
                .truecolor(100, 100, 100),
        );

        total += claim.execution + claim.priority;
    }

    notify_success!(
        "Total claimed for execution and priority: {}",
        format!("{} MIST", total.to_formatted_string(&Locale::en)).truecolor(100, 255, 100),
    );

    json_output(
        &result.leader_claims.iter()
            .map(|(digest, claim)| {
                serde_json::json!({
                    "digest": sui::types::Digest::from_bytes(digest.as_slice()).unwrap_or(sui::types::Digest::ZERO).to_string(),
                    "execution": claim.execution,
                    "priority": claim.priority,
                })
            })
            .collect::<Vec<_>>(),
    )?;

    Ok(())
}
