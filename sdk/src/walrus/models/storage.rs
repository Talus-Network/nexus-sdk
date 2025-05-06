use {
    super::{blob::BlobObject, sui::SuiEvent},
    serde::{Deserialize, Serialize},
};

/// Represents a newly created blob in the Walrus network
#[derive(Debug, Deserialize, Serialize)]
pub struct NewlyCreated {
    #[serde(rename = "blobObject")]
    pub blob_object: BlobObject,
}

/// Represents an already certified blob in the Walrus network
#[derive(Debug, Deserialize, Serialize)]
pub struct AlreadyCertified {
    #[serde(rename = "blobId")]
    pub blob_id: String,
    #[serde(rename = "endEpoch")]
    pub end_epoch: u64,
    pub event: SuiEvent,
}

/// Information about a blob's storage status
#[derive(Debug, Deserialize, Serialize)]
pub struct StorageInfo {
    #[serde(rename = "newlyCreated")]
    pub newly_created: Option<NewlyCreated>,
    #[serde(rename = "alreadyCertified")]
    pub already_certified: Option<AlreadyCertified>,
}
