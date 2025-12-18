//! This module attempts to make a little bit of sense when dealing with Sui
//! types.
//!
//! This way we can use, for example `sui::types::Address` in our code.

pub mod tx {
    pub use sui_transaction_builder::{unresolved::*, *};
}

pub mod types {
    pub use sui_sdk_types::*;
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

/// Move build and package management re-exported for testing.
#[cfg(feature = "test_utils")]
pub mod build {
    pub use {
        move_package::lock_file::{
            schema::{update_managed_address, ManagedAddressUpdate},
            LockFile,
        },
        sui_move_build::{implicit_deps, BuildConfig},
        sui_package_management::system_package_versions::latest_system_packages,
    };
}
