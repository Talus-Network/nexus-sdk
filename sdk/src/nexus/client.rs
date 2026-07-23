//! A [`NexusClient`] combines a [`Signer`] with one [`Gas`] source to perform
//! Nexus operations programmatically.

use {
    crate::{
        events::{NexusEventIngestor, NexusEventQuery},
        nexus::{
            address_balance::{fetch_submission_context, finish_transaction, NonceAllocator},
            crawler::Crawler,
            error::NexusError,
            gas::GasActions,
            scheduler::SchedulerActions,
            signer::{ExecutedTransaction, Signer},
            workflow::WorkflowActions,
        },
        sui,
        types::NexusObjects,
        ToolFqn,
    },
    std::sync::Arc,
    tokio::{
        sync::{Mutex, Notify},
        time::Duration,
    },
};
#[cfg(feature = "walrus")]
use {
    crate::{move_bindings::interface::dag as dag_move, nexus::workflow::fetch_dag_vertices_bcs},
    std::collections::HashSet,
};

/// Gas source configured for a [`NexusClient`].
///
/// A client uses exactly one coin based or address balance based source.
#[derive(Clone)]
pub struct Gas {
    source: GasSource,
}

#[derive(Clone)]
enum GasSource {
    Coin(CoinGasPool),
    AddressBalance(AddressBalanceGas),
}

impl Gas {
    /// Returns the configured gas budget.
    pub fn get_budget(&self) -> u64 {
        match &self.source {
            GasSource::Coin(pool) => pool.budget,
            GasSource::AddressBalance(gas) => gas.budget,
        }
    }

    /// Returns the shared pool when coin based gas is configured.
    pub(crate) fn coin_pool(&self) -> Option<&CoinGasPool> {
        match &self.source {
            GasSource::Coin(pool) => Some(pool),
            GasSource::AddressBalance(_) => None,
        }
    }

    fn reference_gas_price(&self) -> Option<u64> {
        match &self.source {
            GasSource::Coin(pool) => Some(pool.reference_gas_price),
            GasSource::AddressBalance(_) => None,
        }
    }
}

/// Shared owned coin source used for coin based gas.
#[derive(Clone)]
pub(crate) struct CoinGasPool {
    coins: Arc<Mutex<Vec<sui::types::ObjectReference>>>,
    notify: Arc<Notify>,
    budget: u64,
    reference_gas_price: u64,
}

impl CoinGasPool {
    /// Acquires an owned gas coin, waiting until one is available.
    pub(crate) async fn acquire_gas_coin(&self) -> sui::types::ObjectReference {
        loop {
            if let Some(coin) = self.coins.lock().await.pop() {
                return coin;
            }

            self.notify.notified().await;
        }
    }

    /// Returns an owned gas coin to the pool and wakes one waiter.
    pub(crate) async fn release_gas_coin(&self, coin: sui::types::ObjectReference) {
        self.coins.lock().await.push(coin);
        self.notify.notify_one();
    }
}

/// Reusable address balance gas configuration.
///
/// Clones share one [`NonceAllocator`] so transactions from the same sender use
/// distinct nonces.
#[derive(Clone, Debug)]
pub struct AddressBalanceGas {
    budget: u64,
    nonces: NonceAllocator,
}

impl AddressBalanceGas {
    /// Creates a new independent sender nonce authority.
    pub fn new(budget: u64) -> Self {
        Self::with_nonce_allocator(budget, NonceAllocator::default())
    }

    /// Creates a configuration backed by an existing [`NonceAllocator`].
    pub fn with_nonce_allocator(budget: u64, nonces: NonceAllocator) -> Self {
        Self { budget, nonces }
    }

    fn allocate_nonce(&self) -> Result<u32, NexusError> {
        self.nonces.allocate()
    }
}

/// Builder for [`NexusClient`].
#[derive(Default)]
pub struct NexusClientBuilder {
    pk: Option<sui::crypto::Ed25519PrivateKey>,
    rpc_url: Option<String>,
    gas_coins: Vec<sui::types::ObjectReference>,
    gas_budget: Option<u64>,
    address_balance_gas: Option<AddressBalanceGas>,
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

    /// Configures coin based gas with owned coins and a budget.
    pub fn with_gas(mut self, coins: Vec<sui::types::ObjectReference>, budget: u64) -> Self {
        self.gas_coins = coins;
        self.gas_budget = Some(budget);
        self
    }

    /// Configures address balance based gas with an independent nonce authority.
    ///
    /// This creates a nonce authority owned by the resulting client. Use
    /// [`Self::with_address_balance_gas_config`] when several clients submit
    /// for the same sender.
    pub fn with_address_balance_gas(mut self, budget: u64) -> Self {
        self.address_balance_gas = Some(AddressBalanceGas::new(budget));
        self
    }

    /// Configures address balance based gas using a reusable
    /// [`AddressBalanceGas`].
    pub fn with_address_balance_gas_config(mut self, gas: AddressBalanceGas) -> Self {
        self.address_balance_gas = Some(gas);
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

    /// Builds the [`NexusClient`].
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Configuration`] when required configuration is
    /// missing or both gas sources are configured. Returns [`NexusError::Rpc`]
    /// when the client or coin based gas context cannot be initialized.
    pub async fn build(self) -> Result<NexusClient, NexusError> {
        let pk = self
            .pk
            .ok_or_else(|| NexusError::Configuration("User's private key is required".into()))?;

        let rpc_url = self
            .rpc_url
            .ok_or_else(|| NexusError::Configuration("RPC URL is required".into()))?;

        let nexus_objects = Arc::new(
            self.nexus_objects
                .ok_or_else(|| NexusError::Configuration("Nexus objects are required".into()))?,
        );
        let client = Arc::new(Mutex::new(
            sui::grpc::client(&rpc_url).map_err(NexusError::Rpc)?,
        ));

        let coin_gas_requested = self.gas_budget.is_some() || !self.gas_coins.is_empty();
        let source = match (coin_gas_requested, self.address_balance_gas) {
            (true, Some(_)) => {
                return Err(NexusError::Configuration(
                    "coin based gas and address balance based gas cannot both be configured".into(),
                ));
            }
            (true, None) if self.gas_coins.is_empty() => {
                return Err(NexusError::Configuration(
                    "at least one gas coin is required for coin based gas".into(),
                ));
            }
            (true, None) => {
                let reference_gas_price = client
                    .lock()
                    .await
                    .get_reference_gas_price()
                    .await
                    .map_err(|error| NexusError::Rpc(error.into()))?;
                GasSource::Coin(CoinGasPool {
                    coins: Arc::new(Mutex::new(self.gas_coins)),
                    notify: Arc::new(Notify::new()),
                    budget: self.gas_budget.ok_or_else(|| {
                        NexusError::Configuration("gas budget is required".into())
                    })?,
                    reference_gas_price,
                })
            }
            (false, Some(gas)) => GasSource::AddressBalance(gas),
            (false, None) => {
                return Err(NexusError::Configuration("a gas source is required".into()));
            }
        };
        let gas = Gas { source };

        let signer = Signer::new(
            Arc::clone(&client),
            pk,
            self.transaction_timeout.unwrap_or(Duration::from_secs(5)),
            Arc::clone(&nexus_objects),
        );

        Ok(NexusClient {
            signer,
            gas,
            nexus_objects,
            crawler: Crawler::new(client),
            rpc_url,
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
    /// Provide access to an instantiated object crawler.
    pub(super) crawler: Crawler,
    /// RPC URL used by the client.
    pub(super) rpc_url: String,
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

    /// Return a [`NetworkAuthActions`] instance for tool network-auth operations.
    pub fn network_auth(&self) -> crate::nexus::network_auth::NetworkAuthActions {
        crate::nexus::network_auth::NetworkAuthActions {
            client: self.clone(),
        }
    }

    /// Return a [`ToolActions`] instance for tool-related operations.
    pub fn tool(&self) -> crate::nexus::tool::ToolActions {
        crate::nexus::tool::ToolActions {
            client: self.clone(),
        }
    }

    /// Return a [`TapActions`] instance for standard TAP operations.
    pub fn tap(&self) -> crate::nexus::tap::TapActions {
        crate::nexus::tap::TapActions {
            client: self.clone(),
        }
    }

    /// Return a [`Crawler`] instance for object crawling operations.
    pub fn crawler(&self) -> &Crawler {
        &self.crawler
    }

    /// Return the RPC URL configured for this client.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Return a [`Signer`] instance for signing transactions.
    pub fn signer(&self) -> &Signer {
        &self.signer
    }

    /// Returns a [`NexusEventIngestor`] for this Nexus deployment.
    pub fn event_ingestor(&self) -> NexusEventIngestor {
        NexusEventIngestor::new(
            &self.rpc_url,
            NexusEventQuery::new(Arc::clone(&self.nexus_objects)),
        )
    }

    /// Returns a clone of the configured [`Gas`].
    pub fn gas_config(&self) -> Gas {
        self.gas.clone()
    }

    /// Returns the cached reference gas price for coin based submissions.
    ///
    /// Address balance based submissions fetch current network context for each
    /// transaction and therefore return `None` here.
    pub fn get_reference_gas_price(&self) -> Option<u64> {
        self.gas.reference_gas_price()
    }

    /// Get the Nexus objects.
    pub fn get_nexus_objects(&self) -> Arc<NexusObjects> {
        Arc::clone(&self.nexus_objects)
    }

    /// Submits a programmable transaction through this client's configured
    /// [`Gas`] source.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when transaction construction, signing, or
    /// execution fails.
    pub async fn submit_transaction(
        &self,
        tx: sui::types::ProgrammableTransaction,
        address: sui::types::Address,
    ) -> Result<ExecutedTransaction, NexusError> {
        match &self.gas.source {
            GasSource::Coin(pool) => {
                let mut gas_coin = pool.acquire_gas_coin().await;
                let tx = sui::types::Transaction {
                    kind: sui::types::TransactionKind::ProgrammableTransaction(tx),
                    sender: address,
                    gas_payment: sui::types::GasPayment {
                        objects: vec![gas_coin.clone()],
                        owner: address,
                        price: pool.reference_gas_price,
                        budget: pool.budget,
                    },
                    expiration: sui::types::TransactionExpiration::None,
                };
                let signature = self.signer.sign_tx(&tx).await?;
                let response = self.signer.execute_tx(tx, signature, &mut gas_coin).await;
                pool.release_gas_coin(gas_coin).await;
                response
            }
            GasSource::AddressBalance(gas) => {
                let mut client = self.signer.client.lock().await.clone();
                let context = fetch_submission_context(&mut client).await?;
                let nonce = gas.allocate_nonce()?;
                let tx = finish_transaction(tx, address, gas.budget, context, nonce);
                let signature = self.signer.sign_tx(&tx).await?;
                self.signer.execute_tx_without_gas_coin(tx, signature).await
            }
        }
    }

    // == Helpers reused by multiple actions ==

    /// Fetch all [`ToolGas`] derived objects that are relevant to the provided
    /// DAG object ID.
    #[cfg(feature = "walrus")]
    pub(crate) async fn fetch_tool_gas_for_dag(
        &self,
        dag: &dag_move::DAG,
    ) -> anyhow::Result<HashSet<(sui::types::Address, sui::types::Version)>, NexusError> {
        let crawler = self.crawler();
        let gas_service_object_id = *self.nexus_objects.gas_service.object_id();

        let vertices = fetch_dag_vertices_bcs(crawler, dag)
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .map(|(vertex, tool)| tool.kind.tool_fqn().map(|fqn| (vertex, fqn)))
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(NexusError::Parsing)?;

        // Derive `ToolGas` IDs and fetch them in bulk.
        let tool_gas_ids = vertices
            .iter()
            .map(|(_, fqn)| crate::move_bindings::derive_tool_gas_id(gas_service_object_id, fqn))
            .collect::<anyhow::Result<Vec<_>>>()
            .map_err(NexusError::Parsing)?;

        let tool_gas = crawler
            .get_objects_metadata(&tool_gas_ids)
            .await
            .map_err(NexusError::Rpc)?;

        Ok(tool_gas
            .into_iter()
            .map(|resp| (resp.object_id, resp.get_initial_version()))
            .collect())
    }

    /// Derive and fetch a [`Tool`] object based on the provided tool FQN.
    pub(crate) async fn fetch_tool(
        &self,
        tool_fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::ObjectReference, NexusError> {
        let crawler = self.crawler();
        let tool_registry_object_id = *self.nexus_objects.tool_registry.object_id();

        let tool_id = crate::move_bindings::derive_tool_id(tool_registry_object_id, tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool = crawler
            .get_object_metadata(tool_id)
            .await
            .map_err(NexusError::Rpc)?;

        Ok(tool.object_ref())
    }

    /// Derive and fetch a [`ToolGas`] object based on the provided tool FQN.
    pub(crate) async fn fetch_tool_gas(
        &self,
        tool_fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::ObjectReference, NexusError> {
        let crawler = self.crawler();
        let tool_registry_object_id = *self.nexus_objects.tool_registry.object_id();

        let tool_gas_id =
            crate::move_bindings::derive_tool_gas_id(tool_registry_object_id, tool_fqn)
                .map_err(NexusError::Parsing)?;
        let tool_gas = crawler
            .get_object_metadata(tool_gas_id)
            .await
            .map_err(NexusError::Rpc)?;

        Ok(tool_gas.object_ref())
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
    async fn released_coin_can_be_acquired_again() {
        let coin1 = sui_mocks::mock_sui_object_ref();
        let coin2 = sui_mocks::mock_sui_object_ref();

        let gas = CoinGasPool {
            coins: Arc::new(Mutex::new(vec![coin1.clone(), coin2.clone()])),
            notify: Arc::new(Notify::new()),
            budget: 1000,
            reference_gas_price: 1,
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

    #[test]
    fn gas_reports_coin_budget() {
        let gas = Gas {
            source: GasSource::Coin(CoinGasPool {
                coins: Arc::new(Mutex::new(vec![])),
                notify: Arc::new(Notify::new()),
                budget: 5000,
                reference_gas_price: 1,
            }),
        };
        assert_eq!(gas.get_budget(), 5000);
    }

    #[tokio::test]
    async fn coin_acquisition_waits_for_release() {
        let coin = sui_mocks::mock_sui_object_ref();
        let gas = CoinGasPool {
            coins: Arc::new(Mutex::new(vec![])),
            notify: Arc::new(Notify::new()),
            budget: 100,
            reference_gas_price: 1,
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

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(objects)
            .with_gas(coins, budget);

        let client = builder.build().await.unwrap();
        assert_eq!(client.gas.get_budget(), budget);
        assert_eq!(client.signer.transaction_timeout, Duration::from_secs(5));
    }

    #[tokio::test]
    async fn address_balance_builder_does_not_require_gas_coins() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());

        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .with_address_balance_gas(7_000)
            .build()
            .await
            .unwrap();

        assert_eq!(client.gas.get_budget(), 7_000);
        assert_eq!(client.get_reference_gas_price(), None);
    }

    #[tokio::test]
    async fn builder_rejects_two_gas_sources() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let coin = sui_mocks::mock_sui_object_ref();

        let result = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url("http://127.0.0.1:1")
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .with_gas(vec![coin], 1_000)
            .with_address_balance_gas(1_000)
            .build()
            .await;

        let Err(error) = result else {
            panic!("builder accepted two gas sources");
        };
        assert!(matches!(error, NexusError::Configuration(_)));
        assert!(error.to_string().contains("cannot both be configured"));
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

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let builder = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
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
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        assert_eq!(client.get_reference_gas_price(), Some(1000));

        let mut gas_coin = client.gas.coin_pool().unwrap().acquire_gas_coin().await;
        let sender = client.signer.get_active_address();
        let tx = sui::types::Transaction {
            kind: sui::types::TransactionKind::ProgrammableTransaction(
                sui::types::ProgrammableTransaction {
                    inputs: vec![],
                    commands: vec![],
                },
            ),
            sender,
            gas_payment: sui::types::GasPayment {
                objects: vec![gas_coin.clone()],
                owner: sender,
                price: 1000,
                budget: 1000,
            },
            expiration: sui::types::TransactionExpiration::None,
        };
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

    #[tokio::test]
    async fn execute_tx_without_gas_coin_does_not_refresh_an_object() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let chain = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_execute_transaction_without_gas_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            vec![],
            vec![],
            vec![],
            |request| {
                let transaction = request.transaction.as_ref().unwrap();
                let transaction = sui::types::Transaction::try_from(transaction).unwrap();
                assert!(transaction.gas_payment.objects.is_empty());
            },
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;
        let sender = client.signer.get_active_address();
        let tx = crate::nexus::address_balance::finish_transaction(
            sui::types::ProgrammableTransaction {
                inputs: vec![],
                commands: vec![],
            },
            sender,
            1000,
            crate::nexus::address_balance::SubmissionContext {
                reference_gas_price: 1000,
                epoch: 1,
                chain,
            },
            0,
        );
        let signature = client.signer.sign_tx(&tx).await.unwrap();

        let response = client
            .signer
            .execute_tx_without_gas_coin(tx, signature)
            .await
            .unwrap();

        assert_eq!(response.digest, digest);
    }

    #[tokio::test]
    async fn address_balance_submission_fetches_fresh_context_and_uses_no_gas_object() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let chain = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_submission_context(&mut ledger_service_mock, 17, 23, chain);
        sui_mocks::grpc::mock_execute_transaction_without_gas_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            vec![],
            vec![],
            vec![],
            move |request| {
                let transaction = request.transaction.as_ref().unwrap();
                let transaction = sui::types::Transaction::try_from(transaction).unwrap();
                assert!(transaction.gas_payment.objects.is_empty());
                assert_eq!(transaction.gas_payment.price, 17);
                assert_eq!(transaction.gas_payment.budget, 9_000);
                assert_eq!(
                    transaction.expiration,
                    sui::types::TransactionExpiration::ValidDuring {
                        min_epoch: Some(23),
                        max_epoch: Some(24),
                        min_timestamp: None,
                        max_timestamp: None,
                        chain,
                        nonce: 0,
                    }
                );
            },
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let sender = pk.public_key().derive_address();
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_objects)
            .with_address_balance_gas(9_000)
            .build()
            .await
            .unwrap();

        let result = client
            .submit_transaction(
                sui::types::ProgrammableTransaction {
                    inputs: vec![],
                    commands: vec![],
                },
                sender,
            )
            .await
            .unwrap();

        assert_eq!(result.digest, digest);
    }

    #[allow(dead_code)]
    async fn submit_transaction_accepts_canonical_ptb(
        client: &NexusClient,
        sender: sui::types::Address,
        ptb: sui::types::ProgrammableTransaction,
    ) {
        let _ = client.submit_transaction(ptb, sender).await;
    }
}
