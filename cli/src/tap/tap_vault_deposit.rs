use {
    super::*,
    nexus_sdk::{idents::tap::TapStandard, nexus::tap::DepositAgentVaultParams},
};

pub(crate) async fn deposit_agent_vault(
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
    amount: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let conf = CliConf::load().await.unwrap_or_default();
    let agent_id = agent_id_from_alias_or_arg(&conf, alias, agent_id)?;

    command_title!("Depositing {amount} MIST into agent {agent_id} payment vault");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .deposit_agent_payment_vault(DepositAgentVaultParams { agent_id, amount })
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Deposited {amount} MIST (digest {digest})",
        amount = result.amount,
        digest = result.tx_digest.to_string().truecolor(100, 100, 100),
    );

    json_output(&json!({
        "standard_tap": true,
        "function": TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name.to_string(),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "agent_id": result.agent_id,
        "amount": result.amount,
    }))
}
