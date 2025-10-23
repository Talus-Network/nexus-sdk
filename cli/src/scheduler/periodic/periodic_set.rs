use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
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
pub(crate) struct SetArgs {
    /// Task object ID.
    #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
    task_id: sui::ObjectID,
    /// Period between occurrences in milliseconds.
    #[arg(long = "period-ms", value_name = "MILLIS")]
    period_ms: u64,
    /// Deadline offset from each start time in milliseconds.
    #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
    deadline_offset_ms: Option<u64>,
    /// Maximum number of generated occurrences (None for infinite).
    #[arg(long = "max-iterations", value_name = "COUNT")]
    max_iterations: Option<u64>,
    /// Gas price associated with occurrences.
    #[arg(long = "gas-price", value_name = "AMOUNT", default_value_t = 0u64)]
    gas_price: u64,
    #[command(flatten)]
    gas: GasArgs,
}

/// Configure or update the periodic schedule for a scheduler task.
pub(crate) async fn set_periodic(args: SetArgs) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Configuring periodic schedule for task '{task_id}'",
        task_id = args.task_id
    );

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();
    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;
    let objects = &get_nexus_objects(&mut conf).await?;

    // Fetch the target task object.
    let task = fetch_one::<Structure<Task>>(&sui, args.task_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    // Craft the periodic schedule configuration transaction.
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    scheduler_tx::new_or_modify_periodic_for_task(
        &mut tx,
        objects,
        &task.object_ref(),
        args.period_ms,
        args.deadline_offset_ms,
        args.max_iterations,
        args.gas_price,
    )
    .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    // Fetch gas coin and reference gas price.
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

    notify_success!("Periodic schedule set");

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json!({
        "digest": response.digest,
        "task_id": args.task_id,
        "period_ms": args.period_ms,
        "deadline_offset_ms": args.deadline_offset_ms,
        "max_iterations": args.max_iterations,
        "gas_price": args.gas_price,
    }))?;

    Ok(())
}
