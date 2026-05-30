//! Commands related to gas management in Nexus.

use crate::{
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::gas,
    types::{PriorityFeeVaultState, PriorityFeeWithdrawalQuote},
};

pub struct BuyExpiryTicketResult {
    pub tx_digest: sui::types::Digest,
}

pub struct EnableExpiryExtensionResult {
    pub tx_digest: sui::types::Digest,
}

pub struct DisableExpiryExtensionResult {
    pub tx_digest: sui::types::Digest,
}

pub struct BuyLimitedInvocationsTicketResult {
    pub tx_digest: sui::types::Digest,
}

pub struct EnableLimitedInvocationsExtensionResult {
    pub tx_digest: sui::types::Digest,
}

pub struct DisableLimitedInvocationsExtensionResult {
    pub tx_digest: sui::types::Digest,
}

pub struct ConfigurePriorityFeeVaultResult {
    pub tx_digest: sui::types::Digest,
}

pub struct SwapUsForSuiResult {
    pub tx_digest: sui::types::Digest,
}

#[derive(Debug)]
pub struct DrainPriorityFeeVaultSuiResult {
    pub tx_digest: sui::types::Digest,
    pub exchange_rate: u64,
    pub sui_balance_before: u64,
    pub min_sui_out: u64,
}

pub struct WithdrawPriorityFeeResult {
    pub tx_digest: sui::types::Digest,
}

pub struct GasActions {
    pub(super) client: NexusClient,
}

impl GasActions {
    /// Fetch and decode the priority fee vault state.
    pub async fn fetch_priority_fee_vault_state(
        &self,
    ) -> Result<PriorityFeeVaultState, NexusError> {
        self.client
            .crawler()
            .get_object::<PriorityFeeVaultState>(
                *self.client.nexus_objects.priority_fee_vault.object_id(),
            )
            .await
            .map(|response| response.data)
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch priority fee vault state: {e}"
                ))
            })
    }

    /// Return the current vault share for a leader cap object ID.
    pub async fn priority_fee_share(
        &self,
        leader_cap: sui::types::Address,
    ) -> Result<u64, NexusError> {
        let state = self.fetch_priority_fee_vault_state().await?;
        state.leader_share(leader_cap).ok_or_else(|| {
            NexusError::Configuration(format!(
                "Leader cap '{leader_cap}' has no priority fee share in the vault"
            ))
        })
    }

    /// Quote a leader priority-fee withdrawal before constructing the PTB.
    pub async fn quote_priority_fee_withdrawal(
        &self,
        leader_cap: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Result<PriorityFeeWithdrawalQuote, NexusError> {
        let state = self.fetch_priority_fee_vault_state().await?;
        state
            .quote_leader_withdrawal(leader_cap, share_to_withdraw)
            .ok_or_else(|| {
                NexusError::Configuration(format!(
                    "Invalid priority fee withdrawal for leader cap '{leader_cap}' and share '{share_to_withdraw}'"
                ))
            })
    }

    /// Configure the priority fee vault exchange rate and embedded TAP agent.
    pub async fn configure_priority_fee_vault(
        &self,
        exchange_rate: u64,
        operator: sui::types::Address,
    ) -> Result<ConfigurePriorityFeeVaultResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let mut tx = sui::tx::TransactionBuilder::new();

        gas::configure_priority_fee_vault(&mut tx, nexus_objects, exchange_rate, operator)
            .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

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
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();
        let us_coin = crawler
            .get_object_metadata(us_coin)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!("Failed to fetch `$US` coin metadata: {e}"))
            })?;

        let mut tx = sui::tx::TransactionBuilder::new();
        let result = gas::swap_us_for_sui(&mut tx, nexus_objects, &us_coin, min_sui_out)
            .map_err(NexusError::TransactionBuilding)?;
        let Some(sui_out) = result.nested(0) else {
            return Err(NexusError::TransactionBuilding(anyhow::anyhow!(
                "Failed to extract SUI output from swap_us_for_sui result"
            )));
        };
        let Some(us_refund) = result.nested(1) else {
            return Err(NexusError::TransactionBuilding(anyhow::anyhow!(
                "Failed to extract `$US` refund from swap_us_for_sui result"
            )));
        };
        let recipient =
            tx.input(crate::idents::pure_arg(&address).map_err(NexusError::TransactionBuilding)?);
        tx.transfer_objects(vec![sui_out, us_refund], recipient);

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(SwapUsForSuiResult {
            tx_digest: response.digest,
        })
    }

    /// Drain all currently available SUI from the priority fee vault by swapping an owned `$US`
    /// coin with strict minimum output set to the current vault SUI balance.
    pub async fn drain_priority_fee_vault_sui(
        &self,
        us_coin: sui::types::Address,
    ) -> Result<DrainPriorityFeeVaultSuiResult, NexusError> {
        let state = self.fetch_priority_fee_vault_state().await?;
        let quote = state.quote_sui_drain().ok_or_else(|| {
            NexusError::Configuration(
                "Priority fee vault must have a configured exchange rate and positive SUI balance to drain"
                    .to_owned(),
            )
        })?;
        let min_sui_out = quote.sui_out;
        let result = self.swap_us_for_sui(us_coin, min_sui_out).await?;

        Ok(DrainPriorityFeeVaultSuiResult {
            tx_digest: result.tx_digest,
            exchange_rate: quote.exchange_rate,
            sui_balance_before: quote.sui_out,
            min_sui_out,
        })
    }

    /// Withdraw a leader's `$US` priority-fee share and transfer it to the sender.
    pub async fn withdraw_priority_fee(
        &self,
        leader_cap: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Result<WithdrawPriorityFeeResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();
        let leader_cap = crawler
            .get_object_metadata(leader_cap)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!("Failed to fetch leader cap metadata: {e}"))
            })?;

        let mut tx = sui::tx::TransactionBuilder::new();
        let us_out = gas::withdraw_priority_fee(
            &mut tx,
            nexus_objects,
            &nexus_objects.priority_fee_vault,
            &leader_cap,
            share_to_withdraw,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let recipient =
            tx.input(crate::idents::pure_arg(&address).map_err(NexusError::TransactionBuilding)?);
        tx.transfer_objects(vec![us_out], recipient);

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(WithdrawPriorityFeeResult {
            tx_digest: response.digest,
        })
    }

    /// Enable the expiry gas extension for the specified tool.
    pub async fn enable_expiry_extension(
        &self,
        tool_fqn: crate::tool_fqn::ToolFqn,
        owner_cap: sui::types::Address,
        cost_per_minute: u64,
    ) -> Result<EnableExpiryExtensionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();

        let owner_cap = crawler
            .get_object_metadata(owner_cap)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}"
                ))
            })?;

        let tool = self.client.fetch_tool(&tool_fqn).await?;
        let tool_gas = self.client.fetch_tool_gas(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::enable_expiry(
            &mut tx,
            nexus_objects,
            &tool_gas,
            &tool,
            &owner_cap,
            cost_per_minute,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(EnableExpiryExtensionResult {
            tx_digest: response.digest,
        })
    }

    /// Disable the expiry gas extension for the specified tool.
    pub async fn disable_expiry_extension(
        &self,
        tool_fqn: crate::tool_fqn::ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<DisableExpiryExtensionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();

        let owner_cap = crawler
            .get_object_metadata(owner_cap)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}",
                ))
            })?;

        let tool = self.client.fetch_tool(&tool_fqn).await?;
        let tool_gas = self.client.fetch_tool_gas(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::disable_expiry(&mut tx, nexus_objects, &tool_gas, &tool, &owner_cap) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(DisableExpiryExtensionResult {
            tx_digest: response.digest,
        })
    }

    /// Buy a limited invocations gas ticket for a tool.
    pub async fn buy_limited_invocations_ticket(
        &self,
        tool_fqn: crate::tool_fqn::ToolFqn,
        invocations: u64,
        coin: sui::types::Address,
    ) -> Result<BuyLimitedInvocationsTicketResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();

        let pay_with_coin = crawler
            .get_object_metadata(coin)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch coin object metadata for '{coin}': {e}"
                ))
            })?;

        let tool = self.client.fetch_tool(&tool_fqn).await?;
        let tool_gas = self.client.fetch_tool_gas(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::buy_limited_invocations_gas_ticket(
            &mut tx,
            nexus_objects,
            &tool_gas,
            &tool,
            &pay_with_coin,
            invocations,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(BuyLimitedInvocationsTicketResult {
            tx_digest: response.digest,
        })
    }

    /// Enable the limited invocations gas extension for the specified tool.
    pub async fn enable_limited_invocations_extension(
        &self,
        tool_fqn: crate::tool_fqn::ToolFqn,
        owner_cap: sui::types::Address,
        cost_per_invocation: u64,
        min_invocations: u64,
        max_invocations: u64,
    ) -> Result<EnableLimitedInvocationsExtensionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();

        let owner_cap = crawler
            .get_object_metadata(owner_cap)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}"
                ))
            })?;

        let tool = self.client.fetch_tool(&tool_fqn).await?;
        let tool_gas = self.client.fetch_tool_gas(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::enable_limited_invocations(
            &mut tx,
            nexus_objects,
            &tool_gas,
            &tool,
            &owner_cap,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(EnableLimitedInvocationsExtensionResult {
            tx_digest: response.digest,
        })
    }

    /// Disable the limited invocations gas extension for the specified tool.
    pub async fn disable_limited_invocations_extension(
        &self,
        tool_fqn: crate::tool_fqn::ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<DisableLimitedInvocationsExtensionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();

        let owner_cap = crawler
            .get_object_metadata(owner_cap)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}"
                ))
            })?;

        let tool = self.client.fetch_tool(&tool_fqn).await?;
        let tool_gas = self.client.fetch_tool_gas(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) =
            gas::disable_limited_invocations(&mut tx, nexus_objects, &tool_gas, &tool, &owner_cap)
        {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(DisableLimitedInvocationsExtensionResult {
            tx_digest: response.digest,
        })
    }

    /// Buy an expiry gas ticket for a tool for a given number of minutes.
    pub async fn buy_expiry_ticket(
        &self,
        tool_fqn: crate::tool_fqn::ToolFqn,
        minutes: u64,
        coin: sui::types::Address,
    ) -> Result<BuyExpiryTicketResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();

        let pay_with_coin = crawler
            .get_object_metadata(coin)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch coin object metadata for '{coin}': {e}"
                ))
            })?;

        let tool = self.client.fetch_tool(&tool_fqn).await?;
        let tool_gas = self.client.fetch_tool_gas(&tool_fqn).await?;

        // Craft the transaction.
        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::buy_expiry_gas_ticket(
            &mut tx,
            nexus_objects,
            &tool_gas,
            &tool,
            &pay_with_coin,
            minutes,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(BuyExpiryTicketResult {
            tx_digest: response.digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        fqn,
        sui,
        test_utils::{nexus_mocks, sui_mocks},
    };

    #[tokio::test]
    async fn test_gas_actions_enable_expiry_extension() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let owner_cap_id = sui::types::Address::generate(&mut rng);
        let owner_cap_object_ref = sui::types::ObjectReference::new(owner_cap_id, 0, tx_digest);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock owner cap object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            owner_cap_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x3")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        // Mock tool gas object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .gas()
            .enable_expiry_extension(tool_fqn, owner_cap_id, 1234)
            .await
            .expect("Failed to enable expiry extension");

        assert_eq!(result.tx_digest, tx_digest);
    }

    #[tokio::test]
    async fn test_gas_actions_disable_expiry_extension() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let owner_cap_id = sui::types::Address::generate(&mut rng);
        let owner_cap_object_ref = sui::types::ObjectReference::new(owner_cap_id, 0, tx_digest);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock owner cap object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            owner_cap_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x3")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        // Mock tool gas object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .gas()
            .disable_expiry_extension(tool_fqn, owner_cap_id)
            .await
            .expect("Failed to disable expiry extension");

        assert_eq!(result.tx_digest, tx_digest);
    }

    #[tokio::test]
    async fn test_gas_actions_buy_limited_invocations_ticket() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let coin_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock coin object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(coin_object_id, 0, tx_digest),
            sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        // Mock tool gas object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .gas()
            .buy_limited_invocations_ticket(tool_fqn, 42, coin_object_id)
            .await
            .expect("Failed to buy limited invocations ticket");

        assert_eq!(result.tx_digest, tx_digest);
    }

    #[tokio::test]
    async fn test_gas_actions_enable_limited_invocations_extension() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let owner_cap_id = sui::types::Address::generate(&mut rng);
        let owner_cap_object_ref = sui::types::ObjectReference::new(owner_cap_id, 0, tx_digest);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock owner cap object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            owner_cap_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x3")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        // Mock tool gas object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .gas()
            .enable_limited_invocations_extension(tool_fqn, owner_cap_id, 555, 10, 100)
            .await
            .expect("Failed to enable limited invocations extension");

        assert_eq!(result.tx_digest, tx_digest);
    }

    #[tokio::test]
    async fn test_gas_actions_disable_limited_invocations_extension() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let owner_cap_id = sui::types::Address::generate(&mut rng);
        let owner_cap_object_ref = sui::types::ObjectReference::new(owner_cap_id, 0, tx_digest);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock owner cap object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            owner_cap_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x3")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        // Mock tool gas object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .gas()
            .disable_limited_invocations_extension(tool_fqn, owner_cap_id)
            .await
            .expect("Failed to disable limited invocations extension");

        assert_eq!(result.tx_digest, tx_digest);
    }

    #[tokio::test]
    async fn test_gas_actions_buy_expiry_ticket() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let coin_object_id = sui::types::Address::generate(&mut rng);

        // Tool FQN and derived tool id
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock coin object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(coin_object_id, 0, tx_digest),
            sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        // Mock tool gas object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .gas()
            .buy_expiry_ticket(tool_fqn, 60, coin_object_id)
            .await
            .expect("Failed to buy expiry ticket");

        assert_eq!(result.tx_digest, tx_digest);
    }

    #[tokio::test]
    async fn test_gas_actions_drain_priority_fee_vault_sui_rejects_empty_vault() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            nexus_objects.priority_fee_vault.clone(),
            sui::types::Owner::Shared(1),
            serde_json::json!({
                "fields": {
                    "sui_balance": { "fields": { "value": "0" } },
                    "us_balance": { "fields": { "value": "0" } },
                    "exchange_rate": "10",
                    "total_share": "0",
                    "leader_accounts": { "fields": { "contents": [] } },
                    "tap_agent_registered": true,
                    "tap_agent_operator": "0x7"
                }
            }),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let err = client
            .gas()
            .drain_priority_fee_vault_sui(sui::types::Address::from_static("0x99"))
            .await
            .expect_err("empty vault should not build a drain swap");

        assert!(
            err.to_string()
                .contains("configured exchange rate and positive SUI balance"),
            "unexpected error: {err}"
        );
    }
}
