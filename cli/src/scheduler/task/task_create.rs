use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        scheduler::helpers,
        sui::get_nexus_client,
        workflow,
    },
    nexus_sdk::{
        events::NexusEventKind,
        nexus::scheduler::{CreateTaskParams, GeneratorKind, OccurrenceRequest},
        types::{EncryptionMode, PolicySymbol, StorageConf},
    },
    std::{collections::HashMap, sync::Arc},
};

/// Create a scheduler task and optionally enqueue the initial occurrence.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_task(
    dag_id: sui::types::Address,
    entry_group: String,
    mut input_json: Option<serde_json::Value>,
    remote: Vec<String>,
    metadata: Vec<String>,
    execution_gas_price: u64,
    schedule_start_ms: Option<u64>,
    schedule_start_offset_ms: Option<u64>,
    schedule_deadline_offset_ms: Option<u64>,
    schedule_gas_price: u64,
    generator: GeneratorKind,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Creating scheduler task for DAG '{dag_id}'",
        dag_id = dag_id
    );

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_default();

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    // Parse metadata arguments and prepare the DAG input payload.
    let metadata_pairs = helpers::parse_metadata(&metadata)?;
    let input_json = input_json.take().unwrap_or_else(|| serde_json::json!({}));

    // Fetch encrypted entry ports.
    let encrypt_handles =
        helpers::fetch_encryption_targets(nexus_client.crawler(), &dag_id, &entry_group).await?;

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
        EncryptionMode::LimitedPersistent,
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

    let schedule_requested = schedule_start_ms.is_some()
        || schedule_start_offset_ms.is_some()
        || schedule_deadline_offset_ms.is_some();

    let initial_schedule = if schedule_requested {
        Some(
            OccurrenceRequest::new(
                schedule_start_ms,
                None,
                schedule_start_offset_ms,
                schedule_deadline_offset_ms,
                schedule_gas_price,
                true,
            )
            .map_err(NexusCliError::Nexus)?,
        )
    } else {
        None
    };

    if matches!(generator, GeneratorKind::Periodic) && initial_schedule.is_some() {
        return Err(NexusCliError::Any(anyhow!(
            "Periodic tasks cannot enqueue an initial occurrence. Configure scheduling with `nexus scheduler periodic set`."
        )));
    }

    let tx_handle = loading!("Submitting scheduler task transaction...");

    let result = nexus_client
        .scheduler()
        .create_task(CreateTaskParams {
            dag_id,
            entry_group: entry_group.clone(),
            input_data,
            metadata: metadata_pairs,
            execution_gas_price,
            initial_schedule,
            generator,
        })
        .await
        .map_err(NexusCliError::Nexus)?;

    tx_handle.success();

    let mut result_json = serde_json::json!({
        "digest": result.tx_digest,
        "task_id": result.task_id,
    });

    let mut scheduled_event_display = None;

    if let Some(schedule) = result.initial_schedule.as_ref() {
        result_json["schedule_digest"] = serde_json::json!(schedule.tx_digest);
        if let Some(event) = schedule.event.clone() {
            result_json["scheduled"] = serde_json::to_value(&event).unwrap_or_default();
            scheduled_event_display = describe_occurrence_event(&event);
        }
    }

    notify_success!(
        "Scheduler task created: {task_id}",
        task_id = result.task_id.to_string().truecolor(100, 100, 100)
    );

    if let Some(description) = scheduled_event_display {
        notify_success!(
            "Initial occurrence scheduled: {event}",
            event = description.truecolor(100, 100, 100)
        );
    }

    // Always save the updated config.
    conf.save().await.map_err(NexusCliError::Any)?;

    json_output(&result_json)?;

    Ok(())
}

fn describe_occurrence_event(event: &NexusEventKind) -> Option<String> {
    match event {
        // TODO: @david to re-implement or to simplify by removing generic.
        // NexusEventKind::Scheduled(envelope) => Some(format!("start_ms={}", envelope.start_ms)),
        NexusEventKind::OccurrenceScheduled(e) => Some(format!(
            "task={} (generator={})",
            e.task,
            describe_generator(&e.generator)
        )),
        _ => None,
    }
}

fn describe_generator(symbol: &PolicySymbol) -> String {
    match symbol {
        PolicySymbol::Witness(name) => name.name.clone(),
        PolicySymbol::Uid(uid) => uid.to_string(),
    }
}
