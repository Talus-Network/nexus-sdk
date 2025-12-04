//! [`NexusClient`] can accept be built with a [`Signer`] instance and a [`Gas`]
//! instance to perform various Nexus-related operations programmatically.

use {
    crate::{
        nexus::{
            crypto::CryptoActions,
            error::NexusError,
            gas::GasActions,
            scheduler::SchedulerActions,
            workflow::WorkflowActions,
        },
        sui::{self, traits::*},
        types::NexusObjects,
    },
    std::{str::FromStr, sync::Arc},
    tokio::{
        sync::{Mutex, MutexGuard, Notify},
        time::Duration,
    },
};

/// Resulting struct from executing a transaction.
pub struct ExecutedTransaction {
    pub effects: sui::types::TransactionEffectsV2,
    pub events: sui::types::TransactionEvents,
    pub objects: Vec<sui::types::Object>,
    pub digest: sui::types::Digest,
}

/// We want to provide flexibility when it comes to signing transactions. We
/// accept both - a [`sui::WalletContext`] and a tuple of a [`sui::Client`] and
/// a secret mnemonic string.
#[derive(Clone)]
pub struct Signer {
    client: Arc<Mutex<sui::grpc::Client>>,
    pk: sui::crypto::Ed25519PrivateKey,
    transaction_timeout: Duration,
}

impl Signer {
    /// Get a reference to the Sui client.
    pub(super) async fn get_client(&self) -> MutexGuard<'_, sui::grpc::Client> {
        self.client.lock().await
    }

    /// Get the active address from the signer.
    pub(super) fn get_active_address(&self) -> sui::types::Address {
        self.pk.public_key().derive_address()
    }

    /// Sign a transaction block using the signer.
    pub(super) async fn sign_tx(
        &self,
        tx: &sui::types::Transaction,
    ) -> Result<sui::types::UserSignature, NexusError> {
        self.pk
            .sign_transaction(tx)
            .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))
    }

    /// Execute a transaction block and return the response.
    pub(super) async fn execute_tx(
        &self,
        tx: sui::types::Transaction,
        signature: sui::types::UserSignature,
        gas_coin: &mut sui::types::ObjectReference,
    ) -> Result<ExecutedTransaction, NexusError> {
        let mut client: MutexGuard<'_, sui_rpc::Client> = self.client.lock().await;

        let request = sui::grpc::ExecuteTransactionRequest::default()
            .with_transaction(tx)
            .with_signatures(vec![signature.into()])
            .with_read_mask(sui::grpc::FieldMask::from_paths(&[
                "effects.bcs",
                "events.bcs",
                "objects.objects.bcs",
                "digest",
            ]));

        let response = client
            .execute_transaction_and_wait_for_checkpoint(request, self.transaction_timeout)
            .await
            .map(|res| res.into_inner().transaction)
            .map_err(|e: sui_rpc::client::ExecuteAndWaitError| {
                NexusError::Wallet(anyhow::anyhow!(e))
            })?
            .ok_or_else(|| NexusError::Wallet(anyhow::anyhow!("No transaction in response")))?;

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

        // Deserialize objects.
        let Ok(objects) = response
            .objects()
            .objects()
            .iter()
            .map(|obj| sui::types::Object::try_from(obj))
            .collect::<Result<Vec<_>, _>>()
        else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Failed to read transaction objects."
            )));
        };

        let digest = response.digest();

        if let sui::types::ExecutionStatus::Failure { error, command } = effects.status() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Transaction execution failed: {:?} in command: {:?}",
                error,
                command
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

        Ok(ExecutedTransaction {
            effects: *effects,
            events,
            objects,
            digest: sui::types::Digest::from_str(digest)
                .map_err(|e| NexusError::Parsing(e.into()))?,
        })
    }
}

/// [`Gas`] struct handles distributing gas coins for Nexus operations.
#[derive(Clone)]
pub struct Gas {
    coins: Arc<Mutex<Vec<sui::types::ObjectReference>>>,
    notify: Arc<Notify>,
    budget: u64,
}

impl Gas {
    /// Acquire a gas coin from the pool.
    pub(super) async fn acquire_gas_coin(&self) -> sui::types::ObjectReference {
        loop {
            // Try to grab one
            if let Some(coin) = self.coins.lock().await.pop() {
                return coin;
            }

            // Otherwise, wait to be notified
            self.notify.notified().await;
        }
    }

    /// Release a gas coin back to the pool.
    pub(super) async fn release_gas_coin(&self, coin: sui::types::ObjectReference) {
        self.coins.lock().await.push(coin);
        self.notify.notify_one();
    }

    /// Get the gas budget.
    pub(super) fn get_budget(&self) -> u64 {
        self.budget
    }
}

/// Builder for [`NexusClient`].
#[derive(Default)]
pub struct NexusClientBuilder {
    pk: Option<sui::crypto::Ed25519PrivateKey>,
    rpc_url: Option<String>,
    gas_coins: Vec<sui::types::ObjectReference>,
    gas_budget: Option<u64>,
    nexus_objects: Option<NexusObjects>,
    transaction_timeout: Option<Duration>,
}

impl NexusClientBuilder {
    /// Create a new builder instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a private key to the builder.
    pub fn with_private_key(mut self, pk: sui::crypto::Ed25519PrivateKey) -> Self {
        self.pk = Some(pk);
        self
    }

    /// Which RPC to connect to.
    pub fn with_rpc_url(mut self, rpc_url: &str) -> Self {
        self.rpc_url = Some(rpc_url.to_string());
        self
    }

    /// Add gas coins and budget to the builder.
    pub fn with_gas(mut self, coins: Vec<sui::types::ObjectReference>, budget: u64) -> Self {
        self.gas_coins = coins;
        self.gas_budget = Some(budget);
        self
    }

    /// Set Nexus objects to use.
    pub fn with_nexus_objects(mut self, nexus_objects: NexusObjects) -> Self {
        self.nexus_objects = Some(nexus_objects);
        self
    }

    /// Set transaction timeout duration.
    pub fn with_transaction_timeout(mut self, timeout: Duration) -> Self {
        self.transaction_timeout = Some(timeout);
        self
    }

    /// Build the [`NexusClient`].
    pub async fn build(self) -> Result<NexusClient, NexusError> {
        let pk = self
            .pk
            .ok_or_else(|| NexusError::Configuration("User's private key is required".into()))?;

        let rpc_url = self
            .rpc_url
            .ok_or_else(|| NexusError::Configuration("RPC URL is required".into()))?;

        // Need at least one gas coin.
        if self.gas_coins.is_empty() {
            return Err(NexusError::Configuration(
                "At least one gas coin is required".into(),
            ));
        }

        let gas_budget = self
            .gas_budget
            .ok_or_else(|| NexusError::Configuration("Gas budget is required".into()))?;

        let nexus_objects = self
            .nexus_objects
            .ok_or_else(|| NexusError::Configuration("Nexus objects are required".into()))?;

        let mut client = sui_rpc::Client::new(&rpc_url).map_err(|e| NexusError::Rpc(e.into()))?;

        let reference_gas_price = client
            .get_reference_gas_price()
            .await
            .map_err(|e| NexusError::Rpc(e.into()))?;

        let signer = Signer {
            client: Arc::new(Mutex::new(client)),
            pk,
            transaction_timeout: self.transaction_timeout.unwrap_or(Duration::from_secs(5)),
        };

        let gas = Gas {
            coins: Arc::new(Mutex::new(self.gas_coins)),
            notify: Arc::new(Notify::new()),
            budget: gas_budget,
        };

        // TODO: swap.
        let sui_client = sui::ClientBuilder::default()
            .build("https://fullnode.testnet.sui.io:443")
            .await
            .map_err(|e| NexusError::Rpc(e.into()))?;

        Ok(NexusClient {
            signer,
            gas,
            nexus_objects: Arc::new(nexus_objects),
            reference_gas_price,
            sui_client,
        })
    }
}

#[derive(Clone)]
pub struct NexusClient {
    /// The wallet context to use for transactions. This defines the TX sender
    /// address and the RPC connection.
    pub(super) signer: Signer,
    /// Gas configuration for Nexus operations.
    pub(super) gas: Gas,
    /// Nexus objects to use.
    pub(super) nexus_objects: Arc<NexusObjects>,
    /// Save reference gas price to avoid fetching it multiple times.
    pub(super) reference_gas_price: u64,
    #[deprecated(since = "0.4.0")]
    pub sui_client: sui::Client,
}

impl NexusClient {
    /// Return a [`NexusClientBuilder`] instance for building a Nexus client.
    pub fn builder() -> NexusClientBuilder {
        NexusClientBuilder::new()
    }

    /// Return a [`GasActions`] instance for performing gas-related operations.
    pub fn gas(&self) -> GasActions {
        GasActions {
            client: self.clone(),
        }
    }

    /// Return a [`CryptoActions`] instance for performing crypto-related operations.
    pub fn crypto(&self) -> CryptoActions {
        CryptoActions {
            client: self.clone(),
        }
    }

    /// Return a [`WorkflowActions`] instance for performing workflow-related operations.
    pub fn workflow(&self) -> WorkflowActions {
        WorkflowActions {
            client: self.clone(),
        }
    }

    /// Return a [`SchedulerActions`] instance for scheduler operations.
    pub fn scheduler(&self) -> SchedulerActions {
        SchedulerActions {
            client: self.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::{
            nexus_mocks,
            sui_mocks::{self},
        },
    };

    #[tokio::test]
    async fn test_acquire_and_release_gas_coin() {
        let coin1 = sui_mocks::mock_sui_object_ref();
        let coin2 = sui_mocks::mock_sui_object_ref();

        let gas = Gas {
            coins: Arc::new(Mutex::new(vec![coin1.clone(), coin2.clone()])),
            notify: Arc::new(Notify::new()),
            budget: 1000,
        };

        // Acquire coins
        let acquired1 = gas.acquire_gas_coin().await;
        let acquired2 = gas.acquire_gas_coin().await;

        assert!(acquired1 == coin2 || acquired1 == coin1);
        assert!(acquired2 == coin2 || acquired2 == coin1);
        assert_ne!(acquired1, acquired2);

        // Release coin
        gas.release_gas_coin(acquired1.clone()).await;

        // Acquire again
        let acquired3 = gas.acquire_gas_coin().await;
        assert_eq!(acquired3, acquired1);
    }

    #[tokio::test]
    async fn test_get_budget() {
        let gas = Gas {
            coins: Arc::new(Mutex::new(vec![])),
            notify: Arc::new(Notify::new()),
            budget: 5000,
        };
        assert_eq!(gas.get_budget(), 5000);
    }

    #[tokio::test]
    async fn test_acquire_gas_coin_waits_for_release() {
        let coin = sui_mocks::mock_sui_object_ref();
        let gas = Gas {
            coins: Arc::new(Mutex::new(vec![])),
            notify: Arc::new(Notify::new()),
            budget: 100,
        };

        let gas_clone = gas.clone();

        let handle = tokio::spawn(async move { gas_clone.acquire_gas_coin().await });

        // Wait a moment to ensure acquire_gas_coin is waiting
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Release coin
        gas.release_gas_coin(coin.clone()).await;

        let acquired = handle.await.unwrap();
        assert_eq!(acquired, coin);
    }

    #[tokio::test]
    async fn test_builder_with_private_key() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let coin = sui_mocks::mock_sui_object_ref();
        let objects = sui_mocks::mock_nexus_objects();
        let coins = vec![coin];
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_nexus_objects(objects)
            .with_gas(coins, budget);

        let client = builder.build().await.unwrap();
        assert_eq!(client.gas.get_budget(), budget);
        assert_eq!(client.signer.transaction_timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_builder_missing_pk() {
        let coin = sui_mocks::mock_sui_object_ref();
        let coins = vec![coin];
        let objects = sui_mocks::mock_nexus_objects();
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_nexus_objects(objects)
            .with_gas(coins, budget);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_missing_rpc_url() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let coin = sui_mocks::mock_sui_object_ref();
        let coins = vec![coin];
        let objects = sui_mocks::mock_nexus_objects();
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_nexus_objects(objects)
            .with_gas(coins, budget);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_missing_gas() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_nexus_objects(objects);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_with_missing_budget() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let objects = sui_mocks::mock_nexus_objects();

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_nexus_objects(objects);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_with_gas_empty_coins() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let coins = vec![];
        let objects = sui_mocks::mock_nexus_objects();
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_nexus_objects(objects)
            .with_gas(coins, budget);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_missing_nexus_objects() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let coin = sui_mocks::mock_sui_object_ref();
        let coins = vec![coin];
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_gas(coins, budget);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_tx_timeout() {
        let mut rng = rand::thread_rng();
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let coin = sui_mocks::mock_sui_object_ref();
        let objects = sui_mocks::mock_nexus_objects();
        let coins = vec![coin];
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("https://fullnode.testnet.sui.io:443")
            .with_nexus_objects(objects)
            .with_gas(coins, budget)
            .with_transaction_timeout(Duration::from_secs(10));

        let client = builder.build().await.unwrap();
        assert_eq!(client.gas.get_budget(), budget);
        assert_eq!(client.signer.transaction_timeout, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_execute_tx_mutates_gas_coin() {
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let (set_digest, get_digest) = std::sync::mpsc::channel::<sui::types::Digest>();

        ledger_service_mock
            .expect_get_epoch()
            .returning(|_request| {
                let mut response = sui::grpc::GetEpochResponse::default();
                let mut epoch = sui::grpc::Epoch::default();
                epoch.set_reference_gas_price(1000);
                response.set_epoch(epoch);
                Ok(tonic::Response::new(response))
            });

        subscription_service_mock
            .expect_subscribe_checkpoints()
            .returning(move |_request| {
                let mut response = sui::grpc::SubscribeCheckpointsResponse::default();
                let mut checkpoint = sui::grpc::Checkpoint::default();
                let mut tx = sui::grpc::ExecutedTransaction::default();
                let digest = get_digest.recv().unwrap();
                tx.set_digest(digest);
                checkpoint.set_transactions(vec![tx]);
                checkpoint.set_sequence_number(1);
                response.set_checkpoint(checkpoint);

                let output = vec![Ok(response)];
                let stream = futures::stream::iter(output.into_iter());

                Ok(tonic::Response::new(
                    Box::pin(stream) as sui_mocks::grpc::BoxCheckpointStream
                ))
            });

        tx_service_mock
            .expect_execute_transaction()
            .returning(|_request| {
                let mut response = sui::grpc::ExecuteTransactionResponse::default();
                let mut tx = sui::grpc::ExecutedTransaction::default();

                let objects = sui::grpc::ObjectSet::default();
                tx.set_objects(objects);

                let mut effects = sui::grpc::TransactionEffects::default();
                let effect = sui::types::TransactionEffectsV2 {
                    status: sui::types::ExecutionStatus::Success,
                    epoch: 1,
                    gas_used: sui::types::GasCostSummary {
                        computation_cost: 0,
                        storage_cost: 0,
                        storage_rebate: 0,
                        non_refundable_storage_fee: 0,
                    },
                    transaction_digest: sui::types::Digest::from_static("123abc"),
                    gas_object_index: Some(0),
                    events_digest: None,
                    dependencies: vec![],
                    lamport_version: 1,
                    changed_objects: vec![sui::types::ChangedObject {
                        object_id: sui::types::Address::from_static("0x1"),
                        input_state: sui::types::ObjectIn::NotExist,
                        output_state: sui::types::ObjectOut::ObjectWrite {
                            digest: sui::types::Digest::from_static("456def"),
                            owner: sui::types::Owner::Address(sui::types::Address::from_static(
                                "0x1",
                            )),
                        },
                        id_operation: sui::types::IdOperation::None,
                    }],
                    unchanged_consensus_objects: vec![],
                    auxiliary_data_digest: None,
                };
                effects.set_bcs(
                    bcs::to_bytes(&sui::types::TransactionEffects::V2(Box::new(effect))).unwrap(),
                );
                tx.set_effects(effects);

                let mut events = sui::grpc::TransactionEvents::default();
                let event = sui::types::TransactionEvents(vec![]);
                events.set_bcs(bcs::to_bytes(&event).unwrap());
                tx.set_events(events);

                tx.set_digest("123abc");

                response.set_transaction(tx);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(subscription_service_mock),
        });
        let client = nexus_mocks::mock_nexus_client(&rpc_url).await;

        assert_eq!(client.reference_gas_price, 1000);

        let mut gas_coin = client.gas.acquire_gas_coin().await;
        let mut tx = sui::tx::TransactionBuilder::new();

        tx.set_sender(client.signer.get_active_address());
        tx.set_gas_budget(1000);
        tx.set_gas_price(1000);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx.finish().unwrap();

        set_digest.send(tx.digest()).unwrap();

        let signature = client.signer.sign_tx(&tx).await.unwrap();

        let response = client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await
            .unwrap();

        assert_eq!(response.digest, sui::types::Digest::from_static("123abc"));

        assert_eq!(gas_coin.version(), 2);
        assert_eq!(
            gas_coin.digest(),
            &sui::types::Digest::from_static("456def")
        );
    }
}
