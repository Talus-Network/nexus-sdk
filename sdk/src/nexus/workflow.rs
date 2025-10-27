//! Commands related to gas management in Nexus.

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
    /// Deploy the provided JSON DAG.
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
