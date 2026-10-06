//! Native Walrus storage through a shared Nexus wallet. Walrus supplies encoding,
//! node communication and Move calls; the Nexus wallet signs every transaction.

use {
    super::{StoredBlob, WalrusNetwork, WalrusReader},
    crate::{
        execution_limits::MAX_RESOLVED_DATA_BYTES,
        nexus::wallet::WalletClient,
        sui::{self, traits::*},
    },
    anyhow::{ensure, Context as _},
    serde::{Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::sync::Arc,
    walrus_sdk::{
        config::ClientConfig,
        core::{
            encoding::{EncodingConfig, EncodingFactory as _, SliverPair},
            metadata::{BlobMetadataApi as _, VerifiedBlobMetadataWithId},
            DEFAULT_ENCODING,
        },
        node_client::WalrusNodeClient,
        uploader::TailHandling,
    },
    walrus_sui::{
        client::{
            contract_config::ContractConfig,
            transaction_builder::WalrusPtbBuilder,
            BlobObjectMetadata,
            BlobPersistence,
            ReadClient as _,
            SuiReadClient,
        },
        contracts::AssociatedContractStruct as _,
        types::move_structs::Blob,
        utils::storage_units_from_size,
    },
};

/// Explicit upload policy supplied by a user or a tool operator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadOptions {
    pub epochs: u32,
    pub deletable: bool,
    /// Maximum estimated WAL storage cost per blob, in FROST. None skips the quote check.
    /// Actual charges use the prices at execution.
    pub max_storage_cost_frost: Option<u64>,
    /// Maximum Sui gas per transaction. Registration and certification are separate.
    pub gas_budget_mist: u64,
}

impl Default for UploadOptions {
    fn default() -> Self {
        Self {
            epochs: 2,
            deletable: false,
            max_storage_cost_frost: None,
            gas_budget_mist: crate::nexus::client::DEFAULT_GAS_BUDGET,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadQuote {
    pub blob_id: String,
    pub size: usize,
    pub encoded_size: u64,
    pub storage_cost_frost: u64,
    pub current_epoch: u32,
    pub epochs: u32,
}

/// Prepared data is bound to its digest, encoding and network before payment.
pub struct PreparedUpload {
    metadata: VerifiedBlobMetadataWithId,
    slivers: Arc<Vec<SliverPair>>,
    digest: String,
    chain_id: String,
    quote: UploadQuote,
    options: UploadOptions,
}

impl PreparedUpload {
    pub fn quote(&self) -> &UploadQuote {
        &self.quote
    }
}

/// Persist this transaction before submission. Retrying these exact bytes is safe;
/// building another registration can purchase duplicate storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UploadRegistration {
    pub network: WalrusNetwork,
    pub chain_id: String,
    pub blob_id: String,
    pub sha256: String,
    pub size: usize,
    pub owner: sui::types::Address,
    pub options: UploadOptions,
    pub transaction: sui::types::Transaction,
    pub signature: sui::types::UserSignature,
}

/// A registered blob whose data still needs uploading or certification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingUpload {
    pub registration: UploadRegistration,
    pub object_id: sui::types::Address,
}

/// Native storage owns no key. Cloning it shares the caller's wallet authority.
pub struct WalrusStorage {
    wallet: WalletClient,
    network: WalrusNetwork,
    read: Arc<SuiReadClient>,
    nodes: WalrusNodeClient<SuiReadClient>,
    reader: WalrusReader,
}

impl WalrusStorage {
    pub async fn new(wallet: WalletClient, aggregator: Option<&str>) -> anyhow::Result<Self> {
        let network = wallet.chain().await?.parse::<WalrusNetwork>()?;
        let (system, staking) = network.contracts();
        let contract = ContractConfig::new(system.parse()?, staking.parse()?);
        let read =
            SuiReadClient::new_for_rpc_urls(&[wallet.rpc_url()], &contract, Default::default())
                .await?;
        let config = ClientConfig::new_from_contract_config(contract);
        let nodes = WalrusNodeClient::new_read_client_with_refresher(config, read.clone()).await?;
        let reader = WalrusReader::new(
            aggregator.unwrap_or(network.aggregator_url()),
            MAX_RESOLVED_DATA_BYTES,
        )?;
        Ok(Self {
            wallet,
            network,
            read: Arc::new(read),
            nodes,
            reader,
        })
    }

    pub fn network(&self) -> WalrusNetwork {
        self.network
    }

    pub fn wallet(&self) -> &WalletClient {
        &self.wallet
    }

    pub async fn prepare(
        &self,
        bytes: Vec<u8>,
        options: UploadOptions,
    ) -> anyhow::Result<PreparedUpload> {
        ensure!(
            bytes.len() <= MAX_RESOLVED_DATA_BYTES,
            "blob exceeds execution byte limit"
        );
        ensure!(
            options.epochs > 0
                && options.epochs <= self.read.fixed_system_parameters().max_epochs_ahead,
            "storage duration is outside the network epoch range"
        );
        ensure!(
            options.gas_budget_mist > 0,
            "Sui gas budget must be positive"
        );
        let size = bytes.len();
        let digest = hex::encode(Sha256::digest(&bytes));
        let config =
            EncodingConfig::new(self.read.n_shards().await?).get_for_type(DEFAULT_ENCODING);
        let (slivers, metadata) =
            tokio::task::spawn_blocking(move || config.encode_with_metadata(bytes)).await??;
        let encoded_size = metadata
            .metadata()
            .encoded_size()
            .context("invalid encoded blob size")?;
        let (storage, write) = self.read.storage_and_write_price_per_unit_size().await?;
        let cost = storage_cost(encoded_size, storage, write, options.epochs)?;
        ensure!(
            options.max_storage_cost_frost.is_none_or(|max| cost <= max),
            "estimated storage cost exceeds the configured limit"
        );
        let quote = UploadQuote {
            blob_id: metadata.blob_id().to_string(),
            size,
            encoded_size,
            storage_cost_frost: cost,
            current_epoch: self.read.current_epoch().await?,
            epochs: options.epochs,
        };
        Ok(PreparedUpload {
            metadata,
            slivers: Arc::new(slivers),
            digest,
            chain_id: self.wallet.chain_id().await?.into(),
            quote,
            options,
        })
    }

    /// Builds and signs the registration transaction without submitting it or spending funds.
    pub async fn registration(
        &self,
        upload: &PreparedUpload,
    ) -> anyhow::Result<UploadRegistration> {
        ensure!(
            upload.chain_id == self.wallet.chain_id().await?,
            "upload was prepared for another network"
        );
        let mut builder = self.builder()?;
        let storage = builder
            .reserve_space(upload.quote.encoded_size, upload.options.epochs)
            .await?;
        builder
            .register_blob(
                storage.into(),
                BlobObjectMetadata::try_from(&upload.metadata)?,
                BlobPersistence::from_deletable(upload.options.deletable),
            )
            .await?;
        let transaction = self
            .wallet
            .prepare_transaction(finish_ptb(builder).await?, upload.options.gas_budget_mist)
            .await?;
        let signature = self.wallet.sign_transaction(&transaction)?;
        Ok(UploadRegistration {
            network: self.network,
            chain_id: self.wallet.chain_id().await?.into(),
            blob_id: upload.quote.blob_id.clone(),
            sha256: upload.digest.clone(),
            size: upload.quote.size,
            owner: self.wallet.owner(),
            options: upload.options.clone(),
            transaction,
            signature,
        })
    }

    /// Executes a saved registration. An ambiguous submission retains its digest
    /// in the wallet error; callers must retry or reconcile this same transaction.
    pub async fn register(
        &self,
        registration: UploadRegistration,
    ) -> anyhow::Result<PendingUpload> {
        self.check_registration(&registration).await?;
        let response = self
            .wallet
            .execute_signed_transaction(
                registration.transaction.clone(),
                registration.signature.clone(),
            )
            .await?;
        let blob_type = self.blob_type()?;
        let blob = response.objects().objects().iter().filter_map(|object| decode_blob(object, &blob_type).ok())
            .find(|blob| blob.blob_id.to_string() == registration.blob_id)
            .context("registration completed without the expected Blob object; retain the transaction for recovery")?;
        Ok(PendingUpload {
            object_id: blob.id.to_string().parse()?,
            registration,
        })
    }

    /// Completes an existing paid registration without buying more storage.
    pub async fn finish(
        &self,
        upload: PreparedUpload,
        pending: &PendingUpload,
    ) -> anyhow::Result<StoredBlob> {
        self.check_registration(&pending.registration).await?;
        ensure!(
            upload.chain_id == self.wallet.chain_id().await?,
            "upload was prepared for another network"
        );
        ensure!(
            upload.digest == pending.registration.sha256
                && upload.quote.blob_id == pending.registration.blob_id,
            "prepared contents differ from the paid registration"
        );
        let object: walrus_sdk::ObjectID = pending.object_id.to_string().parse()?;
        let blob = self.owned_blob(pending.object_id).await?;
        ensure!(
            blob.blob_id.to_string() == pending.registration.blob_id
                && blob.size == pending.registration.size as u64
                && blob.deletable == pending.registration.options.deletable,
            "owned Blob object does not match the registration"
        );
        ensure!(
            blob.storage.end_epoch > self.read.current_epoch().await?,
            "blob registration has expired"
        );
        if !blob.is_certified() {
            let certificate = self
                .nodes
                .send_blob_data_and_get_certificate(
                    &upload.metadata,
                    upload.slivers,
                    &blob.blob_persistence_type(),
                    None,
                    TailHandling::Blocking,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
            let mut builder = self.builder()?;
            builder.certify_blob(object.into(), &certificate).await?;
            self.submit(builder, pending.registration.options.gas_budget_mist)
                .await?;
        }
        let blob = self.owned_blob(pending.object_id).await?;
        ensure!(
            blob.is_certified(),
            "Walrus did not certify the uploaded blob"
        );
        let stored = StoredBlob {
            blob_id: blob.blob_id.to_string(),
            sha256: pending.registration.sha256.clone(),
            size: pending.registration.size,
            object_id: pending.object_id,
            owner: self.wallet.owner(),
            end_epoch: blob.storage.end_epoch,
            deletable: blob.deletable,
        };
        let bytes = self
            .reader
            .read_verified(&stored.blob_id, &hex::decode(&stored.sha256)?, stored.size)
            .await?;
        stored.verify_bytes(&bytes)?;
        Ok(stored)
    }

    /// Convenience for tools. Errors after payment include the pending upload so
    /// an operator can resume it instead of registering and paying again.
    pub async fn upload(
        &self,
        bytes: Vec<u8>,
        options: UploadOptions,
    ) -> Result<StoredBlob, UploadError> {
        let prepared = self
            .prepare(bytes, options)
            .await
            .map_err(UploadError::before_registration)?;
        let registration = self
            .registration(&prepared)
            .await
            .map_err(UploadError::before_registration)?;
        let pending = self
            .register(registration.clone())
            .await
            .map_err(|source| UploadError {
                registration: Some(Box::new(registration)),
                pending: None,
                source,
            })?;
        self.finish(prepared, &pending)
            .await
            .map_err(|source| UploadError {
                registration: None,
                pending: Some(Box::new(pending)),
                source,
            })
    }

    pub async fn current_epoch(&self) -> anyhow::Result<u32> {
        Ok(self.read.current_epoch().await?)
    }

    pub async fn inspect(&self, reference: &StoredBlob) -> anyhow::Result<StoredBlob> {
        let blob = self.owned_blob(reference.object_id).await?;
        ensure!(
            blob.blob_id.to_string() == reference.blob_id
                && blob.size == reference.size as u64
                && reference.owner == self.wallet.owner(),
            "Blob object no longer matches the reference"
        );
        Ok(StoredBlob {
            end_epoch: blob.storage.end_epoch,
            deletable: blob.deletable,
            ..reference.clone()
        })
    }

    /// Checks the estimated extension cost before submitting at the prices used by Walrus.
    pub async fn extend(
        &self,
        reference: &StoredBlob,
        epochs: u32,
        max_cost_frost: u64,
        gas_budget: u64,
    ) -> anyhow::Result<StoredBlob> {
        let blob = self.owned_blob(reference.object_id).await?;
        ensure!(
            epochs > 0 && epochs <= self.read.fixed_system_parameters().max_epochs_ahead,
            "invalid extension duration"
        );
        ensure!(
            blob.blob_id.to_string() == reference.blob_id,
            "Blob object does not match the reference"
        );
        let current_epoch = self.current_epoch().await?;
        ensure!(
            blob.storage.end_epoch > current_epoch,
            "expired storage cannot be extended"
        );
        let new_end = blob
            .storage
            .end_epoch
            .checked_add(epochs)
            .context("expiry overflow")?;
        ensure!(
            new_end - current_epoch <= self.read.fixed_system_parameters().max_epochs_ahead,
            "extension exceeds the network retention horizon"
        );
        let (storage, _) = self.read.storage_and_write_price_per_unit_size().await?;
        let cost = storage_cost(blob.storage.storage_size, storage, 0, epochs)?;
        ensure!(
            cost <= max_cost_frost,
            "estimated extension cost exceeds the configured limit"
        );
        let mut builder = self.builder()?;
        builder
            .extend_blob(blob.id.into(), epochs, blob.storage.storage_size)
            .await?;
        self.submit(builder, gas_budget).await?;
        self.inspect(reference).await
    }

    pub async fn delete(&self, reference: &StoredBlob, gas_budget: u64) -> anyhow::Result<()> {
        let blob = self.owned_blob(reference.object_id).await?;
        ensure!(
            blob.deletable && blob.blob_id.to_string() == reference.blob_id,
            "only the matching deletable Blob object can be deleted"
        );
        let mut builder = self.builder()?;
        builder.delete_blob(blob.id.into()).await?;
        self.submit(builder, gas_budget).await?;
        Ok(())
    }

    /// Returns owned Blob objects with current lifecycle metadata. The digest is
    /// not stored on chain; use the saved reference when verifying content.
    pub async fn list(&self, include_expired: bool) -> anyhow::Result<Vec<BlobStatus>> {
        let current_epoch = self.current_epoch().await?;
        let blob_type = self.blob_type()?;
        let mut client = self.wallet.grpc_client().as_ref().clone();
        let mut result = Vec::new();
        let mut page_token = None;
        loop {
            let mut request = sui::grpc::ListOwnedObjectsRequest::default()
                .with_owner(self.wallet.owner())
                .with_object_type(blob_type.clone())
                .with_page_size(100)
                .with_read_mask(sui::grpc::FieldMask::from_paths(["bcs"]));
            request.page_token = page_token;
            let response = client
                .state_client()
                .list_owned_objects(request)
                .await?
                .into_inner();
            for object in &response.objects {
                let decoded = sui::types::Object::try_from(object)?;
                ensure!(
                    decoded.owner() == &sui::types::Owner::Address(self.wallet.owner()),
                    "owned object response has a different owner"
                );
                let blob = decode_blob(object, &blob_type)?;
                if include_expired || blob.storage.end_epoch > current_epoch {
                    result.push(BlobStatus {
                        blob_id: blob.blob_id.to_string(),
                        object_id: blob.id.to_string().parse()?,
                        size: blob.size,
                        end_epoch: blob.storage.end_epoch,
                        certified: blob.is_certified(),
                        deletable: blob.deletable,
                    });
                }
            }
            page_token = response.next_page_token;
            if page_token.is_none() {
                break;
            }
        }
        Ok(result)
    }

    fn blob_type(&self) -> anyhow::Result<sui::types::StructTag> {
        let tag = Blob::CONTRACT_STRUCT
            .to_move_struct_tag_with_type_map(&self.read.type_origin_map(), &[])?;
        Ok(bcs::from_bytes(&bcs::to_bytes(&tag)?)?)
    }

    fn builder(&self) -> anyhow::Result<WalrusPtbBuilder> {
        Ok(WalrusPtbBuilder::new(
            Arc::clone(&self.read),
            self.wallet.owner().to_string().parse()?,
        ))
    }

    async fn check_registration(&self, registration: &UploadRegistration) -> anyhow::Result<()> {
        ensure!(
            registration.network == self.network
                && registration.chain_id == self.wallet.chain_id().await?
                && registration.owner == self.wallet.owner()
                && registration.transaction.sender == self.wallet.owner(),
            "registration belongs to another wallet or network"
        );
        Ok(())
    }

    async fn owned_blob(&self, id: sui::types::Address) -> anyhow::Result<Blob> {
        let response = self
            .wallet
            .grpc_client()
            .as_ref()
            .clone()
            .ledger_client()
            .get_object(sui::grpc::GetObjectRequest::new(&id).with_read_mask(
                sui::grpc::FieldMask::from_paths(["bcs", "owner", "object_type", "contents"]),
            ))
            .await?
            .into_inner();
        let object = sui::types::Object::try_from(response.object())?;
        ensure!(
            object.owner() == &sui::types::Owner::Address(self.wallet.owner()),
            "active wallet does not own this Blob object"
        );
        decode_blob(response.object(), &self.blob_type()?)
    }

    async fn submit(
        &self,
        builder: WalrusPtbBuilder,
        gas_budget: u64,
    ) -> anyhow::Result<sui::grpc::ExecutedTransaction> {
        let transaction = self
            .wallet
            .prepare_transaction(finish_ptb(builder).await?, gas_budget)
            .await?;
        Ok(self.wallet.execute_transaction(transaction).await?)
    }
}

fn decode_blob(
    object: &sui::grpc::Object,
    blob_type: &sui::types::StructTag,
) -> anyhow::Result<Blob> {
    let object = sui::types::Object::try_from(object)?;
    let data = object.as_struct().context("expected a Move object")?;
    ensure!(
        data.object_type() == blob_type,
        "expected a Walrus Blob object"
    );
    Ok(bcs::from_bytes(data.contents())?)
}

#[derive(Debug, thiserror::Error)]
#[error("Walrus upload failed: {source}")]
pub struct UploadError {
    pub registration: Option<Box<UploadRegistration>>,
    pub pending: Option<Box<PendingUpload>>,
    #[source]
    pub source: anyhow::Error,
}

impl UploadError {
    fn before_registration(source: anyhow::Error) -> Self {
        Self {
            registration: None,
            pending: None,
            source,
        }
    }
}

/// Metadata available directly from an owned Blob object.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlobStatus {
    pub blob_id: String,
    pub object_id: sui::types::Address,
    pub size: u64,
    pub end_epoch: u32,
    pub certified: bool,
    pub deletable: bool,
}

fn storage_cost(size: u64, storage: u64, write: u64, epochs: u32) -> anyhow::Result<u64> {
    storage
        .checked_mul(u64::from(epochs))
        .and_then(|price| price.checked_add(write))
        .and_then(|price| price.checked_mul(storage_units_from_size(size)))
        .context("storage price overflow")
}

// Native transaction completion would create a second wallet. Use its PTB only,
// then apply the shared wallet's gas and signing policy.
#[allow(deprecated)]
async fn finish_ptb(
    builder: WalrusPtbBuilder,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    let (ptb, extra_sui) = builder.finish().await?;
    ensure!(extra_sui == 0, "unexpected additional SUI payment");
    Ok(bcs::from_bytes(&bcs::to_bytes(&ptb)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_round_storage_units_and_reject_overflow() {
        assert_eq!(storage_cost(1, 10, 3, 2).unwrap(), 23);
        assert_eq!(storage_cost(1024 * 1024 + 1, 10, 3, 2).unwrap(), 46);
        assert!(storage_cost(1, u64::MAX, 1, 2).is_err());
    }

    /// Reads network configuration and encodes data without signing or spending.
    #[tokio::test]
    #[ignore = "requires public Sui and Walrus network access"]
    async fn public_network_quotes_use_the_shared_wallet_connection() {
        for (network, rpc) in [
            (
                WalrusNetwork::Testnet,
                "https://fullnode.testnet.sui.io:443",
            ),
            (
                WalrusNetwork::Mainnet,
                "https://fullnode.mainnet.sui.io:443",
            ),
        ] {
            let wallet = WalletClient::connect(rpc, sui::crypto::Ed25519PrivateKey::new([23; 32]))
                .await
                .unwrap();
            let storage = WalrusStorage::new(wallet.clone(), None).await.unwrap();
            assert_eq!(storage.network(), network);
            assert!(Arc::ptr_eq(
                &storage.wallet().grpc_client(),
                &wallet.grpc_client()
            ));
            let prepared = storage
                .prepare(vec![7; 100_000], UploadOptions::default())
                .await
                .unwrap();
            assert_eq!(prepared.quote().size, 100_000);
            assert!(prepared.quote().encoded_size >= 100_000);
            assert!(prepared.quote().storage_cost_frost > 0);
        }
    }
}
