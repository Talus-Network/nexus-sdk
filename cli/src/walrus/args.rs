//! Command arguments and their SDK upload options.

use {
    crate::prelude::*,
    nexus_sdk::walrus::{UploadOptions, WalrusNetwork},
};

#[derive(Args, Clone, Debug)]
pub(crate) struct UploadArgs {
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=53))]
    pub(crate) epochs: Option<u32>,
    /// Maximum WAL storage cost per blob, in FROST. Defaults to the current quote.
    #[arg(long)]
    pub(crate) max_storage_cost_frost: Option<u64>,
    /// Maximum Sui gas per storage transaction, in MIST.
    #[arg(long, default_value_t = DEFAULT_GAS_BUDGET)]
    pub(crate) storage_gas_budget: u64,
}

impl Default for UploadArgs {
    fn default() -> Self {
        Self {
            epochs: None,
            max_storage_cost_frost: None,
            storage_gas_budget: DEFAULT_GAS_BUDGET,
        }
    }
}

impl UploadArgs {
    pub(super) fn options(&self, default_epochs: u32, deletable: bool) -> UploadOptions {
        UploadOptions {
            epochs: self.epochs.unwrap_or(default_epochs),
            deletable,
            max_storage_cost_frost: self.max_storage_cost_frost,
            gas_budget_mist: self.storage_gas_budget,
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum WalrusCommand {
    /// Show the storage network, wallet address, aggregator and defaults.
    Status,
    /// Set local storage defaults. The network follows the active Sui RPC.
    Configure {
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=53))]
        epochs: Option<u8>,
        #[arg(long, requires = "network")]
        aggregator: Option<reqwest::Url>,
        #[arg(long, requires = "aggregator", value_name = "testnet|mainnet")]
        network: Option<WalrusNetwork>,
        #[arg(long, conflicts_with_all = ["aggregator", "network"])]
        reset_aggregator: bool,
    },
    /// Upload JSON using the Nexus wallet and save a reusable reference.
    Upload {
        #[arg(value_parser = ValueParser::from(expand_tilde))]
        file: PathBuf,
        #[command(flatten)]
        storage: UploadArgs,
        /// Treat an array as a Many port, uploading each element separately.
        #[arg(long)]
        many: bool,
        /// Permit the owner to delete this storage before expiry.
        #[arg(long)]
        deletable: bool,
        /// Display quotes without submitting transactions.
        #[arg(long, conflicts_with = "resume")]
        estimate: bool,
        /// Resume the saved registration rather than buying storage again.
        #[arg(long)]
        resume: bool,
        /// Reference destination; defaults to FILE.walrus.json.
        #[arg(long, value_parser = ValueParser::from(expand_tilde))]
        out: Option<PathBuf>,
    },
    /// Show saved metadata and verify current content and its digest.
    Inspect { reference: PathBuf },
    /// Download and verify data without a signing key.
    Download {
        reference: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// List Blob objects owned by the active wallet, with expiry and certification.
    List {
        #[arg(long)]
        include_expired: bool,
    },
    /// Extend owned storage and refresh its reference from chain.
    Extend {
        reference: PathBuf,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=53))]
        epochs: u32,
        /// Maximum WAL storage cost per blob, in FROST.
        #[arg(long)]
        max_storage_cost_frost: u64,
        #[arg(long, default_value_t = DEFAULT_GAS_BUDGET)]
        storage_gas_budget: u64,
    },
    /// Delete owned deletable Blob objects. Tasks may lose their data.
    Delete {
        reference: PathBuf,
        #[arg(long, required = true)]
        yes: bool,
        #[arg(long, default_value_t = DEFAULT_GAS_BUDGET)]
        storage_gas_budget: u64,
    },
}
