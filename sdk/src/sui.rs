//! This module attempts to make a little bit of sense when dealing with Sui
//! types.
//!
//! Some are prefixed with `Sui`, some are not. This re-export will nest all
//! Sui types under `predule::sui` and remove all `Sui` prefixes.
//!
//! This way we can use, for example `sui::ObjectID` in our code.
//!
//! All move types are now also prefixed with `Move` to avoid confusion.

pub use {
    move_core_types::{
        ident_str as move_ident_str,
        identifier::IdentStr as MoveIdentStr,
        language_storage::{StructTag as MoveStructTag, TypeTag as MoveTypeTag},
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
    sui_keys::{
        key_derive::generate_new_key,
        keystore::{FileBasedKeystore, Keystore},
    },
    sui_sdk::{
        error::Error,
        json::SuiJsonValue,
        rpc_types::{
            BcsEvent,
            Coin,
            EventFilter,
            EventPage,
            ObjectChange,
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
            SuiTransactionBlockResponse as TransactionBlockResponse,
            SuiTransactionBlockResponseOptions as TransactionBlockResponseOptions,
        },
        sui_client_config::{SuiClientConfig as ClientConfig, SuiEnv as Env},
        types::{
            base_types::{ObjectID, SequenceNumber, SuiAddress as Address},
            crypto::SignatureScheme,
            digests::{ObjectDigest, TransactionDigest},
            dynamic_field::{DynamicFieldInfo, DynamicFieldName},
            event::EventID,
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

/// Sui traits re-exported so that we can `use sui::traits::*` in our code.
pub mod traits {
    pub use {
        sui_config::Config,
        sui_keys::keystore::AccountKeystore,
        sui_sdk::rpc_types::SuiTransactionBlockEffectsAPI as TransactionBlockEffectsAPI,
    };
}

/// Use this whenever you need da Clock (=`0x06`) as Tx argument.
pub const CLOCK_OBJ_ARG: ObjectArg = ObjectArg::SharedObject {
    id: CLOCK_OBJECT_ID,
    initial_shared_version: CLOCK_OBJECT_SHARED_VERSION,
    mutable: false,
};
