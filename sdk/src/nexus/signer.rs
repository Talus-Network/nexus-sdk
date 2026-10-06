//! Module defining a [`Signer`] struct that can sign and execute transactions
//! on Sui in Nexus context.
use {
    crate::{
        events::{NexusEvent, NexusEventDecoder},
        nexus::{
            crawler::Crawler,
            error::{NexusError, TransactionError},
        },
        sui,
    },
    std::sync::Arc,
    tokio::time::Duration,
};

/// Resulting struct from executing a transaction.
pub struct ExecutedTransaction {
    pub effects: sui::types::TransactionEffectsV2,
    pub events: Vec<NexusEvent>,
    pub objects: Vec<sui::types::Object>,
    pub digest: sui::types::Digest,
    pub checkpoint: u64,
}

/// The Signer struct capable of signing and executing transactions based on the
/// provided [`sui::crypto::Ed25519PrivateKey`].
#[derive(Clone)]
pub struct Signer {
    pub(super) client: Arc<sui::grpc::Client>,
    wallet: super::wallet::WalletClient,
    event_decoder: NexusEventDecoder,
}

impl Signer {
    pub fn new(
        client: Arc<sui::grpc::Client>,
        pk: sui::crypto::Ed25519PrivateKey,
        transaction_timeout: Duration,
        event_decoder: NexusEventDecoder,
    ) -> Self {
        Self::with_server_checkpoint_wait(client, pk, transaction_timeout, event_decoder, false)
    }

    pub(super) fn with_server_checkpoint_wait(
        client: Arc<sui::grpc::Client>,
        pk: sui::crypto::Ed25519PrivateKey,
        transaction_timeout: Duration,
        event_decoder: NexusEventDecoder,
        server_checkpoint_wait_supported: bool,
    ) -> Self {
        let wallet = super::wallet::WalletClient::from_client(
            Arc::clone(&client),
            pk,
            String::new(),
            String::new(),
            transaction_timeout,
            server_checkpoint_wait_supported,
        );
        Self {
            client,
            wallet,
            event_decoder,
        }
    }

    pub(crate) fn with_wallet(
        wallet: super::wallet::WalletClient,
        event_decoder: NexusEventDecoder,
    ) -> Self {
        Self {
            client: wallet.grpc_client(),
            wallet,
            event_decoder,
        }
    }

    pub fn wallet(&self) -> &super::wallet::WalletClient {
        &self.wallet
    }

    /// Get the active address from the signer.
    pub fn get_active_address(&self) -> sui::types::Address {
        self.wallet.owner()
    }

    /// Sign a transaction block using the signer.
    pub async fn sign_tx(
        &self,
        tx: &sui::types::Transaction,
    ) -> Result<sui::types::UserSignature, NexusError> {
        self.wallet.sign_transaction(tx)
    }

    /// Executes a coin based transaction and refreshes its owned gas coin.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when execution fails or the updated gas coin
    /// cannot be fetched.
    pub async fn execute_tx(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
        gas_coin: &mut sui::types::ObjectReference,
    ) -> Result<ExecutedTransaction, NexusError> {
        let executed = self.execute_tx_without_gas_coin(tx, signature).await?;

        // Fetch the gas coin reference produced by execution.
        let crawler = Crawler::new(Arc::clone(&self.client));
        let gas_coin_ref = crawler
            .get_object_metadata(*gas_coin.object_id())
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        *gas_coin = gas_coin_ref;

        Ok(executed)
    }

    /// Executes a transaction without refreshing an owned gas coin.
    ///
    /// This is the address balance based execution boundary and is also useful
    /// when callers do not own the gas object lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when execution fails or the response cannot be
    /// decoded.
    pub async fn execute_tx_without_gas_coin(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
    ) -> Result<ExecutedTransaction, NexusError> {
        let (response, digest, checkpoint) = self
            .wallet
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
            return Err(TransactionError::execution_failed(
                digest,
                checkpoint,
                error.clone(),
                *command,
            )
            .into());
        }

        // Deserialize events.
        let Ok(events) = sui::types::TransactionEvents::try_from(response.events()) else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Failed to read transaction events."
            )));
        };

        let mut nexus_events = Vec::new();
        for (index, event) in events.0.iter().enumerate() {
            if let Some(event) = self
                .event_decoder
                .decode_sui_event(index as u64, digest, event)
                .await
                .map_err(|error| NexusError::Parsing(error.into()))?
            {
                nexus_events.push(event);
            }
        }

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

        Ok(ExecutedTransaction {
            effects: *effects,
            events: nexus_events,
            objects,
            digest,
            checkpoint,
        })
    }
}
