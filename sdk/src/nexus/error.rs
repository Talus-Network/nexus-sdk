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
    #[error("Protocol validation error: {0}")]
    ProtocolValidation(anyhow::Error),
    #[error(
        "Protocol version {protocol_version} exceeds the newest version {maximum} supported by \
         this SDK"
    )]
    UnsupportedProtocolVersion { protocol_version: u64, maximum: u64 },
    #[error("Object '{object}' requires protocol version {required}, active version is {current}")]
    MigrationRequired {
        object: crate::sui::types::Address,
        current: u64,
        required: u64,
    },
}
