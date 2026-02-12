//! [`NexusClient`] can accept be built with a [`Signer`] instance and a [`Gas`]
//! instance to perform various Nexus-related operations programmatically.

use {
    crate::{
        events::EventFetcher,
        nexus::{
            crawler::Crawler,
            crypto::CryptoActions,
            error::NexusError,
            gas::GasActions,
            scheduler::SchedulerActions,
            signer::Signer,
            workflow::WorkflowActions,
        },
        sui,
        types::NexusObjects,
    },
    std::sync::Arc,
    tokio::{
        sync::{Mutex, Notify},
        time::Duration,
    },
};

/// [`Gas`] struct handles distributing gas coins for Nexus operations.
#[derive(Clone)]
pub struct Gas {
    coins: Arc<Mutex<Vec<sui::types::ObjectReference>>>,
    notify: Arc<Notify>,
    budget: u64,
}

impl Gas {
    /// Acquire a gas coin from the pool.
    pub async fn acquire_gas_coin(&self) -> sui::types::ObjectReference {
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
    pub async fn release_gas_coin(&self, coin: sui::types::ObjectReference) {
        self.coins.lock().await.push(coin);
        self.notify.notify_one();
    }

    /// Get the gas budget.
    pub fn get_budget(&self) -> u64 {
        self.budget
    }
}

/// Builder for [`NexusClient`].
#[derive(Default)]
pub struct NexusClientBuilder {
    pk: Option<sui::crypto::Ed25519PrivateKey>,
    rpc_url: Option<String>,
    gql_url: Option<String>,
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

    /// Which GraphQL to connect to.
    pub fn with_gql_url(mut self, gql_url: &str) -> Self {
        self.gql_url = Some(gql_url.to_string());
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

        let nexus_objects = Arc::new(
            self.nexus_objects
                .ok_or_else(|| NexusError::Configuration("Nexus objects are required".into()))?,
        );

        let client = Arc::new(Mutex::new(
            sui_rpc::Client::new(&rpc_url).map_err(|e| NexusError::Rpc(e.into()))?,
        ));

        let reference_gas_price = client
            .lock()
            .await
            .get_reference_gas_price()
            .await
            .map_err(|e| NexusError::Rpc(e.into()))?;

        let signer = Signer::new(
            client.clone(),
            pk,
            self.transaction_timeout.unwrap_or(Duration::from_secs(5)),
            Arc::clone(&nexus_objects),
        );

        let gas = Gas {
            coins: Arc::new(Mutex::new(self.gas_coins)),
            notify: Arc::new(Notify::new()),
            budget: gas_budget,
        };

        Ok(NexusClient {
            signer,
            gas,
            nexus_objects: Arc::clone(&nexus_objects),
            reference_gas_price,
            crawler: Crawler::new(client),
            event_fetcher: EventFetcher::new(
                &self
                    .gql_url
                    .unwrap_or_else(|| format!("{}/graphql", &rpc_url)),
                Arc::clone(&nexus_objects),
            ),
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
    /// Provide access to an instantiated object crawler.
    pub(super) crawler: Crawler,
    /// Provide access to an instantiated event fetcher.
    pub(super) event_fetcher: EventFetcher,
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

    /// Return a [`Crawler`] instance for object crawling operations.
    pub fn crawler(&self) -> &Crawler {
        &self.crawler
    }

    /// Return a [`Signer`] instance for signing transactions.
    pub fn signer(&self) -> &Signer {
        &self.signer
    }

    /// Return an [`EventFetcher`] instance for fetching Nexus events.
    pub fn event_fetcher(&self) -> &EventFetcher {
        &self.event_fetcher
    }

    /// Return a reference to the [`Gas`] instance.
    pub fn gas_config(&self) -> Gas {
        self.gas.clone()
    }

    /// Get the reference gas price.
    pub fn get_reference_gas_price(&self) -> u64 {
        self.reference_gas_price
    }

    /// Get the Nexus objects.
    pub fn get_nexus_objects(&self) -> Arc<NexusObjects> {
        Arc::clone(&self.nexus_objects)
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
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url, None).await;

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
        let signature = client.signer.sign_tx(&tx).await.unwrap();

        let response = client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await
            .unwrap();

        assert_eq!(response.digest, digest);

        assert_eq!(gas_coin.version(), gas_coin_ref.version());
        assert_eq!(gas_coin.digest(), gas_coin_ref.digest());
    }
}
