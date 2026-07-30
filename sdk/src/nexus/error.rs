//! Common error types for Nexus-related functionality.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NexusError {
    #[error("a private key is required for this operation")]
    MissingPrivateKey,
    #[error("Sui wallet error: {0}")]
    Wallet(anyhow::Error),
    #[error("Client configuration error: {0}")]
    Configuration(String),
    #[error("a gas source is already configured")]
    GasSourceAlreadyConfigured,
    #[error("Transaction building error: {0}")]
    TransactionBuilding(anyhow::Error),
    #[error("RPC error: {0}")]
    Rpc(anyhow::Error),
    #[error("Parsing error: {0}")]
    Parsing(anyhow::Error),
    #[error("Timeout error: {0}")]
    Timeout(anyhow::Error),
    #[error("Channel error: {0}")]
    Channel(anyhow::Error),
    #[error("Storage error: {0}")]
    Storage(anyhow::Error),
    #[error("Release validation error: {0}")]
    ReleaseValidation(anyhow::Error),
    #[error("Release {release} requires SDK API {required}, but this SDK supports {supported}")]
    UnsupportedSdkApi {
        release: u64,
        required: u64,
        supported: u64,
    },
    #[error("Object '{object}' requires migration from release {current} to release {required}")]
    MigrationRequired {
        object: crate::sui::types::Address,
        current: u64,
        required: u64,
    },
}
