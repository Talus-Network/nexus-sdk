use {super::*, crate::types::AgentId};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_agent_dag_skill(
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
    command_title!("Executing agent DAG skill '{agent_id}:{skill_id}'");

    let options = agent_execute_options_from_cli(
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

    let tx_handle = loading!("Crafting and executing agent DAG transaction...");
    let result = match nexus_client
        .workflow()
        .execute_agent_dag(
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

    json_output(&agent_execute_result_json(agent_id, skill_id, &result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn execute_rejects_invalid_payment_source_before_rpc_client() {
        let error = execute_agent_dag_skill(
            sui::types::Address::from_static("0xa"),
            11,
            DEFAULT_ENTRY_GROUP.to_string(),
            serde_json::json!({}),
            Vec::new(),
            0,
            "0xinvalid".to_string(),
            0,
            0,
            None,
            None,
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("invalid payment source hex");

        assert!(
            error.to_string().contains("invalid payment-source hex"),
            "unexpected error: {error}"
        );
    }
}
