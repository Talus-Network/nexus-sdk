use crate::{
    command_title,
    dag::dag_validate::validate_dag,
    display::json_output,
    loading,
    notify_success,
    prelude::*,
    sui::*,
};

/// Publish the provided Nexus DAG to the currently active Sui net. This also
/// performs validation on the DAG before publishing.
pub(crate) async fn publish_dag(
    path: PathBuf,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let dag = validate_dag(path).await?;

    command_title!("Publishing Nexus DAG");

    let (nexus_client, _) = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    let tx_handle = loading!("Crafting and executing transaction...");

    let response = match nexus_client.workflow().publish(dag).await {
        Ok(res) => res,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Nexus(e));
        }
    };

    tx_handle.success();

    notify_success!(
        "Published DAG with object ID: {id}",
        id = response.dag_object_id.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.tx_digest, "dag_id": response.dag_object_id }))?;

    Ok(())
}
