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

/// Schedule a one-off occurrence for a scheduler task.
pub(crate) async fn add_occurrence_to_task(
    task_id: sui::ObjectID,
    start_ms: Option<u64>,
    deadline_ms: Option<u64>,
    start_offset_ms: Option<u64>,
    deadline_offset_ms: Option<u64>,
    gas_price: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scheduling occurrence for task '{task_id}'",
        task_id = task_id
    );

    // Validate schedule options.
    helpers::validate_schedule_options(
        start_ms,
        deadline_ms,
        start_offset_ms,
        deadline_offset_ms,
        true,
    )?;

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();
    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;
    let objects = &get_nexus_objects(&mut conf).await?;
    let GasArgs {
        sui_gas_coin,
        sui_gas_budget,
    } = gas;

    // Fetch the target task object.
    let task = fetch_one::<Structure<Task>>(&sui, task_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    // Craft the occurrence scheduling transaction.
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Some(start) = start_ms {
        // Use absolute start and deadline timestamps.
        scheduler_tx::add_occurrence_absolute_for_task(
            &mut tx,
            objects,
            &task.object_ref(),
            start,
            deadline_ms,
            gas_price,
        )
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    } else {
        // Use offsets relative to the current clock.
        scheduler_tx::add_occurrence_with_offsets_from_now_for_task(
            &mut tx,
            objects,
            &task.object_ref(),
            start_offset_ms.expect("validated above"),
            deadline_offset_ms,
            gas_price,
        )
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    }

    // Fetch gas coin and reference gas price.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    let response = sign_and_execute_transaction(&sui, &wallet, tx_data).await?;

    notify_success!("Occurrence scheduled");

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json!({
        "digest": response.digest,
        "task_id": task_id,
        "start_ms": start_ms,
        "deadline_ms": deadline_ms,
        "start_offset_ms": start_offset_ms,
        "deadline_offset_ms": deadline_offset_ms,
        "gas_price": gas_price,
    }))?;

    Ok(())
}
