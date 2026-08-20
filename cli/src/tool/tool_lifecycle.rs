use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

#[derive(Clone, Copy)]
pub(super) enum LifecycleAction {
    Close,
    DrainCashier,
    Retire,
}

pub(super) async fn run(
    action: LifecycleAction,
    tool_fqn: ToolFqn,
    owner_cap: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let verb = match action {
        LifecycleAction::Close => "Closing",
        LifecycleAction::DrainCashier => "Draining cashier for",
        LifecycleAction::Retire => "Retiring",
    };
    command_title!("{verb} Tool '{tool_fqn}'");

    let conf = CliConf::load().await.unwrap_or_default();
    let owner_cap = owner_cap
        .or_else(|| conf.tools.get(&tool_fqn).map(|caps| caps.over_tool))
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!("No OwnerCap object ID found for Tool '{tool_fqn}'"))
        })?;
    let client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let progress = loading!("Submitting Tool lifecycle transaction...");
    let result = match action {
        LifecycleAction::Close => client.tool().close(&tool_fqn, owner_cap).await,
        LifecycleAction::DrainCashier => client.tool().drain_cashier(&tool_fqn, owner_cap).await,
        LifecycleAction::Retire => client.tool().retire(&tool_fqn, owner_cap).await,
    }
    .map_err(NexusCliError::Nexus)?;
    progress.success();

    notify_success!(
        "Transaction digest: {digest}",
        digest = result.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({ "digest": result.tx_digest }))
}
