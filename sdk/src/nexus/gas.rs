//! Commands related to gas management in Nexus.
//!
//! - [`GasActions::add_budget`] to add gas budget for Nexus workflows.

use crate::{
    nexus::{client::NexusClient, error::NexusError},
    object_crawler::fetch_one,
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
        coin_object_id: sui::ObjectID,
    ) -> Result<AddBudgetResult, NexusError> {
        let address = self.client.signer.get_active_address().await?;
        let nexus_objects = &self.client.nexus_objects;
        let sui_client = &self.client.signer.get_client().await?;
        let coin = fetch_one::<serde_json::Value>(sui_client, coin_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        if let Err(e) = gas::add_budget(&mut tx, nexus_objects, address.into(), &coin.object_ref())
        {
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
    use {
        crate::{
            sui,
            test_utils::{nexus_mocks, sui_mocks},
        },
        std::collections::BTreeMap,
    };

    #[tokio::test]
    async fn test_gas_actions_add_budget() {
        let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;

        let coin_object_id = sui::ObjectID::random();
        let coin_object = sui::ParsedMoveObject {
            type_: sui::MoveStructTag {
                address: *nexus_client.nexus_objects.workflow_pkg_id,
                module: sui::move_ident_str!("coin").into(),
                name: sui::move_ident_str!("Coin").into(),
                type_params: vec![],
            },
            has_public_transfer: false,
            fields: sui::MoveStruct::WithFields(BTreeMap::from([(
                "test".into(),
                sui::MoveValue::Number(1),
            )])),
        };

        let get_object_call =
            sui_mocks::rpc::mock_read_api_get_object(&mut server, coin_object_id, coin_object);

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

        let result = nexus_client
            .gas()
            .add_budget(coin_object_id)
            .await
            .expect("Failed to add budget");

        get_object_call.assert_async().await;

        execute_call.assert_async().await;
        confirm_call.assert_async().await;

        assert_eq!(result.tx_digest, tx_digest);
    }
}
