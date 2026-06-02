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
        types::{PolicySymbol, StorageConf},
    },
    std::collections::HashMap,
};

/// Create a scheduler task and optionally enqueue the initial occurrence.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_task(
    dag_id: sui::types::Address,
    entry_group: String,
    mut input_json: Option<serde_json::Value>,
    remote: Vec<String>,
    metadata: Vec<String>,
    execution_priority_fee_per_gas_unit: u64,
    schedule_start_ms: Option<u64>,
    schedule_start_offset_ms: Option<u64>,
    schedule_deadline_offset_ms: Option<u64>,
    schedule_priority_fee_per_gas_unit: u64,
    generator: GeneratorKind,
    agent_id: Option<sui::types::Address>,
    skill_id: Option<u64>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    if agent_id.is_some() != skill_id.is_some() {
        return Err(NexusCliError::Any(anyhow!(
            "--agent-id and --skill-id must be provided together (or both omitted) to scope the task to a registered TAP agent skill"
        )));
    }
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

    // Build the remote storage configuration.
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf: StorageConf = conf.data_storage.clone().into();

    let ports_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;

    let mut input_data = HashMap::new();
    for (vertex, data) in ports_data {
        let committed = data
            .commit_all(&storage_conf)
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
                schedule_priority_fee_per_gas_unit,
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
            execution_priority_fee_per_gas_unit,
            initial_schedule,
            generator,
            agent_id,
            skill_id,
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
        NexusEventKind::RequestScheduledOccurrence(env) => Some(format!(
            "task={} start_ms={} (generator={}, priority={})",
            env.request.task,
            env.start_ms,
            describe_generator(&env.request.generator),
            env.priority
        )),
        NexusEventKind::RequestScheduledWalk(env) => Some(format!(
            "walk for dag execution start_ms={} (priority={})",
            env.start_ms, env.priority
        )),
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

#[cfg(test)]
mod tests {
    use {super::*, crate::prelude::GasArgs};

    fn gas_args() -> GasArgs {
        GasArgs {
            sui_gas_coin: None,
            sui_gas_budget: 100_000_000,
        }
    }

    /// Half-supplied `(--agent-id, --skill-id)` must be caught locally before
    /// any RPC traffic so the user sees a clear error instead of a confusing
    /// chain-side failure. The CLI validates the pair up-front; the SDK
    /// validates again as a defense-in-depth layer. We exercise only the CLI
    /// guard here (no Nexus client is constructed) — a regression that drops
    /// the early-return would surface as a missing-RPC error instead.
    #[tokio::test]
    async fn create_task_rejects_agent_id_without_skill_id() {
        let dag_id = sui::types::Address::from_static("0xd");
        let err = create_task(
            dag_id,
            "entry".to_string(),
            None,
            Vec::new(),
            Vec::new(),
            0,
            None,
            None,
            None,
            0,
            GeneratorKind::Queue,
            Some(sui::types::Address::from_static("0xa")),
            None,
            gas_args(),
        )
        .await
        .expect_err("agent-only must error before any RPC");
        let msg = err.to_string();
        assert!(
            msg.contains("--agent-id") && msg.contains("--skill-id"),
            "expected agent/skill validation error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn create_task_rejects_skill_id_without_agent_id() {
        let dag_id = sui::types::Address::from_static("0xd");
        let err = create_task(
            dag_id,
            "entry".to_string(),
            None,
            Vec::new(),
            Vec::new(),
            0,
            None,
            None,
            None,
            0,
            GeneratorKind::Queue,
            None,
            Some(7),
            gas_args(),
        )
        .await
        .expect_err("skill-only must error before any RPC");
        let msg = err.to_string();
        assert!(
            msg.contains("--agent-id") && msg.contains("--skill-id"),
            "expected agent/skill validation error, got: {msg}"
        );
    }
}
