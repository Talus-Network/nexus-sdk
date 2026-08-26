use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::nexus::network::CollectPriorityFeeDepositsResult,
};

pub(crate) async fn configure_leader_registry(
    unbonding_duration_ms: u64,
    min_stake_us: u64,
    max_transaction_budget_mist: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Configuring leader registry policy");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Crafting and executing transaction...");
    let response = match nexus_client
        .network()
        .configure_leader_registry(
            unbonding_duration_ms,
            min_stake_us,
            max_transaction_budget_mist,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    tx_handle.success();
    notify_success!(
        "Transaction digest: {digest}",
        digest = response.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&json!({
        "digest": response.tx_digest,
        "unbonding_duration_ms": unbonding_duration_ms,
        "min_stake_us": min_stake_us,
        "max_transaction_budget_mist": max_transaction_budget_mist,
    }))?;
    Ok(())
}

pub(crate) async fn collect_priority_fees(
    leader_cap_id: sui::types::Address,
    batch_size: usize,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Collecting priority fee deposits");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Discovering deposits and executing collection transaction(s)...");
    let result = nexus_client
        .network()
        .collect_priority_fee_deposits(leader_cap_id, batch_size)
        .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            tx_handle.error();
            return Err(NexusCliError::Nexus(error));
        }
    };
    tx_handle.success();
    notify_success!(
        "Collected {count} priority fee deposit(s) in {transactions} transaction(s)",
        count = result.collected_deposit_ids.len(),
        transactions = result.tx_digests.len(),
    );
    json_output(&collect_priority_fees_result_json(leader_cap_id, &result))?;
    Ok(())
}

fn collect_priority_fees_result_json(
    leader_cap_id: sui::types::Address,
    result: &CollectPriorityFeeDepositsResult,
) -> serde_json::Value {
    json!({
        "leader_cap_id": leader_cap_id,
        "transaction_digests": result.tx_digests,
        "collected_deposit_ids": result.collected_deposit_ids,
        "skipped_old_leader_deposits": result
            .skipped_old_leader_deposits
            .iter()
            .map(|deposit| json!({
                "deposit_id": deposit.deposit_id,
                "leader_cap_id": deposit.leader_cap_id,
            }))
            .collect::<Vec<_>>(),
        "unavailable_deposit_ids": result.unavailable_deposit_ids,
    })
}

pub(crate) async fn configure_priority_fee_vault(
    exchange_rate_million_mists_us: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Configuring priority fee vault");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let tx_handle = loading!("Crafting and executing transaction...");
    let response = match nexus_client
        .network()
        .configure_priority_fee_vault(exchange_rate_million_mists_us)
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
        .network()
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
        .network()
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
    json_output(&drain_priority_fee_vault_sui_result_json(&response))?;
    Ok(())
}

fn drain_priority_fee_vault_sui_result_json(
    response: &nexus_sdk::nexus::network::DrainPriorityFeeVaultSuiResult,
) -> serde_json::Value {
    json!({
        "digest": response.tx_digest,
        "exchange_rate_million_mists_us": response.exchange_rate_million_mists_us,
        "sui_balance_before": response.sui_balance_before,
        "min_sui_out": response.min_sui_out,
    })
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
            match nexus_client.network().priority_fee_share(leader_cap).await {
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
        .network()
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::nexus::network::{DrainPriorityFeeVaultSuiResult, SkippedPriorityFeeDeposit},
    };

    #[test]
    fn drain_result_json_uses_million_mists_exchange_rate_key() {
        let response = DrainPriorityFeeVaultSuiResult {
            tx_digest: sui::types::Digest::from_static(
                "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv",
            ),
            exchange_rate_million_mists_us: 1_000_000,
            sui_balance_before: 1_000_000_000,
            min_sui_out: 1_000_000_000,
        };

        let value = drain_priority_fee_vault_sui_result_json(&response);
        assert_eq!(value["exchange_rate_million_mists_us"], 1_000_000);
        assert!(value.get("exchange_rate_sui_us").is_none());
        assert_eq!(value["sui_balance_before"], 1_000_000_000u64);
        assert_eq!(value["min_sui_out"], 1_000_000_000u64);
    }

    #[test]
    fn priority_fee_collection_result_json_has_stable_top_level_keys() {
        let result = CollectPriorityFeeDepositsResult {
            tx_digests: vec![sui::types::Digest::from_static(
                "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv",
            )],
            collected_deposit_ids: vec![sui::types::Address::from_static("0x701")],
            skipped_old_leader_deposits: vec![SkippedPriorityFeeDeposit {
                deposit_id: sui::types::Address::from_static("0x702"),
                leader_cap_id: sui::types::Address::from_static("0x703"),
            }],
            unavailable_deposit_ids: vec![sui::types::Address::from_static("0x704")],
        };

        let leader_cap_id = sui::types::Address::from_static("0x705");
        let value = collect_priority_fees_result_json(leader_cap_id, &result);
        assert_eq!(value["leader_cap_id"], leader_cap_id.to_string());
        assert_eq!(value["transaction_digests"].as_array().unwrap().len(), 1);
        assert_eq!(
            value["collected_deposit_ids"][0],
            sui::types::Address::from_static("0x701").to_string()
        );
        assert_eq!(
            value["skipped_old_leader_deposits"][0]["leader_cap_id"],
            sui::types::Address::from_static("0x703").to_string()
        );
        assert_eq!(
            value["unavailable_deposit_ids"][0],
            sui::types::Address::from_static("0x704").to_string()
        );
    }
}
