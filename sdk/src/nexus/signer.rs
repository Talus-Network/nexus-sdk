//! Module defining a [`Signer`] struct that can sign and execute transactions
//! on Sui in Nexus context.
use {
    crate::{
        events::{FromSuiGrpcEvent, NexusEvent},
        nexus::error::NexusError,
        sui::{self, traits::*},
        types::NexusObjects,
    },
    std::sync::Arc,
    tokio::{sync::Mutex, time::Duration},
};

/// Resulting struct from executing a transaction.
pub struct ExecutedTransaction {
    pub effects: sui::types::TransactionEffectsV2,
    pub events: Vec<NexusEvent>,
    pub objects: Vec<sui::types::Object>,
    pub digest: sui::types::Digest,
    pub checkpoint: u64,
}

/// We want to provide flexibility when it comes to signing transactions. We
/// accept both - a [`sui::WalletContext`] and a tuple of a [`sui::Client`] and
/// a secret mnemonic string.
#[derive(Clone)]
pub struct Signer {
    pub(super) client: Arc<Mutex<sui::grpc::Client>>,
    pub(super) pk: sui::crypto::Ed25519PrivateKey,
    pub(super) transaction_timeout: Duration,
    pub(super) nexus_objects: Arc<NexusObjects>,
}

impl Signer {
    pub fn new(
        client: Arc<Mutex<sui::grpc::Client>>,
        pk: sui::crypto::Ed25519PrivateKey,
        transaction_timeout: Duration,
        nexus_objects: Arc<NexusObjects>,
    ) -> Self {
        Self {
            client,
            pk,
            transaction_timeout,
            nexus_objects,
        }
    }

    /// Get the active address from the signer.
    pub fn get_active_address(&self) -> sui::types::Address {
        self.pk.public_key().derive_address()
    }

    /// Sign a transaction block using the signer.
    pub async fn sign_tx(
        &self,
        tx: &sui::types::Transaction,
    ) -> Result<sui::types::UserSignature, NexusError> {
        self.pk
            .sign_transaction(tx)
            .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))
    }

    /// Execute a transaction block and return the response.
    pub async fn execute_tx(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
        gas_coin: &mut sui::types::ObjectReference,
    ) -> Result<ExecutedTransaction, NexusError> {
        let mut client = self.client.lock().await;

        let request = sui::grpc::ExecuteTransactionRequest::default()
            .with_transaction(tx)
            .with_signatures(vec![signature.into()])
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "effects.bcs",
                "events.events",
                "objects.objects",
                "digest",
                "checkpoint",
            ]));

        let response = client
            .execution_client()
            .execute_transaction(request)
            .await
            .map(|res| res.into_inner().transaction)
            .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))?
            .ok_or_else(|| NexusError::Wallet(anyhow::anyhow!("No transaction in response")))?;

        drop(client);

        let checkpoint = response.checkpoint();
        let digest = response
            .digest()
            .parse()
            .map_err(|e: sui::types::DigestParseError| NexusError::Parsing(e.into()))?;

        // Wait for the transaction to be included in a checkpoint.
        tokio::select! {
            _ = self.confirm_tx(checkpoint, &digest) => (),
            _ = tokio::time::sleep(self.transaction_timeout) => {
                return Err(NexusError::Timeout(anyhow::anyhow!("Transaction confirmation timed out")));
            }
        }

        // Deserialize effects.
        let Ok(sui::types::TransactionEffects::V2(effects)) =
            sui::types::TransactionEffects::try_from(response.effects())
        else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Failed to read transaction effects."
            )));
        };

        // Deserialize events.
        let Ok(events) = sui::types::TransactionEvents::try_from(response.events()) else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Failed to read transaction events."
            )));
        };

        let nexus_events = events.0.iter().enumerate().filter_map(|(index, event)| {
            NexusEvent::from_sui_grpc_event(index as u64, digest, event, &self.nexus_objects).ok()
        });

        // Deserialize objects.
        let Ok(objects) = response
            .objects()
            .objects()
            .iter()
            .map(sui::types::Object::try_from)
            .collect::<Result<Vec<_>, _>>()
        else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Failed to read transaction objects."
            )));
        };

        if let sui::types::ExecutionStatus::Failure { error, command } = effects.status() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Transaction execution failed: {error:?} in command: {command:?}"
            )));
        }

        if let Some(new_gas_object) = effects
            .gas_object_index
            .and_then(|index| effects.changed_objects.get(index as usize))
        {
            let sui::types::ObjectOut::ObjectWrite { digest, .. } = new_gas_object.output_state
            else {
                return Err(NexusError::Wallet(anyhow::anyhow!(
                    "Gas object change is not an ObjectWrite."
                )));
            };

            // Version is incremented and digest is updated.
            *gas_coin = sui::types::ObjectReference::new(
                new_gas_object.object_id,
                gas_coin.version() + 1,
                digest,
            );
        }

        let evts = nexus_events.collect();

        Ok(ExecutedTransaction {
            effects: *effects,
            events: evts,
            objects,
            digest,
            checkpoint,
        })
    }

    /// Confirm that a transaction has been included in a checkpoint.
    async fn confirm_tx(&self, checkpoint: u64, digest: &sui::types::Digest) -> () {
        loop {
            let mut client = self.client.lock().await;

            let request = sui::grpc::GetCheckpointRequest::default()
                .with_sequence_number(checkpoint)
                .with_read_mask(sui::grpc::FieldMask::from_paths(["transactions.digest"]));

            let response = match client
                .ledger_client()
                .get_checkpoint(request)
                .await
                .map(|res| res.into_inner().checkpoint)
            {
                Ok(Some(resp)) => resp,
                _ => continue,
            };

            if response
                .transactions()
                .iter()
                .any(|tx| tx.digest() == digest.to_string())
            {
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}
