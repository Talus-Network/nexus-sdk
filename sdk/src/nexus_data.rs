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
    crate::{crypto::session::Session, types::StorageKind, walrus::WalrusClient},
    enum_dispatch::enum_dispatch,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// Nexus submit walk evaluation transaction size base size without any data.
pub const NEXUS_BASE_TRANSACTION_SIZE: usize = 3 * 1024;
/// Max transaction size supported by Sui.
pub const MAX_TRANSACTION_SIZE: usize = 128 * 1024;
/// The length of a Walrus blob ID.
pub const WALRUS_BLOB_ID_LENGTH: usize = 44;

// == NexusData ==

// TODO: add remote storage configurations to CLI
// TODO: add CLI docs

/// Note that the sole reason the top-level [`NexusData`] struct exists is to
/// ensure that the inner `data` can only be accessed via [`DataStorage`]'s
/// `fetch` and `commit` methods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NexusData {
    data: DataStorage,
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
        session: &mut Session,
    ) -> anyhow::Result<DataStorage> {
        self.data.fetch(conf, session).await?;

        Ok(self.data)
    }

    /// Commits the data to its storage location. This consumes `self`.
    pub async fn commit(
        mut self,
        conf: &StorageConf,
        session: &mut Session,
    ) -> anyhow::Result<DataStorage> {
        self.data.commit(conf, session).await?;

        Ok(self.data)
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
    pub walrus_save_for_epochs: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineStorage {
    data: serde_json::Value,
    /// Whether the data is encrypted and should be decrypted before use or
    /// encrypted before committing.
    encrypted: bool,
}

impl Storable for InlineStorage {
    async fn fetch(&mut self, _: &StorageConf, session: &mut Session) -> anyhow::Result<()> {
        if self.encrypted {
            self.data = session.decrypt_nexus_data_json(&self.data)?;
        }

        Ok(())
    }

    async fn commit(&mut self, _: &StorageConf, session: &mut Session) -> anyhow::Result<()> {
        if self.encrypted {
            session.encrypt_nexus_data_json(&mut self.data)?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalrusStorage {
    data: serde_json::Value,
    /// Whether the data is encrypted and should be decrypted before use or
    /// encrypted before committing.
    encrypted: bool,
}

impl Storable for WalrusStorage {
    async fn fetch(&mut self, conf: &StorageConf, session: &mut Session) -> anyhow::Result<()> {
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
            data = session.decrypt_nexus_data_json(&data)?;
        }

        self.data = data;

        Ok(())
    }

    async fn commit(&mut self, conf: &StorageConf, session: &mut Session) -> anyhow::Result<()> {
        let walrus_publisher_url = conf
            .walrus_publisher_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus publisher URL is not set in storage config"))?;

        let store_for_epochs = conf.walrus_save_for_epochs.ok_or_else(|| {
            anyhow::anyhow!("Walrus save for epochs is not set in storage config")
        })?;

        let client = WalrusClient::builder()
            .with_publisher_url(walrus_publisher_url)
            .build();

        // Encrypt the data if needed.
        if self.encrypted {
            session.encrypt_nexus_data_json(&mut self.data)?;
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
    async fn fetch(&mut self, conf: &StorageConf, session: &mut Session) -> anyhow::Result<()>;

    /// Commit the data to its storage location.
    async fn commit(&mut self, conf: &StorageConf, session: &mut Session) -> anyhow::Result<()>;

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
/// be stored remotely to avoid exceeding [`crate::nexus_data::MAX_TRANSACTION_SIZE`].
pub fn hint_remote_fields(json: &serde_json::Value) -> anyhow::Result<Vec<String>> {
    let Some(obj) = json.as_object() else {
        anyhow::bail!("Expected JSON object");
    };

    // Calculate the size of each field.
    let mut fields: Vec<(&String, usize)> = obj
        .iter()
        .map(|(key, value)| (key, serde_json::to_vec(value).map(|v| v.len()).unwrap_or(0)))
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

// == Parsing ==

mod parser {
    //! We represent nexus data onchain as a struct of
    //! `{ storage: u8[], one: u8[], many: u8[][], encrypted: bool }`.
    //!
    //! However, storage has a special value [NEXUS_DATA_INLINE_STORAGE_TAG].
    //! Therefore we represent [NexusData] as an enum within the codebase.
    //!
    //! `one` and `many` are mutually exclusive, meaning that if one is
    //! present, the other cannot be. The `one` field is used for single values,
    //! while the `many` field is used for arrays of values. The `encrypted` field
    //! indicates whether the data is encrypted and should be decrypted before
    //! use.

    /// This is a hard-coded identifier for inline data in nexus.
    /// Inline means you can parse it as is, without any additional processing.
    ///
    /// This is as opposed to data stored in some storage.
    const NEXUS_DATA_INLINE_STORAGE_TAG: &[u8] = b"inline";

    /// This is a hard-coded identifier for remote storage via Walrus.
    /// Meaning we only store a reference to the data on-chain and fetch it
    /// from Walrus when needed.
    const NEXUS_DATA_WALRUS_STORAGE_TAG: &[u8] = b"walrus";

    use {
        super::*,
        serde::{de::Deserializer, ser::Serializer},
    };

    #[derive(Serialize, Deserialize)]
    struct NexusDataAsStruct {
        /// Either identifies some remote storage or is equal to [NEXUS_DATA_INLINE_STORAGE_TAG]
        /// if the data can be parsed as is.
        storage: Vec<u8>,
        one: Vec<u8>,
        many: Vec<Vec<u8>>,
        encrypted: bool,
    }

    impl<'de> Deserialize<'de> for NexusData {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let data: NexusDataAsStruct = Deserialize::deserialize(deserializer)?;

            let value = if !data.one.is_empty() {
                // If we're dealing with a single value, we assume that
                // the data is a JSON string that can be parsed directly.
                let str = String::from_utf8(data.one).map_err(serde::de::Error::custom)?;

                serde_json::from_str(&str).map_err(serde::de::Error::custom)?
            } else {
                // If we're dealing with multiple values, we assume that
                // the data is an array of JSON strings that can be parsed.
                let mut values = Vec::with_capacity(data.many.len());

                for value in data.many {
                    let str = String::from_utf8(value).map_err(serde::de::Error::custom)?;

                    values.push(serde_json::from_str(&str).map_err(serde::de::Error::custom)?);
                }

                serde_json::Value::Array(values)
            };

            match data.storage.as_ref() {
                NEXUS_DATA_INLINE_STORAGE_TAG => Ok(NexusData {
                    data: DataStorage::Inline(InlineStorage {
                        data: value,
                        encrypted: data.encrypted,
                    }),
                }),
                NEXUS_DATA_WALRUS_STORAGE_TAG => Ok(NexusData {
                    data: DataStorage::Walrus(WalrusStorage {
                        data: value,
                        encrypted: data.encrypted,
                    }),
                }),
                // Add more...
                _ => unimplemented!("Unknown storage type"),
            }
        }
    }

    impl Serialize for NexusData {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let data = match &self.data {
                DataStorage::Inline(storage) => &storage.data,
                DataStorage::Walrus(storage) => &storage.data,
                // Add more...
            };

            let encrypted = self.data.is_encrypted();

            let storage = match &self.data {
                DataStorage::Inline { .. } => NEXUS_DATA_INLINE_STORAGE_TAG.to_vec(),
                DataStorage::Walrus { .. } => NEXUS_DATA_WALRUS_STORAGE_TAG.to_vec(),
                // Add more...
            };

            let (one, many) = match data {
                serde_json::Value::Array(values) => {
                    // If the data is an array, we serialize it as an array of
                    // JSON strings in the `many` field.
                    let mut many = Vec::with_capacity(values.len());

                    for value in values {
                        let str =
                            serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
                        many.push(str.into_bytes());
                    }

                    (vec![], many)
                }
                _ => {
                    // If the data is a single value, we serialize it as a
                    // single JSON string in the `one` field.
                    (
                        serde_json::to_string(data)
                            .map_err(serde::ser::Error::custom)?
                            .into_bytes(),
                        vec![],
                    )
                }
            };

            NexusDataAsStruct {
                storage,
                one,
                many,
                encrypted,
            }
            .serialize(serializer)
        }
    }

    /// Tests for parsing only.
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_inline_dag_data_sers_and_desers() {
            // Single value.
            let dag_data = NexusData {
                data: DataStorage::Inline(InlineStorage {
                    data: serde_json::json!({
                        "key": "value"
                    }),
                    encrypted: false,
                }),
            };

            assert_eq!(
                dag_data,
                NexusData::new_inline(serde_json::json!({"key": "value"}))
            );

            let serialized = serde_json::to_string(&dag_data).unwrap();

            // this is where the storage tag comes from
            assert_eq!(
                NEXUS_DATA_INLINE_STORAGE_TAG,
                [105, 110, 108, 105, 110, 101]
            );

            // The byte representation of the JSON object
            // {"key":"value"} is [123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]
            assert_eq!(
                serialized,
                r#"{"storage":[105,110,108,105,110,101],"one":[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],"many":[],"encrypted":false}"#
            );

            let deserialized = serde_json::from_str(&serialized).unwrap();

            assert_eq!(dag_data, deserialized);

            // Array of values.
            let dag_data = NexusData {
                data: DataStorage::Inline(InlineStorage {
                    data: serde_json::json!([
                        { "key": "value" },
                        { "key": "value" }
                    ]),
                    encrypted: true,
                }),
            };

            assert_eq!(
                dag_data,
                NexusData::new_inline_encrypted(serde_json::json!([
                    { "key": "value" },
                    { "key": "value" }
                ]))
            );

            let serialized = serde_json::to_string(&dag_data).unwrap();

            assert_eq!(
                serialized,
                r#"{"storage":[105,110,108,105,110,101],"one":[],"many":[[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]],"encrypted":true}"#
            );

            let deserialized = serde_json::from_str(&serialized).unwrap();

            assert_eq!(dag_data, deserialized);
        }

        #[test]
        fn test_walrus_dag_data_sers_and_desers() {
            // Single value.
            let dag_data = NexusData {
                data: DataStorage::Walrus(WalrusStorage {
                    data: serde_json::json!({
                        "key": "value"
                    }),
                    encrypted: false,
                }),
            };

            assert_eq!(
                dag_data,
                NexusData::new_walrus(serde_json::json!({"key": "value"}))
            );

            let serialized = serde_json::to_string(&dag_data).unwrap();

            // this is where the storage tag comes from
            assert_eq!(NEXUS_DATA_WALRUS_STORAGE_TAG, [119, 97, 108, 114, 117, 115]);

            // The byte representation of the JSON object
            // {"key":"value"} is [123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]
            assert_eq!(
                serialized,
                r#"{"storage":[119,97,108,114,117,115],"one":[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],"many":[],"encrypted":false}"#
            );

            let deserialized = serde_json::from_str(&serialized).unwrap();

            assert_eq!(dag_data, deserialized);

            // Array of values.
            let dag_data = NexusData {
                data: DataStorage::Walrus(WalrusStorage {
                    data: serde_json::json!([
                        { "key": "value" },
                        { "key": "value" }
                    ]),
                    encrypted: true,
                }),
            };

            assert_eq!(
                dag_data,
                NexusData::new_walrus_encrypted(serde_json::json!([
                    { "key": "value" },
                    { "key": "value" }
                ]))
            );

            let serialized = serde_json::to_string(&dag_data).unwrap();

            assert_eq!(
                serialized,
                r#"{"storage":[119,97,108,114,117,115],"one":[],"many":[[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]],"encrypted":true}"#
            );

            let deserialized = serde_json::from_str(&serialized).unwrap();

            assert_eq!(dag_data, deserialized);
        }
    }
}

/// E2E tests for encryption, walrus integration and the helpers.
#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::crypto::{
            session::Message,
            x3dh::{IdentityKey, PreKeyBundle},
        },
        assert_matches::assert_matches,
        serde_json::json,
    };

    const WALRUS_PUBLISHER_URL: &str = "https://publisher.walrus-testnet.walrus.space";
    const WALRUS_AGGREGATOR_URL: &str = "https://aggregator.walrus-testnet.walrus.space";

    /// Helper to create sender and receiver sessions for testing
    /// Returns (nexus_session, user_session) where:
    /// - nexus_session: represents the Nexus system that encrypts output data
    /// - user_session: represents the user inspecting execution and decrypting data
    fn create_test_sessions() -> (Session, Session) {
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

        (sender_sess, receiver_sess)
    }

    fn create_storage_conf() -> StorageConf {
        StorageConf {
            walrus_publisher_url: Some(WALRUS_PUBLISHER_URL.to_string()),
            walrus_aggregator_url: Some(WALRUS_AGGREGATOR_URL.to_string()),
            walrus_save_for_epochs: Some(2),
        }
    }

    #[tokio::test]
    async fn test_inline_plain_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_inline(data.clone());

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Inline);

        // Nothing should change when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_eq!(committed.as_json(), &data);
        assert_eq!(committed.storage_kind(), StorageKind::Inline);

        let committed_data: NexusData = committed.into();

        // Nothing should change when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_inline_non_array_encrypted_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_inline_encrypted(data.clone());

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Inline);

        // Data should be encrypted when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert_eq!(committed.storage_kind(), StorageKind::Inline);

        let committed_data: NexusData = committed.into();

        // Data should be decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_inline_array_encrypted_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let nexus_data = NexusData::new_inline_encrypted(data.clone());

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Inline);

        // Data should be encrypted when we commit as Nexus. Elements should
        // also be encrypted individually.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
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
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Inline);
    }

    #[tokio::test]
    async fn test_walrus_plain_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_walrus(data.clone());

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_string());
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        // We can fetch and parse the blob from Walrus to get back the original data.
        let key = committed.as_json().as_str().unwrap();
        let client = WalrusClient::builder()
            .with_aggregator_url(WALRUS_AGGREGATOR_URL)
            .build();
        let fetched_data = client
            .read_json::<serde_json::Value>(key)
            .await
            .expect("Failed to fetch data from Walrus");

        assert_eq!(fetched_data, data);

        let committed_data: NexusData = committed.into();

        // Data should be fetched from walrus when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
    }

    #[tokio::test]
    async fn test_walrus_empty_arr_plain_roundrip() {
        // Empty array should not contact walrus at all.
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!([]);

        let nexus_data = NexusData::new_walrus(data.clone());

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Nothing should change when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_eq!(committed.as_json(), &data);
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        let committed_data: NexusData = committed.into();

        // Nothing should change when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
    }

    #[tokio::test]
    async fn test_walrus_non_array_encrypted_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!({"key": "value"});

        let nexus_data = NexusData::new_walrus_encrypted(data.clone());

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be encrypted and stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_string());
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        // We can fetch and parse the blob from Walrus to get back the original data.
        let key = committed.as_json().as_str().unwrap();
        let client = WalrusClient::builder()
            .with_aggregator_url(WALRUS_AGGREGATOR_URL)
            .build();
        let fetched_data = client
            .read_json::<serde_json::Value>(key)
            .await
            .expect("Failed to fetch data from Walrus");
        let decrypted_data = nexus_session
            .decrypt_nexus_data_json(&fetched_data)
            .expect("Failed to decrypt data");
        assert_eq!(decrypted_data, data);

        let committed_data: NexusData = committed.into();
        // Data should be fetched from walrus and decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
    }

    #[tokio::test]
    async fn test_walrus_array_plain_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let nexus_data = NexusData::new_walrus(data.clone());

        // Inspect the inner data.
        assert!(!nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_array());
        assert_eq!(
            committed.as_json().as_array().unwrap().len(),
            data.as_array().unwrap().len()
        );
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        // We can fetch and parse the blob from Walrus to get back the original data.
        let key = committed.as_json().as_array().unwrap()[0].as_str().unwrap();
        let client = WalrusClient::builder()
            .with_aggregator_url(WALRUS_AGGREGATOR_URL)
            .build();
        let fetched_data = client
            .read_json::<serde_json::Value>(key)
            .await
            .expect("Failed to fetch data from Walrus");

        assert_eq!(fetched_data, data);

        let committed_data: NexusData = committed.into();

        // Data should be fetched from walrus when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
    }

    #[tokio::test]
    async fn test_walrus_array_encrypted_roundrip() {
        let storage_conf = create_storage_conf();
        let (mut nexus_session, mut user_session) = create_test_sessions();
        let data = json!([{"key": "value"}, {"key": "value2"}]);

        let nexus_data = NexusData::new_walrus_encrypted(data.clone());

        // Inspect the inner data.
        assert!(nexus_data.data.is_encrypted());
        assert_eq!(nexus_data.data.as_json(), &data);
        assert_eq!(nexus_data.data.storage_kind(), StorageKind::Walrus);

        // Data should be encrypted and stored on walrus when we commit as Nexus.
        let committed = nexus_data
            .commit(&storage_conf, &mut nexus_session)
            .await
            .expect("Failed to commit data");

        assert_ne!(committed.as_json(), &data);
        assert!(committed.as_json().is_array());
        assert_eq!(
            committed.as_json().as_array().unwrap().len(),
            data.as_array().unwrap().len()
        );
        assert_eq!(committed.storage_kind(), StorageKind::Walrus);

        // We can fetch and parse the blob from Walrus to get back the original data.
        let key = committed.as_json().as_array().unwrap()[0].as_str().unwrap();
        let client = WalrusClient::builder()
            .with_aggregator_url(WALRUS_AGGREGATOR_URL)
            .build();
        let fetched_data = client
            .read_json::<serde_json::Value>(key)
            .await
            .expect("Failed to fetch data from Walrus");
        // Each element should be encrypted individually.
        assert!(fetched_data.is_array());
        let decrypted_data = nexus_session
            .decrypt_nexus_data_json(&fetched_data)
            .expect("Failed to decrypt data");
        assert_eq!(decrypted_data, data);

        let committed_data: NexusData = committed.into();
        // Data should be fetched from walrus and decrypted when we fetch as user.
        let fetched = committed_data
            .fetch(&storage_conf, &mut user_session)
            .await
            .expect("Failed to fetch data");

        assert_eq!(fetched.as_json(), &data);
        assert_eq!(fetched.storage_kind(), StorageKind::Walrus);
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
