//! [`NexusData`] is a wrapper around any raw data stored on-chain. This can be
//! data for input ports, output ports or default values. It is represented as
//! an enum because default values can be stored remotely.
//!
//! See: <https://github.com/Talus-Network/nexus-next/issues/30>
//!
//! The [`NexusData::data`] field is a byte array on-chain but we assume that,
//! upon decoding it, it will be a valid JSON object.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NexusData {
    Inline {
        data: serde_json::Value,
        /// Whether the data is encrypted and should be decrypted before sending
        /// it to a tool.
        encrypted: bool,
    },
    #[allow(dead_code)]
    Remote {
        // TODO: <https://github.com/Talus-Network/nexus-next/issues/30>
    },
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
                NEXUS_DATA_INLINE_STORAGE_TAG => Ok(NexusData::Inline {
                    data: value,
                    encrypted: data.encrypted,
                }),
                _ => todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>"),
            }
        }
    }

    impl Serialize for NexusData {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let data = match self {
                NexusData::Inline { data, encrypted } => {
                    let (one, many) = if let serde_json::Value::Array(ref values) = data {
                        // If the data is an array, we serialize it as an array of JSON strings.
                        let mut many = Vec::with_capacity(values.len());

                        for value in data.as_array().unwrap() {
                            let str =
                                serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
                            many.push(str.into_bytes());
                        }

                        (vec![], many)
                    } else {
                        // If the data is a single value, we serialize it as a single JSON string.
                        (
                            serde_json::to_string(data)
                                .map_err(serde::ser::Error::custom)?
                                .into_bytes(),
                            vec![],
                        )
                    };

                    NexusDataAsStruct {
                        storage: NEXUS_DATA_INLINE_STORAGE_TAG.to_vec(),
                        one,
                        many,
                        encrypted: *encrypted,
                    }
                }
                NexusData::Remote {} => {
                    todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/30>")
                }
            };

            data.serialize(serializer)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_dag_data_sers_and_desers() {
            // Single value.
            let dag_data = NexusData::Inline {
                data: serde_json::json!({
                    "key": "value"
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
            let dag_data = NexusData::Inline {
                data: serde_json::json!([
                    {
                        "key": "value"
                    },
                    {
                        "key": "value"
                    }
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
        #[should_panic(
            expected = "not yet implemented: TODO: <https://github.com/Talus-Network/nexus-next/issues/30>"
        )]
        fn test_dag_data_only_supports_inline_ser() {
            let data = NexusData::Remote {};
            let _ = serde_json::to_string(&data);
        }

        #[test]
        #[should_panic(
            expected = "not yet implemented: TODO: <https://github.com/Talus-Network/nexus-next/issues/30>"
        )]
        fn test_dag_data_only_supports_inline_deser() {
            let data = r#"{"storage":[1,2,3],"one":[],"many":[[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]],"encrypted":false}"#;
            let _ = serde_json::from_str::<NexusData>(&data);
        }
    }
}
