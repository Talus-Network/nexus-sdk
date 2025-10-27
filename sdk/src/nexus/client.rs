//! [`NexusClient`] can accept be built with a [`Signer`] instance and a [`Gas`]
//! instance to perform various Nexus-related operations programmatically.

use {
    crate::{
        nexus::error::NexusError,
        sui::{self, traits::*},
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
    pub async fn get_client(&self) -> Result<Arc<sui::Client>, NexusError> {
        match self {
            Signer::Wallet(ctx) => {
                let wallet = ctx.lock().await;
                let client = wallet.get_client().await.map_err(NexusError::WalletError)?;

                Ok(Arc::new(client))
            }
            Signer::Mnemonic(client, _) => Ok(Arc::clone(client)),
        }
    }

    /// Sign a transaction block using the signer.
    pub async fn sign_tx(&self, tx: sui::TransactionData) -> Result<sui::Transaction, NexusError> {
        match self {
            Signer::Wallet(ctx) => {
                let wallet = ctx.lock().await;

                Ok(wallet.sign_transaction(&tx))
            }
            Signer::Mnemonic(_, keystore) => {
                let keystore = keystore.lock().await;

                let addresses = keystore.addresses();
                let addr = addresses.first().ok_or_else(|| {
                    NexusError::WalletError(anyhow::anyhow!("No address found in keystore"))
                })?;

                let signature = keystore
                    .sign_secure(addr, &tx, sui::Intent::sui_transaction())
                    .map_err(|e| NexusError::WalletError(anyhow::anyhow!(e)))?;

                Ok(sui::Transaction::from_data(tx, vec![signature]))
            }
        }
    }

    /// Execute a transaction block and return the response.
    pub async fn execute_tx(
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
            .map_err(|e| NexusError::WalletError(anyhow::anyhow!(e)))?;

        if !response.errors.is_empty() {
            return Err(NexusError::WalletError(anyhow::anyhow!(
                "Transaction execution failed: {:?}",
                response.errors
            )));
        }

        let Some(sui::TransactionBlockEffects::V1(effects)) = response.effects.clone() else {
            return Err(NexusError::WalletError(anyhow::anyhow!(
                "Transactions has no effects."
            )));
        };

        // Check if any effects failed in the TX.
        if effects.clone().into_status().is_err() {
            return Err(NexusError::WalletError(anyhow::anyhow!(
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
    pub async fn acquire_gas_coin(&mut self) -> sui::ObjectRef {
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
    pub async fn release_gas_coin(&mut self, coin: sui::ObjectRef) {
        self.coins.lock().await.push(coin);
        self.notify.notify_one();
    }

    /// Get the gas budget.
    pub fn get_budget(&self) -> u64 {
        self.budget
    }
}

/// Builder for [`NexusClient`].
pub struct NexusClientBuilder {
    signer: Option<Signer>,
    gas: Option<Gas>,
}

impl NexusClientBuilder {
    /// Create a new builder instance.
    pub fn new() -> Self {
        Self {
            signer: None,
            gas: None,
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
            .map_err(|e| NexusError::WalletError(anyhow::anyhow!(e)))?;

        let signer = Signer::Mnemonic(Arc::new(client), Arc::new(Mutex::new(keystore)));

        self.signer = Some(signer);
        Ok(self)
    }

    /// Add gas coins and budget to the builder.
    pub fn with_gas(mut self, coins: Vec<&sui::Coin>, budget: u64) -> Result<Self, NexusError> {
        // Need at least one gas coin.
        if coins.is_empty() {
            return Err(NexusError::ConfigurationError(
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

    /// Build the [`NexusClient`].
    pub fn build(self) -> Result<NexusClient, NexusError> {
        let signer = self
            .signer
            .ok_or_else(|| NexusError::ConfigurationError("Signer is required".into()))?;

        let gas = self.gas.ok_or_else(|| {
            NexusError::ConfigurationError("Gas configuration is required".into())
        })?;

        Ok(NexusClient { signer, gas })
    }
}

#[derive(Clone)]
pub struct NexusClient {
    /// The wallet context to use for transactions. This defines the TX sender
    /// address and the RPC connection.
    signer: Signer,
    /// Gas configuration for Nexus operations.
    gas: Gas,
}

impl NexusClient {
    /// Add a [`sui::Coin`] as Nexus budget.
    pub async fn add_budget(&self, coin: &sui::ObjectRef) -> Result<(), NexusError> {
        todo!();
    }
}
#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks::mock_sui_object_ref};

    #[tokio::test]
    async fn test_acquire_and_release_gas_coin() {
        let coin1 = mock_sui_object_ref();
        let coin2 = mock_sui_object_ref();

        let gas = Gas {
            coins: Arc::new(Mutex::new(vec![coin1.clone(), coin2.clone()])),
            notify: Arc::new(Notify::new()),
            budget: 1000,
        };

        let mut gas = gas;

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
        let mut gas = Gas {
            coins: Arc::new(Mutex::new(vec![])),
            notify: Arc::new(Notify::new()),
            budget: 100,
        };

        let mut gas_clone = gas.clone();

        let handle = tokio::spawn(async move { gas_clone.acquire_gas_coin().await });

        // Wait a moment to ensure acquire_gas_coin is waiting
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Release coin
        gas.release_gas_coin(coin.clone()).await;

        let acquired = handle.await.unwrap();
        assert_eq!(acquired, coin);
    }
}
