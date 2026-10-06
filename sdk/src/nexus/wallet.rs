//! Shared Sui signing and submission authority for Nexus and storage clients.

use {
    crate::{
        nexus::{
            address_balance::{fetch_submission_context, finish_transaction, NonceAllocator},
            error::{NexusError, TransactionError},
        },
        sui::{self, traits::*},
    },
    std::sync::Arc,
    sui_rpc::client::ExecuteAndWaitError,
    tokio::{sync::OnceCell, time::Duration},
};

/// Clones share the same RPC connection, private key and cached network metadata.
/// Domain clients borrow this authority instead of constructing another wallet.
#[derive(Clone)]
pub struct WalletClient {
    client: Arc<sui::grpc::Client>,
    key: Arc<sui::crypto::Ed25519PrivateKey>,
    rpc_url: String,
    network_info: Arc<OnceCell<NetworkInfo>>,
    transaction_timeout: Duration,
}

struct NetworkInfo {
    chain_id: String,
    chain: Option<String>,
    server_checkpoint_wait_supported: bool,
}

impl WalletClient {
    pub async fn connect(
        rpc_url: &str,
        key: sui::crypto::Ed25519PrivateKey,
    ) -> Result<Self, NexusError> {
        let client = Arc::new(sui::grpc::client(rpc_url).map_err(NexusError::Rpc)?);
        let wallet = Self::from_client(client, key, Duration::from_secs(60));
        wallet.chain_id().await?;
        Ok(wallet)
    }

    pub(crate) fn from_client(
        client: Arc<sui::grpc::Client>,
        key: sui::crypto::Ed25519PrivateKey,
        transaction_timeout: Duration,
    ) -> Self {
        let rpc_url = client.uri().to_string();
        Self {
            client,
            key: Arc::new(key),
            rpc_url,
            network_info: Arc::new(OnceCell::new()),
            transaction_timeout,
        }
    }

    pub fn owner(&self) -> sui::types::Address {
        self.key.public_key().derive_address()
    }

    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Returns the chain identity, fetching metadata once through this wallet's client.
    pub async fn chain_id(&self) -> Result<&str, NexusError> {
        Ok(&self.network_info().await?.chain_id)
    }

    /// Returns the network name from the same cached response as the chain identity.
    pub async fn chain(&self) -> Result<&str, NexusError> {
        self.network_info()
            .await?
            .chain
            .as_deref()
            .ok_or_else(|| NexusError::Configuration("Sui RPC omitted its network".into()))
    }

    async fn network_info(&self) -> Result<&NetworkInfo, NexusError> {
        self.network_info
            .get_or_try_init(|| self.fetch_network_info())
            .await
    }

    async fn fetch_network_info(&self) -> Result<NetworkInfo, NexusError> {
        let response = self
            .client
            .as_ref()
            .clone()
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await
            .map_err(|error| NexusError::Rpc(error.into()))?;
        let server_checkpoint_wait_supported = sui::grpc::checkpoint_wait::is_supported(&response);
        let info = response.into_inner();
        Ok(NetworkInfo {
            chain_id: info
                .chain_id
                .filter(|id| !id.is_empty())
                .ok_or_else(|| NexusError::Configuration("Sui RPC omitted its chain ID".into()))?,
            chain: info.chain.filter(|name| !name.is_empty()),
            server_checkpoint_wait_supported,
        })
    }

    /// Rechecks an initialized wallet before attaching it to a Nexus client.
    pub(crate) async fn validate_chain(&self) -> Result<&str, NexusError> {
        if let Some(info) = self.network_info.get() {
            let actual = self.fetch_network_info().await?.chain_id;
            if actual != info.chain_id {
                return Err(NexusError::ChainMismatch {
                    expected: info.chain_id.clone(),
                    actual,
                });
            }
            Ok(&info.chain_id)
        } else {
            self.chain_id().await
        }
    }

    pub fn transaction_timeout(&self) -> Duration {
        self.transaction_timeout
    }

    pub fn with_transaction_timeout(mut self, timeout: Duration) -> Self {
        self.transaction_timeout = timeout;
        self
    }

    pub fn grpc_client(&self) -> Arc<sui::grpc::Client> {
        Arc::clone(&self.client)
    }

    pub fn sign_transaction(
        &self,
        tx: &sui::types::Transaction,
    ) -> Result<sui::types::UserSignature, NexusError> {
        self.key
            .sign_transaction(tx)
            .map_err(|error| NexusError::Wallet(error.into()))
    }

    /// Completes a PTB with address balance gas using the process nonce authority.
    pub async fn prepare_transaction(
        &self,
        ptb: sui::types::ProgrammableTransaction,
        gas_budget: u64,
    ) -> Result<sui::types::Transaction, NexusError> {
        if gas_budget == 0 {
            return Err(NexusError::Configuration(
                "gas budget must be positive".into(),
            ));
        }
        let chain_id = self.chain_id().await?;
        let mut client = self.client.as_ref().clone();
        let context = fetch_submission_context(&mut client).await?;
        if context.chain.to_string() != chain_id {
            return Err(NexusError::ChainMismatch {
                expected: chain_id.to_owned(),
                actual: context.chain.to_string(),
            });
        }
        let nonce = NonceAllocator::default().allocate()?;
        Ok(finish_transaction(
            ptb,
            self.owner(),
            gas_budget,
            context,
            nonce,
        ))
    }

    /// Submits an already prepared transaction through the same confirmation
    /// boundary used by Nexus. Callers can persist its digest before submission.
    pub async fn execute_transaction(
        &self,
        tx: sui::types::Transaction,
    ) -> Result<sui::grpc::ExecutedTransaction, NexusError> {
        let signature = self.sign_transaction(&tx)?;
        self.execute_signed_transaction(tx, signature).await
    }

    /// Submits a previously signed transaction, without signing any supplied bytes.
    /// A saved upload journal can therefore be retried without becoming a signing oracle.
    pub async fn execute_signed_transaction(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
    ) -> Result<sui::grpc::ExecutedTransaction, NexusError> {
        use sui::crypto::SuiVerifier as _;
        if tx.sender != self.owner() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "transaction sender differs from this wallet"
            )));
        }
        sui::crypto::ed25519::Ed25519VerifyingKey::new(&self.key.public_key())
            .and_then(|key| key.verify_transaction(&tx, &signature))
            .map_err(|error| NexusError::Wallet(error.into()))?;
        let (response, digest, checkpoint) = self
            .execute_tx_and_wait_for_checkpoint(tx, signature)
            .await?;
        let effects = sui::types::TransactionEffects::try_from(response.effects())
            .map_err(|error| NexusError::Parsing(error.into()))?;
        if let sui::types::ExecutionStatus::Failure { error, command } = effects.status() {
            return Err(TransactionError::execution_failed(
                digest,
                checkpoint,
                error.clone(),
                *command,
            )
            .into());
        }
        Ok(response)
    }

    /// Executes a transaction and waits for its checkpoint confirmation.
    pub(crate) async fn execute_tx_and_wait_for_checkpoint(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
    ) -> Result<(sui::grpc::ExecutedTransaction, sui::types::Digest, u64), NexusError> {
        let info = self.network_info().await?;
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

        let response = if info.server_checkpoint_wait_supported {
            // Request metadata makes the node wait inside ExecuteTransaction.
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

    #[tokio::test]
    async fn saved_transactions_are_verified_before_any_submission() {
        use super::WalletClient;
        let key = sui::crypto::Ed25519PrivateKey::new([17; 32]);
        let client = std::sync::Arc::new(sui::grpc::client("http://127.0.0.1:1").unwrap());
        let chain = sui::types::Digest::new([9; 32]);
        let wallet = WalletClient::from_client(client, key, Duration::from_secs(1));
        let mut tx = crate::nexus::address_balance::finish_transaction(
            sui::types::ProgrammableTransaction {
                inputs: vec![],
                commands: vec![],
            },
            wallet.owner(),
            1_000_000,
            crate::nexus::address_balance::SubmissionContext {
                reference_gas_price: 1,
                epoch: 1,
                chain,
            },
            1,
        );
        let signature = wallet.sign_transaction(&tx).unwrap();
        tx.gas_payment.budget += 1;
        assert!(matches!(
            wallet
                .execute_signed_transaction(tx.clone(), signature)
                .await,
            Err(NexusError::Wallet(_))
        ));
        tx.sender = sui::types::Address::TWO;
        let signature = wallet.sign_transaction(&tx).unwrap();
        assert!(matches!(
            wallet.execute_signed_transaction(tx, signature).await,
            Err(NexusError::Wallet(_))
        ));
    }

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

    #[test]
    fn incomplete_confirmation_retains_the_response_for_recovery() {
        let digest = sui::types::Digest::new([9; 32]);
        for response in [
            sui::grpc::ExecuteTransactionResponse::default(),
            sui::grpc::ExecuteTransactionResponse::default()
                .with_transaction(sui::grpc::ExecutedTransaction::default().with_digest(digest)),
        ] {
            let NexusError::Transaction(error) =
                validated_execution_response(digest, response.clone()).unwrap_err()
            else {
                panic!("expected a transaction error");
            };
            assert_eq!(error.state(), TransactionErrorState::ConfirmationUnknown);
            assert_eq!(error.digest(), &digest);
            assert_eq!(error.response(), Some(&response));
        }
    }

    #[test]
    fn interrupted_checkpoint_stream_preserves_execution_for_recovery() {
        let digest = sui::types::Digest::new([9; 32]);
        let response = sui::grpc::ExecuteTransactionResponse::default()
            .with_transaction(sui::grpc::ExecutedTransaction::default().with_digest(digest));
        let NexusError::Transaction(error) = map_execute_and_wait_error(
            digest,
            Duration::from_secs(30),
            ExecuteAndWaitError::CheckpointStreamError {
                response: tonic::Response::new(response.clone()),
                error: tonic::Status::unavailable("checkpoint stream interrupted"),
            },
        ) else {
            panic!("expected a transaction error");
        };
        assert_eq!(error.state(), TransactionErrorState::ConfirmationUnknown);
        assert_eq!(error.digest(), &digest);
        assert_eq!(error.response(), Some(&response));
    }
}
