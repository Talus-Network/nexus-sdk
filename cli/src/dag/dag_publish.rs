use {
    crate::{
        command_title,
        dag::dag_validate::validate_dag,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{idents::workflow, transactions::dag},
};

/// Publish the provided Nexus DAG to the currently active Sui net. This also
/// performs validation on the DAG before publishing.
pub(crate) async fn publish_dag(
    path: PathBuf,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let dag = validate_dag(path).await?;

    command_title!("Publishing Nexus DAG");

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft a TX to publish the DAG.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // Create an empty DAG.
    let mut dag_arg = dag::empty(&mut tx, objects);

    // Create DAG PTB from Dag struct.
    dag_arg = match dag::create(&mut tx, objects, dag_arg, dag) {
        Ok(dag_arg) => dag_arg,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    // Public share the DAG, locking it.
    dag::publish(&mut tx, objects, dag_arg);

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign the transaction and send it to the network.
    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    // We need to parse the DAG object ID from the response.
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
                && object_type.module == workflow::Dag::DAG.module.into()
                && object_type.name == workflow::Dag::DAG.name.into() =>
            {
                Some(object_id)
            }
            _ => None,
        });

    let Some(object_id) = dag else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the DAG object ID in the transaction response."
        )));
    };

    notify_success!(
        "Published DAG with object ID: {id}",
        id = object_id.to_string().truecolor(100, 100, 100)
    );

    json_output(&json!({ "digest": response.digest, "dag_id": object_id }))?;

    Ok(())
}
