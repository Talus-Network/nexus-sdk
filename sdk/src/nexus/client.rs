//! [`NexusClient`] can accept be built with a [`Signer`] instance and a [`Gas`]
//! instance to perform various Nexus-related operations programmatically.

use {
    crate::{
        nexus::{
            crypto::CryptoActions,
            error::NexusError,
            gas::GasActions,
            workflow::WorkflowActions,
        },
        sui::{self, traits::*},
        types::NexusObjects,
    },
    std::sync::Arc,
    sui_keys::keystore::AccountKeystore,
    tokio::sync::{Mutex, Notify},
};

/// We want to provide flexibility when it comes to signing transactions. We
/// accept both - a [`sui::WalletContext`] and a tuple of a [`sui::Client`] and
/// a secret mnemonic string.
#[derive(Clone)]
pub enum Signer {
    Wallet(Arc<Mutex<sui::WalletContext>>),
    Mnemonic(Arc<sui::Client>, Arc<Mutex<sui::Keystore>>),
}

impl Signer {
    /// Get a reference to the Sui client.
    pub(super) async fn get_client(&self) -> Result<Arc<sui::Client>, NexusError> {
        match self {
            Signer::Wallet(ctx) => {
                let wallet = ctx.lock().await;
                let client = wallet.get_client().await.map_err(NexusError::Wallet)?;

                Ok(Arc::new(client))
            }
            Signer::Mnemonic(client, _) => Ok(Arc::clone(client)),
        }
    }

    /// Get the active address from the signer.
    pub(super) async fn get_active_address(&self) -> Result<sui::Address, NexusError> {
        match self {
            Signer::Wallet(ctx) => {
                let mut wallet = ctx.lock().await;
                let address = wallet
                    .active_address()
                    .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))?;

                Ok(address)
            }
            Signer::Mnemonic(_, keystore) => {
                let keystore = keystore.lock().await;
                let addresses = keystore.addresses();
                let address = addresses.first().ok_or_else(|| {
                    NexusError::Wallet(anyhow::anyhow!("No address found in keystore"))
                })?;

                Ok(*address)
            }
        }
    }

    /// Sign a transaction block using the signer.
    pub(super) async fn sign_tx(
        &self,
        tx: sui::TransactionData,
    ) -> Result<sui::Transaction, NexusError> {
        match self {
            Signer::Wallet(ctx) => {
                let wallet = ctx.lock().await;

                Ok(wallet.sign_transaction(&tx))
            }
            Signer::Mnemonic(_, keystore) => {
                let keystore = keystore.lock().await;

                let addresses = keystore.addresses();
                let addr = addresses.first().ok_or_else(|| {
                    NexusError::Wallet(anyhow::anyhow!("No address found in keystore"))
                })?;

                let signature = keystore
                    .sign_secure(addr, &tx, sui::Intent::sui_transaction())
                    .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))?;

                Ok(sui::Transaction::from_data(tx, vec![signature]))
            }
        }
    }

    /// Execute a transaction block and return the response.
    pub(super) async fn execute_tx(
        &self,
        tx: sui::Transaction,
        gas_coin: &mut sui::ObjectRef,
    ) -> Result<sui::TransactionBlockResponse, NexusError> {
        let client = self.get_client().await?;

        let resp_options = sui::TransactionBlockResponseOptions::new()
            .with_events()
            .with_effects()
            .with_object_changes()
            .with_balance_changes();

        let resp_finality = sui::ExecuteTransactionRequestType::WaitForLocalExecution;

        let response = client
            .quorum_driver_api()
            .execute_transaction_block(tx, resp_options, Some(resp_finality))
            .await
            .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))?;

        if !response.errors.is_empty() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Transaction execution failed: {:?}",
                response.errors
            )));
        }

        let Some(sui::TransactionBlockEffects::V1(effects)) = response.effects.clone() else {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Transactions has no effects."
            )));
        };

        // Check if any effects failed in the TX.
        if effects.clone().into_status().is_err() {
            return Err(NexusError::Wallet(anyhow::anyhow!(
                "Transaction has erroneous effects: {effects:?}"
            )));
        }

        // Update the gas coin reference after execution.
        for o in effects.mutated {
            match o.reference.object_id {
                id if id == gas_coin.object_id => {
                    *gas_coin = o.reference;
                }
                _ => {}
            }
        }

        Ok(response)
    }
}

/// [`Gas`] struct handles distributing gas coins for Nexus operations.
#[derive(Clone)]
pub struct Gas {
    coins: Arc<Mutex<Vec<sui::ObjectRef>>>,
    notify: Arc<Notify>,
    budget: u64,
}

impl Gas {
    /// Acquire a gas coin from the pool.
    pub(super) async fn acquire_gas_coin(&self) -> sui::ObjectRef {
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
    pub(super) async fn release_gas_coin(&self, coin: sui::ObjectRef) {
        self.coins.lock().await.push(coin);
        self.notify.notify_one();
    }

    /// Get the gas budget.
    pub(super) fn get_budget(&self) -> u64 {
        self.budget
    }
}

/// Builder for [`NexusClient`].
pub struct NexusClientBuilder {
    signer: Option<Signer>,
    gas: Option<Gas>,
    nexus_objects: Option<NexusObjects>,
}

impl NexusClientBuilder {
    /// Create a new builder instance.
    pub fn new() -> Self {
        Self {
            signer: None,
            gas: None,
            nexus_objects: None,
        }
    }

    /// Create a new Nexus instance with a wallet context.
    pub fn with_wallet_context(mut self, wallet_context: sui::WalletContext) -> Self {
        let signer = Signer::Wallet(Arc::new(Mutex::new(wallet_context)));

        self.signer = Some(signer);
        self
    }

    /// Create a new Nexus instance with a Sui client and a keystore from a
    /// mnemonic.
    pub fn with_mnemonic(
        mut self,
        client: sui::Client,
        mnemonic: &str,
        sig_scheme: sui::SignatureScheme,
    ) -> Result<Self, NexusError> {
        let mut keystore = sui::Keystore::InMem(Default::default());

        let derivation_path = None;
        let alias = None;

        keystore
            .import_from_mnemonic(mnemonic, sig_scheme, derivation_path, alias)
            .map_err(|e| NexusError::Wallet(anyhow::anyhow!(e)))?;

        let signer = Signer::Mnemonic(Arc::new(client), Arc::new(Mutex::new(keystore)));

        self.signer = Some(signer);
        Ok(self)
    }

    /// Add gas coins and budget to the builder.
    pub fn with_gas(mut self, coins: Vec<&sui::Coin>, budget: u64) -> Result<Self, NexusError> {
        // Need at least one gas coin.
        if coins.is_empty() {
            return Err(NexusError::Configuration(
                "At least one gas coin is required".into(),
            ));
        }

        let coins = coins.into_iter().map(|c| c.object_ref().into()).collect();

        self.gas = Some(Gas {
            coins: Arc::new(Mutex::new(coins)),
            notify: Arc::new(Notify::new()),
            budget,
        });
        Ok(self)
    }

    /// Set Nexus objects to use.
    pub fn with_nexus_objects(mut self, nexus_objects: NexusObjects) -> Self {
        self.nexus_objects = Some(nexus_objects);
        self
    }

    /// Build the [`NexusClient`].
    pub async fn build(self) -> Result<NexusClient, NexusError> {
        let signer = self
            .signer
            .ok_or_else(|| NexusError::Configuration("Signer is required".into()))?;

        let gas = self
            .gas
            .ok_or_else(|| NexusError::Configuration("Gas configuration is required".into()))?;

        let nexus_objects = self
            .nexus_objects
            .ok_or_else(|| NexusError::Configuration("Nexus objects are required".into()))?;

        let client = signer.get_client().await?;

        let reference_gas_price = client
            .read_api()
            .get_reference_gas_price()
            .await
            .map_err(|e| NexusError::Rpc(e.into()))?;

        Ok(NexusClient {
            signer,
            gas,
            nexus_objects,
            reference_gas_price,
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
    pub(super) nexus_objects: NexusObjects,
    /// Save reference gas price to avoid fetching it multiple times.
    pub(super) reference_gas_price: u64,
}

impl NexusClient {
    /// Return a [`NexusClientBuilder`] instance for building a Nexus client.
    pub fn builder() -> NexusClientBuilder {
        NexusClientBuilder::new()
    }

    /// Return a [`sui::Client`] instance for interacting with the Sui network.
    pub async fn get_sui_client(&self) -> Result<Arc<sui::Client>, NexusError> {
        self.signer.get_client().await
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
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::{
            sui_mocks::{
                mock_nexus_objects,
                mock_sui_coin,
                mock_sui_mnemonic,
                mock_sui_object_ref,
            },
            wallet::create_ephemeral_wallet_context_testnet,
        },
    };

    #[tokio::test]
    async fn test_acquire_and_release_gas_coin() {
        let coin1 = mock_sui_object_ref();
        let coin2 = mock_sui_object_ref();

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
        let coin = mock_sui_object_ref();
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
    async fn test_builder_with_wallet_context() {
        let (wallet_context, _) =
            create_ephemeral_wallet_context_testnet().expect("Failed to create wallet context.");
        let coin = mock_sui_coin(100);
        let objects = mock_nexus_objects();
        let coins = vec![&coin];
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_wallet_context(wallet_context)
            .with_nexus_objects(objects)
            .with_gas(coins, budget)
            .unwrap();

        let client = builder.build().await.unwrap();
        assert_eq!(client.gas.get_budget(), budget);
    }

    #[tokio::test]
    async fn test_builder_with_mnemonic() {
        let (_, mnemonic) = mock_sui_mnemonic();
        let client = sui::ClientBuilder::default()
            .build_testnet()
            .await
            .expect("Failed to build sui client");
        let coin = mock_sui_coin(100);
        let objects = mock_nexus_objects();
        let coins = vec![&coin];
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_nexus_objects(objects)
            .with_mnemonic(client, &mnemonic, sui::SignatureScheme::ED25519)
            .unwrap()
            .with_gas(coins, budget)
            .unwrap();

        let nexus_client = builder.build().await.unwrap();
        assert_eq!(nexus_client.gas.get_budget(), budget);
    }

    #[tokio::test]
    async fn test_builder_missing_signer() {
        let coin = mock_sui_coin(100);
        let coins = vec![&coin];
        let objects = mock_nexus_objects();
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_nexus_objects(objects)
            .with_gas(coins, budget)
            .unwrap();

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_missing_gas() {
        let objects = mock_nexus_objects();
        let (wallet_context, _) =
            create_ephemeral_wallet_context_testnet().expect("Failed to create wallet context.");

        let builder = NexusClientBuilder::new()
            .with_wallet_context(wallet_context)
            .with_nexus_objects(objects);

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_with_gas_empty_coins() {
        let builder = NexusClientBuilder::new();
        let result = builder.with_gas(vec![], 1000);
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }

    #[tokio::test]
    async fn test_builder_missing_nexus_objects() {
        let (wallet_context, _) =
            create_ephemeral_wallet_context_testnet().expect("Failed to create wallet context.");
        let coin = mock_sui_coin(100);
        let coins = vec![&coin];
        let budget = 1000;

        let builder = NexusClientBuilder::new()
            .with_wallet_context(wallet_context)
            .with_gas(coins, budget)
            .unwrap();

        let result = builder.build().await;
        assert!(matches!(result, Err(NexusError::Configuration(_))));
    }
}
