//! Commands related to gas management in Nexus.
//!
//! - [`GasActions::add_budget`] to add gas budget for Nexus workflows.

use crate::{
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::gas,
    types::Tool,
};

pub struct AddBudgetResult {
    pub tx_digest: sui::types::Digest,
}

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

pub struct GasActions {
    pub(super) client: NexusClient,
}

impl GasActions {
    /// Add a Coin [`sui::types::ObjectReference`] as gas budget for Nexus workflows.
    pub async fn add_budget(
        &self,
        coin_object_id: sui::types::Address,
    ) -> Result<AddBudgetResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        let coin = self
            .client
            .crawler()
            .get_object_metadata(coin_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::add_budget(&mut tx, nexus_objects, address, &coin.object_ref()) {
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

        Ok(AddBudgetResult {
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
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}",
                    owner_cap = owner_cap,
                    e = e
                ))
            })?;

        let tool = self.fetch_tool_object_ref(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) =
            gas::enable_expiry(&mut tx, nexus_objects, &tool, &owner_cap, cost_per_minute)
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

        let tool = self.fetch_tool_object_ref(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::disable_expiry(&mut tx, nexus_objects, &tool, &owner_cap) {
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
                    "Failed to fetch coin object metadata for '{coin}': {e}",
                    coin = coin,
                    e = e
                ))
            })?;

        let tool = self.fetch_tool_object_ref(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::buy_limited_invocations_gas_ticket(
            &mut tx,
            nexus_objects,
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
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}",
                    owner_cap = owner_cap,
                    e = e
                ))
            })?;

        let tool = self.fetch_tool_object_ref(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::enable_limited_invocations(
            &mut tx,
            nexus_objects,
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
                    "Failed to fetch OwnerCap object metadata for '{owner_cap}': {e}",
                    owner_cap = owner_cap,
                    e = e
                ))
            })?;

        let tool = self.fetch_tool_object_ref(&tool_fqn).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = gas::disable_limited_invocations(&mut tx, nexus_objects, &tool, &owner_cap)
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
                    "Failed to fetch coin object metadata for '{coin}': {e}",
                    coin = coin,
                    e = e
                ))
            })?;

        let tool = self.fetch_tool_object_ref(&tool_fqn).await?;

        // Craft the transaction.
        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) =
            gas::buy_expiry_gas_ticket(&mut tx, nexus_objects, &tool, &pay_with_coin, minutes)
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

        Ok(BuyExpiryTicketResult {
            tx_digest: response.digest,
        })
    }

    /// Helper to fetch the tool object metadata reference from a ToolFqn.
    async fn fetch_tool_object_ref(
        &self,
        tool_fqn: &crate::tool_fqn::ToolFqn,
    ) -> Result<sui::types::ObjectReference, NexusError> {
        let crawler = self.client.crawler();

        let tool_id = Tool::derive_id(
            *self.client.nexus_objects.tool_registry.object_id(),
            tool_fqn,
        )
        .map_err(NexusError::Parsing)?;

        crawler
            .get_object_metadata(tool_id)
            .await
            .map(|resp| resp.object_ref())
            .map_err(|e| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch tool derived object for tool '{tool_fqn}': {e}",
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_gas_actions_enable_expiry_extension() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_id =
            crate::types::Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
                .unwrap();
        let tool_object_ref = sui::types::ObjectReference::new(tool_id, 0, tx_digest);
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
            tool_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x2")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

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
        let tool_id =
            crate::types::Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
                .unwrap();
        let tool_object_ref = sui::types::ObjectReference::new(tool_id, 0, tx_digest);
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
            tool_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x2")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

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
        let tool_id =
            crate::types::Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
                .unwrap();
        let tool_object_ref = sui::types::ObjectReference::new(tool_id, 0, tx_digest);
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
            tool_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x2")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

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
        let tool_id =
            crate::types::Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
                .unwrap();
        let tool_object_ref = sui::types::ObjectReference::new(tool_id, 0, tx_digest);
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
            tool_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x2")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

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
        let tool_id =
            crate::types::Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
                .unwrap();
        let tool_object_ref = sui::types::ObjectReference::new(tool_id, 0, tx_digest);
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
            tool_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x2")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

        let result = client
            .gas()
            .disable_limited_invocations_extension(tool_fqn, owner_cap_id)
            .await
            .expect("Failed to disable limited invocations extension");

        assert_eq!(result.tx_digest, tx_digest);
    }
    use crate::{
        fqn,
        sui,
        test_utils::{nexus_mocks, sui_mocks},
    };

    #[tokio::test]
    async fn test_gas_actions_add_budget() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let coin_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(coin_object_id, 0, tx_digest),
            sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

        let result = client
            .gas()
            .add_budget(coin_object_id)
            .await
            .expect("Failed to add budget");

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
        let tool_id =
            crate::types::Tool::derive_id(*nexus_objects.tool_registry.object_id(), &tool_fqn)
                .unwrap();
        let tool_object_ref = sui::types::ObjectReference::new(tool_id, 0, tx_digest);

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
            tool_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x2")),
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
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

        let result = client
            .gas()
            .buy_expiry_ticket(tool_fqn, 60, coin_object_id)
            .await
            .expect("Failed to buy expiry ticket");

        assert_eq!(result.tx_digest, tx_digest);
    }
}
