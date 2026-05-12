use {super::*, crate::types::AgentId};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_standard_tap_skill(
    agent_id: AgentId,
    skill_id: u64,
    entry_group: String,
    input_json: serde_json::Value,
    remote: Vec<String>,
    priority_fee_per_gas_unit: u64,
    payment_source_hex: String,
    payment_max_budget: u64,
    payment_refund_mode: u8,
    authorization_plan_commitment_hex: Option<String>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing standard TAP skill '{agent_id}:{skill_id}'");

    let options = standard_execute_options_from_cli(
        payment_source_hex,
        payment_max_budget,
        payment_refund_mode,
        authorization_plan_commitment_hex,
    )?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let conf = CliConf::load().await.unwrap_or_default();
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf = conf.data_storage.clone().into();
    let input_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;

    let tx_handle = loading!("Crafting and executing standard TAP transaction...");
    let result = match nexus_client
        .workflow()
        .execute_standard_tap(
            agent_id,
            skill_id,
            input_data,
            priority_fee_per_gas_unit,
            Some(&entry_group),
            &storage_conf,
            options,
        )
        .await
    {
        Ok(result) => result,
        Err(NexusError::Storage(e)) => {
            tx_handle.error();
            return Err(NexusCliError::Any(anyhow!(
                "{e}.\nEnsure remote storage is configured."
            )));
        }
        Err(error) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(error));
        }
    };

    tx_handle.success();

    notify_success!(
        "DAGExecution object ID: {id}",
        id = result
            .execution_object_id
            .to_string()
            .truecolor(100, 100, 100)
    );

    json_output(&standard_execute_result_json(agent_id, skill_id, &result))
}

pub(crate) fn standard_execute_result_json(
    agent_id: AgentId,
    skill_id: SkillId,
    result: &nexus_sdk::nexus::workflow::ExecuteResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "agent_id": agent_id,
        "skill_id": skill_id,
        "execution_id": result.execution_object_id,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "submit": result.standard_tap.as_ref().map(|submit| json!({
            "agent_id": submit.agent_id,
            "skill_id": submit.skill_id,
            "dag_id": submit.dag_id,
            "endpoint_key": submit.endpoint_key,
            "endpoint_object_id": submit.endpoint_object.object_id(),
            "endpoint_object_version": submit.endpoint_object.version(),
            "payment_max_budget": submit.payment_max_budget,
            "payment_refund_mode": submit.payment_refund_mode,
            "authorization_plan_commitment": submit.authorization_plan_commitment,
        }))
    })
}
