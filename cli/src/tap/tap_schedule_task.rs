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
    dag_id_override: Option<sui::types::Address>,
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

    let is_default_dag_executor = agent_id
        == nexus_client
            .get_nexus_objects()
            .default_dag_executor
            .agent_id;

    if is_default_dag_executor {
        if matches!(payment_source, TapTaskPaymentSourceArg::AgentFunded) {
            return Err(NexusCliError::Any(anyhow!(
                "--payment-source agent-funded is not supported for the default DAG executor agent; \
                 use --payment-source user-funded"
            )));
        }
    } else {
        match payment_source {
            TapTaskPaymentSourceArg::AgentFunded => {
                ensure_cli_mutable_agent(&nexus_client, agent_id).await?;
            }
            TapTaskPaymentSourceArg::UserFunded => {
                ensure_cli_agent_owner(&nexus_client, agent_id).await?;
            }
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
    let dag_id = match (target.data.skill.dag_binding, dag_id_override) {
        (SkillDagBinding::Pinned { dag_id }, Some(override_id)) if override_id != dag_id => {
            return Err(NexusCliError::Any(anyhow!(
                "skill is pinned to DAG '{dag_id}' but --dag-id '{override_id}' was supplied; \
                 omit --dag-id or republish the skill with the new binding"
            )));
        }
        (SkillDagBinding::Pinned { dag_id }, _) => dag_id,
        (SkillDagBinding::RuntimeSelected, Some(override_id)) => override_id,
        (SkillDagBinding::RuntimeSelected, None) => {
            return Err(NexusCliError::Any(anyhow!(
                "active TAP skill for agent '{agent_id}' skill '{skill_id}' is runtime-DAG selected; \
                 pass --dag-id to select the DAG to execute (the default DAG executor's skill uses \
                 runtime-selected binding so every caller must supply --dag-id)"
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

    // The default DAG executor agent is wrapped inside a dynamic field on the
    // agent registry, so it can't be fetched as a top-level Sui object. The
    // scheduler exposes `new_default_agent_task` which resolves the agent
    // through the registry; pass `agent_id`/`skill_id` as None so the SDK
    // builds that PTB instead of the regular agent-bound one.
    let (params_agent_id, params_skill_id, tap_payment) = if is_default_dag_executor {
        (
            None,
            None,
            CreateTaskTapPayment::UserFunded {
                prepay_amount,
                refund_recipient: None,
                occurrence_budget,
                selected_dag: None,
                authorization_templates: Vec::new(),
            },
        )
    } else {
        // Move's `resolve_agent_execution_config_dag` requires `selected_dag`
        // to be Some for runtime-selected skills and None for pinned ones
        // (`EDagSelectionRequired` / `EDagSelectionRedundant`).
        let selected_dag = match target.data.skill.dag_binding {
            SkillDagBinding::RuntimeSelected => Some(dag_id),
            SkillDagBinding::Pinned { .. } => None,
        };
        let payment = match payment_source {
            TapTaskPaymentSourceArg::UserFunded => CreateTaskTapPayment::UserFunded {
                prepay_amount,
                refund_recipient,
                occurrence_budget,
                selected_dag,
                authorization_templates: Vec::new(),
            },
            TapTaskPaymentSourceArg::AgentFunded => CreateTaskTapPayment::AgentFunded {
                prepay_amount,
                occurrence_budget,
                selected_dag,
                authorization_templates: Vec::new(),
            },
        };
        (Some(agent_id), Some(skill_id), payment)
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
            agent_id: params_agent_id,
            skill_id: params_skill_id,
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
