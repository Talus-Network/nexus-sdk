use serde::{Deserialize, Serialize};

/// Represents information about a stored blob on the Walrus network
#[derive(Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    /// The unique identifier of the blob
    pub blob_id: String,
    /// The Sui transaction digest where the blob was registered
    pub tx_digest: String,
    /// The Sui object ID of the blob
    pub object_id: String,
    /// The expiration time in epochs
    pub expiration_time: u64,
    /// The size of the blob in bytes
    pub size: u64,
}
