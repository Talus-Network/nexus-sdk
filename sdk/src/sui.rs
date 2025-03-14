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
    },
    shared_crypto::intent::Intent,
    sui_keys::{key_derive::generate_new_key, keystore::Keystore},
    sui_sdk::{
        error::Error,
        rpc_types::{
            Coin,
            EventFilter,
            EventPage,
            ObjectChange,
            SuiEvent as Event,
            SuiExecutionStatus as ExecutionStatus,
            SuiObjectData as ObjectData,
            SuiObjectDataFilter as ObjectDataFilter,
            SuiObjectDataOptions as ObjectDataOptions,
            SuiObjectRef as ObjectRef,
            SuiObjectResponse as ObjectResponse,
            SuiObjectResponseQuery as ObjectResponseQuery,
            SuiParsedData as ParsedData,
            SuiTransactionBlockEffects as TransactionBlockEffects,
            SuiTransactionBlockResponse as TransactionBlockResponse,
            SuiTransactionBlockResponseOptions as TransactionBlockResponseOptions,
        },
        types::{
            base_types::{ObjectID, SequenceNumber, SuiAddress as Address},
            crypto::SignatureScheme,
            digests::TransactionDigest,
            dynamic_field::{DynamicFieldInfo, DynamicFieldName},
            event::EventID,
            gas_coin::MIST_PER_SUI,
            id::UID,
            object::Owner,
            programmable_transaction_builder::ProgrammableTransactionBuilder,
            quorum_driver_types::ExecuteTransactionRequestType,
            transaction::{Argument, ObjectArg, Transaction, TransactionData},
            Identifier,
            MOVE_STDLIB_PACKAGE_ID,
            SUI_CLOCK_OBJECT_ID as CLOCK_OBJECT_ID,
            SUI_CLOCK_OBJECT_SHARED_VERSION as CLOCK_OBJECT_SHARED_VERSION,
            SUI_FRAMEWORK_PACKAGE_ID as FRAMEWORK_PACKAGE_ID,
        },
        wallet_context::WalletContext,
        SuiClient as Client,
        SuiClientBuilder as ClientBuilder,
    },
};

/// Sui traits re-exported so that we can `use sui::traits::*` in our code.
pub mod traits {
    pub use {
        sui_keys::keystore::AccountKeystore,
        sui_sdk::rpc_types::SuiTransactionBlockEffectsAPI as TransactionBlockEffectsAPI,
    };
}
