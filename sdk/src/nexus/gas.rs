//! Commands related to gas management in Nexus.
//!
//! - [`GasActions::add_budget`] to add gas budget for Nexus workflows.

use crate::{
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::gas,
};

pub struct AddBudgetResult {
    pub tx_digest: sui::TransactionDigest,
}

pub struct GasActions {
    pub(super) client: NexusClient,
}

impl GasActions {
    /// Add a Coin [`sui::ObjectRef`] as gas budget for Nexus workflows.
    pub async fn add_budget(
        &self,
        budget_coin: &sui::ObjectRef,
    ) -> Result<AddBudgetResult, NexusError> {
        let address = self.client.signer.get_active_address().await?;
        let nexus_objects = &self.client.nexus_objects;

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        if let Err(e) = gas::add_budget(&mut tx, nexus_objects, address.into(), &budget_coin) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        let tx_data = sui::TransactionData::new_programmable(
            address,
            vec![gas_coin.to_object_ref()],
            tx.finish(),
            self.client.gas.get_budget(),
            self.client.reference_gas_price,
        );

        let envelope = self.client.signer.sign_tx(tx_data).await?;

        let response = self
            .client
            .signer
            .execute_tx(envelope, &mut gas_coin)
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
        let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;

        let tx_digest = sui::TransactionDigest::random();
        let (execute_call, confirm_call) =
            sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
                &mut server,
                tx_digest,
                None,
                None,
                None,
                None,
            );

        let coin = sui_mocks::mock_sui_object_ref();

        let result = nexus_client
            .gas()
            .add_budget(&coin)
            .await
            .expect("Failed to add budget");

        execute_call.assert_async().await;
        confirm_call.assert_async().await;

        assert_eq!(result.tx_digest, tx_digest);
    }
}
