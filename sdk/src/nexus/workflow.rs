//! Commands related to gas management in Nexus.

use {
    crate::{
        idents::workflow,
        nexus::{client::NexusClient, error::NexusError},
        sui,
        transactions::dag,
        types::Dag,
    },
    anyhow::anyhow,
};

pub struct PublishResult {
    pub tx_digest: sui::TransactionDigest,
    pub dag_object_id: sui::ObjectID,
}

pub struct WorkflowActions {
    pub(super) client: NexusClient,
}

impl WorkflowActions {
    /// Publish the provided JSON [`Dag`].
    pub async fn publish(&self, json_dag: Dag) -> Result<PublishResult, NexusError> {
        let address = self.client.signer.get_active_address().await?;
        let nexus_objects = &self.client.nexus_objects;

        // == Craft and submit the publish DAG transaction ==

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        let mut dag_arg = dag::empty(&mut tx, nexus_objects);

        dag_arg = match dag::create(&mut tx, nexus_objects, dag_arg, json_dag) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                return Err(NexusError::TransactionBuilding(e));
            }
        };

        dag::publish(&mut tx, nexus_objects, dag_arg);

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

        // == Find the published DAG object ID ==

        let dag_object_id = response
            .object_changes
            .unwrap_or_default()
            .into_iter()
            .find_map(|change| match change {
                sui::ObjectChange::Created {
                    object_type,
                    object_id,
                    ..
                } if object_type.address == *nexus_objects.workflow_pkg_id
                    && object_type.module == workflow::Dag::DAG.module.into()
                    && object_type.name == workflow::Dag::DAG.name.into() =>
                {
                    Some(object_id)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG object ID not found in TX response"))
            })?;

        Ok(PublishResult {
            tx_digest: response.digest,
            dag_object_id,
        })
    }
}
