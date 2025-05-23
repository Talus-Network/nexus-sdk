//! [`NexusData`] is a wrapper around any raw data stored on-chain. This can be
//! data for input ports, output ports or default values. It is represented as
//! an enum because default values can be stored remotely.
//!
//! See: <https://github.com/Talus-Network/nexus-next/issues/30>
//!
//! The [`NexusData::data`] field is a byte array on-chain but we assume that,
//! upon decoding it, it will be a valid JSON object.

use {
    crate::types::{
        deserialize_array_of_bytes_to_json_value,
        serialize_json_value_to_array_of_bytes,
    },
    serde::{Deserialize, Serialize},
};

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
    //! `{ storage: u8[], data: u8[][] }`.
    //!
    //! However, storage has a special value [NEXUS_DATA_INLINE_STORAGE_TAG].
    //! Therefore we represent [NexusData] as an enum within the codebase.

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
        #[serde(
            deserialize_with = "deserialize_array_of_bytes_to_json_value",
            serialize_with = "serialize_json_value_to_array_of_bytes"
        )]
        data: serde_json::Value,
        encrypted: bool,
    }

    pub(super) fn deserialize_onchain_repr_to_enum<'de, D>(
        deserializer: D,
    ) -> Result<NexusData, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data: NexusDataAsStruct = Deserialize::deserialize(deserializer)?;

        match data.storage.as_ref() {
            NEXUS_DATA_INLINE_STORAGE_TAG => Ok(NexusData::Inline {
                data: data.data,
                encrypted: data.encrypted,
            }),
            _ => todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>"),
        }
    }

    pub(super) fn serialize_enum_to_onchain_repr<S>(
        value: &NexusData,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let data = match value {
            NexusData::Inline { data, encrypted } => NexusDataAsStruct {
                storage: NEXUS_DATA_INLINE_STORAGE_TAG.to_vec(),
                data: data.clone(),
                encrypted: *encrypted,
            },
            NexusData::Remote {} => {
                todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
            }
        };

        data.serialize(serializer)
    }

    impl<'de> Deserialize<'de> for NexusData {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            parser::deserialize_onchain_repr_to_enum(deserializer)
        }
    }

    impl Serialize for NexusData {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            parser::serialize_enum_to_onchain_repr(self, serializer)
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

            assert_eq!(
                serialized,
                r#"{"storage":[105,110,108,105,110,101],"data":[[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]],"encrypted":false}"#
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
                r#"{"storage":[105,110,108,105,110,101],"data":[[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125]],"encrypted":false}"#
            );

            let deserialized = serde_json::from_str(&serialized).unwrap();

            assert_eq!(dag_data, deserialized);
        }
    }
}
