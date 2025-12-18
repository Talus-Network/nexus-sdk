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
    nexus_sdk::{nexus::error::NexusError, types::EncryptionMode},
    std::sync::Arc,
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

    // Get the active session for potential encryption
    let session = CryptoConf::get_active_session(None).await.map_err(|e|
        NexusCliError::Any(
            anyhow!(
                "Failed to get active session: {}.\nPlease initiate a session first.\n\n{init_key}\n{crypto_auth}",
                e,
                init_key = "$ nexus crypto init-key --force",
                crypto_auth = "$ nexus crypto auth"
            )
        )
    )?;

    // Fetch information about entry ports that need to be encrypted.
    let encrypt =
        workflow::fetch_encrypted_entry_ports(nexus_client.crawler(), entry_group.clone(), &dag_id)
            .await?;

    // Encrypt ports that need to be encrypted and store ports remote if they
    // need to be stored remotely.
    let input_data = workflow::process_entry_ports(
        &input_json,
        preferred_remote_storage,
        &encrypt,
        &remote,
        EncryptionMode::Standard,
    )
    .await?;

    let tx_handle = loading!("Crafting and executing transaction...");

    let result = match nexus_client
        .workflow()
        .execute(
            dag_id,
            input_data,
            priority_fee_per_gas_unit,
            Some(&entry_group),
            &storage_conf,
            Arc::clone(&session),
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

    // Advance the session ratchet if encryption was used.
    if !encrypt.is_empty() {
        session.lock().await.commit_sender(None);
    }

    tx_handle.success();

    notify_success!(
        "DAGExecution object ID: {id}",
        id = result
            .execution_object_id
            .to_string()
            .truecolor(100, 100, 100)
    );

    // Update the session in the configuration.
    CryptoConf::release_session(session, None)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!("Failed to release session: {e}")))?;

    if inspect {
        inspect_dag_execution(result.execution_object_id, result.tx_checkpoint).await?;
    } else {
        json_output(
            &json!({ "digest": result.tx_digest, "execution_id": result.execution_object_id }),
        )?;
    }

    Ok(())
}
