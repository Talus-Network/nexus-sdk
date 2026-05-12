use {super::*, nexus_sdk::types::AgentId};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn schedule_skill_execution(
    agent_id: AgentId,
    skill_id: u64,
    long_term_gas_coin_id: sui::types::Address,
    input_commitment_hex: String,
    refill_policy_hex: String,
    authorization_plan_commitment_hex: Option<String>,
    schedule_entries_commitment_hex: String,
    recurrence_kind: String,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
    first_after_ms: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let input_commitment = decode_hex_arg(&input_commitment_hex, "input-commitment")?;
    let refill_policy = decode_hex_arg(&refill_policy_hex, "refill-policy")?;
    let authorization_plan_commitment =
        decode_optional_hex_arg(authorization_plan_commitment_hex, "authorization-plan-hash")?;
    let schedule_entries_commitment =
        decode_hex_arg(&schedule_entries_commitment_hex, "schedule-entries-hash")?;
    let schedule_policy = nexus_sdk::types::TapSchedulePolicy {
        recurrence_kind,
        min_interval_ms,
        max_occurrences,
        allow_recursive,
    };

    command_title!("Scheduling TAP skill execution for '{agent_id}:{skill_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .schedule_skill_execution(
            agent_id,
            skill_id,
            long_term_gas_coin_id,
            input_commitment,
            refill_policy,
            authorization_plan_commitment,
            schedule_policy,
            schedule_entries_commitment,
            first_after_ms,
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&schedule_result_json(long_term_gas_coin_id, &result))
}
