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
pub(crate) struct AddArgs {
    /// Task object ID receiving the occurrence.
    #[arg(long = "task-id", short = 't', value_name = "OBJECT_ID")]
    task_id: sui::ObjectID,
    /// Absolute start time in milliseconds since epoch.
    #[arg(long = "start-ms", value_name = "MILLIS")]
    start_ms: Option<u64>,
    /// Absolute deadline time in milliseconds since epoch.
    #[arg(long = "deadline-ms", value_name = "MILLIS")]
    deadline_ms: Option<u64>,
    /// Start offset in milliseconds from now.
    #[arg(long = "start-offset-ms", value_name = "MILLIS")]
    start_offset_ms: Option<u64>,
    /// Deadline offset in milliseconds after the scheduled start.
    #[arg(long = "deadline-offset-ms", value_name = "MILLIS")]
    deadline_offset_ms: Option<u64>,
    /// Gas price paid as priority fee associated with the occurrence.
    #[arg(long = "gas-price", value_name = "AMOUNT", default_value_t = 0u64)]
    gas_price: u64,
    #[command(flatten)]
    gas: GasArgs,
}

/// Schedule a one-off occurrence for a scheduler task.
pub(crate) async fn add_occurrence(args: AddArgs) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scheduling occurrence for task '{task_id}'",
        task_id = args.task_id
    );

    // Validate schedule options.
    helpers::validate_schedule_options(
        args.start_ms,
        args.deadline_ms,
        args.start_offset_ms,
        args.deadline_offset_ms,
        true,
    )?;

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

    // Craft the occurrence scheduling transaction.
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Some(start) = args.start_ms {
        // Use absolute start and deadline timestamps.
        scheduler_tx::add_occurrence_absolute_for_task(
            &mut tx,
            objects,
            &task.object_ref(),
            start,
            args.deadline_ms,
            args.gas_price,
        )
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    } else {
        // Use offsets relative to the current clock.
        scheduler_tx::add_occurrence_with_offsets_from_now_for_task(
            &mut tx,
            objects,
            &task.object_ref(),
            args.start_offset_ms.expect("validated above"),
            args.deadline_offset_ms,
            args.gas_price,
        )
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    }

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

    notify_success!("Occurrence scheduled");

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json!({
        "digest": response.digest,
        "task_id": args.task_id,
        "start_ms": args.start_ms,
        "deadline_ms": args.deadline_ms,
        "start_offset_ms": args.start_offset_ms,
        "deadline_offset_ms": args.deadline_offset_ms,
        "gas_price": args.gas_price,
    }))?;

    Ok(())
}
