//! Common error types for Nexus-related functionality.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexusError {
    #[error("Sui wallet error: {0}")]
    WalletError(anyhow::Error),
    #[error("Client configuration error: {0}")]
    ConfigurationError(String),
    #[error("Transaction building error: {0}")]
    TransactionBuildingError(anyhow::Error),
    #[error("RPC error: {0}")]
    RpcError(anyhow::Error),
}
