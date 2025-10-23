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
    nexus_sdk::{idents::workflow as workflow_idents, transactions::dag},
};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    entry_group: String,
    mut input_json: serde_json::Value,
    inspect: bool,
    price_priority_fee: u64,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;

    // Get the active session for potential encryption
    let session = get_active_session(&mut conf)?;

    // Fetch information about entry ports that need to be encrypted.
    let encrypt = workflow::fetch_encrypted_entry_ports(&sui, entry_group.clone(), &dag_id).await?;

    if !encrypt.is_empty() {
        workflow::encrypt_entry_ports_once(session, &mut input_json, &encrypt)?;
    }

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Fetch DAG object for its ObjectRef.
    let dag = fetch_object_by_id(&sui, dag_id).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Err(e) = dag::execute(
        &mut tx,
        objects,
        &dag,
        price_priority_fee,
        &entry_group,
        input_json,
        &encrypt,
    ) {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    }

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and send the TX.
    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    // We need to parse the DAGExecution object ID from the response.
    let dag = response
        .object_changes
        .unwrap_or_default()
        .into_iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.address == *objects.workflow_pkg_id
                && object_type.module == workflow_idents::Dag::DAG_EXECUTION.module.into()
                && object_type.name == workflow_idents::Dag::DAG_EXECUTION.name.into() =>
            {
                Some(object_id)
            }
            _ => None,
        });

    let Some(object_id) = dag else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the DAGExecution object ID in the transaction response."
        )));
    };

    notify_success!(
        "DAGExecution object ID: {id}",
        id = object_id.to_string().truecolor(100, 100, 100)
    );

    if inspect {
        inspect_dag_execution(object_id, response.digest).await?;
    } else {
        json_output(&json!({ "digest": response.digest, "execution_id": object_id }))?;
    }

    // Always save the updated config
    conf.save().await.map_err(NexusCliError::Any)?;

    Ok(())
}

/// Gets the active session for encryption/decryption
fn get_active_session(
    conf: &mut CliConf,
) -> Result<&mut nexus_sdk::crypto::session::Session, NexusCliError> {
    match &mut conf.crypto {
        Some(crypto_secret) => {
            if crypto_secret.sessions.is_empty() {
                return Err(NexusCliError::Any(anyhow!(
                    "Authentication required — run `nexus crypto auth` first"
                )));
            }

            let session_id = *crypto_secret.sessions.values().next().unwrap().id();
            crypto_secret
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| NexusCliError::Any(anyhow!("Session not found in config")))
        }
        None => Err(NexusCliError::Any(anyhow!(
            "Authentication required — run `nexus crypto auth` first"
        ))),
    }
}
