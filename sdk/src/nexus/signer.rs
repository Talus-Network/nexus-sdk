//! Module defining a [`Signer`] struct that can sign and execute transactions
//! on Sui in Nexus context.
use {
    crate::{
        events::{NexusEvent, NexusEventDecoder},
        nexus::{
            crawler::Crawler,
            error::{NexusError, TransactionError},
        },
        sui::{self, traits::*},
    },
    std::sync::Arc,
    sui_rpc::client::ExecuteAndWaitError,
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
    pub(super) pk: sui::crypto::Ed25519PrivateKey,
    pub(super) transaction_timeout: Duration,
    event_decoder: NexusEventDecoder,
    checkpoint_wait_supported: bool,
}

impl Signer {
    pub fn new(
        client: Arc<sui::grpc::Client>,
        pk: sui::crypto::Ed25519PrivateKey,
        transaction_timeout: Duration,
        event_decoder: NexusEventDecoder,
    ) -> Self {
        Self::with_checkpoint_wait(client, pk, transaction_timeout, event_decoder, false)
    }

    pub(super) fn with_checkpoint_wait(
        client: Arc<sui::grpc::Client>,
        pk: sui::crypto::Ed25519PrivateKey,
        transaction_timeout: Duration,
        event_decoder: NexusEventDecoder,
        checkpoint_wait_supported: bool,
    ) -> Self {
        Self {
            client,
            pk,
            transaction_timeout,
            event_decoder,
            checkpoint_wait_supported,
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

    /// Executes a transaction and waits for its checkpoint confirmation.
    async fn execute_tx_and_wait_for_checkpoint(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
    ) -> Result<(sui::grpc::ExecutedTransaction, sui::types::Digest, u64), NexusError> {
        let mut client = self.client.as_ref().clone();
        let digest = tx.digest();

        let tx_request = sui::grpc::ExecuteTransactionRequest::default()
            .with_transaction(tx)
            .with_signatures(vec![signature.into()])
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "effects.bcs",
                "events.events",
                "objects.objects",
                "digest",
                "checkpoint",
            ]));

        let response = if self.checkpoint_wait_supported {
            client
                .execution_client()
                .execute_transaction(sui::grpc::checkpoint_wait::execution_request(tx_request))
                .await
                .map_err(|source| map_execute_rpc_error(digest, source))?
                .into_inner()
        } else {
            client
                .execute_transaction_and_wait_for_checkpoint(tx_request, self.transaction_timeout)
                .await
                .map_err(|error| {
                    map_execute_and_wait_error(digest, self.transaction_timeout, error)
                })?
                .into_inner()
        };

        let (executed, checkpoint) = validated_execution_response(digest, response)?;
        Ok((executed, digest, checkpoint))
    }
}

fn validated_execution_response(
    digest: sui::types::Digest,
    mut response: sui::grpc::ExecuteTransactionResponse,
) -> Result<(sui::grpc::ExecutedTransaction, u64), NexusError> {
    let Some(executed) = response.transaction.as_ref() else {
        return Err(TransactionError::confirmation_response_invalid(
            digest,
            response,
            "transaction is missing",
        )
        .into());
    };
    if executed.digest.as_deref() != Some(digest.to_string().as_str()) {
        return Err(TransactionError::confirmation_response_invalid(
            digest,
            response,
            "transaction digest does not match the submitted transaction",
        )
        .into());
    }
    let Some(checkpoint) = executed.checkpoint else {
        return Err(TransactionError::confirmation_response_invalid(
            digest,
            response,
            "checkpoint is missing",
        )
        .into());
    };

    Ok((
        response
            .transaction
            .take()
            .expect("the transaction was validated before extraction"),
        checkpoint,
    ))
}

fn map_execute_and_wait_error(
    digest: sui::types::Digest,
    timeout: Duration,
    error: ExecuteAndWaitError,
) -> NexusError {
    match error {
        ExecuteAndWaitError::RpcError(source) => map_execute_rpc_error(digest, source),
        ExecuteAndWaitError::MissingTransaction => NexusError::TransactionBuilding(
            anyhow::anyhow!("transaction {digest} request is missing the transaction"),
        ),
        ExecuteAndWaitError::ProtoConversionError(source) => NexusError::TransactionBuilding(
            anyhow::anyhow!("transaction {digest} request could not be decoded: {source}"),
        ),
        ExecuteAndWaitError::CheckpointTimeout(response) => {
            TransactionError::confirmation_timed_out(digest, timeout, response.into_inner()).into()
        }
        ExecuteAndWaitError::CheckpointStreamError { response, error } => {
            TransactionError::confirmation_failed(digest, response.into_inner(), error).into()
        }
        other => NexusError::TransactionBuilding(anyhow::anyhow!(
            "transaction {digest} could not be executed: {other}"
        )),
    }
}

fn map_execute_rpc_error(digest: sui::types::Digest, source: tonic::Status) -> NexusError {
    if is_submission_rejection(source.code()) {
        TransactionError::submission_rejected(digest, source).into()
    } else {
        TransactionError::submission_unknown(digest, source).into()
    }
}

fn is_submission_rejection(code: tonic::Code) -> bool {
    matches!(
        code,
        tonic::Code::InvalidArgument
            | tonic::Code::NotFound
            | tonic::Code::AlreadyExists
            | tonic::Code::PermissionDenied
            | tonic::Code::Unauthenticated
            | tonic::Code::FailedPrecondition
            | tonic::Code::OutOfRange
            | tonic::Code::Unimplemented
    )
}

#[cfg(test)]
mod tests {
    use {
        super::{map_execute_and_wait_error, validated_execution_response},
        crate::{
            nexus::error::{NexusError, TransactionErrorState},
            sui,
        },
        std::time::Duration,
        sui_rpc::client::ExecuteAndWaitError,
    };

    #[test]
    fn checkpoint_timeout_retains_the_execution_response() {
        let digest = sui::types::Digest::new([9; 32]);
        let response = sui::grpc::ExecuteTransactionResponse::default().with_transaction(
            sui::grpc::ExecutedTransaction::default().with_digest(digest.to_string()),
        );

        let error = map_execute_and_wait_error(
            digest,
            Duration::from_secs(30),
            ExecuteAndWaitError::CheckpointTimeout(tonic::Response::new(response.clone())),
        );

        let NexusError::Transaction(error) = error else {
            panic!("expected a transaction error");
        };
        assert_eq!(error.state(), TransactionErrorState::ConfirmationUnknown);
        assert_eq!(error.digest(), &digest);
        assert_eq!(error.response(), Some(&response));
    }

    #[test]
    fn rpc_failure_preserves_unknown_submission_state() {
        let digest = sui::types::Digest::new([9; 32]);

        let error = map_execute_and_wait_error(
            digest,
            Duration::from_secs(30),
            ExecuteAndWaitError::RpcError(tonic::Status::unavailable("connection closed")),
        );

        let NexusError::Transaction(error) = error else {
            panic!("expected a transaction error");
        };
        assert_eq!(error.state(), TransactionErrorState::SubmissionUnknown);
        assert_eq!(error.digest(), &digest);
        assert!(error.response().is_none());
    }

    #[test]
    fn rpc_rejection_preserves_known_submission_state() {
        let digest = sui::types::Digest::new([9; 32]);

        let error = map_execute_and_wait_error(
            digest,
            Duration::from_secs(30),
            ExecuteAndWaitError::RpcError(tonic::Status::invalid_argument(
                "invalid gas reservation",
            )),
        );

        let NexusError::Transaction(error) = error else {
            panic!("expected a transaction error");
        };
        assert_eq!(error.state(), TransactionErrorState::SubmissionRejected);
        assert_eq!(error.digest(), &digest);
        assert!(!error.to_string().contains("unknown"));
    }

    #[test]
    fn confirmation_rejects_a_different_transaction_digest() {
        let digest = sui::types::Digest::new([9; 32]);
        let response = sui::grpc::ExecuteTransactionResponse::default().with_transaction(
            sui::grpc::ExecutedTransaction::default()
                .with_digest(sui::types::Digest::new([8; 32]).to_string())
                .with_checkpoint(42),
        );

        let error = validated_execution_response(digest, response.clone()).unwrap_err();

        let NexusError::Transaction(error) = error else {
            panic!("expected a transaction error");
        };

        assert_eq!(error.state(), TransactionErrorState::ConfirmationUnknown);
        assert_eq!(error.digest(), &digest);
        assert_eq!(error.response(), Some(&response));
        assert!(error.to_string().contains("digest does not match"));
    }
}
