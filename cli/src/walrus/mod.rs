//! CLI arguments, local files and presentation for the SDK storage client.

pub(crate) mod receipt;

use {
    crate::{
        display::{human_output, json_output},
        prelude::*,
    },
    anyhow::{ensure, Context as _},
    nexus_sdk::{
        execution_limits::MAX_RESOLVED_INPUT_BYTES,
        nexus::wallet::WalletClient,
        walrus::{
            UploadOptions,
            WalrusNetwork,
            WalrusReader,
            WalrusReference,
            WalrusStorage,
            WalrusUploadData,
        },
    },
    receipt::{atomic_write, Journal},
    serde_json::Value,
    std::path::Path,
    tokio::io::AsyncReadExt as _,
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
    fn options(&self, settings: &Settings, deletable: bool) -> UploadOptions {
        UploadOptions {
            epochs: self.epochs.unwrap_or(settings.epochs),
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

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) network: WalrusNetwork,
    pub(crate) chain_id: String,
    rpc_url: String,
    aggregator: String,
    epochs: u32,
}

impl Settings {
    pub(crate) async fn load(conf: &CliConf) -> AnyResult<Self> {
        let rpc_url = std::env::var("SUI_RPC_URL")
            .ok()
            .or_else(|| conf.sui.rpc_url.as_ref().map(ToString::to_string))
            .context("Set the Sui RPC URL with nexus conf set --sui.rpc-url")?;
        let info = sui::grpc::client(&rpc_url)?
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await?
            .into_inner();
        Self::new(
            &conf.data_storage,
            rpc_url,
            info.chain_id.context("Sui RPC omitted its chain ID")?,
            info.chain
                .as_deref()
                .context("Sui RPC omitted its network")?,
        )
    }

    pub(crate) fn for_wallet(conf: &DataStorageConf, wallet: &WalletClient) -> AnyResult<Self> {
        Self::new(
            conf,
            wallet.rpc_url().into(),
            wallet.chain_id().into(),
            wallet.chain(),
        )
    }

    fn new(
        conf: &DataStorageConf,
        rpc_url: String,
        chain_id: String,
        chain: &str,
    ) -> AnyResult<Self> {
        let network = chain.parse()?;
        let aggregator = aggregator_for(conf, network)?;
        let epochs = u32::from(conf.walrus_save_for_epochs.unwrap_or(2));
        ensure!(
            (1..=53).contains(&epochs),
            "Walrus epochs must be between 1 and 53"
        );
        WalrusReader::new(&aggregator, MAX_RESOLVED_INPUT_BYTES)?;
        Ok(Self {
            network,
            chain_id,
            rpc_url,
            aggregator,
            epochs,
        })
    }

    pub(crate) fn reader(&self) -> AnyResult<WalrusReader> {
        WalrusReader::new(&self.aggregator, MAX_RESOLVED_INPUT_BYTES)
    }

    async fn wallet(&self, conf: &CliConf) -> AnyResult<WalletClient> {
        let wallet =
            WalletClient::connect(&self.rpc_url, crate::sui::get_signing_key(conf).await?).await?;
        ensure!(
            wallet.chain_id() == self.chain_id,
            "Sui network changed while loading the wallet"
        );
        Ok(wallet)
    }

    pub(crate) async fn verify(&self, reference: &WalrusReference) -> AnyResult<Vec<Vec<u8>>> {
        reference.check_network(self.network, &self.chain_id)?;
        reference.download(&self.reader()?).await
    }
}

fn aggregator_for(conf: &DataStorageConf, network: WalrusNetwork) -> AnyResult<String> {
    let Some(url) = &conf.walrus_aggregator_url else {
        return Ok(network.aggregator_url().into());
    };
    if [WalrusNetwork::Testnet, WalrusNetwork::Mainnet]
        .iter()
        .any(|net| url.as_str().trim_end_matches('/') == net.aggregator_url())
    {
        return Ok(network.aggregator_url().into());
    }
    ensure!(conf.walrus_network == Some(network), "Custom aggregator requires a matching network; use nexus walrus configure --aggregator URL --network testnet|mainnet");
    Ok(url.to_string())
}

pub(crate) async fn handle(command: WalrusCommand) -> AnyResult<(), NexusCliError> {
    run(command).await.map_err(NexusCliError::Any)
}

async fn run(command: WalrusCommand) -> AnyResult<()> {
    let mut conf = load_conf().await?;
    if let WalrusCommand::Configure {
        epochs,
        aggregator,
        network,
        reset_aggregator,
    } = command
    {
        if let Some(epochs) = epochs {
            conf.data_storage.walrus_save_for_epochs = Some(epochs);
        }
        if let Some(url) = aggregator {
            WalrusReader::new(url.as_str(), MAX_RESOLVED_INPUT_BYTES)?;
            conf.data_storage.walrus_aggregator_url = Some(url);
            conf.data_storage.walrus_network = network;
        }
        if reset_aggregator {
            conf.data_storage.walrus_aggregator_url = None;
            conf.data_storage.walrus_network = None;
        }
        conf.save().await?;
        return print_value(&conf.data_storage);
    }
    let settings = Settings::load(&conf).await?;
    match command {
        WalrusCommand::Status => {
            let owner = if conf.sui.pk.is_some() || std::env::var_os("SUI_PK").is_some() {
                Some(
                    crate::sui::get_signing_key(&conf)
                        .await?
                        .public_key()
                        .derive_address(),
                )
            } else {
                None
            };
            print_value(
                &json!({"network": settings.network, "chain_id": settings.chain_id, "payer": owner,
                "aggregator": settings.aggregator, "epochs": settings.epochs, "max_input_bytes": MAX_RESOLVED_INPUT_BYTES}),
            )?;
        }
        WalrusCommand::Upload {
            file,
            storage,
            many,
            deletable,
            estimate,
            resume,
            out,
        } => {
            let data = WalrusUploadData::from_json_document(
                read_bounded(&file, MAX_RESOLVED_INPUT_BYTES).await?,
                many,
            )?;
            let uploader =
                Uploader::new(settings.wallet(&conf).await?, settings, storage, deletable).await?;
            let out =
                out.unwrap_or_else(|| PathBuf::from(format!("{}.walrus.json", file.display())));
            if estimate {
                print_value(&uploader.estimate(&data).await?)?;
            } else {
                let reference = uploader.upload(&data, &out, resume).await?;
                human_output(&format!("Saved Walrus reference: {}", out.display()));
                json_output(&json!({"reference": out, "receipt": reference}))?;
            }
        }
        WalrusCommand::Inspect { reference } => {
            let saved = receipt::load(&reference).await?;
            settings.verify(&saved).await?;
            print_value(&json!({"reference": saved, "content_verified": true}))?;
        }
        WalrusCommand::Download { reference, out } => {
            let saved = receipt::load(&reference).await?;
            let values = settings.verify(&saved).await?;
            let bytes = if saved.many {
                serde_json::to_vec(
                    &values
                        .iter()
                        .map(|bytes| serde_json::from_slice::<Value>(bytes))
                        .collect::<Result<Vec<_>, _>>()?,
                )?
            } else {
                values
                    .into_iter()
                    .next()
                    .context("reference contains no blob")?
            };
            atomic_write(&out, &bytes, false)?;
            human_output(&format!("Verified data saved to {}", out.display()));
            json_output(&json!({"out": out, "bytes": bytes.len(), "verified": true}))?;
        }
        WalrusCommand::List { include_expired } => {
            let client =
                WalrusStorage::new(settings.wallet(&conf).await?, Some(&settings.aggregator))
                    .await?;
            print_value(&client.list(include_expired).await?)?;
        }
        WalrusCommand::Extend {
            reference,
            epochs,
            max_storage_cost_frost,
            storage_gas_budget,
        } => {
            let mut saved = receipt::load(&reference).await?;
            saved.check_network(settings.network, &settings.chain_id)?;
            let client =
                WalrusStorage::new(settings.wallet(&conf).await?, Some(&settings.aggregator))
                    .await?;
            for index in 0..saved.blobs.len() {
                saved.blobs[index] = client
                    .extend(
                        &saved.blobs[index],
                        epochs,
                        max_storage_cost_frost,
                        storage_gas_budget,
                    )
                    .await?;
                receipt::save(&saved, &reference, true)?;
            }
            print_value(&saved)?;
        }
        WalrusCommand::Delete {
            reference,
            yes: _,
            storage_gas_budget,
        } => {
            let saved = receipt::load(&reference).await?;
            saved.check_network(settings.network, &settings.chain_id)?;
            let client =
                WalrusStorage::new(settings.wallet(&conf).await?, Some(&settings.aggregator))
                    .await?;
            for blob in &saved.blobs {
                ensure!(
                    client.inspect(blob).await?.deletable,
                    "permanent storage cannot be deleted"
                );
            }
            for blob in &saved.blobs {
                client.delete(blob, storage_gas_budget).await?;
            }
            print_value(&json!({"reference": reference, "deleted": true}))?;
        }
        WalrusCommand::Configure { .. } => unreachable!(),
    }
    Ok(())
}

pub(crate) async fn load_conf() -> AnyResult<CliConf> {
    match CliConf::load().await {
        Ok(conf) => Ok(conf),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(CliConf::default())
        }
        Err(error) => Err(error),
    }
}

fn print_value(value: &impl Serialize) -> AnyResult<()> {
    human_output(&serde_json::to_string_pretty(value)?);
    json_output(value)?;
    Ok(())
}

pub(crate) async fn read_bounded(path: &Path, max: usize) -> AnyResult<Vec<u8>> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("cannot read {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes).await?;
    ensure!(
        bytes.len() <= max,
        "{} exceeds the {max} byte limit",
        path.display()
    );
    Ok(bytes)
}

pub(crate) struct Uploader {
    settings: Settings,
    client: WalrusStorage,
    options: UploadOptions,
}

impl Uploader {
    pub(crate) async fn new(
        wallet: WalletClient,
        settings: Settings,
        args: UploadArgs,
        deletable: bool,
    ) -> AnyResult<Self> {
        let options = args.options(&settings, deletable);
        let client = WalrusStorage::new(wallet, Some(&settings.aggregator)).await?;
        Ok(Self {
            settings,
            client,
            options,
        })
    }

    async fn estimate(&self, data: &WalrusUploadData) -> AnyResult<Value> {
        let mut quotes = Vec::new();
        for bytes in data.values() {
            quotes.push(
                self.client
                    .prepare(bytes.clone(), self.options.clone())
                    .await?
                    .quote()
                    .clone(),
            );
        }
        Ok(
            json!({"network": self.settings.network, "payer": self.client.wallet().owner(), "quotes": quotes, "includes_sui_gas": false}),
        )
    }

    pub(crate) async fn upload(
        &self,
        data: &WalrusUploadData,
        out: &Path,
        resume: bool,
    ) -> AnyResult<WalrusReference> {
        if resume && out.exists() {
            let saved = receipt::load(out).await?;
            let bytes = self.settings.verify(&saved).await?;
            ensure!(
                saved.many == data.is_many() && bytes == data.values(),
                "saved reference differs from this upload"
            );
            return Ok(saved);
        }
        ensure!(
            !out.exists(),
            "reference already exists; reuse it with --input-ref or choose another destination"
        );
        let recovery = out.with_extension("uploads");
        if resume {
            ensure!(
                recovery.is_dir(),
                "upload recovery directory does not exist"
            );
        } else {
            std::fs::create_dir_all(recovery.parent().unwrap_or_else(|| Path::new(".")))?;
            std::fs::create_dir(&recovery)?;
        }
        use sha2::{Digest as _, Sha256};
        let manifest = json!({"version":1, "network":self.settings.network, "chain_id":self.settings.chain_id,
            "owner":self.client.wallet().owner(), "many":data.is_many(), "options":self.options,
            "digests":data.values().iter().map(|bytes| hex::encode(Sha256::digest(bytes))).collect::<Vec<_>>()});
        let manifest_path = recovery.join("manifest.json");
        if resume {
            let saved: Value =
                serde_json::from_slice(&read_bounded(&manifest_path, 256 * 1024).await?)?;
            ensure!(saved == manifest, "upload recovery requires the same contents, cardinality, wallet, network and options");
        } else {
            atomic_write(
                &manifest_path,
                &serde_json::to_vec_pretty(&manifest)?,
                false,
            )?;
        }
        let mut reference = WalrusReference {
            version: 1,
            network: self.settings.network,
            chain_id: self.settings.chain_id.clone(),
            many: data.is_many(),
            blobs: Vec::new(),
        };
        for (index, bytes) in data.values().iter().enumerate() {
            let path = recovery.join(format!("{index}.json"));
            let prepared = self
                .client
                .prepare(bytes.clone(), self.options.clone())
                .await?;
            let mut journal = if resume && path.exists() {
                Journal::load(&path).await?
            } else {
                let journal = Journal {
                    registration: self.client.registration(&prepared).await?,
                    pending: None,
                    stored: None,
                };
                journal.save(&path, false)?;
                journal
            };
            journal.check(bytes)?;
            ensure!(
                journal.registration.network == reference.network
                    && journal.registration.chain_id == reference.chain_id,
                "upload recovery belongs to another network"
            );
            if journal.stored.is_none() {
                if journal.pending.is_none() {
                    journal.pending = Some(self.client.register(journal.registration.clone()).await
                        .with_context(|| format!("registration unresolved; resume the same upload using --resume; recovery: {}", recovery.display()))?);
                    journal.save(&path, true)?;
                }
                journal.stored = Some(
                    self.client
                        .finish(prepared, journal.pending.as_ref().unwrap())
                        .await
                        .with_context(|| {
                            format!(
                                "upload unfinished; use --resume; paid registration: {}",
                                path.display()
                            )
                        })?,
                );
                journal.save(&path, true)?;
            }
            let blob = journal.stored.context("upload has no stored reference")?;
            blob.verify_bytes(bytes)?;
            reference.blobs.push(blob);
        }
        // Verify current availability even when all entries came from recovery.
        self.settings.verify(&reference).await?;
        receipt::save(&reference, out, false)?;
        Ok(reference)
    }
}

/// Adds local artifact paths to the normal task receipt for machine consumers.
pub(crate) fn print_task_receipt(
    receipt: &impl Serialize,
    references: &std::collections::BTreeMap<String, PathBuf>,
) -> Result<(), NexusCliError> {
    let mut value =
        serde_json::to_value(receipt).map_err(|error| NexusCliError::Any(error.into()))?;
    if !references.is_empty() {
        value["walrus_references"] =
            serde_json::to_value(references).map_err(|error| NexusCliError::Any(error.into()))?;
    }
    json_output(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_aggregators_follow_the_rpc_and_custom_readers_are_network_bound() {
        let mut conf = DataStorageConf {
            walrus_aggregator_url: Some(WalrusNetwork::Testnet.aggregator_url().parse().unwrap()),
            ..Default::default()
        };
        assert_eq!(
            aggregator_for(&conf, WalrusNetwork::Mainnet).unwrap(),
            WalrusNetwork::Mainnet.aggregator_url()
        );
        conf.walrus_aggregator_url = Some("https://storage.example".parse().unwrap());
        assert!(aggregator_for(&conf, WalrusNetwork::Mainnet).is_err());
        conf.walrus_network = Some(WalrusNetwork::Mainnet);
        assert_eq!(
            aggregator_for(&conf, WalrusNetwork::Mainnet).unwrap(),
            "https://storage.example/"
        );
    }

    #[tokio::test]
    async fn reference_reads_need_no_wallet_and_reject_a_different_network() {
        use sha2::{Digest as _, Sha256};
        let mut server = mockito::Server::new_async().await;
        let bytes = b"  \"public data\"\n";
        let blob_id = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let get = server
            .mock("GET", format!("/v1/blobs/{blob_id}").as_str())
            .with_body(bytes)
            .expect(1)
            .create_async()
            .await;
        let settings = Settings {
            network: WalrusNetwork::Testnet,
            chain_id: "chain".into(),
            rpc_url: "unused".into(),
            aggregator: server.url(),
            epochs: 2,
        };
        let reference = WalrusReference {
            version: 1,
            network: WalrusNetwork::Testnet,
            chain_id: "chain".into(),
            many: false,
            blobs: vec![nexus_sdk::walrus::StoredBlob {
                blob_id: blob_id.into(),
                sha256: hex::encode(Sha256::digest(bytes)),
                size: bytes.len(),
                object_id: sui::types::Address::TWO,
                owner: sui::types::Address::TWO,
                end_epoch: 10,
                deletable: false,
            }],
        };
        assert_eq!(
            settings.verify(&reference).await.unwrap(),
            vec![bytes.to_vec()]
        );
        assert!(settings
            .verify(&WalrusReference {
                network: WalrusNetwork::Mainnet,
                ..reference
            })
            .await
            .is_err());
        get.assert_async().await;
    }
}
