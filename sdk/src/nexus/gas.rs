//! Commands related to gas management in Nexus.
//!
//! - [`GasActions::add_budget`] to add gas budget for Nexus workflows.

use crate::{
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::gas,
};

pub struct AddBudgetResult {
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
}

#[cfg(test)]
mod tests {
    use crate::{
        sui,
        test_utils::{nexus_mocks, sui_mocks},
    };

    #[tokio::test]
    async fn test_gas_actions_add_budget() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_digest = sui::types::Digest::generate(&mut rng);
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
            tx_digest,
            gas_coin_digest,
            vec![],
            vec![],
            vec![],
        );

        let grpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &grpc_url, None).await;

        let result = client
            .gas()
            .add_budget(coin_object_id)
            .await
            .expect("Failed to add budget");

        assert_eq!(result.tx_digest, tx_digest);
    }
}
