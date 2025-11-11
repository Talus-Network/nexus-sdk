use {
    crate::{
        command_title,
        display::json_output,
        loading,
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
        workflow,
    },
    nexus_sdk::{
        events::{NexusEvent, NexusEventKind},
        idents::workflow as workflow_idents,
        object_crawler::{fetch_one, Structure},
        sui::{self, move_ident_str},
        transactions::scheduler as scheduler_tx,
        types::{StorageConf, Task},
    },
    std::{collections::HashMap, sync::Arc},
};

/// Create a scheduler task and optionally enqueue the initial occurrence.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_task(
    dag_id: sui::ObjectID,
    entry_group: String,
    mut input_json: Option<serde_json::Value>,
    remote: Vec<String>,
    metadata: Vec<String>,
    execution_gas_price: u64,
    schedule_start_ms: Option<u64>,
    schedule_deadline_ms: Option<u64>,
    schedule_start_offset_ms: Option<u64>,
    schedule_deadline_offset_ms: Option<u64>,
    schedule_gas_price: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Creating scheduler task for DAG '{dag_id}'",
        dag_id = dag_id
    );

    // Validate schedule options provided via CLI flags.
    helpers::validate_schedule_options(
        schedule_start_ms,
        schedule_deadline_ms,
        schedule_start_offset_ms,
        schedule_deadline_offset_ms,
        false,
    )?;

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;
    let GasArgs {
        sui_gas_coin,
        sui_gas_budget,
    } = gas;

    // Parse metadata arguments and prepare the DAG input payload.
    let metadata_pairs = helpers::parse_metadata(&metadata)?;
    let input_json = input_json.take().unwrap_or_else(|| serde_json::json!({}));

    // Fetch encrypted entry ports.
    let encrypt_handles = helpers::fetch_encryption_targets(&sui, &dag_id, &entry_group).await?;

    // Build the remote storage configuration.
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf: StorageConf = conf.data_storage.clone().into();

    // Acquire a session for potential encryption/remote storage commits.
    let session = CryptoConf::get_active_session(None).await.map_err(|e| {
        NexusCliError::Any(anyhow!(
            "Failed to get active session: {}.\nPlease initiate a session first.\n\n{init_key}\n{crypto_auth}",
            e,
            init_key = "$ nexus crypto init-key --force",
            crypto_auth = "$ nexus crypto auth"
        ))
    })?;

    let ports_data = workflow::process_entry_ports(
        &input_json,
        preferred_remote_storage,
        &encrypt_handles,
        &remote,
    )
    .await?;

    let mut input_data = HashMap::new();
    for (vertex, data) in ports_data {
        let committed = data
            .commit_all(&storage_conf, Arc::clone(&session))
            .await
            .map_err(|e| {
                NexusCliError::Any(anyhow!(
                    "Failed to store data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                    e = e,
                    command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                    testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
                ))
            })?;
        input_data.insert(vertex, committed);
    }

    if encrypt_handles.values().any(|ports| !ports.is_empty()) {
        session.lock().await.commit_sender(None);
    }

    CryptoConf::release_session(session, None)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!("Failed to release session: {}", e)))?;

    // Craft the task creation transaction.
    let tx_handle = loading!("Crafting transaction...");
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    let metadata_arg = scheduler_tx::new_metadata(&mut tx, objects, metadata_pairs.iter().cloned())
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    let constraints_arg = scheduler_tx::new_constraints_policy(&mut tx, objects)
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    let execution_arg = scheduler_tx::new_execution_policy(
        &mut tx,
        objects,
        dag_id,
        execution_gas_price,
        entry_group.as_str(),
        &input_data,
    )
    .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    let task = scheduler_tx::new_task(
        &mut tx,
        objects,
        metadata_arg,
        constraints_arg,
        execution_arg,
    )
    .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    let task_type =
        workflow_idents::into_type_tag(objects.workflow_pkg_id, workflow_idents::Scheduler::TASK);
    tx.programmable_move_call(
        sui::FRAMEWORK_PACKAGE_ID,
        move_ident_str!("transfer").into(),
        move_ident_str!("public_share_object").into(),
        vec![task_type],
        vec![task],
    );

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

    let mut created_task: Option<sui::ObjectID> = None;
    // Parse the TaskCreated event to discover the task ID.
    if let Some(events) = response.events.as_ref() {
        for raw_event in &events.data {
            let Ok(event): Result<NexusEvent, _> = raw_event.clone().try_into() else {
                continue;
            };
            if let NexusEventKind::TaskCreated(created) = event.data {
                created_task = Some(created.task);
                break;
            }
        }
    }

    let task_id = created_task.ok_or_else(|| {
        NexusCliError::Any(anyhow!(
            "TaskCreatedEvent not found in transaction response"
        ))
    })?;

    tx_handle.success();

    let mut result_json = serde_json::json!({
        "digest": response.digest,
        "task_id": task_id,
    });

    let mut scheduled_event: Option<serde_json::Value> = None;

    // Optionally schedule the first occurrence for the new task.
    if schedule_start_ms.is_some() || schedule_start_offset_ms.is_some() {
        let schedule_handle = loading!("Scheduling initial occurrence...");
        // Load the newly created task as a shared object.
        let task = fetch_one::<Structure<Task>>(&sui, task_id)
            .await
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

        let mut schedule_tx = sui::ProgrammableTransactionBuilder::new();

        if let Some(start_ms) = schedule_start_ms {
            scheduler_tx::add_occurrence_absolute_for_task(
                &mut schedule_tx,
                objects,
                &task.object_ref(),
                start_ms,
                schedule_deadline_ms,
                schedule_gas_price,
            )
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
        } else {
            let start_offset = schedule_start_offset_ms.expect("validated above");
            scheduler_tx::add_occurrence_with_offsets_from_now_for_task(
                &mut schedule_tx,
                objects,
                &task.object_ref(),
                start_offset,
                schedule_deadline_offset_ms,
                schedule_gas_price,
            )
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
        }

        // Submit the scheduling transaction.
        let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;
        let schedule_tx_data = sui::TransactionData::new_programmable(
            address,
            vec![gas_coin.object_ref()],
            schedule_tx.finish(),
            sui_gas_budget,
            reference_gas_price,
        );
        let schedule_response =
            sign_and_execute_transaction(&sui, &wallet, schedule_tx_data).await?;

        if let Some(events) = schedule_response.events.as_ref() {
            for raw_event in &events.data {
                let Ok(event): Result<NexusEvent, _> = raw_event.clone().try_into() else {
                    continue;
                };
                match &event.data {
                    // Handle events wrapped in RequestScheduledExecution envelopes.
                    NexusEventKind::Scheduled(envelope)
                        if matches!(
                            envelope.request.as_ref(),
                            NexusEventKind::OccurrenceScheduled(_)
                        ) =>
                    {
                        scheduled_event = Some(serde_json::to_value(envelope).unwrap_or_default());
                        break;
                    }
                    // Handle plain OccurrenceScheduled events.
                    NexusEventKind::OccurrenceScheduled(_) => {
                        scheduled_event =
                            Some(serde_json::to_value(&event.data).unwrap_or_default());
                        break;
                    }
                    _ => {}
                }
            }
        }

        result_json["schedule_digest"] = serde_json::json!(schedule_response.digest);
        if let Some(envelope) = &scheduled_event {
            result_json["scheduled"] = envelope.clone();
        }

        schedule_handle.success();
    }

    notify_success!(
        "Scheduler task created: {task_id}",
        task_id = task_id.to_string().truecolor(100, 100, 100)
    );

    if let Some(envelope) = scheduled_event {
        notify_success!(
            "Initial occurrence scheduled: {event}",
            event = envelope
                .get("start_ms")
                .and_then(|v| v.as_u64())
                .map(|start| format!("start_ms={start}"))
                .unwrap_or_else(|| "see JSON output".to_string())
                .truecolor(100, 100, 100)
        );
    }

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&result_json)?;

    Ok(())
}
