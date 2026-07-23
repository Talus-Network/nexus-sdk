use {
    super::{RecurrenceStartOptions, ScheduleStartOptions},
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
        nexus::scheduler::{
            CreateTaskParams,
            RecurrenceSpec,
            TaskExecution,
            TaskFailureMode,
            TaskFunding,
        },
        walrus::StorageConf,
    },
    std::collections::HashMap,
};

pub(crate) struct CreateTaskOptions {
    pub(crate) dag_id: Option<sui::types::Address>,
    pub(crate) agent_id: Option<sui::types::Address>,
    pub(crate) skill_id: Option<u64>,
    pub(crate) agent_funded: bool,
    pub(crate) refund_recipient: Option<sui::types::Address>,
    pub(crate) entry_group: String,
    pub(crate) input_json: Option<serde_json::Value>,
    pub(crate) remote: Vec<String>,
    pub(crate) schedule: bool,
    pub(crate) schedule_start: ScheduleStartOptions,
    pub(crate) schedule_deadline_offset_ms: Option<u64>,
    pub(crate) schedule_priority_fee_percentage: Option<u64>,
    pub(crate) recurrence_interval_ms: Option<u64>,
    pub(crate) recurrence_start: RecurrenceStartOptions,
    pub(crate) recurrence_deadline_offset_ms: Option<u64>,
    pub(crate) recurrence_occurrences: Option<u64>,
    pub(crate) recurrence_priority_fee_percentage: Option<u64>,
    pub(crate) pause_on_failure: bool,
    pub(crate) prepay_amount_mist: u64,
    pub(crate) occurrence_budget_mist: u64,
    pub(crate) gas: GasArgs,
}

/// Creates a Task with its initial schedule in one transaction.
pub(crate) async fn create_task(options: CreateTaskOptions) -> AnyResult<(), NexusCliError> {
    let execution = execution_target(options.dag_id, options.agent_id, options.skill_id)?;
    let target = match &execution {
        TaskExecution::Default { dag_id } => format!("DAG '{dag_id}'"),
        TaskExecution::AgentSkill {
            agent_id, skill_id, ..
        } => format!("Agent '{agent_id}' skill '{skill_id}'"),
    };
    command_title!("Creating scheduled Task for {target}");

    let conf = CliConf::load().await.unwrap_or_default();
    let nexus_client =
        get_nexus_client(options.gas.sui_gas_coin, options.gas.sui_gas_budget).await?;
    let input_json = options.input_json.unwrap_or_else(|| serde_json::json!({}));
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf: StorageConf = conf.data_storage.clone().into();
    let ports_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &options.remote)
            .await?;

    let mut input_data = HashMap::new();
    for (vertex, data) in ports_data {
        let committed = data.commit_all(&storage_conf).await.map_err(|error| {
            NexusCliError::Any(anyhow!(
                "Failed to store data: {error}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
            ))
        })?;
        input_data.insert(vertex, committed.into_map());
    }

    let manual_requested = options.schedule
        || options.schedule_start.start_ms.is_some()
        || options.schedule_start.start_offset_ms.is_some()
        || options.schedule_deadline_offset_ms.is_some()
        || options.schedule_priority_fee_percentage.is_some();
    let recurrence_requested = options.recurrence_interval_ms.is_some()
        || options.recurrence_start.start_ms.is_some()
        || options.recurrence_start.start_offset_ms.is_some()
        || options.recurrence_deadline_offset_ms.is_some()
        || options.recurrence_occurrences.is_some()
        || options.recurrence_priority_fee_percentage.is_some();
    let clock_ms = if manual_requested || recurrence_requested {
        nexus_client
            .scheduler()
            .clock_timestamp_ms()
            .await
            .map_err(NexusCliError::Nexus)?
    } else {
        0
    };
    let occurrences = if manual_requested {
        vec![helpers::occurrence_spec(
            clock_ms,
            options.schedule_start.start_ms,
            options.schedule_start.start_offset_ms,
            options.schedule_deadline_offset_ms,
            options.schedule_priority_fee_percentage,
        )?]
    } else {
        Vec::new()
    };
    let recurrence = if recurrence_requested {
        let interval_ms = options.recurrence_interval_ms.ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "--recurrence-interval-ms is required when recurrence options are present"
            ))
        })?;
        Some(RecurrenceSpec {
            first: helpers::occurrence_spec(
                clock_ms,
                options.recurrence_start.start_ms,
                options.recurrence_start.start_offset_ms,
                options.recurrence_deadline_offset_ms,
                options.recurrence_priority_fee_percentage,
            )?,
            interval_ms,
            occurrences: options.recurrence_occurrences,
        })
    } else {
        None
    };
    let funding = if options.agent_funded {
        TaskFunding::Agent {
            prepay_amount_mist: options.prepay_amount_mist,
        }
    } else {
        TaskFunding::Address {
            prepay_amount_mist: options.prepay_amount_mist,
            refund_recipient: options.refund_recipient,
        }
    };
    let failure_mode = if options.pause_on_failure {
        TaskFailureMode::Pause
    } else {
        TaskFailureMode::Continue
    };

    let transaction = loading!("Submitting Task creation transaction...");
    let result = nexus_client
        .scheduler()
        .create_task(CreateTaskParams {
            execution,
            entry_group: options.entry_group,
            input_data,
            funding,
            occurrence_budget_mist: options.occurrence_budget_mist,
            failure_mode,
            occurrences,
            recurrence,
        })
        .await
        .map_err(NexusCliError::Nexus)?;
    transaction.success();

    notify_success!(
        "Task created: {task_id}",
        task_id = result.task_id.to_string().truecolor(100, 100, 100)
    );
    conf.save().await.map_err(NexusCliError::Any)?;
    json_output(&serde_json::json!({
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "task_id": result.task_id,
        "advertised": result.advertised,
    }))
}

fn execution_target(
    dag_id: Option<sui::types::Address>,
    agent_id: Option<sui::types::Address>,
    skill_id: Option<u64>,
) -> AnyResult<TaskExecution, NexusCliError> {
    match (agent_id, skill_id) {
        (None, None) => dag_id
            .map(|dag_id| TaskExecution::Default { dag_id })
            .ok_or_else(|| {
                NexusCliError::Any(anyhow!("--dag-id is required for default execution"))
            }),
        (Some(agent_id), Some(skill_id)) => Ok(TaskExecution::AgentSkill {
            agent_id,
            skill_id,
            selected_dag: dag_id,
            authorization_templates: Vec::new(),
        }),
        _ => Err(NexusCliError::Any(anyhow!(
            "--agent-id and --skill-id must be supplied together"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_target_requires_a_dag() {
        let error = execution_target(None, None, None).expect_err("DAG is required");
        assert!(error.to_string().contains("--dag-id"));
    }

    #[test]
    fn agent_target_uses_dag_as_optional_selection() {
        let selected_dag = sui::types::Address::from_static("0xd");
        let target = execution_target(
            Some(selected_dag),
            Some(sui::types::Address::from_static("0xa")),
            Some(7),
        )
        .expect("Agent target is valid");

        assert!(matches!(
            target,
            TaskExecution::AgentSkill {
                skill_id: 7,
                selected_dag: Some(value),
                ..
            } if value == selected_dag
        ));
    }
}
