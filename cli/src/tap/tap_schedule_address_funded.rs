use {
    super::*,
    nexus_sdk::{
        idents::registry::AgentRegistry,
        nexus::tap::{
            ScheduleDefaultDagExecutorSkillExecutionAddressFundedParams,
            ScheduleSkillExecutionAddressFundedParams,
            ScheduleSkillExecutionFromAgentVaultParams,
        },
        types::TapSchedulePolicy,
    },
};

fn build_schedule_policy(
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
) -> TapSchedulePolicy {
    TapSchedulePolicy {
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_address_funded(
    scheduler_task_id: sui::types::Address,
    agent_id: sui::types::Address,
    skill_id: u64,
    prepay_amount: u64,
    refund_recipient: Option<sui::types::Address>,
    occurrence_budget: u64,
    refund_mode: u8,
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
    let schedule_policy = build_schedule_policy(
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    );
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
            refund_mode,
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

    json_output(&json!({
        "standard_tap": true,
        "function": AgentRegistry::SCHEDULE_SKILL_EXECUTION_ADDRESS_FUNDED.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "scheduler_task_id": scheduler_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "prepay_amount": prepay_amount,
        "occurrence_budget": occurrence_budget,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_from_vault(
    scheduler_task_id: sui::types::Address,
    agent_id: sui::types::Address,
    skill_id: u64,
    prepay_amount: u64,
    occurrence_budget: u64,
    refund_mode: u8,
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
    let schedule_policy = build_schedule_policy(
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    );
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
            refund_mode,
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

    json_output(&json!({
        "standard_tap": true,
        "function": AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "scheduler_task_id": scheduler_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "prepay_amount": prepay_amount,
        "occurrence_budget": occurrence_budget,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_default_address_funded(
    scheduler_task_id: sui::types::Address,
    prepay_amount: u64,
    refund_recipient: Option<sui::types::Address>,
    occurrence_budget: u64,
    refund_mode: u8,
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
    command_title!("Scheduling default-executor TAP skill attached to task '{scheduler_task_id}'");

    let refill_policy_commitment = decode_hex_arg(&refill_policy_hex, "refill-policy")?;
    let schedule_entries_commitment =
        decode_hex_arg(&schedule_entries_commitment_hex, "schedule-entries-hash")?;
    let schedule_policy = build_schedule_policy(
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    );
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
                refund_mode,
                schedule_policy,
                refill_policy_commitment,
                schedule_entries_commitment,
                first_after_ms,
            },
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Scheduled default-executor TAP task {scheduled_task_id} (digest {digest})",
        scheduled_task_id = result
            .scheduled_task_id
            .to_string()
            .truecolor(100, 100, 100),
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&json!({
        "standard_tap": true,
        "function": AgentRegistry::SCHEDULE_DEFAULT_DAG_EXECUTOR_SKILL_EXECUTION_ADDRESS_FUNDED.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.scheduled_task_id,
        "scheduler_task_id": scheduler_task_id,
        "agent_id": result.agent_id,
        "skill_id": result.skill_id,
        "prepay_amount": prepay_amount,
        "occurrence_budget": occurrence_budget,
    }))
}
