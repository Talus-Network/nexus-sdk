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

// TODO: Remove all old Sui SDK re-exports once they're gone from test utils.
#[cfg(feature = "test_utils")]
pub use {
    move_core_types::{
        account_address::AccountAddress as MoveAccountAddress,
        ident_str as move_ident_str,
        identifier::{IdentStr as MoveIdentStr, Identifier as MoveIdentifier},
        language_storage::{
            ModuleId as MoveModuleId,
            StructTag as MoveStructTag,
            TypeTag as MoveTypeTag,
        },
        u256::U256 as MoveU256,
    },
    shared_crypto::intent::Intent,
    sui_config::{
        sui_config_dir as config_dir,
        Config,
        PersistedConfig,
        SUI_CLIENT_CONFIG as CLIENT_CONFIG,
        SUI_KEYSTORE_FILENAME as KEYSTORE_FILENAME,
    },
    sui_keys::keystore::AccountKeystore,
    sui_keys::{
        key_derive::generate_new_key,
        keystore::{FileBasedKeystore, Keystore},
    },
    sui_sdk::{
        error::Error,
        json::SuiJsonValue,
        rpc_types::{
            BalanceChange,
            BcsEvent,
            Coin,
            EventFilter,
            EventPage,
            ObjectChange,
            OwnedObjectRef,
            SuiEvent as Event,
            SuiExecutionStatus as ExecutionStatus,
            SuiMoveNormalizedFunction as MoveNormalizedFunction,
            SuiMoveNormalizedModule as MoveNormalizedModule,
            SuiMoveNormalizedType as MoveNormalizedType,
            SuiMoveStruct as MoveStruct,
            SuiMoveValue as MoveValue,
            SuiObjectData as ObjectData,
            SuiObjectDataFilter as ObjectDataFilter,
            SuiObjectDataOptions as ObjectDataOptions,
            SuiObjectRef as ObjectRef,
            SuiObjectResponse as ObjectResponse,
            SuiObjectResponseQuery as ObjectResponseQuery,
            SuiParsedData as ParsedData,
            SuiParsedMoveObject as ParsedMoveObject,
            SuiTransactionBlockEffects as TransactionBlockEffects,
            SuiTransactionBlockEffectsAPI,
            SuiTransactionBlockEffectsV1 as TransactionBlockEffectsV1,
            SuiTransactionBlockEvents as TransactionBlockEvents,
            SuiTransactionBlockResponse as TransactionBlockResponse,
            SuiTransactionBlockResponseOptions as TransactionBlockResponseOptions,
        },
        sui_client_config::{SuiClientConfig as ClientConfig, SuiEnv as Env},
        types::{
            base_types::{ObjectID, SequenceNumber, SuiAddress as Address},
            crypto::{SignatureScheme, SuiKeyPair as KeyPair},
            digests::{ObjectDigest, TransactionDigest},
            dynamic_field::{DynamicFieldInfo, DynamicFieldName},
            event::EventID,
            gas::GasCostSummary,
            gas_coin::MIST_PER_SUI,
            id::UID,
            object::Owner,
            programmable_transaction_builder::ProgrammableTransactionBuilder,
            quorum_driver_types::ExecuteTransactionRequestType,
            transaction::{
                Argument,
                CallArg,
                Command,
                ObjectArg,
                ProgrammableTransaction,
                Transaction,
                TransactionData,
            },
            type_input::{StructInput as MoveStructInput, TypeInput as MoveTypeInput},
            Identifier,
            MOVE_STDLIB_PACKAGE_ID,
            SUI_CLOCK_OBJECT_ID as CLOCK_OBJECT_ID,
            SUI_CLOCK_OBJECT_SHARED_VERSION as CLOCK_OBJECT_SHARED_VERSION,
            SUI_FRAMEWORK_PACKAGE_ID as FRAMEWORK_PACKAGE_ID,
        },
        wallet_context::WalletContext,
        SuiClient as Client,
        SuiClientBuilder as ClientBuilder,
        SUI_DEVNET_URL as DEVNET_URL,
        SUI_LOCAL_NETWORK_URL as LOCAL_NETWORK_URL,
        SUI_MAINNET_URL as MAINNET_URL,
        SUI_TESTNET_URL as TESTNET_URL,
    },
};
