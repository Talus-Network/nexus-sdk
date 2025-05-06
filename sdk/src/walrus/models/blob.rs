use serde::{Deserialize, Serialize};

/// Represents a blob object in the Walrus network
#[derive(Debug, Deserialize, Serialize)]
pub struct BlobObject {
    #[serde(rename = "blobId")]
    pub blob_id: String,
    pub id: String,
    pub storage: BlobStorage,
}

/// Storage information for a blob
#[derive(Debug, Deserialize, Serialize)]
pub struct BlobStorage {
    #[serde(rename = "endEpoch")]
    pub end_epoch: u64,
}
