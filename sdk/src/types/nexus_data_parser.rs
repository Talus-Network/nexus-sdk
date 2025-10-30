//! Module that defines serde serialization and deserialization for [`NexusData`]
//!
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
    crate::types::{DataStorage, InlineStorage, NexusData, Storable, WalrusStorage},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
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
                    let str = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
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
