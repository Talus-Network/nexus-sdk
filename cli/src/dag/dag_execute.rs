use {
    crate::{
        command_title,
        dag::dag_inspect_execution::inspect_dag_execution,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
        workflow,
    },
    anyhow::anyhow,
    nexus_sdk::nexus::error::NexusError,
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_dag(
    dag_id: sui::types::Address,
    entry_group: String,
    input_json: serde_json::Value,
    remote: Vec<String>,
    inspect: bool,
    priority_fee_per_gas_unit: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    // Build the remote storage conf.
    let conf = CliConf::load().await.unwrap_or_default();
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf = conf.data_storage.clone().into();

    // Store ports remote if they need to be stored remotely.
    let input_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;

    let tx_handle = loading!("Crafting and executing transaction...");

    let result = match nexus_client
        .workflow()
        .execute(
            dag_id,
            input_data,
            priority_fee_per_gas_unit,
            Some(&entry_group),
            &storage_conf,
        )
        .await
    {
        Ok(r) => r,
        Err(NexusError::Storage(e)) => {
            tx_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "{e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                e = e,
                command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
            )));
        }
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Nexus(e));
        }
    };

    tx_handle.success();

    notify_success!(
        "DAGExecution object ID: {id}",
        id = result
            .execution_object_id
            .to_string()
            .truecolor(100, 100, 100)
    );

    notify_success!(
        "DAGExecution checkpoint: {id}",
        id = result.tx_checkpoint.to_string().truecolor(100, 100, 100)
    );

    if inspect {
        inspect_dag_execution(result.execution_object_id, result.tx_checkpoint).await?;
    } else {
        json_output(&json!({
            "execution_id": result.execution_object_id,
            "digest": result.tx_digest,
            "tx_checkpoint": result.tx_checkpoint
        }))?;
    }

    Ok(())
}
