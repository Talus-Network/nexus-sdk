//! Common error types for Nexus-related functionality.

use {crate::crypto::session::SessionError, thiserror::Error};

#[derive(Debug, Error)]
pub enum NexusError {
    #[error("Sui wallet error: {0}")]
    Wallet(anyhow::Error),
    #[error("Client configuration error: {0}")]
    Configuration(String),
    #[error("Transaction building error: {0}")]
    TransactionBuilding(anyhow::Error),
    #[error("RPC error: {0}")]
    Rpc(anyhow::Error),
    #[error("Parsing error: {0}")]
    Parsing(anyhow::Error),
    #[error("Crypto error: {0}")]
    Crypto(SessionError),
    #[error("Timeout error: {0}")]
    Timeout(anyhow::Error),
    #[error("Channel error: {0}")]
    Channel(anyhow::Error),
    #[error("Storage error: {0}")]
    Storage(anyhow::Error),
}
