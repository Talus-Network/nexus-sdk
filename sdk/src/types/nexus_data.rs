//! [`NexusData`] is a wrapper around any raw data stored on-chain. This can be
//! data for input ports, output ports or default values. It is represented as
//! an enum because default values can be stored remotely.
//!
//! The [`NexusData::Inline::data`] field is a byte array on-chain but we assume
//! that, upon decoding it, it will be a valid JSON object.
//!
//! The [`NexusData::Walrus::data`] field is also a byte array on-chain but we
//! assume that, upon decoding it, it will be a valid JSON object containing
//! a reference to Walrus storage.
//!
//! Note: As an optimization to reduce storage costs, array types are stored as
//! one blob containing a JSON array, instead of multiple blobs. This means that
//! the array of keys referencing the data will contain only one key repeated N
//! times where N is the length of the array.

use serde::{Deserialize, Serialize};

// TODO: rethink the enum approach. the only difference seems to be how to fetch
// the data. inline just return. walrus make a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NexusData {
    Inline { data: DataBag, encrypted: bool },
    Walrus { data: DataBag, encrypted: bool },
}

impl NexusData {
    /// Returns true if the data is encrypted and should be decrypted before
    /// sending it to a tool.
    pub fn is_encrypted(&self) -> bool {
        match self {
            NexusData::Inline { encrypted, .. } => *encrypted,
            NexusData::Walrus { encrypted, .. } => *encrypted,
        }
    }

    /// Returns a reference to the data.
    pub fn data(&self) -> &DataBag {
        match self {
            NexusData::Inline { data, .. } => data,
            NexusData::Walrus { data, .. } => data,
        }
    }
}

/// We need to distinguish between single values and arrays of values to support
/// looping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataBag {
    One(serde_json::Value),
    Many(Vec<serde_json::Value>),
}

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

                DataBag::One(serde_json::from_str(&str).map_err(serde::de::Error::custom)?)
            } else {
                // If we're dealing with multiple values, we assume that
                // the data is an array of JSON strings that can be parsed.
                let mut values = Vec::with_capacity(data.many.len());

                for value in data.many {
                    let str = String::from_utf8(value).map_err(serde::de::Error::custom)?;

                    values.push(serde_json::from_str(&str).map_err(serde::de::Error::custom)?);
                }

                DataBag::Many(values)
            };

            match data.storage.as_ref() {
                NEXUS_DATA_INLINE_STORAGE_TAG => Ok(NexusData::Inline {
                    data: value,
                    encrypted: data.encrypted,
                }),
                NEXUS_DATA_WALRUS_STORAGE_TAG => Ok(NexusData::Walrus {
                    data: value,
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
            let data = self.data();
            let encrypted = self.is_encrypted();

            let storage = match self {
                NexusData::Inline { .. } => NEXUS_DATA_INLINE_STORAGE_TAG.to_vec(),
                NexusData::Walrus { .. } => NEXUS_DATA_WALRUS_STORAGE_TAG.to_vec(),
                // Add more...
            };

            let (one, many) = match data {
                DataBag::One(value) => {
                    // If the data is a single value, we serialize it as a
                    // single JSON string in the `one` field.
                    (
                        serde_json::to_string(value)
                            .map_err(serde::ser::Error::custom)?
                            .into_bytes(),
                        vec![],
                    )
                }
                DataBag::Many(values) => {
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
            let dag_data = NexusData::Inline {
                data: DataBag::One(serde_json::json!({
                    "key": "value"
                })),
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
            let dag_data = NexusData::Inline {
                data: DataBag::Many(vec![
                    serde_json::json!({
                        "key": "value"
                    }),
                    serde_json::json!({
                        "key": "value"
                    }),
                ]),
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
            let dag_data = NexusData::Walrus {
                data: DataBag::One(serde_json::json!({
                    "key": "value"
                })),
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
            let dag_data = NexusData::Walrus {
                data: DataBag::Many(vec![
                    serde_json::json!({
                        "key": "value"
                    }),
                    serde_json::json!({
                        "key": "value"
                    }),
                ]),
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
