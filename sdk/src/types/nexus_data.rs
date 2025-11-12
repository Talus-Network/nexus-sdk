//! [`NexusData`] is a wrapper around any raw data stored on-chain. This can be
//! data for input ports, output ports or default values. It is represented as
//! an enum because default values can be stored remotely.
//!
//! The [`DataStorage`] enum has multiple implementations to support inline
//! or remote storage.
//!
//! Note: As an optimization to reduce storage costs, array types are stored as
//! one blob containing a JSON array, instead of multiple blobs. This means that
//! the array of keys referencing the data will contain only one key repeated N
//! times where N is the length of the array.

use {
    crate::{
        crypto::session::Session,
        types::StorageKind,
        walrus::{WalrusClient, WALRUS_MAX_EPOCHS},
    },
    enum_dispatch::enum_dispatch,
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Mutex,
};

/// Nexus submit walk evaluation transaction size base size without any data.
pub const NEXUS_BASE_TRANSACTION_SIZE: usize = 8 * 1024;
/// Max transaction size supported by Sui.
pub const MAX_TRANSACTION_SIZE: usize = 128 * 1024;
/// The length of a Walrus blob ID.
pub const WALRUS_BLOB_ID_LENGTH: usize = 44;
/// How much extra space per encrypted item is needed?
pub const ENCRYPTION_BASE_SIZE: usize = 440;
/// What is the inflation factor for encrypted data size?
pub const ENCRYPTION_INFLATION_FACTOR: usize = 4;

// == NexusData ==

/// Note that the sole reason the top-level [`NexusData`] struct exists is to
/// ensure that the inner `data` can only be accessed via [`DataStorage`]'s
/// `fetch` and `commit` methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NexusData {
    pub(super) data: DataStorage,
}

#[cfg(test)]
impl From<DataStorage> for NexusData {
    fn from(data: DataStorage) -> Self {
        Self { data }
    }
}

impl NexusData {
    /// Fetches the data from its storage location. This consumes `self`.
    pub async fn fetch(
        mut self,
        conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<DataStorage> {
        self.data.fetch(conf, session).await?;

        Ok(self.data)
    }

    /// Commits the data to its storage location. This consumes `self`.
    pub async fn commit(
        mut self,
        conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<DataStorage> {
        self.data.commit(conf, session).await?;

        Ok(self.data)
    }

    /// Convenience function that synchronously and infallibly creates a new
    /// `NexusData` instance with inline, unencrypted data.
    ///
    /// This [`panic!`]s if used for any remote or encrypted storage.
    ///
    /// Example:
    ///
    /// ```
    /// use nexus_sdk::types::{NexusData, StorageKind, Storable};
    ///
    /// let data = NexusData::new_inline(serde_json::json!({"key": "value"})).commit_inline_plain();
    ///
    /// assert_eq!(data.as_json(), &serde_json::json!({"key": "value"}));
    /// assert!(!data.is_encrypted());
    /// assert_eq!(data.storage_kind(), StorageKind::Inline);
    /// ```
    pub fn commit_inline_plain(self) -> DataStorage {
        if self.data.storage_kind() != StorageKind::Inline || self.data.is_encrypted() {
            panic!("commit_inline_plain can only be used for inline, unencrypted data");
        }

        self.data
    }

    /// Create inline data that is not encrypted.
    pub fn new_inline(data: serde_json::Value) -> Self {
        Self {
            data: DataStorage::Inline(InlineStorage {
                data,
                encrypted: false,
            }),
        }
    }

    /// Create inline data that is encrypted.
    pub fn new_inline_encrypted(data: serde_json::Value) -> Self {
        Self {
            data: DataStorage::Inline(InlineStorage {
                data,
                encrypted: true,
            }),
        }
    }

    /// Create walrus data that is not encrypted.
    pub fn new_walrus(data: serde_json::Value) -> Self {
        Self {
            data: DataStorage::Walrus(WalrusStorage {
                data,
                encrypted: false,
            }),
        }
    }

    /// Create walrus data that is encrypted.
    pub fn new_walrus_encrypted(data: serde_json::Value) -> Self {
        Self {
            data: DataStorage::Walrus(WalrusStorage {
                data,
                encrypted: true,
            }),
        }
    }
}

// == DataStorage ==

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageConf {
    pub walrus_publisher_url: Option<String>,
    pub walrus_aggregator_url: Option<String>,
    pub walrus_save_for_epochs: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[enum_dispatch(Storable)]
pub enum DataStorage {
    Inline(InlineStorage),
    Walrus(WalrusStorage),
}

#[cfg(test)]
impl TryFrom<serde_json::Value> for DataStorage {
    type Error = anyhow::Error;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        let kind = value
            .get("kind")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'kind' field"))?;

        let data = value
            .get("data")
            .ok_or_else(|| anyhow::anyhow!("Missing 'data' field"))?;

        let encrypted = value
            .get("encrypted")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid 'encrypted' field"))?;

        match kind {
            "inline" => Ok(Self::Inline(InlineStorage {
                data: data.clone(),
                encrypted,
            })),
            "walrus" => Ok(Self::Walrus(WalrusStorage {
                data: data.clone(),
                encrypted,
            })),
            _ => Err(anyhow::anyhow!("Invalid 'kind' value")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct InlineStorage {
    pub(super) data: serde_json::Value,
    /// Whether the data is encrypted and should be decrypted before use or
    /// encrypted before committing.
    pub(super) encrypted: bool,
}

impl Storable for InlineStorage {
    async fn fetch(&mut self, _: &StorageConf, session: Arc<Mutex<Session>>) -> anyhow::Result<()> {
        if self.encrypted {
            self.data = session.lock().await.decrypt_nexus_data_json(&self.data)?;
        }

        Ok(())
    }

    async fn commit(
        &mut self,
        _: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<()> {
        if self.encrypted {
            session
                .lock()
                .await
                .encrypt_nexus_data_json(&mut self.data)?;
        }

        Ok(())
    }

    fn storage_kind(&self) -> StorageKind {
        StorageKind::Inline
    }

    fn into_json(self) -> serde_json::Value {
        self.data
    }

    fn as_json(&self) -> &serde_json::Value {
        &self.data
    }

    fn is_encrypted(&self) -> bool {
        self.encrypted
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct WalrusStorage {
    pub(super) data: serde_json::Value,
    /// Whether the data is encrypted and should be decrypted before use or
    /// encrypted before committing.
    pub(super) encrypted: bool,
}

impl Storable for WalrusStorage {
    async fn fetch(
        &mut self,
        conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<()> {
        let walrus_aggregator_url = conf
            .walrus_aggregator_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus aggregator URL is not set in storage config"))?;

        let client = WalrusClient::builder()
            .with_aggregator_url(walrus_aggregator_url)
            .build();

        // Fetch the data from Walrus using the client. The convention is that
        // The key to read is either the data itself or the first element of
        // the array if we're dealing with an array of values.
        let blob_id = match &self.data {
            serde_json::Value::Array(values) => {
                if values.is_empty() {
                    self.data = serde_json::Value::Array(vec![]);

                    return Ok(());
                }

                values[0]
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("Cannot fetch data from Walrus: expected string key")
                    })?
                    .to_string()
            }
            serde_json::Value::String(key) => key.clone(),
            _ => {
                return Err(anyhow::anyhow!(
                    "Cannot fetch data from Walrus: expected string key or array of string keys"
                ))
            }
        };

        let mut data = client.read_json::<serde_json::Value>(&blob_id).await?;

        // Decrypt the data if needed.
        if self.encrypted {
            data = session.lock().await.decrypt_nexus_data_json(&data)?;
        }

        self.data = data;

        Ok(())
    }

    async fn commit(
        &mut self,
        conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<()> {
        let walrus_publisher_url = conf
            .walrus_publisher_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus publisher URL is not set in storage config"))?;

        let store_for_epochs = conf.walrus_save_for_epochs.ok_or_else(|| {
            anyhow::anyhow!("Walrus save for epochs is not set in storage config")
        })?;

        if store_for_epochs > WALRUS_MAX_EPOCHS {
            return Err(anyhow::anyhow!(
                "Walrus save for epochs exceeds maximum allowed ({})",
                WALRUS_MAX_EPOCHS
            ));
        }

        let client = WalrusClient::builder()
            .with_publisher_url(walrus_publisher_url)
            .build();

        // Encrypt the data if needed.
        if self.encrypted {
            session
                .lock()
                .await
                .encrypt_nexus_data_json(&mut self.data)?;
        }

        // Store data on Walrus using the client. If it's an array, we store
        // the entire array as a single blob and repeat the key N times where
        // N is the length of the array. This is a storage optimization. while
        // keeping the information about the length of the array.

        // Figure out if we're dealing with a single value or an array of values.
        enum DataKind {
            One,
            Many(usize),
        }

        let data_kind = match &self.data {
            serde_json::Value::Array(values) => {
                if values.is_empty() {
                    // No need to store empty arrays.
                    self.data = serde_json::Value::Array(vec![]);

                    return Ok(());
                }

                DataKind::Many(values.len())
            }
            _ => DataKind::One,
        };

        // Make the request.
        let response = client
            .upload_json(&self.data, store_for_epochs, None)
            .await?;

        let Some(info) = response.newly_created else {
            // This should never happen, we just uploaded.
            return Err(anyhow::anyhow!(
                "Failed to store data on Walrus: no newly created blob info"
            ));
        };

        // Now we have to return the key(s) referencing the data.
        let data = match data_kind {
            DataKind::One => serde_json::Value::String(info.blob_object.blob_id),
            DataKind::Many(len) => serde_json::Value::Array(
                std::iter::repeat_n(
                    serde_json::Value::String(info.blob_object.blob_id.clone()),
                    len,
                )
                .collect(),
            ),
        };

        self.data = data;

        Ok(())
    }

    fn storage_kind(&self) -> StorageKind {
        StorageKind::Walrus
    }

    fn into_json(self) -> serde_json::Value {
        self.data
    }

    fn as_json(&self) -> &serde_json::Value {
        &self.data
    }

    fn is_encrypted(&self) -> bool {
        self.encrypted
    }
}

/// Trait defining two methods for accessing and saving data based on its storage
/// type.
#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait Storable {
    /// Fetch the data from its storage location.
    async fn fetch(
        &mut self,
        conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<()>;

    /// Commit the data to its storage location.
    async fn commit(
        &mut self,
        conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<()>;

    /// Get the kind of storage used.
    fn storage_kind(&self) -> StorageKind;

    /// Extract the inner JSON value.
    fn into_json(self) -> serde_json::Value;

    /// Get a reference to the inner JSON value.
    fn as_json(&self) -> &serde_json::Value;

    /// Whether the data is encrypted and should be encrypted before committing
    /// and decrypted after fetching.
    fn is_encrypted(&self) -> bool;
}

// == Helpers ==

/// Take a [`serde_json::Value`] object, and create a [`HashMap<String, NexusData>`].
///
/// This helper also takes:
/// - [`Vec<String>`] that indicates which fields should be encrypted
/// - [`Vec<String>`] that indicates which fields should be stored remotely
/// - [`Option<StorageKind>`] that indicates the preferred remote storage kind
pub fn json_to_nexus_data_map(
    json: &serde_json::Value,
    encrypt_fields: &[String],
    remote_fields: &[String],
    preferred_remote_storage: Option<StorageKind>,
) -> anyhow::Result<HashMap<String, NexusData>> {
    let preferred_remote_storage = preferred_remote_storage.unwrap_or(StorageKind::Walrus);

    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    let mut map = HashMap::new();

    for (key, value) in obj {
        let encrypt = encrypt_fields.contains(key);
        let remote = remote_fields.contains(key);

        let key = key.clone();
        let value = value.clone();

        match (encrypt, remote) {
            (false, false) => map.insert(key, NexusData::new_inline(value)),
            (true, false) => map.insert(key, NexusData::new_inline_encrypted(value)),
            (false, true) => match preferred_remote_storage {
                StorageKind::Walrus => map.insert(key, NexusData::new_walrus(value)),
                StorageKind::Inline => {
                    anyhow::bail!("Cannot store data remotely using inline storage")
                }
            },
            (true, true) => match preferred_remote_storage {
                StorageKind::Walrus => map.insert(key, NexusData::new_walrus_encrypted(value)),
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
pub fn hint_remote_fields(json: &serde_json::Value) -> anyhow::Result<Vec<String>> {
    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    // Calculate the size of each field.
    let mut fields: Vec<(&String, usize)> = obj
        .iter()
        .map(|(key, value)| {
            let key_size = key.len();
            let data_size = value.to_string().len();

            // Assume each field is encrypted for size calculation.
            let encrypted_data_size = match value {
                serde_json::Value::Array(arr) => {
                    ENCRYPTION_BASE_SIZE * arr.len() + (data_size * ENCRYPTION_INFLATION_FACTOR)
                }
                _ => ENCRYPTION_BASE_SIZE + (data_size * ENCRYPTION_INFLATION_FACTOR),
            };

            (key, key_size + encrypted_data_size)
        })
        .collect();

    // Sort them largest to smallest.
    fields.sort_by(|a, b| b.1.cmp(&a.1));

    let available_size = MAX_TRANSACTION_SIZE - NEXUS_BASE_TRANSACTION_SIZE;
    let mut required_size = fields.iter().map(|(_, size)| size).sum::<usize>();

    if required_size <= available_size {
        // All good, nothing to do.
        return Ok(vec![]);
    }

    let mut remote_fields = vec![];

    for (key, size) in fields {
        let key = key.clone();
        let value = obj.get(&key).expect("Key must exist");

        // Storing plain values costs [`WALRUS_BLOB_ID_LENGTH`] bytes. Storing
        // arrays costs [`WALRUS_BLOB_ID_LENGTH`] * N bytes where N is the
        // length of the array.
        let storage_cost = match value {
            serde_json::Value::Array(arr) => WALRUS_BLOB_ID_LENGTH * arr.len(),
            _ => WALRUS_BLOB_ID_LENGTH,
        };

        // Subtract the size of the field from the required size and add the
        // storage cost.
        required_size = required_size.saturating_sub(size) + storage_cost;

        // Add the field to the remote fields list.
        remote_fields.push(key);

        // Check whether we need to continue.
        if required_size <= available_size {
            break;
        }
    }

    // Check that we were successful.
    if required_size > available_size {
        anyhow::bail!(
            "Cannot fit data within max transaction size, even after storing all fields remotely"
        );
    }

    Ok(remote_fields)
}

/// Tests encryption, walrus integration and the helpers.
#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            crypto::{
                session::Message,
                x3dh::{IdentityKey, PreKeyBundle},
            },
            walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo},
        },
        assert_matches::assert_matches,
        mockito::{Server, ServerGuard},
        serde_json::json,
    };

    /// Helper to create sender and receiver sessions for testing
    /// Returns (nexus_session, user_session) where:
    /// - nexus_session: represents the Nexus system that encrypts output data
    /// - user_session: represents the user inspecting execution and decrypting data
    fn create_test_sessions() -> (Arc<Mutex<Session>>, Arc<Mutex<Session>>) {
        let sender_id = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = IdentityKey::generate().secret().clone();
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let (message, mut sender_sess) =
            Session::initiate(&sender_id, &bundle, b"test").expect("Failed to initiate session");

        let initial_msg = match message {
            Message::Initial(msg) => msg,
            _ => panic!("Expected Initial message type"),
        };

        let (mut receiver_sess, _) =
            Session::recv(&receiver_id, &spk_secret, &bundle, &initial_msg, None)
                .expect("Failed to receive session");

        // Exchange messages to establish the ratchet properly
        let setup_msg = sender_sess
            .encrypt(b"setup")
            .expect("Failed to encrypt setup message");
        let _ = receiver_sess
            .decrypt(&setup_msg)
            .expect("Failed to decrypt setup message");

        (
            Arc::new(Mutex::new(sender_sess)),
            Arc::new(Mutex::new(receiver_sess)),
        )
    }

    /// Setup mock server for Walrus testing
    async fn setup_mock_server_and_conf() -> anyhow::Result<(ServerGuard, StorageConf)> {
        // Create mock server
        let server = Server::new_async().await;
        let server_url = server.url();

        // Create a Walrus client that points to our mock server
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(server_url.clone()),
            walrus_aggregator_url: Some(server_url),
            walrus_save_for_epochs: Some(2),
        };

        Ok((server, storage_conf))
    }

    #[tokio::test]
    async fn test_inline_plain_roundrip() {
        let storage_conf = StorageConf::default();
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_inline(data.clone());

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Inline);

        // Nothing should change when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_eq!(committed.as_json(), &data);
        assert_eq!(committed.storage_kind(), StorageKind::Inline);

        let committed_data: NexusData = committed.into();

        // Nothing should change when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_inline_non_array_encrypted_roundrip() {
        let storage_conf = StorageConf::default();
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_inline_encrypted(data.clone());

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Inline);

        // Data should be encrypted when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert_eq!(committed.storage_kind(), StorageKind::Inline);

        let committed_data: NexusData = committed.into();

        // Data should be decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_inline_array_encrypted_roundrip() {
        let storage_conf = StorageConf::default();
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let nexus_data = NexusData::new_inline_encrypted(data.clone());

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Inline);

        // Data should be encrypted when we commit as Nexus. Elements should
        // also be encrypted individually.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert_eq!(
            committed.as_json().as_array().unwrap().len(),
            data.as_array().unwrap().len()
        );
        assert_eq!(committed.storage_kind(), StorageKind::Inline);

        let committed_data: NexusData = committed.into();

        // Data should be decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_walrus_plain_roundrip() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Mock server should start");
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        // Setup mock Walrus response
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

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_string());
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let committed_data: NexusData = committed.into();

        // Data should be fetched from walrus when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);

        // Verify the requests were made
        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[tokio::test]
    async fn test_walrus_empty_arr_plain_roundrip() {
        let storage_conf = StorageConf {
            walrus_publisher_url: Some("https://publisher.url".to_string()),
            walrus_aggregator_url: Some("https://aggregator.url".to_string()),
            walrus_save_for_epochs: Some(2),
        };
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!([]);

        let nexus_data = NexusData::new_walrus(data.clone());

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Nothing should change when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_eq!(committed.as_json(), &data);
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let committed_data: NexusData = committed.into();

        // Nothing should change when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
    }

    #[tokio::test]
    async fn test_walrus_non_array_encrypted_roundrip() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Mock server should start");
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_walrus_encrypted(data.clone());

        // Setup mock Walrus response
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

        let mut encrypted_data = data.clone();
        nexus_session
            .lock()
            .await
            .encrypt_nexus_data_json(&mut encrypted_data)
            .expect("Must encrypt data");

        let mock_get = server
            .mock("GET", "/v1/blobs/json_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&encrypted_data).expect("Must serialize"))
            .create_async()
            .await;

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be encrypted and stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_string());
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let committed_data: NexusData = committed.into();
        // Data should be fetched from walrus and decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);

        // Verify the requests were made
        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[tokio::test]
    async fn test_walrus_array_plain_roundrip() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Mock server should start");
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let nexus_data = NexusData::new_walrus(data.clone());

        // Setup mock Walrus response
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

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_array());
        assert_eq!(
            committed.as_json().as_array().unwrap().len(),
            data.as_array().unwrap().len()
        );
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let committed_data: NexusData = committed.into();

        // Data should be fetched from walrus when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);

        // Verify the requests were made
        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[tokio::test]
    async fn test_walrus_array_encrypted_roundrip() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("Mock server should start");
        let (nexus_session, user_session) = create_test_sessions();
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let nexus_data = NexusData::new_walrus_encrypted(data.clone());

        // Setup mock Walrus response
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

        let mut encrypted_data = data.clone();
        nexus_session
            .lock()
            .await
            .encrypt_nexus_data_json(&mut encrypted_data)
            .expect("Must encrypt data");

        let mock_get = server
            .mock("GET", "/v1/blobs/json_blob_id")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&encrypted_data).expect("Must serialize"))
            .create_async()
            .await;

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be encrypted and stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_array());
        assert_eq!(
            committed.as_json().as_array().unwrap().len(),
            data.as_array().unwrap().len()
        );
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let committed_data: NexusData = committed.into();
        // Data should be fetched from walrus and decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);

        // Verify the requests were made
        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[test]
    fn test_json_to_nexus_data_map_all_branches() {
        let json = json!({
            "plain_inline": 1,
            "encrypted_inline": 2,
            "plain_walrus": 3,
            "encrypted_walrus": 4
        });

        let encrypt_fields = vec![
            "encrypted_inline".to_string(),
            "encrypted_walrus".to_string(),
        ];
        let remote_fields = vec!["plain_walrus".to_string(), "encrypted_walrus".to_string()];

        let map = json_to_nexus_data_map(&json, &encrypt_fields, &remote_fields, None).unwrap();

        assert_eq!(
            map.get("plain_inline").unwrap(),
            &NexusData::new_inline(json.get("plain_inline").unwrap().clone())
        );
        assert_eq!(
            map.get("encrypted_inline").unwrap(),
            &NexusData::new_inline_encrypted(json.get("encrypted_inline").unwrap().clone())
        );
        assert_eq!(
            map.get("plain_walrus").unwrap(),
            &NexusData::new_walrus(json.get("plain_walrus").unwrap().clone())
        );
        assert_eq!(
            map.get("encrypted_walrus").unwrap(),
            &NexusData::new_walrus_encrypted(json.get("encrypted_walrus").unwrap().clone())
        );
    }

    #[test]
    fn test_json_to_nexus_data_map_empty_object() {
        let json = json!({});
        let encrypt_fields = vec![];
        let remote_fields = vec![];

        let map = json_to_nexus_data_map(&json, &encrypt_fields, &remote_fields, None).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn test_json_to_nexus_data_map_non_object_error() {
        let json = json!(["not", "an", "object"]);
        let encrypt_fields = vec![];
        let remote_fields = vec![];

        let result = json_to_nexus_data_map(&json, &encrypt_fields, &remote_fields, None);
        assert!(result.is_err());
        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Expected JSON object")
        );
    }

    #[test]
    fn test_json_to_nexus_data_map_remote_inline_error() {
        // Try to store data remotely using inline storage, which should error.
        let json = json!({
            "remote_inline": 42
        });
        let encrypt_fields = vec![];
        let remote_fields = vec!["remote_inline".to_string()];
        // Explicitly request Inline as preferred remote storage kind.
        let result = json_to_nexus_data_map(
            &json,
            &encrypt_fields,
            &remote_fields,
            Some(StorageKind::Inline),
        );
        assert!(result.is_err());
        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Cannot store data remotely using inline storage")
        );
    }

    #[test]
    fn test_json_to_nexus_data_map_remote_inline_encrypted_error() {
        // Try to store encrypted data remotely using inline storage, which should error.
        let json = json!({
            "remote_inline_encrypted": 99
        });
        let encrypt_fields = vec!["remote_inline_encrypted".to_string()];
        let remote_fields = vec!["remote_inline_encrypted".to_string()];
        let result = json_to_nexus_data_map(
            &json,
            &encrypt_fields,
            &remote_fields,
            Some(StorageKind::Inline),
        );
        assert!(result.is_err());
        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Cannot store data remotely using inline storage")
        );
    }

    #[test]
    fn test_hint_remote_fields_all_fit() {
        // All fields fit within the transaction size.
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
        // Create a large field to overflow the transaction size.
        let mut large_string = String::new();
        for _ in 0..(MAX_TRANSACTION_SIZE) {
            large_string.push('x');
        }
        let json = json!({
            "small": "ok",
            "large": large_string
        });
        let result = hint_remote_fields(&json).unwrap();
        // Only "large" should be hinted as remote.
        assert_eq!(result, vec!["large"]);
    }

    #[test]
    fn test_hint_remote_fields_multiple_fields_overflow() {
        // Create several large fields to overflow the transaction size.
        let mut json_obj = serde_json::Map::new();
        let mut expected_remote = Vec::new();
        let field_count = 5;
        let field_size = MAX_TRANSACTION_SIZE / field_count;
        for i in 0..field_count {
            let key = format!("field{}", i);
            let value = "x".repeat(field_size);
            json_obj.insert(key.clone(), serde_json::Value::String(value));
            expected_remote.push(key);
        }
        let json = serde_json::Value::Object(json_obj);
        let result = hint_remote_fields(&json).unwrap();
        // At least one field should be hinted as remote, possibly more.
        assert!(!result.is_empty());
        // If all fields are large, all may be hinted.
        assert!(expected_remote.iter().any(|k| result.contains(k)));
    }

    #[test]
    fn test_hint_remote_fields_array_storage_cost() {
        // Create a large array to overflow the transaction size.
        let arr_len =
            ((MAX_TRANSACTION_SIZE - NEXUS_BASE_TRANSACTION_SIZE) / WALRUS_BLOB_ID_LENGTH) - 10;
        let arr: Vec<serde_json::Value> = (0..arr_len)
            // Make the data longer than the walrus key storage cost to ensure
            // storing remotely is beneficial.
            .map(|_| json!({"x": "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"}))
            .collect();
        let json = json!({
            "big_array": arr,
            "small": "ok"
        });
        let result = hint_remote_fields(&json).unwrap();
        // "big_array" should be hinted as remote.
        assert!(result.contains(&"big_array".to_string()));
    }

    #[test]
    fn test_hint_remote_fields_cannot_fit_even_if_all_remote() {
        // Create fields so large that even storing all remotely can't fit.
        let mut json_obj = serde_json::Map::new();
        let huge_arr_len = (MAX_TRANSACTION_SIZE * 2) / WALRUS_BLOB_ID_LENGTH;
        let huge_arr: Vec<serde_json::Value> = (0..huge_arr_len)
            // Make the data longer than the walrus key storage cost to ensure
            // storing remotely is beneficial.
            .map(|_| json!("yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"))
            .collect();
        json_obj.insert("huge_array".to_string(), serde_json::Value::Array(huge_arr));
        let json = serde_json::Value::Object(json_obj);
        let result = hint_remote_fields(&json);
        assert!(result.is_err());
        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Cannot fit data within max transaction size")
        );
    }

    #[test]
    fn test_hint_remote_fields_non_object_error() {
        let json = json!(["not", "an", "object"]);
        let result = hint_remote_fields(&json);
        assert!(result.is_err());
        assert_matches!(
            result,
            Err(e) if e.to_string().contains("Expected JSON object")
        );
    }
}
