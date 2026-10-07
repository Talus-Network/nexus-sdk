//! Persist upload progress so the same payment can be resumed after interruption.

use {
    super::{
        args::UploadArgs,
        receipt::{self, atomic_write, read_bounded, Journal},
        settings::Settings,
    },
    crate::prelude::*,
    anyhow::{ensure, Context as _},
    nexus_sdk::{
        nexus::wallet::WalletClient,
        walrus::{UploadOptions, WalrusReference, WalrusStorage, WalrusUploadData},
    },
    serde_json::Value,
    std::path::Path,
};

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
        let options = args.options(settings.epochs, deletable);
        let client = WalrusStorage::new(wallet, Some(&settings.aggregator)).await?;
        Ok(Self {
            settings,
            client,
            options,
        })
    }

    pub(super) async fn estimate(&self, data: &WalrusUploadData) -> AnyResult<Value> {
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
