use {
    super::*,
    nexus_sdk::nexus::tap::{
        ScheduleDefaultDagExecutorSkillExecutionAddressFundedParams,
        ScheduleSkillExecutionAddressFundedParams,
        ScheduleSkillExecutionFromAgentVaultParams,
    },
};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_address_funded(
    scheduler_task_id: sui::types::Address,
    agent_id: sui::types::Address,
    skill_id: u64,
    prepay_amount: u64,
    refund_recipient: Option<sui::types::Address>,
    occurrence_budget: u64,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    refill_policy_hex: String,
    schedule_entries_commitment_hex: String,
    first_after_ms: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scheduling address-funded TAP skill execution for '{agent_id}:{skill_id}' attached to task '{scheduler_task_id}'"
    );

    let refill_policy_commitment = decode_hex_arg(&refill_policy_hex, "refill-policy")?;
    let schedule_entries_commitment =
        decode_hex_arg(&schedule_entries_commitment_hex, "schedule-entries-hash")?;
    let schedule_policy = schedule_policy_from_cli(
        &recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    )?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let scheduler_task = nexus_client
        .crawler()
        .get_object_metadata(scheduler_task_id)
        .await
        .map_err(NexusCliError::Any)?
        .object_ref();
    let result = nexus_client
        .tap()
        .schedule_skill_execution_address_funded(ScheduleSkillExecutionAddressFundedParams {
            scheduler_task,
            agent_id,
            skill_id,
            prepay_amount,
            refund_recipient,
            payment_source: Vec::new(),
            occurrence_budget,
            schedule_policy,
            refill_policy_commitment,
            schedule_entries_commitment,
            first_after_ms,
            grant_templates: Vec::new(),
        })
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Scheduled TAP task {scheduled_task_id} (digest {digest})",
        scheduled_task_id = result
            .scheduled_task_id
            .to_string()
            .truecolor(100, 100, 100),
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&schedule_address_funded_result_json(
        scheduler_task_id,
        prepay_amount,
        occurrence_budget,
        &result,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_from_vault(
    scheduler_task_id: sui::types::Address,
    agent_id: sui::types::Address,
    skill_id: u64,
    prepay_amount: u64,
    occurrence_budget: u64,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    refill_policy_hex: String,
    schedule_entries_commitment_hex: String,
    first_after_ms: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Scheduling vault-funded TAP skill execution for '{agent_id}:{skill_id}' attached to task '{scheduler_task_id}'"
    );

    let refill_policy_commitment = decode_hex_arg(&refill_policy_hex, "refill-policy")?;
    let schedule_entries_commitment =
        decode_hex_arg(&schedule_entries_commitment_hex, "schedule-entries-hash")?;
    let schedule_policy = schedule_policy_from_cli(
        &recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    )?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let scheduler_task = nexus_client
        .crawler()
        .get_object_metadata(scheduler_task_id)
        .await
        .map_err(NexusCliError::Any)?
        .object_ref();
    let result = nexus_client
        .tap()
        .schedule_skill_execution_from_agent_vault(ScheduleSkillExecutionFromAgentVaultParams {
            scheduler_task,
            agent_id,
            skill_id,
            prepay_amount,
            occurrence_budget,
            schedule_policy,
            refill_policy_commitment,
            schedule_entries_commitment,
            first_after_ms,
            grant_templates: Vec::new(),
        })
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Scheduled TAP task {scheduled_task_id} (digest {digest})",
        scheduled_task_id = result
            .scheduled_task_id
            .to_string()
            .truecolor(100, 100, 100),
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&schedule_from_vault_result_json(
        scheduler_task_id,
        prepay_amount,
        occurrence_budget,
        &result,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_default_address_funded(
    scheduler_task_id: sui::types::Address,
    prepay_amount: u64,
    refund_recipient: Option<sui::types::Address>,
    occurrence_budget: u64,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    refill_policy_hex: String,
    schedule_entries_commitment_hex: String,
    first_after_ms: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Scheduling default-agent TAP skill attached to task '{scheduler_task_id}'");

    let refill_policy_commitment = decode_hex_arg(&refill_policy_hex, "refill-policy")?;
    let schedule_entries_commitment =
        decode_hex_arg(&schedule_entries_commitment_hex, "schedule-entries-hash")?;
    let schedule_policy = schedule_policy_from_cli(
        &recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    )?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let scheduler_task = nexus_client
        .crawler()
        .get_object_metadata(scheduler_task_id)
        .await
        .map_err(NexusCliError::Any)?
        .object_ref();
    let result = nexus_client
        .tap()
        .schedule_default_dag_executor_skill_execution_address_funded(
            ScheduleDefaultDagExecutorSkillExecutionAddressFundedParams {
                scheduler_task,
                prepay_amount,
                refund_recipient,
                payment_source: Vec::new(),
                occurrence_budget,
                schedule_policy,
                refill_policy_commitment,
                schedule_entries_commitment,
                first_after_ms,
            },
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Scheduled default-agent TAP task {scheduled_task_id} (digest {digest})",
        scheduled_task_id = result
            .scheduled_task_id
            .to_string()
            .truecolor(100, 100, 100),
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&schedule_default_address_funded_result_json(
        scheduler_task_id,
        prepay_amount,
        occurrence_budget,
        &result,
    ))
}
