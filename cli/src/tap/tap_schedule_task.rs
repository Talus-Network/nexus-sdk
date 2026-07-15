use {
    super::*,
    crate::scheduler::helpers,
    nexus_sdk::{
        move_bindings::interface::agent::SkillDagBinding,
        nexus::{
            scheduler::{CreateTaskParams, CreateTaskTapPayment, GeneratorKind, OccurrenceRequest},
            tap::fetch_configured_active_tap_skill_execution_target,
        },
        types::effective_priority_fee_percentage,
        walrus::StorageConf,
    },
    std::collections::HashMap,
};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub(crate) enum TapTaskPaymentSourceArg {
    UserFunded,
    AgentFunded,
}

fn validated_priority_fee_percentages(
    execution_priority_fee_percentage: Option<u64>,
    schedule_priority_fee_percentage: Option<u64>,
) -> AnyResult<(Option<u64>, Option<u64>), NexusCliError> {
    effective_priority_fee_percentage(execution_priority_fee_percentage)
        .map_err(NexusCliError::Any)?;
    effective_priority_fee_percentage(schedule_priority_fee_percentage)
        .map_err(NexusCliError::Any)?;
    Ok((
        execution_priority_fee_percentage,
        schedule_priority_fee_percentage,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_tap_task(
    agent_id: sui::types::Address,
    skill_id: u64,
    entry_group: String,
    selected_dag: Option<sui::types::Address>,
    mut input_json: Option<serde_json::Value>,
    remote: Vec<String>,
    metadata: Vec<String>,
    execution_priority_fee_percentage: Option<u64>,
    schedule_start_ms: Option<u64>,
    schedule_start_offset_ms: Option<u64>,
    schedule_deadline_offset_ms: Option<u64>,
    schedule_priority_fee_percentage: Option<u64>,
    generator: GeneratorKind,
    payment_source: TapTaskPaymentSourceArg,
    prepay_amount_mist: u64,
    refund_recipient: Option<sui::types::Address>,
    occurrence_budget_mist: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let (execution_priority_fee_percentage, schedule_priority_fee_percentage) =
        validated_priority_fee_percentages(
            execution_priority_fee_percentage,
            schedule_priority_fee_percentage,
        )?;

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
    let (dag_id, runtime_selected_dag) = resolve_scheduled_tap_dag_selection(
        agent_id,
        skill_id,
        target.data.skill.dag_binding().clone(),
        selected_dag,
    )?;

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
                schedule_priority_fee_percentage,
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
            prepay_amount_mist,
            refund_recipient,
            occurrence_budget_mist,
            selected_dag: runtime_selected_dag,
            authorization_templates: Vec::new(),
        },
        TapTaskPaymentSourceArg::AgentFunded => CreateTaskTapPayment::AgentFunded {
            prepay_amount_mist,
            occurrence_budget_mist,
            selected_dag: runtime_selected_dag,
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
            execution_priority_fee_percentage,
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

fn resolve_scheduled_tap_dag_selection(
    agent_id: sui::types::Address,
    skill_id: u64,
    binding: SkillDagBinding,
    selected_dag: Option<sui::types::Address>,
) -> AnyResult<(sui::types::Address, Option<sui::types::Address>), NexusCliError> {
    match binding {
        SkillDagBinding::Pinned { dag_id } => {
            if let Some(selected_dag) = selected_dag {
                if selected_dag != dag_id {
                    return Err(NexusCliError::Any(anyhow!(
                        "selected DAG '{selected_dag}' does not match pinned DAG '{dag_id}' for agent '{agent_id}' skill '{skill_id}'"
                    )));
                }
            }
            Ok((dag_id, None))
        }
        SkillDagBinding::RuntimeSelected => {
            let Some(selected_dag) = selected_dag else {
                return Err(NexusCliError::Any(anyhow!(
                    "active TAP skill for agent '{agent_id}' skill '{skill_id}' is runtime-DAG selected; provide --dag-id"
                )));
            };
            Ok((selected_dag, Some(selected_dag)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_cli_priority_fee_percentages_reach_validation_as_none() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tap",
            "schedule-task",
            "--agent-id",
            "0xa",
            "--skill-id",
            "7",
            "--payment-source",
            "user-funded",
            "--prepay-amount-mist",
            "1",
            "--occurrence-budget-mist",
            "1",
        ])
        .expect("schedule-task arguments parse");
        let crate::Command::Tap(TapCommand::ScheduleTask {
            execution_priority_fee_percentage,
            schedule_priority_fee_percentage,
            ..
        }) = cli.command
        else {
            panic!("expected TAP schedule-task command")
        };
        let percentages = validated_priority_fee_percentages(
            execution_priority_fee_percentage,
            schedule_priority_fee_percentage,
        )
        .expect("omitted percentages are valid");

        assert_eq!(percentages, (None, None));
    }

    #[test]
    fn explicit_zero_priority_fee_percentage_is_rejected() {
        let err = validated_priority_fee_percentages(Some(0), None)
            .expect_err("explicit zero must fail SDK percentage validation");

        assert!(
            err.to_string()
                .contains("priority fee percentage must be in"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn valid_explicit_priority_fee_percentages_are_forwarded() {
        let percentages = validated_priority_fee_percentages(Some(10), Some(25))
            .expect("valid explicit percentages are accepted");

        assert_eq!(percentages, (Some(10), Some(25)));
    }

    #[tokio::test]
    async fn schedule_tap_task_rejects_refund_for_agent_vault_before_rpc() {
        let err = schedule_tap_task(
            sui::types::Address::from_static("0xa"),
            7,
            DEFAULT_ENTRY_GROUP.to_string(),
            None,
            None,
            Vec::new(),
            Vec::new(),
            None,
            None,
            None,
            None,
            None,
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

    #[test]
    fn pinned_skill_uses_pinned_dag_when_selection_is_absent() {
        let pinned = sui::types::Address::from_static("0xd");
        let (dag_id, runtime_selection) = resolve_scheduled_tap_dag_selection(
            sui::types::Address::from_static("0xa"),
            7,
            SkillDagBinding::Pinned { dag_id: pinned },
            None,
        )
        .expect("pinned skill resolves without explicit dag");

        assert_eq!(dag_id, pinned);
        assert_eq!(runtime_selection, None);
    }

    #[test]
    fn pinned_skill_accepts_matching_explicit_dag_without_runtime_selection() {
        let pinned = sui::types::Address::from_static("0xd");
        let (dag_id, runtime_selection) = resolve_scheduled_tap_dag_selection(
            sui::types::Address::from_static("0xa"),
            7,
            SkillDagBinding::Pinned { dag_id: pinned },
            Some(pinned),
        )
        .expect("matching pinned dag resolves");

        assert_eq!(dag_id, pinned);
        assert_eq!(runtime_selection, None);
    }

    #[test]
    fn pinned_skill_rejects_mismatched_explicit_dag() {
        let err = resolve_scheduled_tap_dag_selection(
            sui::types::Address::from_static("0xa"),
            7,
            SkillDagBinding::Pinned {
                dag_id: sui::types::Address::from_static("0xd"),
            },
            Some(sui::types::Address::from_static("0xe")),
        )
        .expect_err("mismatched dag should fail locally");

        assert!(
            err.to_string().contains("does not match pinned DAG"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn runtime_selected_skill_requires_explicit_dag() {
        let err = resolve_scheduled_tap_dag_selection(
            sui::types::Address::from_static("0xa"),
            7,
            SkillDagBinding::RuntimeSelected,
            None,
        )
        .expect_err("runtime-selected skill should require dag");

        assert!(
            err.to_string().contains("provide --dag-id"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn runtime_selected_skill_passes_explicit_dag_to_execution_config() {
        let selected = sui::types::Address::from_static("0xd");
        let (dag_id, runtime_selection) = resolve_scheduled_tap_dag_selection(
            sui::types::Address::from_static("0xa"),
            7,
            SkillDagBinding::RuntimeSelected,
            Some(selected),
        )
        .expect("runtime-selected dag resolves");

        assert_eq!(dag_id, selected);
        assert_eq!(runtime_selection, Some(selected));
    }
}
