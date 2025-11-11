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

#[derive(Clone, Copy, Debug)]
pub(crate) enum TaskStateRequest {
    Pause,
    Resume,
    Cancel,
}

impl std::fmt::Display for TaskStateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let verb = match self {
            TaskStateRequest::Pause => "Pausing",
            TaskStateRequest::Resume => "Resuming",
            TaskStateRequest::Cancel => "Canceling",
        };
        write!(f, "{verb}")
    }
}

/// Toggle scheduler task state between paused, resumed, or canceled.
pub(crate) async fn set_task_state(
    task_id: sui::ObjectID,
    gas: GasArgs,
    request: TaskStateRequest,
) -> AnyResult<(), NexusCliError> {
    command_title!("{request} scheduler task '{task_id}'");

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

    // Craft the appropriate state transition transaction.
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    match request {
        TaskStateRequest::Pause => {
            scheduler_tx::pause_time_constraint_for_task(&mut tx, objects, &task.object_ref())
        }
        TaskStateRequest::Resume => {
            scheduler_tx::resume_time_constraint_for_task(&mut tx, objects, &task.object_ref())
        }
        TaskStateRequest::Cancel => {
            scheduler_tx::cancel_time_constraint_for_task(&mut tx, objects, &task.object_ref())
        }
    }
    .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

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

    match request {
        TaskStateRequest::Pause => notify_success!("Task paused"),
        TaskStateRequest::Resume => notify_success!("Task resumed"),
        TaskStateRequest::Cancel => notify_success!("Task canceled"),
    }

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&json!({
        "digest": response.digest,
        "task_id": task_id,
        "state": match request {
            TaskStateRequest::Pause => "paused",
            TaskStateRequest::Resume => "resumed",
            TaskStateRequest::Cancel => "canceled",
        },
    }))?;

    Ok(())
}
