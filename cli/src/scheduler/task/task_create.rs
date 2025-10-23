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
    },
    nexus_sdk::{
        events::{NexusEvent, NexusEventKind},
        idents::workflow as workflow_idents,
        object_crawler::{fetch_one, Structure},
        sui::{self, move_ident_str},
        transactions::scheduler as scheduler_tx,
        types::{Task, DEFAULT_ENTRY_GROUP},
    },
};

#[derive(Args, Debug)]
pub(crate) struct CreateArgs {
    /// DAG object ID providing the execution definition.
    #[arg(long = "dag-id", short = 'd', value_name = "OBJECT_ID")]
    dag_id: sui::ObjectID,
    /// Entry group to invoke when executing the DAG.
    #[arg(
        long = "entry-group",
        short = 'e',
        default_value = DEFAULT_ENTRY_GROUP,
        value_name = "NAME",
    )]
    entry_group: String,
    /// Initial input JSON for the DAG execution.
    #[arg(
        long = "input-json",
        short = 'i',
        value_parser = ValueParser::from(parse_json_string),
        value_name = "JSON",
    )]
    input_json: Option<serde_json::Value>,
    /// Metadata entries to attach to the task as key=value pairs.
    #[arg(long = "metadata", short = 'm', value_name = "KEY=VALUE")]
    metadata: Vec<String>,
    /// Gas price paid as priority fee for the DAG execution.
    #[arg(
        long = "execution-gas-price",
        value_name = "AMOUNT",
        default_value_t = 0u64
    )]
    execution_gas_price: u64,
    /// Absolute start time in milliseconds since epoch for the first occurrence.
    #[arg(long = "schedule-start-ms", value_name = "MILLIS")]
    schedule_start_ms: Option<u64>,
    /// Absolute deadline time in milliseconds since epoch for the first occurrence.
    #[arg(long = "schedule-deadline-ms", value_name = "MILLIS")]
    schedule_deadline_ms: Option<u64>,
    /// Start offset in milliseconds from now for the first occurrence.
    #[arg(long = "schedule-start-offset-ms", value_name = "MILLIS")]
    schedule_start_offset_ms: Option<u64>,
    /// Deadline offset in milliseconds after the scheduled start for the first occurrence.
    #[arg(long = "schedule-deadline-offset-ms", value_name = "MILLIS")]
    schedule_deadline_offset_ms: Option<u64>,
    /// Gas price paid as priority fee associated with this occurrence.
    #[arg(
        long = "schedule-gas-price",
        value_name = "AMOUNT",
        default_value_t = 0u64
    )]
    schedule_gas_price: u64,
    #[command(flatten)]
    gas: GasArgs,
}

/// Create a scheduler task and optionally enqueue the initial occurrence.
pub(crate) async fn create_task(args: CreateArgs) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Creating scheduler task for DAG '{dag_id}'",
        dag_id = args.dag_id
    );

    // Validate schedule options provided via CLI flags.
    helpers::validate_schedule_options(
        args.schedule_start_ms,
        args.schedule_deadline_ms,
        args.schedule_start_offset_ms,
        args.schedule_deadline_offset_ms,
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

    // Parse metadata arguments and prepare the DAG input payload.
    let metadata_pairs = helpers::parse_metadata(&args.metadata)?;
    let mut input_json = args.input_json.unwrap_or_else(|| serde_json::json!({}));

    // Fetch encrypted entry ports and encrypt inputs when necessary.
    let encrypt_handles =
        helpers::fetch_encryption_targets(&sui, &args.dag_id, &args.entry_group).await?;
    if !encrypt_handles.is_empty() {
        let session = helpers::get_active_session(&mut conf)?;
        helpers::encrypt_inputs_once(session, &mut input_json, &encrypt_handles)?;
    }

    // Craft the task creation transaction.
    let tx_handle = loading!("Crafting transaction...");
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    let metadata_arg = helpers::metadata_argument(&mut tx, objects, &metadata_pairs)?;
    let constraints_arg = helpers::constraints_policy_argument(&mut tx, objects)?;
    let execution_arg = helpers::execution_policy_argument(
        &mut tx,
        objects,
        args.dag_id,
        args.execution_gas_price,
        &input_json,
        Some(&args.entry_group),
        if encrypt_handles.is_empty() {
            None
        } else {
            Some(&encrypt_handles)
        },
    )?;

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
    if args.schedule_start_ms.is_some() || args.schedule_start_offset_ms.is_some() {
        let schedule_handle = loading!("Scheduling initial occurrence...");
        // Load the newly created task as a shared object.
        let task = fetch_one::<Structure<Task>>(&sui, task_id)
            .await
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

        let mut schedule_tx = sui::ProgrammableTransactionBuilder::new();

        if let Some(start_ms) = args.schedule_start_ms {
            scheduler_tx::add_occurrence_absolute_for_task(
                &mut schedule_tx,
                objects,
                &task.object_ref(),
                start_ms,
                args.schedule_deadline_ms,
                args.schedule_gas_price,
            )
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
        } else {
            let start_offset = args.schedule_start_offset_ms.expect("validated above");
            scheduler_tx::add_occurrence_with_offsets_from_now_for_task(
                &mut schedule_tx,
                objects,
                &task.object_ref(),
                start_offset,
                args.schedule_deadline_offset_ms,
                args.schedule_gas_price,
            )
            .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
        }

        // Submit the scheduling transaction.
        let gas_coin = fetch_gas_coin(&sui, address, args.gas.sui_gas_coin).await?;
        let schedule_tx_data = sui::TransactionData::new_programmable(
            address,
            vec![gas_coin.object_ref()],
            schedule_tx.finish(),
            args.gas.sui_gas_budget,
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
