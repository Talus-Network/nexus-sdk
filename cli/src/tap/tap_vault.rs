use super::*;

pub(crate) async fn handle_vault_command(command: VaultCommand) -> AnyResult<(), NexusCliError> {
    match command {
        VaultCommand::Balance { alias, agent_id } => {
            let conf = CliConf::load().await.unwrap_or_default();
            let agent_id = agent_id_from_alias_or_arg(&conf, alias, agent_id)?;
            let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
            let vault = fetch_tap_agent_payment_vault_for_agent(nexus_client.crawler(), agent_id)
                .await
                .map_err(NexusCliError::Any)?;
            json_output(&json!({
                "agent_id": agent_id,
                "vault_id": vault.object_id,
                "available_balance": vault.data.available_balance,
                "locked_amount": vault.data.locked_amount,
                "unlocked_balance": vault.data.available_balance.saturating_sub(vault.data.locked_amount)
            }))
        }
    }
}
