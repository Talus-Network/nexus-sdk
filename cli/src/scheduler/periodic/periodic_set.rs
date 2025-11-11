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

/// Configure or update the periodic schedule for a scheduler task.
pub(crate) async fn set_periodic_task(
    task_id: sui::ObjectID,
    period_ms: u64,
    deadline_offset_ms: Option<u64>,
    max_iterations: Option<u64>,
    gas_price: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Configuring periodic schedule for task '{task_id}'",
        task_id = task_id
    );

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

    // Craft the periodic schedule configuration transaction.
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    scheduler_tx::new_or_modify_periodic_for_task(
        &mut tx,
        objects,
        &task.object_ref(),
        period_ms,
        deadline_offset_ms,
        max_iterations,
        gas_price,
    )
    .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

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

    notify_success!("Periodic schedule set");

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json!({
        "digest": response.digest,
        "task_id": task_id,
        "period_ms": period_ms,
        "deadline_offset_ms": deadline_offset_ms,
        "max_iterations": max_iterations,
        "gas_price": gas_price,
    }))?;

    Ok(())
}
