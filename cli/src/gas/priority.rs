use crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*};

pub(crate) async fn configure_priority_fee_vault(
    exchange_rate_sui_us: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Configuring priority fee vault");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Crafting and executing transaction...");
    let response = match nexus_client
        .gas()
        .configure_priority_fee_vault(exchange_rate_sui_us)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };
    tx_handle.success();
    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({ "digest": response.tx_digest }))?;
    Ok(())
}

pub(crate) async fn swap_us_for_sui(
    us_coin: sui::types::Address,
    min_sui_out: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Swapping `$US` for SUI from the priority fee vault");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Crafting and executing transaction...");
    let response = match nexus_client
        .gas()
        .swap_us_for_sui(us_coin, min_sui_out)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };
    tx_handle.success();
    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "digest": response.tx_digest,
        "us_spent": response.us_spent,
        "us_refunded": response.us_refunded,
        "sui_withdrawn": response.sui_withdrawn,
    }))?;
    Ok(())
}

pub(crate) async fn drain_priority_fee_vault_sui(
    us_coin: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Draining priority fee vault SUI");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Querying vault state and executing drain swap...");
    let response = match nexus_client
        .gas()
        .drain_priority_fee_vault_sui(us_coin)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };
    tx_handle.success();
    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "digest": response.tx_digest,
        "exchange_rate_sui_us": response.exchange_rate_sui_us,
        "sui_balance_before": response.sui_balance_before,
        "min_sui_out": response.min_sui_out,
    }))?;
    Ok(())
}

pub(crate) async fn withdraw_priority_fee(
    leader_cap: sui::types::Address,
    share_to_withdraw: Option<u64>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Withdrawing `$US` priority fees");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let share_to_withdraw = match share_to_withdraw {
        Some(share_to_withdraw) => share_to_withdraw,
        None => {
            let query_handle = loading!("Querying leader vault share...");
            match nexus_client.gas().priority_fee_share(leader_cap).await {
                Ok(share_to_withdraw) => {
                    query_handle.success();
                    share_to_withdraw
                }
                Err(e) => {
                    query_handle.error();
                    return Err(NexusCliError::Nexus(e));
                }
            }
        }
    };
    let tx_handle = loading!("Crafting and executing transaction...");
    let response = match nexus_client
        .gas()
        .withdraw_priority_fee(leader_cap, share_to_withdraw)
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(e));
        }
    };
    tx_handle.success();
    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "digest": response.tx_digest,
        "share_to_withdraw": share_to_withdraw,
    }))?;
    Ok(())
}
