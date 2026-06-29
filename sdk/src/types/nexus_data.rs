//! Helpers for generated `nexus_primitives::data::NexusData`.

use {
    crate::{
        types::StorageKind,
        walrus::{WalrusClient, WALRUS_MAX_EPOCHS},
    },
    serde_json::Value,
    std::collections::HashMap,
};

const NEXUS_DATA_INLINE_STORAGE_TAG: &[u8] = b"inline";
const NEXUS_DATA_WALRUS_STORAGE_TAG: &[u8] = b"walrus";

/// Nexus submit walk evaluation transaction size base size without any data.
pub const NEXUS_BASE_TRANSACTION_SIZE: usize = 8 * 1024;
/// Max transaction size supported by Sui.
pub const MAX_TRANSACTION_SIZE: usize = 128 * 1024;
/// Extra bytes we reserve for workflow metadata (DAG, proofs, etc.) when
/// estimating how much room entry data can take inside a transaction.
const ENTRY_PORTS_RESERVED_BYTES: usize = 64 * 1024;
/// The length of a Walrus blob ID.
pub const WALRUS_BLOB_ID_LENGTH: usize = 44;

impl crate::types::NexusData {
    pub fn new_inline(data: Value) -> Self {
        Self::from_json_value(StorageKind::Inline, data)
    }

    pub fn new_walrus(data: Value) -> Self {
        Self::from_json_value(StorageKind::Walrus, data)
    }

    pub fn commit_inline_plain(self) -> Self {
        if self.storage_kind() != StorageKind::Inline {
            panic!("commit_inline_plain can only be used for inline data");
        }
        self
    }

    pub fn storage_kind(&self) -> StorageKind {
        match self.storage.as_slice() {
            NEXUS_DATA_INLINE_STORAGE_TAG => StorageKind::Inline,
            NEXUS_DATA_WALRUS_STORAGE_TAG => StorageKind::Walrus,
            other => panic!(
                "unsupported NexusData storage kind '{}'",
                String::from_utf8_lossy(other)
            ),
        }
    }

    pub fn as_json(&self) -> Value {
        if self.one.is_empty() && self.many.is_empty() {
            return Value::Array(vec![]);
        }

        if self.many.is_empty() {
            return decode_nexus_data_json(&self.one);
        }

        Value::Array(
            self.many
                .iter()
                .map(|bytes| decode_nexus_data_json(bytes))
                .collect(),
        )
    }

    pub fn into_json(self) -> Value {
        self.as_json()
    }

    pub async fn fetch(mut self, conf: &StorageConf) -> anyhow::Result<Self> {
        if self.storage_kind() != StorageKind::Walrus {
            return Ok(self);
        }

        let walrus_aggregator_url = conf
            .walrus_aggregator_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus aggregator URL is not set in storage config"))?;

        let client = WalrusClient::builder()
            .with_aggregator_url(walrus_aggregator_url)
            .build();

        let blob_id = match self.as_json() {
            Value::Array(values) => {
                if values.is_empty() {
                    return Ok(Self::new_walrus(Value::Array(vec![])));
                }

                values[0]
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("Cannot fetch data from Walrus: expected string key")
                    })?
                    .to_string()
            }
            Value::String(key) => key,
            _ => {
                anyhow::bail!(
                    "Cannot fetch data from Walrus: expected string key or array of string keys"
                )
            }
        };

        let data = client.read_json::<Value>(&blob_id).await?;
        self = Self::new_walrus(data);
        Ok(self)
    }

    pub async fn commit(mut self, conf: &StorageConf) -> anyhow::Result<Self> {
        if self.storage_kind() != StorageKind::Walrus {
            return Ok(self);
        }

        let walrus_publisher_url = conf
            .walrus_publisher_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus publisher URL is not set in storage config"))?;

        let store_for_epochs = conf.walrus_save_for_epochs.ok_or_else(|| {
            anyhow::anyhow!("Walrus save for epochs is not set in storage config")
        })?;

        if store_for_epochs > WALRUS_MAX_EPOCHS {
            anyhow::bail!("Walrus save for epochs exceeds maximum allowed ({WALRUS_MAX_EPOCHS})");
        }

        let data = self.as_json();
        enum DataKind {
            One,
            Many(usize),
        }

        let data_kind = match &data {
            Value::Array(values) => {
                if values.is_empty() {
                    return Ok(Self::new_walrus(Value::Array(vec![])));
                }
                DataKind::Many(values.len())
            }
            _ => DataKind::One,
        };

        let client = WalrusClient::builder()
            .with_publisher_url(walrus_publisher_url)
            .build();
        let response = client.upload_json(&data, store_for_epochs, None).await?;
        let Some(info) = response.newly_created else {
            anyhow::bail!("Failed to store data on Walrus: no newly created blob info");
        };

        let committed = match data_kind {
            DataKind::One => Value::String(info.blob_object.blob_id),
            DataKind::Many(len) => Value::Array(
                std::iter::repeat_n(Value::String(info.blob_object.blob_id), len).collect(),
            ),
        };

        self = Self::new_walrus(committed);
        Ok(self)
    }

    fn from_json_value(storage_kind: StorageKind, data: Value) -> Self {
        let storage = match storage_kind {
            StorageKind::Inline => NEXUS_DATA_INLINE_STORAGE_TAG.to_vec(),
            StorageKind::Walrus => NEXUS_DATA_WALRUS_STORAGE_TAG.to_vec(),
        };

        let (one, many) = match data {
            Value::Array(values) => (
                vec![],
                values
                    .into_iter()
                    .map(|value| serde_json::to_vec(&value).expect("JSON value must encode"))
                    .collect(),
            ),
            value => (
                serde_json::to_vec(&value).expect("JSON value must encode"),
                vec![],
            ),
        };

        Self { storage, one, many }
    }
}

impl TryFrom<Value> for crate::types::NexusData {
    type Error = anyhow::Error;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let kind = value
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'kind' field"))?;

        let data = value
            .get("data")
            .ok_or_else(|| anyhow::anyhow!("Missing 'data' field"))?
            .clone();

        match kind {
            "inline" => Ok(Self::new_inline(data)),
            "walrus" => Ok(Self::new_walrus(data)),
            _ => anyhow::bail!("Invalid 'kind' value"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageConf {
    pub walrus_publisher_url: Option<String>,
    pub walrus_aggregator_url: Option<String>,
    pub walrus_save_for_epochs: Option<u8>,
}

/// Trait defining storage helpers for generated Nexus data.
#[allow(async_fn_in_trait)]
pub trait Storable {
    async fn fetch(self, conf: &StorageConf) -> anyhow::Result<crate::types::NexusData>;
    async fn commit(self, conf: &StorageConf) -> anyhow::Result<crate::types::NexusData>;
    fn storage_kind(&self) -> StorageKind;
    fn into_json(self) -> Value;
    fn as_json(&self) -> Value;
}

impl Storable for crate::types::NexusData {
    async fn fetch(self, conf: &StorageConf) -> anyhow::Result<crate::types::NexusData> {
        crate::types::NexusData::fetch(self, conf).await
    }

    async fn commit(self, conf: &StorageConf) -> anyhow::Result<crate::types::NexusData> {
        crate::types::NexusData::commit(self, conf).await
    }

    fn storage_kind(&self) -> StorageKind {
        crate::types::NexusData::storage_kind(self)
    }

    fn into_json(self) -> Value {
        crate::types::NexusData::into_json(self)
    }

    fn as_json(&self) -> Value {
        crate::types::NexusData::as_json(self)
    }
}

/// Take a [`serde_json::Value`] object, and create a [`HashMap<String, NexusData>`].
pub fn json_to_nexus_data_map(
    json: &Value,
    remote_fields: &[String],
    preferred_remote_storage: Option<StorageKind>,
) -> anyhow::Result<HashMap<String, crate::types::NexusData>> {
    let preferred_remote_storage = preferred_remote_storage.unwrap_or(StorageKind::Walrus);

    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut map = HashMap::new();

    for (key, value) in obj {
        let remote = remote_fields.contains(key);
        let key = key.clone();
        let value = value.clone();

        match remote {
            false => map.insert(key, crate::types::NexusData::new_inline(value)),
            true => match preferred_remote_storage {
                StorageKind::Walrus => map.insert(key, crate::types::NexusData::new_walrus(value)),
                StorageKind::Inline => {
                    anyhow::bail!("Cannot store data remotely using inline storage")
                }
            },
        };
    }

    Ok(map)
}

/// Take a [`serde_json::Value`] object, and hint the user which fields should
/// be stored remotely to avoid exceeding [`crate::types::MAX_TRANSACTION_SIZE`].
pub fn hint_remote_fields(json: &Value) -> anyhow::Result<Vec<String>> {
    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut fields: Vec<(&String, usize)> = obj
        .iter()
        .map(|(key, value)| (key, key.len() + value.to_string().len()))
        .collect();

    fields.sort_by_key(|x| std::cmp::Reverse(x.1));

    let available_size = (MAX_TRANSACTION_SIZE - NEXUS_BASE_TRANSACTION_SIZE)
        .saturating_sub(ENTRY_PORTS_RESERVED_BYTES);
    let mut required_size = fields.iter().map(|(_, size)| size).sum::<usize>();

    if required_size <= available_size {
        return Ok(vec![]);
    }

    let mut remote_fields = vec![];
    for (key, size) in fields {
        let key = key.clone();
        let value = obj.get(&key).expect("Key must exist");
        let storage_cost = match value {
            Value::Array(arr) => WALRUS_BLOB_ID_LENGTH * arr.len(),
            _ => WALRUS_BLOB_ID_LENGTH,
        };

        required_size = required_size.saturating_sub(size) + storage_cost;
        remote_fields.push(key);

        if required_size <= available_size {
            break;
        }
    }

    if required_size > available_size {
        anyhow::bail!(
            "Cannot fit data within max transaction size, even after storing all fields remotely"
        );
    }

    Ok(remote_fields)
}

fn decode_nexus_data_json(bytes: &[u8]) -> Value {
    let text = std::str::from_utf8(bytes).expect("NexusData JSON bytes must be UTF-8");
    let adjusted = wrap_large_numbers_as_string(text.trim());
    serde_json::from_str(&adjusted).expect("NexusData must contain valid JSON")
}

fn is_large_number(s: &str) -> bool {
    if let Some(stripped) = s.strip_prefix('-') {
        stripped.chars().all(|c| c.is_ascii_digit()) && s.len() > 21
    } else {
        s.chars().all(|c| c.is_ascii_digit()) && s.len() > 20
    }
}

fn wrap_large_numbers_as_string(value: &str) -> String {
    if is_large_number(value) {
        format!(r#""{value}""#)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            types::NexusData,
            walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo},
        },
        assert_matches::assert_matches,
        mockito::{Server, ServerGuard},
        serde_json::{json, Value},
    };

    async fn setup_mock_server_and_conf() -> anyhow::Result<(ServerGuard, StorageConf)> {
        let server = Server::new_async().await;
        let server_url = server.url();

        let storage_conf = StorageConf {
            walrus_publisher_url: Some(server_url.clone()),
            walrus_aggregator_url: Some(server_url),
            walrus_save_for_epochs: Some(2),
        };

        Ok((server, storage_conf))
    }

    #[test]
    fn generated_inline_nexus_data_sers_and_desers() {
        let data = json!({"key": "value"});
        let nexus_data = NexusData::new_inline(data.clone());
        let data_bytes = serde_json::to_vec(&data).unwrap();

        assert_eq!(nexus_data.as_json(), data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Inline);
        assert_eq!(
            serde_json::to_value(&nexus_data).unwrap(),
            json!({
                "storage": NEXUS_DATA_INLINE_STORAGE_TAG,
                "one": data_bytes,
                "many": []
            })
        );

        let deserialized: NexusData =
            serde_json::from_str(&serde_json::to_string(&nexus_data).unwrap()).unwrap();
        assert_eq!(deserialized, nexus_data);
        assert_eq!(deserialized.as_json(), data);

        let array_data = json!([{ "key": "value" }, { "key": "value" }]);
        let nexus_data = NexusData::new_inline(array_data.clone());
        let array_item_bytes = serde_json::to_vec(&json!({ "key": "value" })).unwrap();

        assert_eq!(nexus_data.as_json(), array_data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Inline);
        assert_eq!(
            serde_json::to_value(&nexus_data).unwrap(),
            json!({
                "storage": NEXUS_DATA_INLINE_STORAGE_TAG,
                "one": [],
                "many": [array_item_bytes.clone(), array_item_bytes]
            })
        );

        let deserialized: NexusData =
            serde_json::from_str(&serde_json::to_string(&nexus_data).unwrap()).unwrap();
        assert_eq!(deserialized, nexus_data);
        assert_eq!(deserialized.as_json(), array_data);
    }

    #[test]
    fn generated_nexus_data_deserializes_encoded_byte_fields() {
        let deserialized: NexusData = serde_json::from_value(json!({
            "storage": "aW5saW5l",
            "one": "eyJyZXN0YXJ0LWFuZC13aXBlIjp0cnVlfQ==",
            "many": []
        }))
        .unwrap();

        assert_eq!(deserialized.storage_kind(), StorageKind::Inline);
        assert_eq!(deserialized.as_json(), json!({ "restart-and-wipe": true }));
    }

    #[test]
    fn generated_walrus_nexus_data_sers_and_desers() {
        let data = json!({"key": "value"});
        let nexus_data = NexusData::new_walrus(data.clone());
        let data_bytes = serde_json::to_vec(&data).unwrap();

        assert_eq!(nexus_data.as_json(), data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Walrus);
        assert_eq!(
            serde_json::to_value(&nexus_data).unwrap(),
            json!({
                "storage": NEXUS_DATA_WALRUS_STORAGE_TAG,
                "one": data_bytes,
                "many": []
            })
        );

        let deserialized: NexusData =
            serde_json::from_str(&serde_json::to_string(&nexus_data).unwrap()).unwrap();
        assert_eq!(deserialized, nexus_data);
        assert_eq!(deserialized.as_json(), data);

        let array_data = json!([{ "key": "value" }, { "key": "value" }]);
        let nexus_data = NexusData::new_walrus(array_data.clone());
        let array_item_bytes = serde_json::to_vec(&json!({ "key": "value" })).unwrap();

        assert_eq!(nexus_data.as_json(), array_data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Walrus);
        assert_eq!(
            serde_json::to_value(&nexus_data).unwrap(),
            json!({
                "storage": NEXUS_DATA_WALRUS_STORAGE_TAG,
                "one": [],
                "many": [array_item_bytes.clone(), array_item_bytes]
            })
        );

        let deserialized: NexusData =
            serde_json::from_str(&serde_json::to_string(&nexus_data).unwrap()).unwrap();
        assert_eq!(deserialized, nexus_data);
        assert_eq!(deserialized.as_json(), array_data);
    }

    #[test]
    fn large_number_precision_preserved() {
        for large_u256 in [
            "105792089237316195563853351929625371316844592863025172891227567439681422591090",
            "105792089237316195423570985008687907853410267032561502502939405359422902436090",
        ] {
            let raw_large_number = NexusData {
                storage: NEXUS_DATA_INLINE_STORAGE_TAG.to_vec(),
                one: large_u256.as_bytes().to_vec(),
                many: vec![],
            };

            assert_eq!(
                raw_large_number.as_json(),
                Value::String(large_u256.to_string())
            );

            let nexus_data = NexusData::new_inline(Value::String(large_u256.to_string()));
            let serialized = serde_json::to_string(&nexus_data).unwrap();
            let deserialized: NexusData = serde_json::from_str(&serialized).unwrap();

            assert_eq!(
                deserialized.as_json(),
                Value::String(large_u256.to_string())
            );
        }
    }

    #[tokio::test]
    async fn test_inline_plain_roundtrip() {
        let storage_conf = StorageConf::default();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_inline(data.clone());
        assert_eq!(nexus_data.as_json(), data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Inline);

        let committed = nexus_data
            .commit(&storage_conf)
            .await
            .expect("Failed to commit data");

        assert_eq!(committed.as_json(), data);
        assert_eq!(committed.storage_kind(), StorageKind::Inline);

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_walrus_plain_roundtrip() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Mock server should start");
        let data = json!({"key": "value"});

        let mock_put_response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: "json_blob_id".to_string(),
                    id: "json_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };

        let mock_put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_put_response).expect("Must serialize"))
            .create_async()
            .await;

        let mock_get = server
            .mock("GET", "/v1/blobs/json_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&data).expect("Must serialize"))
            .create_async()
            .await;

        let nexus_data = NexusData::new_walrus(data.clone());
        assert_eq!(nexus_data.as_json(), data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Walrus);

        let committed = nexus_data
            .commit(&storage_conf)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), data);
        assert!(committed.as_json().is_string());
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[tokio::test]
    async fn test_walrus_empty_arr_plain_roundtrip() {
        let storage_conf = StorageConf {
            walrus_publisher_url: Some("https://publisher.url".to_string()),
            walrus_aggregator_url: Some("https://aggregator.url".to_string()),
            walrus_save_for_epochs: Some(2),
        };
        let data = json!([]);

        let nexus_data = NexusData::new_walrus(data.clone());
        assert_eq!(nexus_data.as_json(), data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Walrus);

        let committed = nexus_data
            .commit(&storage_conf)
            .await
            .expect("Failed to commit data");

        assert_eq!(committed.as_json(), data);
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
    }

    #[tokio::test]
    async fn test_walrus_array_plain_roundtrip() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Mock server should start");
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let mock_put_response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: "json_blob_id".to_string(),
                    id: "json_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };

        let mock_put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_put_response).expect("Must serialize"))
            .create_async()
            .await;

        let mock_get = server
            .mock("GET", "/v1/blobs/json_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&data).expect("Must serialize"))
            .create_async()
            .await;

        let nexus_data = NexusData::new_walrus(data.clone());
        assert_eq!(nexus_data.as_json(), data);
        assert_eq!(nexus_data.storage_kind(), StorageKind::Walrus);

        let committed = nexus_data
            .commit(&storage_conf)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), data);
        assert!(committed.as_json().is_array());
        assert_eq!(
            committed.as_json().as_array().unwrap().len(),
            data.as_array().unwrap().len()
        );
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[test]
    fn test_json_to_nexus_data_map_all_branches() {
        let json = json!({
            "plain_inline": 1,
            "plain_walrus": 3,
        });

        let remote_fields = vec!["plain_walrus".to_string()];
        let map = json_to_nexus_data_map(&json, &remote_fields, None).unwrap();

        assert_eq!(
            map.get("plain_inline").unwrap(),
            &NexusData::new_inline(json.get("plain_inline").unwrap().clone())
        );
        assert_eq!(
            map.get("plain_walrus").unwrap(),
            &NexusData::new_walrus(json.get("plain_walrus").unwrap().clone())
        );
    }

    #[test]
    fn test_json_to_nexus_data_map_empty_object() {
        let json = json!({});
        let map = json_to_nexus_data_map(&json, &[], None).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_json_to_nexus_data_map_non_object_error() {
        let json = json!(["not", "an", "object"]);
        let result = json_to_nexus_data_map(&json, &[], None);

        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Expected JSON object")
        );
    }

    #[test]
    fn test_json_to_nexus_data_map_remote_inline_error() {
        let json = json!({
            "remote_inline": 42
        });
        let remote_fields = vec!["remote_inline".to_string()];
        let result = json_to_nexus_data_map(&json, &remote_fields, Some(StorageKind::Inline));

        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Cannot store data remotely using inline storage")
        );
    }

    #[test]
    fn test_try_from_json_value_all_branches() {
        let inline = NexusData::try_from(json!({
            "kind": "inline",
            "data": { "key": "value" }
        }))
        .unwrap();
        assert_eq!(inline.storage_kind(), StorageKind::Inline);
        assert_eq!(inline.as_json(), json!({"key": "value"}));

        let walrus = NexusData::try_from(json!({
            "kind": "walrus",
            "data": "blob_id"
        }))
        .unwrap();
        assert_eq!(walrus.storage_kind(), StorageKind::Walrus);
        assert_eq!(walrus.as_json(), json!("blob_id"));

        assert!(NexusData::try_from(json!({"data": 1})).is_err());
        assert!(NexusData::try_from(json!({"kind": "inline"})).is_err());
        assert!(NexusData::try_from(json!({"kind": "unknown", "data": 1})).is_err());
    }

    #[test]
    fn test_hint_remote_fields_all_fit() {
        let json = json!({
            "a": "short",
            "b": 123,
            "c": [1, 2, 3]
        });
        let result = hint_remote_fields(&json).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_hint_remote_fields_one_field_overflows() {
        let json = json!({
            "small": "ok",
            "large": "x".repeat(MAX_TRANSACTION_SIZE)
        });
        let result = hint_remote_fields(&json).unwrap();
        assert_eq!(result, vec!["large"]);
    }

    #[test]
    fn test_hint_remote_fields_multiple_fields_overflow() {
        let mut json_obj = serde_json::Map::new();
        let mut expected_remote = Vec::new();
        let field_count = 5;
        let field_size = MAX_TRANSACTION_SIZE / field_count;

        for i in 0..field_count {
            let key = format!("field{i}");
            json_obj.insert(key.clone(), Value::String("x".repeat(field_size)));
            expected_remote.push(key);
        }

        let json = Value::Object(json_obj);
        let result = hint_remote_fields(&json).unwrap();
        assert!(!result.is_empty());
        assert!(expected_remote.iter().any(|key| result.contains(key)));
    }

    #[test]
    fn test_hint_remote_fields_array_storage_cost() {
        let effective_budget = MAX_TRANSACTION_SIZE
            .saturating_sub(NEXUS_BASE_TRANSACTION_SIZE)
            .saturating_sub(ENTRY_PORTS_RESERVED_BYTES);
        let arr_len = (effective_budget / WALRUS_BLOB_ID_LENGTH).saturating_sub(10);
        let arr: Vec<Value> = (0..arr_len)
            .map(|_| json!({"x": "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"}))
            .collect();
        let json = json!({
            "big_array": arr,
            "small": "ok"
        });

        let result = hint_remote_fields(&json).unwrap();
        assert!(result.contains(&"big_array".to_string()));
    }

    #[test]
    fn test_hint_remote_fields_cannot_fit_even_if_all_remote() {
        let huge_arr_len = (MAX_TRANSACTION_SIZE * 2) / WALRUS_BLOB_ID_LENGTH;
        let huge_arr: Vec<Value> = (0..huge_arr_len)
            .map(|_| json!("yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"))
            .collect();
        let json = json!({
            "huge_array": huge_arr
        });
        let result = hint_remote_fields(&json);

        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Cannot fit data within max transaction size")
        );
    }

    #[test]
    fn test_hint_remote_fields_non_object_error() {
        let json = json!(["not", "an", "object"]);
        let result = hint_remote_fields(&json);

        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Expected JSON object")
        );
    }
}
