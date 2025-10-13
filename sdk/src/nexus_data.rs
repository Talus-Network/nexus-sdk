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
    crate::{crypto::session::Session, walrus::WalrusClient},
    enum_dispatch::enum_dispatch,
    serde::{Deserialize, Serialize},
};

// TODO: tests for these integrations. should work e2e. Also need `from` fns.
// TODO: when fetch/commit, we need to keep the storage type info.

// == NexusData ==

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NexusData {
    data: DataStorage,
    /// Whether the data is encrypted and should be decrypted before use or
    /// encrypted before committing.
    encrypted: bool,
}

impl NexusData {
    /// Fetches the data from its storage location. This consumes `self`.
    pub async fn fetch(
        self,
        conf: &StorageConf,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value> {
        self.data.fetch(conf, self.encrypted, session).await
    }

    /// Commits the data to its storage location. This consumes `self`.
    pub async fn commit(
        self,
        conf: &StorageConf,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value> {
        self.data.commit(conf, self.encrypted, session).await
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineStorage {
    data: serde_json::Value,
}

impl Storable for InlineStorage {
    async fn fetch(
        self,
        _: &StorageConf,
        decrypt: bool,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value> {
        if decrypt {
            return session.decrypt_nexus_data_json(&self.data);
        }

        Ok(self.data)
    }

    async fn commit(
        mut self,
        _: &StorageConf,
        encrypt: bool,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value> {
        if encrypt {
            session.encrypt_nexus_data_json(&mut self.data)?;
        }

        Ok(self.data)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalrusStorage {
    /// We have to differentiate between single values and arrays of values
    /// to support looping.
    data: serde_json::Value,
}

impl Storable for WalrusStorage {
    async fn fetch(
        self,
        conf: &StorageConf,
        decrypt: bool,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value> {
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
                    return Ok(serde_json::Value::Array(vec![]));
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

        let blob = client.read_json::<serde_json::Value>(&blob_id).await?;

        // Decrypt the data if needed.
        if decrypt {
            return session.decrypt_nexus_data_json(&blob);
        }

        Ok(blob)
    }

    async fn commit(
        mut self,
        conf: &StorageConf,
        encrypt: bool,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value> {
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
        if encrypt {
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
                    return Ok(serde_json::Value::Array(vec![]));
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
        match data_kind {
            DataKind::One => Ok(serde_json::Value::String(info.blob_object.blob_id)),
            DataKind::Many(len) => Ok(serde_json::Value::Array(
                std::iter::repeat(serde_json::Value::String(info.blob_object.blob_id.clone()))
                    .take(len)
                    .collect(),
            )),
        }
    }
}

// == DataBag ==

/// We need to distinguish between single values and arrays of values to support
/// looping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataBag {
    One(serde_json::Value),
    Many(Vec<serde_json::Value>),
}

/// Trait defining two methods for accessing and saving data based on its storage
/// type.
#[enum_dispatch]
#[allow(async_fn_in_trait)]
pub trait Storable {
    /// Fetch the data from its storage location.
    async fn fetch(
        self,
        conf: &StorageConf,
        decrypt: bool,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value>;

    /// Commit the data to its storage location.
    async fn commit(
        self,
        conf: &StorageConf,
        encrypt: bool,
        session: &mut Session,
    ) -> anyhow::Result<serde_json::Value>;
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

            let value = if data.one.len() > 0 {
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
                    data: DataStorage::Inline(InlineStorage { data: value }),
                    encrypted: data.encrypted,
                }),
                NEXUS_DATA_WALRUS_STORAGE_TAG => Ok(NexusData {
                    data: DataStorage::Walrus(WalrusStorage { data: value }),
                    encrypted: data.encrypted,
                }),
                // Add more...
                _ => todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>"),
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

            let encrypted = self.encrypted;

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
                }),
                encrypted: false,
            };

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
                }),
                encrypted: false,
            };

            let serialized = serde_json::to_string(&dag_data).unwrap();

            assert_eq!(
                serialized,
                r#"{"storage":[105,110,108,105,110,101],"one":[],"many":[[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]],"encrypted":false}"#
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
                }),
                encrypted: true,
            };

            let serialized = serde_json::to_string(&dag_data).unwrap();

            // this is where the storage tag comes from
            assert_eq!(NEXUS_DATA_WALRUS_STORAGE_TAG, [119, 97, 108, 114, 117, 115]);

            // The byte representation of the JSON object
            // {"key":"value"} is [123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]
            assert_eq!(
                serialized,
                r#"{"storage":[119,97,108,114,117,115],"one":[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],"many":[],"encrypted":true}"#
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
                }),
                encrypted: true,
            };

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
