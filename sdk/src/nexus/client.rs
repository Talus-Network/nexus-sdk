//! A [`NexusClient`] combines a [`Signer`], object crawling, owned-coin
//! selection, and an optional shared [`Gas`] source.

use {
    crate::{
        events::{NexusEventDecoder, NexusEventIngestor, NexusEventQuery},
        nexus::{
            address_balance::{fetch_submission_context, finish_transaction, NonceAllocator},
            crawler::Crawler,
            error::NexusError,
            network::NetworkActions,
            scheduler::Scheduler,
            signer::{ExecutedTransaction, Signer},
            state::StateResolver,
            transaction::NexusTransaction,
            workflow::WorkflowActions,
        },
        sui,
        types::{NexusContext, NexusObjects, PackageRole, SharedRoot},
        ToolFqn,
    },
    std::sync::Arc,
    tokio::{
        sync::{Mutex, Notify, OnceCell},
        time::Duration,
    },
};

/// Default transaction gas budget used by clients that select a default.
pub const DEFAULT_GAS_BUDGET: u64 = sui::MIST_PER_SUI / 10;

fn sort_coins_for_ordinal_selection(coins: &mut [(sui::types::ObjectReference, u64)]) {
    coins.sort_by(|(left_coin, left_balance), (right_coin, right_balance)| {
        right_balance
            .cmp(left_balance)
            .then_with(|| left_coin.object_id().cmp(right_coin.object_id()))
    });
}

/// Gas source used to configure a [`NexusClient`] for transactions.
#[derive(Clone, Debug)]
pub enum GasSource {
    /// Owned coin-object gas with a transaction budget.
    Coin(CoinGasPool),
    /// Address-balance gas with a reusable nonce authority.
    AddressBalance(AddressBalanceGas),
}

/// Configured gas inspected from a [`NexusClient`].
pub type Gas = GasSource;

impl GasSource {
    /// Creates an owned coin-object gas source with a transaction budget.
    pub fn coin(coins: Vec<sui::types::ObjectReference>, budget: u64) -> Self {
        Self::Coin(CoinGasPool {
            coins: Arc::new(Mutex::new(coins)),
            notify: Arc::new(Notify::new()),
            budget,
            reference_gas_price: None,
        })
    }

    /// Returns the configured gas budget.
    pub fn get_budget(&self) -> u64 {
        match self {
            Self::Coin(pool) => pool.budget,
            Self::AddressBalance(gas) => gas.budget,
        }
    }

    /// Returns the shared pool when coin based gas is configured.
    #[cfg(test)]
    pub(crate) fn coin_pool(&self) -> Option<&CoinGasPool> {
        match self {
            Self::Coin(pool) => Some(pool),
            Self::AddressBalance(_) => None,
        }
    }

    fn reference_gas_price(&self) -> Option<u64> {
        match self {
            Self::Coin(pool) => pool.reference_gas_price,
            Self::AddressBalance(_) => None,
        }
    }
}

/// Shared owned coin source used for coin based gas.
#[derive(Clone, Debug)]
pub struct CoinGasPool {
    coins: Arc<Mutex<Vec<sui::types::ObjectReference>>>,
    notify: Arc<Notify>,
    budget: u64,
    reference_gas_price: Option<u64>,
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
    /// Creates a configuration backed by the process nonce authority.
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

    /// Configures address balance based gas with the process nonce authority.
    ///
    /// Independently constructed clients in this process allocate from the
    /// same nonce sequence. Use [`Self::with_address_balance_gas_config`] when
    /// the caller must supply a distinct nonce authority explicitly.
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

    /// Configures stable Nexus environment identities.
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
    /// When no gas strategy is configured, the client is built without gas.
    /// Attach gas later with [`NexusClient::set_gas_source`] before submitting
    /// a transaction.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Configuration`] when required configuration is
    /// missing, both gas sources are configured, or coin based gas is
    /// explicitly configured without a coin. Returns [`NexusError::Rpc`] when
    /// the client or coin based gas context cannot be initialized.
    /// Returns [`NexusError::MissingPrivateKey`] when a gas source is
    /// configured without a signing key.
    pub async fn build(self) -> Result<NexusClient, NexusError> {
        let rpc_url = self
            .rpc_url
            .ok_or_else(|| NexusError::Configuration("RPC URL is required".into()))?;
        let nexus_objects = self
            .nexus_objects
            .ok_or_else(|| NexusError::Configuration("Nexus objects are required".into()))?;
        let coin_gas_requested = self.gas_budget.is_some() || !self.gas_coins.is_empty();
        let gas_source = match (coin_gas_requested, self.address_balance_gas) {
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
            (true, None) => Some(GasSource::coin(
                self.gas_coins,
                self.gas_budget
                    .ok_or_else(|| NexusError::Configuration("gas budget is required".into()))?,
            )),
            (false, Some(gas)) => Some(GasSource::AddressBalance(gas)),
            (false, None) => None,
        };
        if gas_source.is_some() && self.pk.is_none() {
            return Err(NexusError::MissingPrivateKey);
        }

        let client = Arc::new(sui::grpc::client(&rpc_url).map_err(NexusError::Rpc)?);
        let actual_chain = client
            .as_ref()
            .clone()
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await
            .map_err(|error| NexusError::Rpc(error.into()))?
            .into_inner()
            .chain_id
            .ok_or_else(|| NexusError::Rpc(anyhow::anyhow!("Sui service omitted its chain ID")))?;
        if actual_chain != nexus_objects.chain_id {
            return Err(NexusError::ChainMismatch {
                expected: nexus_objects.chain_id,
                actual: actual_chain,
            });
        }
        let nexus_objects = Arc::new(nexus_objects);
        let crawler = Arc::new(Crawler::new(Arc::clone(&client)));
        let state_resolver = StateResolver::new(Arc::clone(&crawler));

        let signer = self.pk.map(|pk| {
            Signer::new(
                client,
                pk,
                self.transaction_timeout.unwrap_or(Duration::from_secs(5)),
                crate::events::NexusEventDecoder::new(
                    state_resolver.clone(),
                    Arc::clone(&nexus_objects),
                ),
            )
        });

        let nexus_client = NexusClient {
            signer,
            gas: Arc::new(OnceCell::new()),
            nexus_objects,
            crawler,
            state_resolver,
            rpc_url,
        };
        if let Some(gas_source) = gas_source {
            nexus_client.set_gas_source(gas_source).await?;
        }

        Ok(nexus_client)
    }
}

#[derive(Clone)]
pub struct NexusClient {
    /// The wallet context to use for transactions. This defines the TX sender
    /// address when a private key was configured.
    pub(super) signer: Option<Signer>,
    /// Shared optional gas configuration for Nexus operations.
    gas: Arc<OnceCell<GasSource>>,
    /// Nexus objects to use.
    pub(super) nexus_objects: Arc<NexusObjects>,
    /// Provide access to an instantiated object crawler.
    pub(super) crawler: Arc<Crawler>,
    /// Resolves live state and immutable package graphs.
    pub(super) state_resolver: StateResolver,
    /// RPC URL used by the client.
    pub(super) rpc_url: String,
}

impl NexusClient {
    /// Return a [`NexusClientBuilder`] instance for building a Nexus client.
    pub fn builder() -> NexusClientBuilder {
        NexusClientBuilder::new()
    }

    /// Return a [`NetworkActions`] instance for Nexus network fee operations.
    pub fn network(&self) -> NetworkActions {
        NetworkActions {
            client: self.clone(),
        }
    }

    /// Return a [`WorkflowActions`] instance for performing workflow-related operations.
    pub fn workflow(&self) -> WorkflowActions {
        WorkflowActions {
            client: self.clone(),
        }
    }

    /// Returns the scheduler facade for this client.
    pub fn scheduler(&self) -> Scheduler {
        Scheduler {
            client: self.clone(),
        }
    }

    /// Starts a programmable transaction for effects on an existing object.
    ///
    /// The object witness is validated for SDK compatibility, while the fixed
    /// runtime authority selects the package graph. This prevents a historical
    /// Task or execution from routing effects back through historical code.
    ///
    /// # Errors
    ///
    /// Returns compatibility, object state, or RPC errors reported by
    /// [`Self::context_for_object`] and [`Self::runtime_context`].
    pub async fn transaction_for_object(
        &self,
        object_id: sui::types::Address,
        required_roots: &[SharedRoot],
    ) -> Result<NexusTransaction, NexusError> {
        self.context_for_object(object_id).await?;
        let context = self.runtime_context(required_roots).await?;
        Ok(NexusTransaction::for_object(
            self.clone(),
            context,
            object_id,
        ))
    }

    /// Starts a programmable transaction whose package graph is selected by
    /// an explicit creator package.
    ///
    /// # Errors
    ///
    /// Returns compatibility, package metadata, or RPC errors reported by
    /// [`Self::context_for_creator`].
    pub async fn transaction_for_creator(
        &self,
        creator_package: sui::types::Address,
        creator_role: PackageRole,
        required_roots: &[SharedRoot],
    ) -> Result<NexusTransaction, NexusError> {
        let context = self
            .context_for_creator(creator_package, creator_role, required_roots)
            .await?;
        Ok(NexusTransaction::for_creator(
            self.clone(),
            context,
            creator_package,
            creator_role,
        ))
    }

    /// Returns a
    /// [`NetworkAuthActions`](crate::nexus::network_auth::NetworkAuthActions)
    /// instance for tool network authorization operations.
    pub fn network_auth(&self) -> crate::nexus::network_auth::NetworkAuthActions {
        crate::nexus::network_auth::NetworkAuthActions {
            client: self.clone(),
        }
    }

    /// Returns a [`ToolActions`](crate::nexus::tool::ToolActions) instance for
    /// tool operations.
    pub fn tool(&self) -> crate::nexus::tool::ToolActions {
        crate::nexus::tool::ToolActions {
            client: self.clone(),
        }
    }

    /// Returns a [`TapActions`](crate::nexus::tap::TapActions) instance for
    /// standard TAP operations.
    pub fn tap(&self) -> crate::nexus::tap::TapActions {
        crate::nexus::tap::TapActions {
            client: self.clone(),
        }
    }

    /// Return a [`Crawler`] instance for object crawling operations.
    pub fn crawler(&self) -> &Crawler {
        &self.crawler
    }

    /// Returns the live object and package state resolver.
    pub fn state_resolver(&self) -> &StateResolver {
        &self.state_resolver
    }

    /// Resolves the operation context selected by one live object witness.
    ///
    /// Package metadata is resolved only for this request. Stable environment
    /// identity remains shared by all contexts created by this client.
    ///
    /// # Errors
    ///
    /// Returns compatibility, object state, or RPC errors reported by
    /// [`StateResolver::resolve_context`].
    pub async fn context_for_object(
        &self,
        object_id: sui::types::Address,
    ) -> Result<Arc<NexusContext>, NexusError> {
        let (_, context) = self
            .state_resolver
            .resolve_context(Arc::clone(&self.nexus_objects), object_id)
            .await?;
        Ok(Arc::new(context))
    }

    /// Resolves the operation context selected by a canonical shared root.
    ///
    /// # Errors
    ///
    /// Returns compatibility, object state, or RPC errors reported by
    /// [`Self::context_for_object`].
    pub async fn context_for_root(
        &self,
        root: &SharedRoot,
    ) -> Result<Arc<NexusContext>, NexusError> {
        self.context_for_object(root.object_id()).await
    }

    /// Resolves one source object and validates every canonical root used by
    /// the operation.
    ///
    /// # Errors
    ///
    /// Returns compatibility, object state, or RPC errors reported by
    /// [`StateResolver::resolve_context_with_roots`].
    pub async fn context_for_object_with_roots(
        &self,
        object_id: sui::types::Address,
        required_roots: &[SharedRoot],
    ) -> Result<Arc<NexusContext>, NexusError> {
        let (_, context) = self
            .state_resolver
            .resolve_context_with_roots(Arc::clone(&self.nexus_objects), object_id, required_roots)
            .await?;
        Ok(Arc::new(context))
    }

    /// Resolves an explicit creator package after validating every root it
    /// will use.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::IncompatiblePackage`] when creator linkage does
    /// not match current root authority. Other resolver and RPC failures are
    /// returned unchanged.
    pub async fn context_for_creator(
        &self,
        creator_package: sui::types::Address,
        creator_role: PackageRole,
        required_roots: &[SharedRoot],
    ) -> Result<Arc<NexusContext>, NexusError> {
        let context = self
            .state_resolver
            .resolve_creator_context(
                Arc::clone(&self.nexus_objects),
                creator_package,
                creator_role,
                required_roots,
            )
            .await?;
        Ok(Arc::new(context))
    }

    /// Resolves the package graph preferred for new proposal construction.
    ///
    /// This context remains available while runtime effects are paused. It is
    /// a routing decision only and does not confer protocol effect authority.
    ///
    /// # Errors
    ///
    /// Returns routing, package compatibility, object state, or RPC errors
    /// reported by [`StateResolver::resolve_routing_context`].
    pub async fn routing_context(
        &self,
        required_roots: &[SharedRoot],
    ) -> Result<Arc<NexusContext>, NexusError> {
        let context = self
            .state_resolver
            .resolve_routing_context(Arc::clone(&self.nexus_objects), required_roots)
            .await?;
        Ok(Arc::new(context))
    }

    /// Resolves the only package graph currently allowed to produce protocol effects.
    ///
    /// # Errors
    ///
    /// Returns runtime authority, package compatibility, object state, or RPC
    /// errors reported by [`StateResolver::resolve_runtime_context`].
    pub async fn runtime_context(
        &self,
        required_roots: &[SharedRoot],
    ) -> Result<Arc<NexusContext>, NexusError> {
        let context = self
            .state_resolver
            .resolve_runtime_context(Arc::clone(&self.nexus_objects), required_roots)
            .await?;
        Ok(Arc::new(context))
    }

    /// Fetches the current transaction input reference for a stable object ID.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Rpc`] when the object metadata cannot be fetched.
    pub(crate) async fn object_reference(
        &self,
        object_id: sui::types::Address,
    ) -> Result<sui::types::ObjectReference, NexusError> {
        self.crawler
            .get_object_metadata(object_id)
            .await
            .map(|object| object.object_ref())
            .map_err(NexusError::Rpc)
    }

    /// Return the owner address derived from this client's signing key.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::MissingPrivateKey`] for a query-only client.
    pub fn owner(&self) -> Result<sui::types::Address, NexusError> {
        Ok(self.signer()?.get_active_address())
    }

    /// Return a shared handle to the gRPC client used by this client.
    pub fn grpc_client(&self) -> Arc<sui::grpc::Client> {
        self.crawler.grpc_client()
    }

    /// Clone the inner gRPC client for independently mutable or long-lived operations.
    ///
    /// The clone reuses the configured transport and does not reconstruct a connection from the
    /// stored RPC URL.
    pub fn clone_grpc_client(&self) -> sui::grpc::Client {
        self.crawler.clone_grpc_client()
    }

    /// Fetch every coin owned by this client's signing address with the exact
    /// requested Move struct tag.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Rpc`] when the owned-object query fails.
    /// Returns [`NexusError::MissingPrivateKey`] for a query-only client.
    pub async fn fetch_coins_by_type(
        &self,
        object_type: sui::types::StructTag,
    ) -> Result<Vec<(sui::types::ObjectReference, u64)>, NexusError> {
        let owner = self.owner()?;
        self.crawler
            .fetch_coins_for_address_by_type(owner, object_type)
            .await
            .map_err(NexusError::Rpc)
    }

    /// Fetch an owned SUI coin by object ID.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Wallet`] when the signing address owns no SUI
    /// coins or does not own `coin_id`. Returns [`NexusError::Rpc`] when the
    /// owned-object query fails.
    /// Returns [`NexusError::MissingPrivateKey`] for a query-only client.
    pub async fn fetch_coin(
        &self,
        coin_id: sui::types::Address,
    ) -> Result<sui::types::ObjectReference, NexusError> {
        self.fetch_coin_with_balance(coin_id)
            .await
            .map(|(coin, _)| coin)
    }

    /// Fetch an owned SUI coin and its balance by object ID.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Wallet`] when the signing address owns no SUI
    /// coins or does not own `coin_id`. Returns [`NexusError::Rpc`] when the
    /// owned-object query fails.
    /// Returns [`NexusError::MissingPrivateKey`] for a query-only client.
    pub async fn fetch_coin_with_balance(
        &self,
        coin_id: sui::types::Address,
    ) -> Result<(sui::types::ObjectReference, u64), NexusError> {
        let coins = self
            .fetch_coins_by_type(sui::types::StructTag::gas_coin())
            .await?;

        if coins.is_empty() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "The wallet does not have enough coins to submit the transaction"
            )));
        }

        coins
            .into_iter()
            .find(|(coin, _)| *coin.object_id() == coin_id)
            .ok_or_else(|| {
                NexusError::Wallet(anyhow::anyhow!("Coin '{coin_id}' not found in wallet"))
            })
    }

    /// Fetch an owned coin with the requested Move type by object ID or
    /// deterministic ordinal.
    ///
    /// When `coin_id` is `None`, coins are ordered by balance descending and
    /// object ID ascending before selecting `ordinal`.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::Wallet`] when no matching coin exists or the
    /// ordinal is out of range. Returns [`NexusError::Rpc`] when the
    /// owned-object query fails.
    /// Returns [`NexusError::MissingPrivateKey`] for a query-only client.
    pub async fn fetch_coin_by_type(
        &self,
        coin_id: Option<sui::types::Address>,
        ordinal: usize,
        object_type: sui::types::StructTag,
    ) -> Result<sui::types::ObjectReference, NexusError> {
        let label = format!("coins of type '{object_type}'");
        let mut coins = self.fetch_coins_by_type(object_type).await?;

        if coins.is_empty() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "The wallet does not have enough {label}"
            )));
        }

        match coin_id {
            Some(coin_id) => coins
                .into_iter()
                .find(|(coin, _)| *coin.object_id() == coin_id)
                .map(|(coin, _)| coin)
                .ok_or_else(|| {
                    NexusError::Wallet(anyhow::anyhow!(
                        "Object '{coin_id}' with {label} not found in wallet"
                    ))
                }),
            None => {
                sort_coins_for_ordinal_selection(&mut coins);
                if ordinal >= coins.len() {
                    return Err(NexusError::Wallet(anyhow::anyhow!(
                        "The wallet does not have enough {label} to select object #{ordinal}"
                    )));
                }

                Ok(coins.swap_remove(ordinal).0)
            }
        }
    }

    /// Return the RPC URL configured for this client.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Return the [`Signer`] configured for this client.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::MissingPrivateKey`] for a query-only client.
    pub fn signer(&self) -> Result<&Signer, NexusError> {
        self.signer.as_ref().ok_or(NexusError::MissingPrivateKey)
    }

    /// Returns a [`NexusEventIngestor`] whose server filter is selected by the
    /// witness on `filter_source`.
    ///
    /// Event decoding still resolves each emitter package independently. The
    /// source controls only the exact wrapper types sent to the Sui server.
    ///
    /// # Errors
    ///
    /// Returns compatibility, object state, or RPC errors reported by
    /// [`Self::context_for_object`].
    pub async fn event_ingestor(
        &self,
        filter_source: sui::types::Address,
    ) -> Result<NexusEventIngestor, NexusError> {
        let context = self.context_for_object(filter_source).await?;
        let decoder =
            NexusEventDecoder::new(self.state_resolver.clone(), Arc::clone(&self.nexus_objects));
        Ok(NexusEventIngestor::new(
            &self.rpc_url,
            NexusEventQuery::new(context, decoder),
        ))
    }

    /// Attaches the gas source shared by this client, its clones, and action facades.
    ///
    /// Coin based gas validates that at least one coin is present and fetches
    /// the current reference gas price before the shared write-once transition.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::GasSourceAlreadyConfigured`] when gas is already
    /// attached, [`NexusError::MissingPrivateKey`] when no signing key is
    /// configured, or [`NexusError::Configuration`] when coin based gas has no
    /// coins. Returns [`NexusError::Rpc`] when the reference gas price cannot be fetched.
    pub async fn set_gas_source(&self, mut source: GasSource) -> Result<(), NexusError> {
        self.signer()?;
        if self.gas.get().is_some() {
            return Err(NexusError::GasSourceAlreadyConfigured);
        }

        match &mut source {
            GasSource::Coin(pool) => {
                if pool.coins.lock().await.is_empty() {
                    return Err(NexusError::Configuration(
                        "at least one gas coin is required for coin based gas".into(),
                    ));
                }
                let mut client = self.clone_grpc_client();
                let reference_gas_price = client
                    .get_reference_gas_price()
                    .await
                    .map_err(|error| NexusError::Rpc(error.into()))?;
                pool.reference_gas_price = Some(reference_gas_price);
            }
            GasSource::AddressBalance(_) => {}
        }

        self.gas
            .set(source)
            .map_err(|_| NexusError::GasSourceAlreadyConfigured)
    }

    /// Returns a clone of the configured [`Gas`], or `None` when unattached.
    pub fn gas_config(&self) -> Option<Gas> {
        self.gas.get().cloned()
    }

    /// Returns the cached reference gas price for coin based submissions.
    ///
    /// Missing gas and address balance based gas return `None`.
    pub fn get_reference_gas_price(&self) -> Option<u64> {
        self.gas.get().and_then(GasSource::reference_gas_price)
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
        let signer = self.signer()?;
        let gas = self.gas_configured()?;
        let response = match &gas {
            GasSource::Coin(pool) => {
                let reference_gas_price = pool.reference_gas_price.ok_or_else(|| {
                    NexusError::Configuration("coin gas source is not prepared".into())
                })?;
                let mut gas_coin = pool.acquire_gas_coin().await;
                let tx = sui::types::Transaction {
                    kind: sui::types::TransactionKind::ProgrammableTransaction(tx),
                    sender: address,
                    gas_payment: sui::types::GasPayment {
                        objects: vec![gas_coin.clone()],
                        owner: address,
                        price: reference_gas_price,
                        budget: pool.budget,
                    },
                    expiration: sui::types::TransactionExpiration::None,
                };
                let signature = signer.sign_tx(&tx).await?;
                let response = signer.execute_tx(tx, signature, &mut gas_coin).await;
                pool.release_gas_coin(gas_coin).await;
                response
            }
            GasSource::AddressBalance(gas) => {
                let mut client = self.clone_grpc_client();
                let context = fetch_submission_context(&mut client).await?;
                let nonce = gas.allocate_nonce()?;
                let tx = finish_transaction(tx, address, gas.budget, context, nonce);
                let signature = signer.sign_tx(&tx).await?;
                signer.execute_tx_without_gas_coin(tx, signature).await
            }
        };

        response
    }

    pub(crate) fn gas_configured(&self) -> Result<Gas, NexusError> {
        self.gas_config().ok_or_else(|| {
            NexusError::Configuration("a gas source is required for transaction operations".into())
        })
    }

    /// Derive and fetch a [`Tool`] object based on the provided tool FQN.
    pub(crate) async fn fetch_tool(
        &self,
        tool_fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::ObjectReference, NexusError> {
        let crawler = self.crawler();
        let tool_registry_object_id = self.nexus_objects.tool_registry.object_id();

        let tool_id = crate::move_bindings::derive_tool_id(tool_registry_object_id, tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool = crawler
            .get_object_metadata(tool_id)
            .await
            .map_err(NexusError::Rpc)?;

        Ok(tool.object_ref())
    }

    /// Derive and fetch a [`ToolCashier`] object based on the provided tool FQN.
    pub(crate) async fn fetch_tool_cashier(
        &self,
        tool_fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::ObjectReference, NexusError> {
        let crawler = self.crawler();
        let tool_registry_object_id = self.nexus_objects.tool_registry.object_id();
        let tool_id = crate::move_bindings::derive_tool_id(tool_registry_object_id, tool_fqn)
            .map_err(NexusError::Parsing)?;
        let context = self.context_for_object(tool_id).await?;
        let tool_package = context
            .require_package(PackageRole::Tool)
            .map_err(|error| NexusError::Configuration(error.to_string()))?;
        let tool_cashier_origin = tool_package
            .type_origin("tool_cashier", "ToolCashierKey")
            .map_err(|error| NexusError::Configuration(error.to_string()))?;
        let tool_cashier_id =
            crate::move_bindings::derive_tool_cashier_id(tool_cashier_origin, tool_id)
                .map_err(NexusError::Parsing)?;
        let tool_cashier = crawler
            .get_object_metadata(tool_cashier_id)
            .await
            .map_err(NexusError::Rpc)?;

        Ok(tool_cashier.object_ref())
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

    fn owned_coin_object(
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Address,
        balance: u64,
        object_type: &sui::types::StructTag,
    ) -> sui::grpc::Object {
        let mut object = sui::grpc::Object::default();
        object.set_object_id(*object_ref.object_id());
        object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Address(owner)));
        object.set_version(object_ref.version());
        object.set_digest(*object_ref.digest());
        object.set_balance(balance);
        object.set_object_type(object_type.to_string());
        object
    }

    async fn client_with_owned_coins(
        pk: sui::crypto::Ed25519PrivateKey,
        object_type: sui::types::StructTag,
        objects: Vec<sui::grpc::Object>,
    ) -> NexusClient {
        let owner = pk.public_key().derive_address();
        let expected_owner = owner.to_string();
        let expected_object_type = object_type.to_string();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once(move |request| {
                assert_eq!(
                    request.get_ref().owner.as_deref(),
                    Some(expected_owner.as_str())
                );
                assert_eq!(
                    request.get_ref().object_type.as_deref(),
                    Some(expected_object_type.as_str())
                );
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(objects);
                Ok(response.into())
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });

        NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build")
    }

    async fn keyless_client(rpc_url: &str) -> NexusClient {
        NexusClientBuilder::new()
            .with_rpc_url(rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("query-only client should build without a private key")
    }

    #[tokio::test]
    async fn stable_environment_identity_is_shared_by_client_clones() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;
        let cloned = client.clone();

        assert!(Arc::ptr_eq(
            &client.get_nexus_objects(),
            &cloned.get_nexus_objects(),
        ));
        assert_eq!(client.get_nexus_objects(), cloned.get_nexus_objects());
    }

    #[tokio::test]
    async fn released_coin_can_be_acquired_again() {
        let coin1 = sui_mocks::mock_sui_object_ref();
        let coin2 = sui_mocks::mock_sui_object_ref();

        let gas = CoinGasPool {
            coins: Arc::new(Mutex::new(vec![coin1.clone(), coin2.clone()])),
            notify: Arc::new(Notify::new()),
            budget: 1000,
            reference_gas_price: Some(1),
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
        let gas = GasSource::coin(vec![sui_mocks::mock_sui_object_ref()], 5000);
        assert_eq!(gas.get_budget(), 5000);
    }

    #[tokio::test]
    async fn coin_acquisition_waits_for_release() {
        let coin = sui_mocks::mock_sui_object_ref();
        let gas = CoinGasPool {
            coins: Arc::new(Mutex::new(vec![])),
            notify: Arc::new(Notify::new()),
            budget: 100,
            reference_gas_price: Some(1),
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
        assert_eq!(
            client
                .gas_config()
                .expect("legacy builder should attach coin gas")
                .get_budget(),
            budget
        );
        assert_eq!(
            client
                .signer()
                .expect("private key should configure a signer")
                .transaction_timeout,
            Duration::from_secs(5)
        );
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

        assert_eq!(
            client
                .gas_config()
                .expect("legacy builder should attach address balance gas")
                .get_budget(),
            7_000
        );
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
    async fn builder_without_private_key_supports_crawler_reads() {
        let object = sui_mocks::mock_sui_object_ref();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            object.clone(),
            sui::types::Owner::Immutable,
            None,
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = keyless_client(&rpc_url).await;

        let response = client
            .crawler()
            .get_object_metadata(*object.object_id())
            .await
            .expect("query-only crawler read should succeed");

        assert_eq!(response.object_ref(), object);
    }

    #[tokio::test]
    async fn keyless_client_supports_read_only_construction() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;

        assert!(matches!(client.owner(), Err(NexusError::MissingPrivateKey)));
        assert!(client.signer().is_err());
    }

    #[tokio::test]
    async fn builder_with_gas_without_private_key_returns_missing_private_key() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let result = NexusClientBuilder::new()
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .with_address_balance_gas(1_000)
            .build()
            .await;

        assert!(matches!(result, Err(NexusError::MissingPrivateKey)));
    }

    #[tokio::test]
    async fn keyless_owner_and_signer_return_missing_private_key() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;

        assert!(matches!(client.owner(), Err(NexusError::MissingPrivateKey)));
        assert!(matches!(
            client.signer(),
            Err(NexusError::MissingPrivateKey)
        ));
    }

    #[tokio::test]
    async fn keyless_owned_coin_query_returns_missing_private_key_before_rpc() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;

        let result = client
            .fetch_coins_by_type(sui::types::StructTag::gas_coin())
            .await;

        assert!(matches!(result, Err(NexusError::MissingPrivateKey)));
    }

    #[tokio::test]
    async fn keyless_gas_attachment_returns_missing_private_key_before_source_validation() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;

        let result = client.set_gas_source(GasSource::coin(vec![], 1_000)).await;

        assert!(matches!(result, Err(NexusError::MissingPrivateKey)));
        assert!(client.gas_config().is_none());
    }

    #[tokio::test]
    async fn keyless_submission_returns_missing_private_key_before_missing_gas() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;

        let result = client
            .submit_transaction(
                sui::types::ProgrammableTransaction {
                    inputs: vec![],
                    commands: vec![],
                },
                sui::types::Address::ZERO,
            )
            .await;

        assert!(matches!(result, Err(NexusError::MissingPrivateKey)));
    }

    #[tokio::test]
    async fn keyless_mutating_action_returns_missing_private_key_before_submission() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = keyless_client(&rpc_url).await;

        let result = client.network().configure_priority_fee_vault(1).await;

        assert!(matches!(result, Err(NexusError::MissingPrivateKey)));
    }

    #[tokio::test]
    async fn keyless_client_exposes_shared_grpc_arc() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("query-only client should build without a private key");
        let client_handle = client.grpc_client();
        let crawler_handle = client.crawler().grpc_client();

        assert!(Arc::ptr_eq(&client_handle, &crawler_handle));
        assert_eq!(client_handle.uri().to_string(), format!("{rpc_url}/"));
    }

    #[tokio::test]
    async fn clone_grpc_client_uses_stored_transport_without_reconstruction() {
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 42);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let mut client = keyless_client(&rpc_url).await;
        client.rpc_url = "not a valid URL".into();

        let mut grpc_client = client.clone_grpc_client();

        assert_eq!(
            grpc_client
                .get_reference_gas_price()
                .await
                .expect("cloned client should use the configured transport"),
            42
        );
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
    async fn builder_without_gas_supports_reads() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let object = sui_mocks::mock_sui_object_ref();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            object.clone(),
            sui::types::Owner::Immutable,
            None,
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");
        let response = client
            .crawler()
            .get_object_metadata(*object.object_id())
            .await
            .expect("read-only query should not require gas coins");

        assert!(client.gas_config().is_none());
        assert_eq!(client.get_reference_gas_price(), None);
        assert_eq!(response.object_ref(), object);
    }

    #[tokio::test]
    async fn client_exposes_its_owner_and_shared_grpc_arc() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");

        assert_eq!(
            client
                .owner()
                .expect("private key should configure an owner"),
            owner
        );
        let client_handle = client.grpc_client();
        let crawler_handle = client.crawler().grpc_client();
        let signer_handle = &client
            .signer()
            .expect("private key should configure a signer")
            .client;
        assert!(Arc::ptr_eq(&client_handle, &crawler_handle));
        assert!(Arc::ptr_eq(&crawler_handle, signer_handle));
    }

    #[tokio::test]
    async fn fetch_coin_uses_client_owner_and_crawler() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let object_type = sui::types::StructTag::gas_coin();
        let coin = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x10"));
        let client = client_with_owned_coins(
            pk,
            object_type.clone(),
            vec![owned_coin_object(coin.clone(), owner, 50, &object_type)],
        )
        .await;

        let selected = client
            .fetch_coin(*coin.object_id())
            .await
            .expect("owned SUI coin should be selected");

        assert_eq!(selected, coin);
    }

    #[tokio::test]
    async fn fetch_coin_with_balance_returns_exact_balance() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let object_type = sui::types::StructTag::gas_coin();
        let coin = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x11"));
        let client = client_with_owned_coins(
            pk,
            object_type.clone(),
            vec![owned_coin_object(coin.clone(), owner, 7_654, &object_type)],
        )
        .await;

        let selected = client
            .fetch_coin_with_balance(*coin.object_id())
            .await
            .expect("owned SUI coin balance should be returned");

        assert_eq!(selected, (coin, 7_654));
    }

    #[tokio::test]
    async fn fetch_coin_rejects_an_id_missing_from_the_wallet() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let object_type = sui::types::StructTag::gas_coin();
        let owned_coin = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x12"));
        let missing_id = sui::types::Address::from_static("0x13");
        let client = client_with_owned_coins(
            pk,
            object_type.clone(),
            vec![owned_coin_object(owned_coin, owner, 100, &object_type)],
        )
        .await;

        let error = client
            .fetch_coin(missing_id)
            .await
            .expect_err("unowned SUI coin should be rejected");

        assert!(matches!(error, NexusError::Wallet(_)));
        assert!(error.to_string().contains("not found in wallet"));
    }

    #[tokio::test]
    async fn fetch_coin_rejects_an_empty_wallet() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let object_type = sui::types::StructTag::gas_coin();
        let client = client_with_owned_coins(pk, object_type, vec![]).await;

        let error = client
            .fetch_coin(sui::types::Address::from_static("0x14"))
            .await
            .expect_err("empty wallet should not provide a SUI coin");

        assert!(matches!(error, NexusError::Wallet(_)));
        assert!(error.to_string().contains("does not have enough coins"));
    }

    #[tokio::test]
    async fn fetch_coins_by_type_maps_crawler_failure_to_rpc() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let object_type = sui::types::StructTag::gas_coin();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once(|_| Err(tonic::Status::unavailable("state service unavailable")));
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");

        let error = client
            .fetch_coins_by_type(object_type)
            .await
            .expect_err("crawler failure should be preserved");

        assert!(matches!(error, NexusError::Rpc(_)));
        assert!(error.to_string().contains("state service unavailable"));
    }

    #[tokio::test]
    async fn fetch_coin_by_type_selects_the_explicit_id() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let object_type = crate::types::UsTokenConfig::new(
            sui::types::Address::from_static("0xa"),
            sui::types::Address::from_static("0xb"),
            sui::types::Address::from_static("0xc"),
        )
        .coin_type_tag();
        let first_coin = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x20"));
        let requested_coin = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x21"));
        let client = client_with_owned_coins(
            pk,
            object_type.clone(),
            vec![
                owned_coin_object(first_coin, owner, 200, &object_type),
                owned_coin_object(requested_coin.clone(), owner, 100, &object_type),
            ],
        )
        .await;

        let selected = client
            .fetch_coin_by_type(Some(*requested_coin.object_id()), 0, object_type)
            .await
            .expect("explicit typed coin should be selected");

        assert_eq!(selected, requested_coin);
    }

    #[test]
    fn ordinal_coin_sort_uses_balance_descending_then_object_id() {
        let smallest_balance =
            sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x1"));
        let lower_tied_id = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x2"));
        let higher_tied_id = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x3"));
        let mut coins = vec![
            (smallest_balance.clone(), 10),
            (higher_tied_id.clone(), 100),
            (lower_tied_id.clone(), 100),
        ];

        sort_coins_for_ordinal_selection(&mut coins);

        assert_eq!(
            coins,
            vec![
                (lower_tied_id, 100),
                (higher_tied_id, 100),
                (smallest_balance, 10),
            ]
        );
    }

    #[tokio::test]
    async fn fetch_coin_by_type_selects_the_deterministic_ordinal() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let object_type = crate::types::UsTokenConfig::new(
            sui::types::Address::from_static("0xa"),
            sui::types::Address::from_static("0xb"),
            sui::types::Address::from_static("0xc"),
        )
        .coin_type_tag();
        let smallest_balance =
            sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x1"));
        let lower_tied_id = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x2"));
        let higher_tied_id = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x3"));
        let client = client_with_owned_coins(
            pk,
            object_type.clone(),
            vec![
                owned_coin_object(smallest_balance, owner, 10, &object_type),
                owned_coin_object(higher_tied_id.clone(), owner, 100, &object_type),
                owned_coin_object(lower_tied_id, owner, 100, &object_type),
            ],
        )
        .await;

        let selected = client
            .fetch_coin_by_type(None, 1, object_type)
            .await
            .expect("ordinal should use deterministic typed-coin ordering");

        assert_eq!(selected, higher_tied_id);
    }

    #[tokio::test]
    async fn fetch_coin_by_type_rejects_an_out_of_range_ordinal() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let object_type = crate::types::UsTokenConfig::new(
            sui::types::Address::from_static("0xa"),
            sui::types::Address::from_static("0xb"),
            sui::types::Address::from_static("0xc"),
        )
        .coin_type_tag();
        let coin = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x30"));
        let client = client_with_owned_coins(
            pk,
            object_type.clone(),
            vec![owned_coin_object(coin, owner, 100, &object_type)],
        )
        .await;

        let error = client
            .fetch_coin_by_type(None, 1, object_type)
            .await
            .expect_err("ordinal beyond owned typed coins should be rejected");

        assert!(matches!(error, NexusError::Wallet(_)));
        assert!(error.to_string().contains("select object #1"));
    }

    #[tokio::test]
    async fn direct_address_balance_attachment_is_shared_with_clones_and_actions() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");
        let client_clone = client.clone();
        let workflow = client.workflow();
        let tap = client.tap();

        client
            .set_gas_source(GasSource::AddressBalance(AddressBalanceGas::new(4_321)))
            .await
            .expect("address balance gas should attach");

        assert_eq!(
            client_clone
                .gas_config()
                .expect("clone should observe attached gas")
                .get_budget(),
            4_321
        );
        assert_eq!(
            workflow
                .client
                .gas_config()
                .expect("workflow facade should observe attached gas")
                .get_budget(),
            4_321
        );
        assert_eq!(
            tap.client
                .gas_config()
                .expect("TAP facade should observe attached gas")
                .get_budget(),
            4_321
        );
    }

    #[tokio::test]
    async fn direct_coin_attachment_fetches_reference_gas_price() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let coin = sui_mocks::mock_sui_object_ref();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 987);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");

        client
            .set_gas_source(GasSource::coin(vec![coin], 8_765))
            .await
            .expect("coin gas should attach");

        assert_eq!(
            client
                .gas_config()
                .expect("coin gas should be configured")
                .get_budget(),
            8_765
        );
        assert_eq!(client.get_reference_gas_price(), Some(987));
    }

    #[tokio::test]
    async fn direct_coin_attachment_rejects_empty_pool() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");

        let error = client
            .set_gas_source(GasSource::coin(vec![], 1_000))
            .await
            .expect_err("empty coin gas should fail");

        assert!(error.to_string().contains("at least one gas coin"));
        assert!(client.gas_config().is_none());
    }

    #[tokio::test]
    async fn gas_attachment_rejects_replacement() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");
        client
            .set_gas_source(GasSource::AddressBalance(AddressBalanceGas::new(1_000)))
            .await
            .expect("first gas attachment should succeed");

        let error = client
            .set_gas_source(GasSource::AddressBalance(AddressBalanceGas::new(2_000)))
            .await
            .expect_err("replacement should fail");

        assert!(matches!(&error, NexusError::GasSourceAlreadyConfigured));
        assert_eq!(error.to_string(), "a gas source is already configured");
        assert_eq!(
            client
                .gas_config()
                .expect("original gas should remain")
                .get_budget(),
            1_000
        );
    }

    #[tokio::test]
    async fn concurrent_gas_attachment_has_one_winner() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");
        let barrier = Arc::new(tokio::sync::Barrier::new(3));
        let first_client = client.clone();
        let first_barrier = Arc::clone(&barrier);
        let first = tokio::spawn(async move {
            first_barrier.wait().await;
            first_client
                .set_gas_source(GasSource::AddressBalance(AddressBalanceGas::new(1_000)))
                .await
        });
        let second_client = client.clone();
        let second_barrier = Arc::clone(&barrier);
        let second = tokio::spawn(async move {
            second_barrier.wait().await;
            second_client
                .set_gas_source(GasSource::AddressBalance(AddressBalanceGas::new(2_000)))
                .await
        });
        barrier.wait().await;
        let results = [first.await.unwrap(), second.await.unwrap()];

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .all(|error| matches!(error, NexusError::GasSourceAlreadyConfigured)));
        assert!(matches!(
            client
                .gas_config()
                .expect("one gas source should win")
                .get_budget(),
            1_000 | 2_000
        ));
    }

    #[tokio::test]
    async fn submit_transaction_without_gas_returns_configuration_error() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let sender = pk.public_key().derive_address();
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("client without gas should build");

        let result = client
            .submit_transaction(
                sui::types::ProgrammableTransaction {
                    inputs: vec![],
                    commands: vec![],
                },
                sender,
            )
            .await;
        let Err(error) = result else {
            panic!("submission without gas should fail");
        };

        assert!(matches!(error, NexusError::Configuration(_)));
        assert!(error
            .to_string()
            .contains("a gas source is required for transaction operations"));
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

        let Err(error) = builder.build().await else {
            panic!("explicit coin gas without coins should fail");
        };
        assert!(matches!(error, NexusError::Configuration(_)));
        assert!(error.to_string().contains("at least one gas coin"));
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
        assert_eq!(
            client
                .gas_config()
                .expect("legacy builder should attach coin gas")
                .get_budget(),
            budget
        );
        assert_eq!(
            client
                .signer()
                .expect("private key should configure a signer")
                .transaction_timeout,
            Duration::from_secs(10)
        );
    }

    #[tokio::test]
    async fn reusable_address_balance_builder_configuration_remains_supported() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(sui_mocks::mock_nexus_objects())
            .with_address_balance_gas_config(AddressBalanceGas::new(6_000))
            .build()
            .await
            .expect("legacy reusable address balance configuration should build");

        assert_eq!(
            client
                .gas_config()
                .expect("legacy builder should attach reusable gas")
                .get_budget(),
            6_000
        );
    }

    #[tokio::test]
    async fn test_execute_tx_mutates_gas_coin() {
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
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

        let gas = client
            .gas_config()
            .expect("mock client should configure coin gas");
        let mut gas_coin = gas.coin_pool().unwrap().acquire_gas_coin().await;
        let signer = client
            .signer()
            .expect("mock client should configure a signer");
        let sender = signer.get_active_address();
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
        let signature = signer.sign_tx(&tx).await.unwrap();

        let response = signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await
            .unwrap();

        assert_eq!(response.digest, submitted.digest());

        assert_eq!(gas_coin.version(), gas_coin_ref.version());
        assert_eq!(gas_coin.digest(), gas_coin_ref.digest());
    }

    #[tokio::test]
    async fn execute_tx_without_gas_coin_does_not_refresh_an_object() {
        let mut rng = rand::thread_rng();
        let chain = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let submitted =
            sui_mocks::grpc::mock_execute_transaction_without_gas_and_wait_for_checkpoint(
                &mut tx_service_mock,
                &mut sub_service_mock,
                &mut ledger_service_mock,
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
        let signer = client
            .signer()
            .expect("mock client should configure a signer");
        let sender = signer.get_active_address();
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
        let signature = signer.sign_tx(&tx).await.unwrap();

        let response = signer
            .execute_tx_without_gas_coin(tx, signature)
            .await
            .unwrap();

        assert_eq!(response.digest, submitted.digest());
    }

    #[tokio::test]
    async fn address_balance_submission_fetches_fresh_context_and_uses_no_gas_object() {
        let mut rng = rand::thread_rng();
        let chain = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_submission_context(&mut ledger_service_mock, 17, 23);
        let submitted =
            sui_mocks::grpc::mock_execute_transaction_without_gas_and_wait_for_checkpoint(
                &mut tx_service_mock,
                &mut sub_service_mock,
                &mut ledger_service_mock,
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
            chain_id: chain,
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let sender = pk.public_key().derive_address();
        let mut nexus_objects = nexus_objects;
        nexus_objects.chain_id = chain.to_string();
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_objects)
            .with_address_balance_gas_config(AddressBalanceGas::with_nonce_allocator(
                9_000,
                NonceAllocator::new(0),
            ))
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

        assert_eq!(result.digest, submitted.digest());
    }

    #[tokio::test]
    async fn attached_address_balance_failure_is_reported_at_submission() {
        let mut rng = rand::thread_rng();
        let chain = sui::types::Digest::generate(&mut rng);
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_submission_context(&mut ledger_service_mock, 17, 23);
        sub_service_mock
            .expect_subscribe_checkpoints()
            .times(1)
            .returning(|_| {
                Ok(tonic::Response::new(
                    Box::pin(futures::stream::empty()) as sui_mocks::grpc::BoxCheckpointStream
                ))
            });
        tx_service_mock
            .expect_execute_transaction()
            .times(1)
            .returning(|_| {
                Err(tonic::Status::failed_precondition(
                    "address balance is insufficient",
                ))
            });
        ledger_service_mock
            .expect_get_transaction()
            .withf(|request| {
                request
                    .get_ref()
                    .read_mask
                    .as_ref()
                    .is_some_and(|mask| mask.paths.iter().any(|path| path == "timestamp"))
            })
            .times(2)
            .returning(|_| Err(tonic::Status::not_found("transaction is not indexed")));
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            chain_id: chain,
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let sender = pk.public_key().derive_address();
        let mut nexus_objects = sui_mocks::mock_nexus_objects();
        nexus_objects.chain_id = chain.to_string();
        let client = NexusClientBuilder::new()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_objects)
            .build()
            .await
            .expect("client should build before gas attachment");
        client
            .set_gas_source(GasSource::AddressBalance(AddressBalanceGas::new(
                DEFAULT_GAS_BUDGET,
            )))
            .await
            .expect("address balance gas should attach without preflight");

        let Err(error) = client
            .submit_transaction(
                sui::types::ProgrammableTransaction {
                    inputs: vec![],
                    commands: vec![],
                },
                sender,
            )
            .await
        else {
            panic!("Sui should reject an insufficient address balance");
        };

        assert!(matches!(
            error,
            NexusError::Transaction(ref error)
                if error.state()
                    == crate::nexus::error::TransactionErrorState::SubmissionRejected
        ));
        assert!(error
            .to_string()
            .contains("address balance is insufficient"));
    }

    #[allow(dead_code)]
    async fn submit_transaction_accepts_canonical_ptb(
        client: &NexusClient,
        sender: sui::types::Address,
        ptb: sui::types::ProgrammableTransaction,
    ) {
        let _ = client.submit_transaction(ptb, sender).await;
    }

    #[tokio::test]
    async fn runtime_context_resolves_only_the_bound_scheduler_graph() {
        let context = sui_mocks::mock_nexus_context();
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut packages = sui_mocks::grpc::MockMovePackageService::new();
        sui_mocks::grpc::mock_runtime_authority(&mut ledger, &context, false);
        sui_mocks::grpc::mock_nexus_package_graph(&mut ledger, &mut packages, context.packages());
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(packages),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;

        let runtime = client.runtime_context(&[]).await.expect("runtime resolves");

        assert_eq!(
            runtime
                .require_package(PackageRole::Scheduler)
                .expect("runtime graph contains Scheduler")
                .storage_id,
            context
                .require_package(PackageRole::Scheduler)
                .expect("fixture contains Scheduler")
                .storage_id,
        );
    }

    #[tokio::test]
    async fn runtime_context_rejects_a_paused_runtime_before_package_resolution() {
        let context = sui_mocks::mock_nexus_context();
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_runtime_authority(&mut ledger, &context, true);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;

        let error = client
            .runtime_context(&[])
            .await
            .expect_err("a paused runtime has no effect authority");

        assert!(matches!(
            error,
            NexusError::InvalidObjectState { object, ref reason }
                if object == context.runtime_authority.object_id()
                    && reason.contains("paused")
        ));
    }

    #[tokio::test]
    async fn routing_context_resolves_the_bound_graph_while_runtime_is_paused() {
        let context = sui_mocks::mock_nexus_context();
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut packages = sui_mocks::grpc::MockMovePackageService::new();
        sui_mocks::grpc::mock_runtime_authority(&mut ledger, &context, true);
        sui_mocks::grpc::mock_nexus_package_graph(&mut ledger, &mut packages, context.packages());
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(packages),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;

        let routing = client
            .routing_context(&[])
            .await
            .expect("a pause removes effect authority, not package routing");

        assert_eq!(
            routing
                .package_id(PackageRole::Scheduler)
                .expect("routing graph contains Scheduler"),
            context
                .package_id(PackageRole::Scheduler)
                .expect("fixture contains Scheduler"),
        );
    }

    #[tokio::test]
    async fn runtime_context_rejects_an_unbound_authority() {
        let context = sui_mocks::mock_nexus_context();
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_unbound_runtime_authority(&mut ledger, &context);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;

        let error = client
            .runtime_context(&[])
            .await
            .expect_err("an unbound root has no effect authority");

        assert!(matches!(
            error,
            NexusError::InvalidObjectState { object, ref reason }
                if object == context.runtime_authority.object_id()
                    && reason.contains("not bound")
        ));
    }
}
