//! Commands related to gas management in Nexus.

use crate::{
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::gas,
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

pub struct GasActions {
    pub(super) client: NexusClient,
}

impl GasActions {
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

        let tx =
            gas::enable_expiry_ptb(nexus_objects, &tool_gas, &tool, &owner_cap, cost_per_minute)
                .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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

        let tx = gas::disable_expiry_ptb(nexus_objects, &tool_gas, &tool, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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

        let tx = gas::buy_limited_invocations_gas_ticket_ptb(
            nexus_objects,
            &tool_gas,
            &tool,
            &pay_with_coin,
            invocations,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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

        let tx = gas::enable_limited_invocations_ptb(
            nexus_objects,
            &tool_gas,
            &tool,
            &owner_cap,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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

        let tx = gas::disable_limited_invocations_ptb(nexus_objects, &tool_gas, &tool, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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

        let tx = gas::buy_expiry_gas_ticket_ptb(
            nexus_objects,
            &tool_gas,
            &tool,
            &pay_with_coin,
            minutes,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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
}
