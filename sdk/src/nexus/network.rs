//! Commands related to Nexus network economics.

use crate::{
    events::NexusEventKind,
    move_bindings::registry::priority_fee_vault::{PriorityFeeVault, PriorityFeeVaultStateV1},
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::network,
    types::PriorityFeeWithdrawalQuote,
};

pub struct ConfigurePriorityFeeVaultResult {
    pub tx_digest: sui::types::Digest,
}

pub struct SwapUsForSuiResult {
    pub tx_digest: sui::types::Digest,
    pub us_spent: u64,
    pub us_refunded: u64,
    pub sui_withdrawn: u64,
}

#[derive(Debug)]
pub struct DrainPriorityFeeVaultSuiResult {
    pub tx_digest: sui::types::Digest,
    pub exchange_rate_million_mists_us: u64,
    pub sui_balance_before: u64,
    pub min_sui_out: u64,
}

pub struct WithdrawPriorityFeeResult {
    pub tx_digest: sui::types::Digest,
}

/// Operations over Nexus network fee state.
pub struct NetworkActions {
    pub(super) client: NexusClient,
}

impl NetworkActions {
    /// Fetch and decode the priority fee vault state.
    pub async fn fetch_priority_fee_vault_state(
        &self,
    ) -> Result<PriorityFeeVaultStateV1, NexusError> {
        let client = self.client.operation_client().await?;
        Self::fetch_priority_fee_vault_state_with(&client).await
    }

    async fn fetch_priority_fee_vault_state_with(
        client: &NexusClient,
    ) -> Result<PriorityFeeVaultStateV1, NexusError> {
        client
            .crawler()
            .get_versioned_object::<PriorityFeeVault, PriorityFeeVaultStateV1>(
                *client.nexus_objects.priority_fee_vault.object_id(),
                1,
            )
            .await
            .map(|response| response.data)
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch priority fee vault state: {error}"
                ))
            })
    }

    /// Return the current vault share for a leader capability object ID.
    pub async fn priority_fee_share(
        &self,
        leader_cap: sui::types::Address,
    ) -> Result<u64, NexusError> {
        let client = self.client.operation_client().await?;
        let state = Self::fetch_priority_fee_vault_state_with(&client).await?;
        state.leader_share(leader_cap).ok_or_else(|| {
            NexusError::Configuration(format!(
                "Leader capability '{leader_cap}' has no priority fee share in the vault"
            ))
        })
    }

    /// Quote a leader priority fee withdrawal before constructing the transaction.
    pub async fn quote_priority_fee_withdrawal(
        &self,
        leader_cap: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Result<PriorityFeeWithdrawalQuote, NexusError> {
        let client = self.client.operation_client().await?;
        let state = Self::fetch_priority_fee_vault_state_with(&client).await?;
        state
            .quote_leader_withdrawal(leader_cap, share_to_withdraw)
            .ok_or_else(|| {
                NexusError::Configuration(format!(
                    "Invalid priority fee withdrawal for leader capability '{leader_cap}' and share '{share_to_withdraw}'"
                ))
            })
    }

    /// Configure the priority fee vault exchange rate.
    pub async fn configure_priority_fee_vault(
        &self,
        exchange_rate_million_mists_us: u64,
    ) -> Result<ConfigurePriorityFeeVaultResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let transaction = network::configure_priority_fee_vault(
            &client.nexus_objects,
            exchange_rate_million_mists_us,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;

        Ok(ConfigurePriorityFeeVaultResult {
            tx_digest: response.digest,
        })
    }

    /// Swap an owned `$US` coin for SUI and transfer both returned coins to the sender.
    pub async fn swap_us_for_sui(
        &self,
        us_coin: sui::types::Address,
        min_sui_out: u64,
    ) -> Result<SwapUsForSuiResult, NexusError> {
        let client = self.client.operation_client().await?;
        Self::swap_us_for_sui_with(&client, us_coin, min_sui_out).await
    }

    async fn swap_us_for_sui_with(
        client: &NexusClient,
        us_coin: sui::types::Address,
        min_sui_out: u64,
    ) -> Result<SwapUsForSuiResult, NexusError> {
        let address = client.owner()?;
        let us_coin = client
            .crawler()
            .get_object_metadata(us_coin)
            .await
            .map(|response| response.object_ref())
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch `$US` coin metadata: {error}"
                ))
            })?;
        let transaction =
            network::swap_us_for_sui(&client.nexus_objects, &us_coin, min_sui_out, address)
                .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        let swap = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::PriorityFeeSwap(swap) => Some(swap),
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "PriorityFeeSwapEvent not found in swap transaction response"
                ))
            })?;
        let us_spent = swap.us_in.checked_sub(swap.us_refunded).ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "PriorityFeeSwapEvent refund exceeds its `$US` input"
            ))
        })?;

        Ok(SwapUsForSuiResult {
            tx_digest: response.digest,
            us_spent,
            us_refunded: swap.us_refunded,
            sui_withdrawn: swap.sui_out,
        })
    }

    /// Drain all available SUI by swapping `$US` with a strict minimum output.
    pub async fn drain_priority_fee_vault_sui(
        &self,
        us_coin: sui::types::Address,
    ) -> Result<DrainPriorityFeeVaultSuiResult, NexusError> {
        let client = self.client.operation_client().await?;
        let state = Self::fetch_priority_fee_vault_state_with(&client).await?;
        let quote = state.quote_sui_drain().ok_or_else(|| {
            NexusError::Configuration(
                "Priority fee vault must have a configured exchange rate and positive SUI balance to drain"
                    .to_owned(),
            )
        })?;
        let min_sui_out = quote.sui_out;
        let result = Self::swap_us_for_sui_with(&client, us_coin, min_sui_out).await?;

        Ok(DrainPriorityFeeVaultSuiResult {
            tx_digest: result.tx_digest,
            exchange_rate_million_mists_us: quote.exchange_rate_million_mists_us,
            sui_balance_before: quote.sui_out,
            min_sui_out,
        })
    }

    /// Withdraw a leader's `$US` priority fee share to the sender.
    pub async fn withdraw_priority_fee(
        &self,
        leader_cap: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Result<WithdrawPriorityFeeResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let leader_cap = client
            .crawler()
            .get_object_metadata(leader_cap)
            .await
            .map(|response| response.object_ref())
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch leader capability metadata: {error}"
                ))
            })?;
        let transaction = network::withdraw_priority_fee(
            &client.nexus_objects,
            &leader_cap,
            share_to_withdraw,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;

        Ok(WithdrawPriorityFeeResult {
            tx_digest: response.digest,
        })
    }
}
