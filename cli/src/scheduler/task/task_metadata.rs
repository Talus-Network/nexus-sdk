use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        scheduler::helpers,
        sui::{
            build_sui_client,
            create_wallet_context,
            fetch_gas_coin,
            fetch_reference_gas_price,
            get_nexus_objects,
            sign_and_execute_transaction,
        },
    },
    nexus_sdk::{
        object_crawler::{fetch_one, Structure},
        transactions::scheduler as scheduler_tx,
        types::Task,
    },
    serde_json::json,
};

#[derive(Args, Debug)]
pub(crate) struct MetadataArgs {
    /// Task object ID to update.
    #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
    pub(crate) task_id: sui::ObjectID,
    /// Metadata entries to write as key=value pairs.
    #[arg(
        long = "metadata",
        short = 'm',
        value_name = "KEY=VALUE",
        required = true
    )]
    pub(crate) metadata: Vec<String>,
    #[command(flatten)]
    pub(crate) gas: GasArgs,
}

/// Update all metadata entries for a scheduler task.
pub(crate) async fn update_metadata(args: MetadataArgs) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Updating scheduler metadata for task '{task_id}'",
        task_id = args.task_id
    );

    // Load CLI configuration and parse metadata inputs.
    let mut conf = CliConf::load().await.unwrap_or_default();
    let metadata_pairs = helpers::parse_metadata(&args.metadata)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;
    let objects = &get_nexus_objects(&mut conf).await?;

    // Fetch the task object targeted for the update.
    let task = fetch_one::<Structure<Task>>(&sui, args.task_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    // Craft the metadata update transaction.
    let mut tx = sui::ProgrammableTransactionBuilder::new();
    let metadata_arg = helpers::metadata_argument(&mut tx, objects, &metadata_pairs)?;

    scheduler_tx::update_metadata(&mut tx, objects, &task.object_ref(), metadata_arg)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    let gas_coin = fetch_gas_coin(&sui, address, args.gas.sui_gas_coin).await?;
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        args.gas.sui_gas_budget,
        reference_gas_price,
    );

    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    notify_success!(
        "Metadata updated for task {task_id}",
        task_id = args.task_id.to_string().truecolor(100, 100, 100)
    );

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json!({
        "digest": response.digest,
        "task_id": args.task_id,
        "metadata_entries": metadata_pairs.len(),
    }))?;

    Ok(())
}
