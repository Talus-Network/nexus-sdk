use {
    super::*,
    crate::types::AgentId,
    nexus_sdk::transactions::dag::{MoveCallTarget, VertexGrantBind},
};

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
    grant_bind: Vec<String>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing agent DAG skill '{agent_id}:{skill_id}'");

    let mut options = agent_execute_options_from_cli(
        payment_source_hex,
        payment_max_budget,
        payment_refund_mode,
        authorization_plan_commitment_hex,
    )?;
    let parsed_binds = parse_grant_binds(&grant_bind)?;
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    if !parsed_binds.is_empty() {
        options.grant_binds = resolve_grant_binds(&nexus_client, parsed_binds).await?;
    }
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

/// Pre-RPC representation of one `--grant-bind` flag value.
struct ParsedGrantBind {
    vertex: String,
    state_object_id: sui::types::Address,
    package: sui::types::Address,
    module: String,
    function: String,
}

fn parse_grant_binds(raw: &[String]) -> AnyResult<Vec<ParsedGrantBind>, NexusCliError> {
    raw.iter().map(|s| parse_grant_bind(s)).collect()
}

fn parse_grant_bind(raw: &str) -> AnyResult<ParsedGrantBind, NexusCliError> {
    // Format: VERTEX:STATE_OBJECT:PACKAGE::MODULE::FUNCTION
    let (vertex, rest) = raw.split_once(':').ok_or_else(|| {
        NexusCliError::Any(anyhow!(
            "invalid --grant-bind '{raw}': expected VERTEX:STATE:PKG::MOD::FN"
        ))
    })?;
    let (state, target) = rest.split_once(':').ok_or_else(|| {
        NexusCliError::Any(anyhow!(
            "invalid --grant-bind '{raw}': missing state object ID"
        ))
    })?;
    let parts: Vec<&str> = target.split("::").collect();
    if parts.len() != 3 {
        return Err(NexusCliError::Any(anyhow!(
            "invalid --grant-bind '{raw}': bind target must be PACKAGE::MODULE::FUNCTION"
        )));
    }
    let package = parts[0].parse().map_err(|e| {
        NexusCliError::Any(anyhow!(
            "invalid --grant-bind '{raw}': bind target package is not a Sui address: {e}"
        ))
    })?;
    let state_object_id = state.parse().map_err(|e| {
        NexusCliError::Any(anyhow!(
            "invalid --grant-bind '{raw}': state object id is not a Sui address: {e}"
        ))
    })?;
    Ok(ParsedGrantBind {
        vertex: vertex.to_string(),
        state_object_id,
        package,
        module: parts[1].to_string(),
        function: parts[2].to_string(),
    })
}

async fn resolve_grant_binds(
    nexus_client: &nexus_sdk::nexus::client::NexusClient,
    parsed: Vec<ParsedGrantBind>,
) -> AnyResult<Vec<VertexGrantBind>, NexusCliError> {
    let mut out = Vec::with_capacity(parsed.len());
    for entry in parsed {
        let state_ref = nexus_client
            .crawler()
            .get_object_metadata(entry.state_object_id)
            .await
            .map_err(NexusCliError::Any)?
            .object_ref();
        out.push(VertexGrantBind {
            vertex: entry.vertex,
            state_object: state_ref,
            bind_target: MoveCallTarget {
                package: entry.package,
                module: sui::types::Identifier::new(&entry.module)
                    .map_err(|e| NexusCliError::Any(anyhow!("invalid bind-target module: {e}")))?,
                function: sui::types::Identifier::new(&entry.function).map_err(|e| {
                    NexusCliError::Any(anyhow!("invalid bind-target function: {e}"))
                })?,
            },
        });
    }
    Ok(out)
}

pub(crate) fn agent_execute_result_json(
    agent_id: AgentId,
    skill_id: SkillId,
    result: &nexus_sdk::nexus::workflow::ExecuteResult,
) -> serde_json::Value {
    json!({
        "agent_dag": true,
        "agent_id": agent_id,
        "skill_id": skill_id,
        "execution_id": result.execution_object_id,
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "submit": result.tap_execution.as_ref().map(|submit| json!({
            "agent_id": submit.agent_id,
            "skill_id": submit.skill_id,
            "dag_id": submit.dag_id,
            "endpoint_key": submit.endpoint_key,
            "payment_max_budget": submit.payment_max_budget,
            "payment_refund_mode": submit.payment_refund_mode,
            "authorization_plan_commitment": submit.authorization_plan_commitment,
        }))
    })
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
            Vec::new(),
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
