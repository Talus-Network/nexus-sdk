pub(crate) use {
    crate::error::NexusCliError,
    anyhow::{anyhow, bail, Result as AnyResult},
    clap::{builder::ValueParser, Args, Parser, Subcommand, ValueEnum},
    colored::Colorize,
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum SuiNet {
    Localnet,
    Testnet,
    Mainnet,
}

impl std::fmt::Display for SuiNet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuiNet::Localnet => write!(f, "localnet"),
            SuiNet::Testnet => write!(f, "testnet"),
            SuiNet::Mainnet => write!(f, "mainnet"),
        }
    }
}

pub(crate) mod sui {

    pub(crate) use {
        move_core_types::{
            identifier::IdentStr as MoveIdentStr,
            language_storage::{StructTag as MoveStructTag, TypeTag as MoveTypeTag},
        },
        sui_sdk::{
            rpc_types::{
                Coin,
                EventFilter,
                EventPage,
                SuiEvent as Event,
                SuiObjectData as ObjectData,
                SuiObjectDataFilter as ObjectDataFilter,
                SuiObjectDataOptions as ObjectDataOptions,
                SuiObjectRef as ObjectRef,
                SuiObjectResponse as ObjectResponse,
                SuiObjectResponseQuery as ObjectResponseQuery,
                SuiParsedData as ParsedData,
                SuiTransactionBlockEffects as TransactionBlockEffects,
                SuiTransactionBlockResponseOptions as TransactionBlockResponseOptions,
            },
            types::{
                base_types::{ObjectID, SequenceNumber, SuiAddress as Address},
                crypto::SignatureScheme,
                dynamic_field::{DynamicFieldInfo, DynamicFieldName},
                event::EventID,
                gas_coin::MIST_PER_SUI,
                id::UID,
                object::Owner,
                programmable_transaction_builder::ProgrammableTransactionBuilder,
                quorum_driver_types::ExecuteTransactionRequestType,
                transaction::{ObjectArg, TransactionData},
                MOVE_STDLIB_PACKAGE_ID,
            },
            wallet_context::WalletContext,
            SuiClient as Client,
            SuiClientBuilder as ClientBuilder,
        },
    };
}

// == Used by clap ==

/// Expands `~/` to the user's home directory in path arguments.
pub(crate) fn expand_tilde(path: &str) -> AnyResult<std::path::PathBuf> {
    if path.starts_with("~/") {
        match home::home_dir() {
            Some(home) => return Ok(home.join(&path[2..])),
            None => return Err(anyhow!("Could not find home directory")),
        }
    }

    Ok(path.into())
}
