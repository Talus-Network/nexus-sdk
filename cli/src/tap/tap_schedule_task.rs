use {
    super::*,
    crate::scheduler::helpers,
    nexus_sdk::{
        nexus::{
            scheduler::{CreateTaskParams, CreateTaskTapPayment, GeneratorKind, OccurrenceRequest},
            tap::fetch_configured_active_tap_skill_execution_target,
        },
        types::{SkillDagBinding, StorageConf},
    },
    std::collections::HashMap,
};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum TapTaskPaymentSourceArg {
    UserFunded,
    AgentFunded,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_tap_task(
    agent_id: sui::types::Address,
    skill_id: u64,
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
    payment_source: TapTaskPaymentSourceArg,
    prepay_amount: u64,
    refund_recipient: Option<sui::types::Address>,
    occurrence_budget: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    if matches!(payment_source, TapTaskPaymentSourceArg::AgentFunded) && refund_recipient.is_some()
    {
        return Err(NexusCliError::Any(anyhow!(
            "--refund-recipient is only valid with --payment-source user-funded"
        )));
    }

    command_title!("Creating TAP scheduled task for agent '{agent_id}' skill '{skill_id}'");

    let conf = CliConf::load().await.unwrap_or_default();
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    match payment_source {
        TapTaskPaymentSourceArg::AgentFunded => {
            ensure_cli_mutable_agent(&nexus_client, agent_id).await?;
        }
        TapTaskPaymentSourceArg::UserFunded => {
            ensure_cli_agent_owner(&nexus_client, agent_id).await?;
        }
    }

    let target = fetch_configured_active_tap_skill_execution_target(
        nexus_client.crawler(),
        &nexus_client.get_nexus_objects(),
        agent_id,
        skill_id,
    )
    .await
    .map_err(NexusCliError::Any)?;
    let dag_id = match target.data.skill.dag_binding {
        SkillDagBinding::Pinned { dag_id } => dag_id,
        SkillDagBinding::RuntimeSelected => {
            return Err(NexusCliError::Any(anyhow!(
                "active TAP skill for agent '{agent_id}' skill '{skill_id}' is runtime-DAG selected; expected a pinned DAG binding"
            )));
        }
    };

    let metadata_pairs = helpers::parse_metadata(&metadata)?;
    let input_json = input_json.take().unwrap_or_else(|| serde_json::json!({}));
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf: StorageConf = conf.data_storage.clone().into();

    let ports_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;
    let mut input_data = HashMap::new();
    for (vertex, data) in ports_data {
        let committed = data.commit_all(&storage_conf).await.map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to store data: {e}.\nEnsure remote storage is configured.\n\n{command}\n{testnet_command}",
                e = e,
                command = "$ nexus conf set --data-storage.walrus-publisher-url <URL> --data-storage.walrus-save-for-epochs <EPOCHS>",
                testnet_command = "Or for testnet simply: $ nexus conf set --data-storage.testnet"
            ))
        })?;
        input_data.insert(vertex, committed.into_map());
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

    let tap_payment = match payment_source {
        TapTaskPaymentSourceArg::UserFunded => CreateTaskTapPayment::UserFunded {
            prepay_amount,
            refund_recipient,
            occurrence_budget,
            selected_dag: Some(dag_id),
            authorization_templates: Vec::new(),
        },
        TapTaskPaymentSourceArg::AgentFunded => CreateTaskTapPayment::AgentFunded {
            prepay_amount,
            occurrence_budget,
            selected_dag: Some(dag_id),
            authorization_templates: Vec::new(),
        },
    };

    let tx_handle = loading!("Submitting TAP scheduled task transaction...");
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
            agent_id: Some(agent_id),
            skill_id: Some(skill_id),
            tap_payment: Some(tap_payment),
        })
        .await
        .map_err(NexusCliError::Nexus)?;
    tx_handle.success();

    notify_success!(
        "TAP scheduled task created: {task_id}",
        task_id = result.task_id.to_string().truecolor(100, 100, 100)
    );

    json_output(&schedule_task_result_json(
        &result, agent_id, skill_id, dag_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn schedule_tap_task_rejects_refund_for_agent_vault_before_rpc() {
        let err = schedule_tap_task(
            sui::types::Address::from_static("0xa"),
            7,
            DEFAULT_ENTRY_GROUP.to_string(),
            None,
            Vec::new(),
            Vec::new(),
            0,
            None,
            None,
            None,
            0,
            GeneratorKind::Queue,
            TapTaskPaymentSourceArg::AgentFunded,
            1,
            Some(sui::types::Address::from_static("0xb")),
            1,
            None,
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("agent-vault refund recipient should fail locally");

        assert!(
            err.to_string().contains("--refund-recipient"),
            "unexpected error: {err}"
        );
    }
}
