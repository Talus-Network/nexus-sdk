use {super::*, crate::types::AgentId};

pub(crate) async fn read_artifact(path: PathBuf) -> AnyResult<TapPublishArtifact, NexusCliError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(NexusCliError::Io)?;
    serde_json::from_str(&text).map_err(|e| NexusCliError::Any(e.into()))
}

pub(crate) fn decode_hex_arg(value: &str, name: &str) -> AnyResult<Vec<u8>, NexusCliError> {
    hex::decode(value.trim_start_matches("0x"))
        .map_err(|e| NexusCliError::Any(anyhow!("invalid {name} hex: {e}")))
}

pub(crate) fn decode_optional_hex_arg(
    value: Option<String>,
    name: &str,
) -> AnyResult<Option<Vec<u8>>, NexusCliError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| decode_hex_arg(&value, name))
        .transpose()
}

pub(crate) fn standard_execute_options_from_cli(
    payment_source_hex: String,
    payment_max_budget: u64,
    payment_refund_mode: u8,
    authorization_plan_commitment_hex: Option<String>,
) -> AnyResult<StandardTapExecuteOptions, NexusCliError> {
    Ok(StandardTapExecuteOptions {
        payment_source: decode_hex_arg(&payment_source_hex, "payment-source")?,
        payment_coin: None,
        payment_coin_balance: None,
        payment_max_budget,
        payment_refund_mode,
        authorization_plan_commitment: decode_optional_hex_arg(
            authorization_plan_commitment_hex,
            "authorization-plan-hash",
        )?,
        authorization_plan: Vec::new(),
    })
}

pub(crate) fn agent_id_from_alias_or_arg(
    conf: &CliConf,
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
) -> AnyResult<AgentId, NexusCliError> {
    if let Some(agent_id) = agent_id {
        return Ok(agent_id);
    }
    if let Some(alias) = alias {
        let agent_id = conf.agents.get(&alias).copied().ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No Talus agent alias '{alias}' found in CLI config"
            ))
        })?;
        return Ok(agent_id);
    }
    Err(NexusCliError::Any(anyhow!(
        "provide either --agent-id or --alias"
    )))
}
