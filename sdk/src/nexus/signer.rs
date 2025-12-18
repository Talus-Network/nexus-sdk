//! Module defining a [`Signer`] struct that can sign and execute transactions
//! on Sui in Nexus context.
use {
    crate::{
        events::{FromSuiGrpcEvent, NexusEvent},
        nexus::{crawler::Crawler, error::NexusError},
        sui::{self, traits::*},
        types::NexusObjects,
    },
    futures::TryStreamExt,
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
        let (response, digest, checkpoint) = self
            .execute_tx_and_wait_for_checkpoint(tx, signature)
            .await?;

        // Deserialize effects.
        let Ok(sui::types::TransactionEffects::V2(effects)) =
            sui::types::TransactionEffects::try_from(response.effects())
        else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Failed to read transaction effects."
            )));
        };

        if let sui::types::ExecutionStatus::Failure { error, command } = effects.status() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Transaction execution failed: {error:?} in command: {command:?}"
            )));
        }

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

        // Re-fetch the gas coin's new reference.
        let crawler = Crawler::new(self.client.clone());

        let gas_coin_ref = crawler
            .get_object_metadata(*gas_coin.object_id())
            .await
            .map_err(|e| NexusError::Rpc(e.into()))?
            .object_ref();

        *gas_coin = gas_coin_ref;

        Ok(ExecutedTransaction {
            effects: *effects,
            events: nexus_events.collect(),
            objects,
            digest,
            checkpoint,
        })
    }

    /// Execute a transaction while subscribing to a checkpoint stream to confirm
    /// its inclusion in a checkpoint.
    async fn execute_tx_and_wait_for_checkpoint(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
    ) -> Result<(sui::grpc::ExecutedTransaction, sui::types::Digest, u64), NexusError> {
        let mut client = self.client.lock().await;

        let checkpoints_request = sui::grpc::SubscribeCheckpointsRequest::default().with_read_mask(
            sui::grpc::FieldMask::from_paths(["transactions.digest", "sequence_number"]),
        );

        let tx_request = sui::grpc::ExecuteTransactionRequest::default()
            .with_transaction(tx)
            .with_signatures(vec![signature.into()])
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "effects.bcs",
                "events.events",
                "objects.objects",
                "digest",
            ]));

        // Subscribe to checkpoint stream before execution.
        let mut checkpoint_stream = match client
            .subscription_client()
            .subscribe_checkpoints(checkpoints_request)
            .await
        {
            Ok(stream) => stream.into_inner(),
            Err(e) => return Err(NexusError::Rpc(e.into())),
        };

        let response = match client
            .execution_client()
            .execute_transaction(tx_request)
            .await
        {
            Ok(resp) => resp.into_inner().transaction.ok_or_else(|| {
                NexusError::Wallet(anyhow::anyhow!("No transaction in execution response"))
            })?,
            Err(e) => return Err(NexusError::Rpc(e.into())),
        };

        // Get the executed transaction digest to find it in the checkpoint
        // stream.
        let digest: sui::types::Digest = response
            .digest()
            .parse()
            .map_err(|e: sui::types::DigestParseError| NexusError::Parsing(e.into()))?;

        // Wait for the transaction to appear in a checkpoint.
        let timeout_future = tokio::time::sleep(self.transaction_timeout);
        let checkpoint_future = async {
            while let Some(response) = checkpoint_stream.try_next().await? {
                let checkpoint = response.checkpoint();

                for tx in checkpoint.transactions() {
                    if tx.digest() == digest.to_string() {
                        return Ok(checkpoint.sequence_number());
                    }
                }
            }

            Err(anyhow::anyhow!(
                "Checkpoint stream closed before transaction was confirmed."
            ))
        };

        tokio::select! {
            result = checkpoint_future => {
                match result {
                    Ok(sequence_number) => Ok((response, digest, sequence_number)),
                    Err(e) => Err(NexusError::Rpc(e))
                }
            },
            _ = timeout_future => {
                Err(NexusError::Timeout(anyhow::anyhow!("Transaction confirmation timed out")))
            }
        }
    }
}
