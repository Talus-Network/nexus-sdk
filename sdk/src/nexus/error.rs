//! Common error types for Nexus-related functionality.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexusError {
    #[error("Sui wallet error: {0}")]
    WalletError(anyhow::Error),
    #[error("Client configuration error: {0}")]
    ConfigurationError(String),
}
