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
#[serde(try_from = "StorageInfoWire")]
pub struct StorageInfo {
    #[serde(rename = "newlyCreated")]
    pub newly_created: Option<NewlyCreated>,
    #[serde(rename = "alreadyCertified")]
    pub already_certified: Option<AlreadyCertified>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageInfoWire {
    newly_created: Option<NewlyCreated>,
    already_certified: Option<AlreadyCertified>,
}

impl TryFrom<StorageInfoWire> for StorageInfo {
    type Error = &'static str;

    fn try_from(value: StorageInfoWire) -> Result<Self, Self::Error> {
        if value.newly_created.is_some() == value.already_certified.is_some() {
            return Err("publisher response must contain exactly one storage result");
        }
        Ok(Self {
            newly_created: value.newly_created,
            already_certified: value.already_certified,
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn publisher_response_requires_exactly_one_result() {
        let created =
            json!({"blobObject": {"blobId": "blob", "id": "object", "storage": {"endEpoch": 1}}});
        let certified = json!({"blobId": "blob", "endEpoch": 1, "event": {"txDigest": "digest"}});
        for value in [
            json!({}),
            json!({"newlyCreated": created, "alreadyCertified": certified}),
        ] {
            assert!(serde_json::from_value::<StorageInfo>(value).is_err());
        }
        for value in [
            json!({"newlyCreated": created}),
            json!({"alreadyCertified": certified}),
        ] {
            assert!(serde_json::from_value::<StorageInfo>(value).is_ok());
        }
    }
}
