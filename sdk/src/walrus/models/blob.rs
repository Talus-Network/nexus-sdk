use serde::{Deserialize, Serialize};

/// Represents a blob metadata in the Walrus network
#[derive(Debug, Serialize, Deserialize)]
pub struct BlobMetadata {
    /// The unique identifier of the blob
    pub blob_id: String,
    /// The size of the blob in bytes
    pub size: u64,
    /// Whether the blob exists in the network
    pub exists: bool,
}
