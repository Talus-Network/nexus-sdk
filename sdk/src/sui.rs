//! This module attempts to make a little bit of sense when dealing with Sui
//! types.
//!
//! This way we can use, for example `sui::types::Address` in our code.

pub mod tx {
    pub use sui_transaction_builder::*;
}

pub mod types {
    pub use sui_sdk_types::*;
    use {
        crate::types::{parse_u64_value, strip_fields_owned},
        serde::{de::Error as _, Deserialize, Serialize},
    };

    #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
    pub struct SuiBalance {
        pub value: u64,
    }

    impl<'de> Deserialize<'de> for SuiBalance {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            if !deserializer.is_human_readable() {
                #[derive(Deserialize)]
                struct RawBalance {
                    value: u64,
                }

                return RawBalance::deserialize(deserializer).map(|balance| Self {
                    value: balance.value,
                });
            }

            let value = strip_fields_owned(serde_json::Value::deserialize(deserializer)?);
            if let Some(parsed) = parse_u64_value(&value).map_err(D::Error::custom)? {
                return Ok(Self { value: parsed });
            }

            let parsed = value
                .as_object()
                .and_then(|object| object.get("value"))
                .and_then(|value| parse_u64_value(value).ok().flatten())
                .ok_or_else(|| D::Error::custom("missing SUI balance value"))?;

            Ok(Self { value: parsed })
        }
    }
}

pub mod crypto {
    pub use sui_crypto::{ed25519::Ed25519PrivateKey, *};
}

pub mod grpc {
    pub use sui_rpc::{field::FieldMask, proto::sui::rpc::v2::*, Client};
}

/// Sui traits re-exported so that we can `use sui::traits::*` in our code.
pub mod traits {
    pub use {sui_crypto::SuiSigner, sui_rpc::field::FieldMaskUtil, sui_sdk_types::bcs::ToBcs};
}

pub const MIST_PER_SUI: u64 = 1_000_000_000;

/// Move build support for production package publishing and tests.
#[cfg(any(feature = "move_publish", feature = "test_utils"))]
pub mod build {
    pub use {
        move_package_alt::schema::Environment,
        sui_move_build::{BuildConfig, CompiledPackage},
    };
}
