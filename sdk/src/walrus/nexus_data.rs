//! Walrus upload and digest-verified resolution for direct Nexus data.

use {
    crate::{
        move_bindings::{
            canonical_walrus_blob_id,
            interface::graph::{InputPort, OutputPort},
            sui_framework::vec_map::{Entry as VecMapEntry, VecMap},
        },
        types::{NexusData, NexusValue},
        walrus::{StorageInfo, WalrusClient, WALRUS_MAX_EPOCHS},
    },
    futures_util::future::{join_all, try_join_all},
    sha2::{Digest as _, Sha256},
    thiserror::Error,
};

/// A successful Walrus fetch whose bytes do not match the committed SHA-256 digest.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("Walrus content digest mismatch for storage key '{storage_key}'")]
pub struct WalrusContentDigestMismatch {
    pub storage_key: String,
}

/// A Walrus resolution failure that distinguishes unavailable content from fetched bytes that fail integrity.
///
/// Unlike [`WalrusContentDigestMismatch`], this type retains the fully resolved value on an integrity failure so an on-chain Tool can receive the concrete bytes while independently checking the committed digest.
#[derive(Debug, Error)]
pub enum WalrusFetchError<T> {
    #[error("Walrus content unavailable: {source}")]
    Unavailable {
        #[source]
        source: anyhow::Error,
    },
    #[error("{mismatch}")]
    DigestMismatch {
        resolved: T,
        #[source]
        mismatch: WalrusContentDigestMismatch,
    },
}

impl<T> WalrusFetchError<T> {
    fn into_strict_error(self) -> anyhow::Error {
        match self {
            Self::Unavailable { source } => source,
            Self::DigestMismatch { mismatch, .. } => mismatch.into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageConf {
    pub walrus_publisher_url: Option<String>,
    pub walrus_aggregator_url: Option<String>,
    pub walrus_save_for_epochs: Option<u8>,
}

impl StorageConf {
    /// Validates every local setting required before a Walrus upload can start.
    pub fn validate_walrus_upload(&self) -> anyhow::Result<()> {
        for (name, value) in [
            ("publisher", self.walrus_publisher_url.as_deref()),
            ("aggregator", self.walrus_aggregator_url.as_deref()),
        ] {
            let value = value
                .ok_or_else(|| anyhow::anyhow!("Walrus {name} URL is not set in storage config"))?;
            let url = reqwest::Url::parse(value)
                .map_err(|error| anyhow::anyhow!("Walrus {name} URL is invalid: {error}"))?;
            anyhow::ensure!(
                matches!(url.scheme(), "http" | "https"),
                "Walrus {name} URL must use http or https"
            );
        }

        let store_for_epochs = self.walrus_save_for_epochs.ok_or_else(|| {
            anyhow::anyhow!("Walrus save for epochs is not set in storage config")
        })?;
        anyhow::ensure!(
            store_for_epochs <= WALRUS_MAX_EPOCHS,
            "Walrus save for epochs exceeds maximum allowed ({WALRUS_MAX_EPOCHS})"
        );
        Ok(())
    }
}

impl NexusData {
    /// Resolves Walrus Data to transient Tool data after verifying its committed digest.
    pub async fn fetch(self, conf: &StorageConf) -> anyhow::Result<Vec<NexusValue>> {
        self.resolve(conf)
            .await
            .map_err(WalrusFetchError::into_strict_error)
    }

    /// Resolves every Walrus value while retaining concrete bytes on a digest mismatch.
    pub async fn resolve(
        self,
        conf: &StorageConf,
    ) -> Result<Vec<NexusValue>, WalrusFetchError<Vec<NexusValue>>> {
        if !self.is_well_formed() {
            return Err(WalrusFetchError::Unavailable {
                source: anyhow::anyhow!("cannot resolve malformed NexusData"),
            });
        }
        let has_walrus = self.has_walrus();
        let client = if has_walrus {
            Some(walrus_reader(conf).map_err(|source| WalrusFetchError::Unavailable { source })?)
        } else {
            None
        };
        let mut resolved = Vec::new();
        let mut mismatch = None;
        for value in self
            .into_values()
            .map_err(|source| WalrusFetchError::Unavailable { source })?
        {
            match value {
                NexusValue::WalrusData {
                    storage_key,
                    content_digest,
                } => {
                    let blob_id = blob_id_from_bytes(&storage_key)
                        .map_err(|source| WalrusFetchError::Unavailable { source })?;
                    let bytes = client
                        .as_ref()
                        .expect("Walrus client exists when a reference is present")
                        .read_file(&blob_id)
                        .await
                        .map_err(|source| WalrusFetchError::Unavailable {
                            source: source.into(),
                        })?;
                    let actual_digest: [u8; 32] = Sha256::digest(&bytes).into();
                    if actual_digest.as_slice() != content_digest && mismatch.is_none() {
                        mismatch = Some(WalrusContentDigestMismatch {
                            storage_key: blob_id,
                        });
                    }
                    resolved.push(NexusValue::InlineData { bytes });
                }
                value => resolved.push(value),
            }
        }
        match mismatch {
            Some(mismatch) => Err(WalrusFetchError::DigestMismatch { resolved, mismatch }),
            None => Ok(resolved),
        }
    }

    /// Verifies existing Walrus references while leaving their committed form unchanged.
    pub async fn commit(self, conf: &StorageConf) -> anyhow::Result<Self> {
        anyhow::ensure!(self.is_well_formed(), "cannot commit malformed NexusData");
        if self.has_walrus() {
            self.clone().fetch(conf).await?;
        }
        Ok(self)
    }

    /// Uploads every inline Data value and returns digest-bound Walrus values.
    pub async fn upload_data_fields(self, conf: &StorageConf) -> anyhow::Result<Self> {
        anyhow::ensure!(self.is_well_formed(), "cannot upload malformed NexusData");
        let (client, store_for_epochs) = walrus_writer(conf)?;
        let many = self.is_many();
        let mut uploaded = Vec::new();
        for value in self.into_values()? {
            match value {
                NexusValue::InlineData { bytes } => {
                    let digest = Sha256::digest(&bytes).to_vec();
                    let response = client
                        .upload_bytes(bytes.clone(), store_for_epochs, None)
                        .await?;
                    let storage_key = blob_id_from_storage_info(response)?.into_bytes();
                    let blob_id = blob_id_from_bytes(&storage_key)?;
                    let read_back = client.read_file_bounded(&blob_id, bytes.len()).await?;
                    let read_back_digest: [u8; 32] = Sha256::digest(&read_back).into();
                    anyhow::ensure!(
                        read_back == bytes && read_back_digest.as_slice() == digest,
                        "Walrus upload read-back mismatch for storage key '{blob_id}'"
                    );
                    uploaded.push(NexusValue::walrus_data(storage_key, digest)?);
                }
                NexusValue::WalrusData { .. } => {
                    anyhow::bail!("Data upload received an already remote value");
                }
                NexusValue::Object { .. } => {
                    anyhow::bail!("Data upload cannot store Object values in Walrus");
                }
            }
        }
        NexusData::from_values(uploaded, many)
    }
}

impl VecMap<InputPort, NexusData> {
    pub async fn commit_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        commit_entries(self.contents, storage_conf)
            .await
            .map(|contents| Self { contents })
    }

    pub async fn fetch_all(
        self,
        storage_conf: &StorageConf,
    ) -> anyhow::Result<VecMap<InputPort, Vec<NexusValue>>> {
        resolve_entries(self.contents, storage_conf)
            .await
            .map(|contents| VecMap { contents })
            .map_err(WalrusFetchError::into_strict_error)
    }

    /// Resolves every port while retaining concrete values on a digest mismatch.
    pub async fn resolve_all(
        self,
        storage_conf: &StorageConf,
    ) -> Result<
        VecMap<InputPort, Vec<NexusValue>>,
        WalrusFetchError<VecMap<InputPort, Vec<NexusValue>>>,
    > {
        match resolve_entries(self.contents, storage_conf).await {
            Ok(contents) => Ok(VecMap { contents }),
            Err(WalrusFetchError::Unavailable { source }) => {
                Err(WalrusFetchError::Unavailable { source })
            }
            Err(WalrusFetchError::DigestMismatch { resolved, mismatch }) => {
                Err(WalrusFetchError::DigestMismatch {
                    resolved: VecMap { contents: resolved },
                    mismatch,
                })
            }
        }
    }
}

impl VecMap<OutputPort, NexusData> {
    pub async fn commit_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        commit_entries(self.contents, storage_conf)
            .await
            .map(|contents| Self { contents })
    }

    pub async fn fetch_all(
        self,
        storage_conf: &StorageConf,
    ) -> anyhow::Result<VecMap<OutputPort, Vec<NexusValue>>> {
        resolve_entries(self.contents, storage_conf)
            .await
            .map(|contents| VecMap { contents })
            .map_err(WalrusFetchError::into_strict_error)
    }

    /// Resolves every port while retaining concrete values on a digest mismatch.
    pub async fn resolve_all(
        self,
        storage_conf: &StorageConf,
    ) -> Result<
        VecMap<OutputPort, Vec<NexusValue>>,
        WalrusFetchError<VecMap<OutputPort, Vec<NexusValue>>>,
    > {
        match resolve_entries(self.contents, storage_conf).await {
            Ok(contents) => Ok(VecMap { contents }),
            Err(WalrusFetchError::Unavailable { source }) => {
                Err(WalrusFetchError::Unavailable { source })
            }
            Err(WalrusFetchError::DigestMismatch { resolved, mismatch }) => {
                Err(WalrusFetchError::DigestMismatch {
                    resolved: VecMap { contents: resolved },
                    mismatch,
                })
            }
        }
    }
}

async fn commit_entries<P>(
    contents: Vec<VecMapEntry<P, NexusData>>,
    storage_conf: &StorageConf,
) -> anyhow::Result<Vec<VecMapEntry<P, NexusData>>> {
    validate_entries(&contents)?;
    try_join_all(contents.into_iter().map(|entry| async move {
        entry
            .value
            .commit(storage_conf)
            .await
            .map(|value| VecMapEntry {
                key: entry.key,
                value,
            })
    }))
    .await
}

async fn resolve_entries<P>(
    contents: Vec<VecMapEntry<P, NexusData>>,
    storage_conf: &StorageConf,
) -> Result<
    Vec<VecMapEntry<P, Vec<NexusValue>>>,
    WalrusFetchError<Vec<VecMapEntry<P, Vec<NexusValue>>>>,
> {
    validate_entries(&contents).map_err(|source| WalrusFetchError::Unavailable { source })?;
    let fetched = join_all(
        contents
            .into_iter()
            .map(|entry| async move { (entry.key, entry.value.resolve(storage_conf).await) }),
    )
    .await;
    let mut resolved = Vec::with_capacity(fetched.len());
    let mut mismatch = None;
    for (key, value) in fetched {
        match value {
            Ok(value) => resolved.push(VecMapEntry { key, value }),
            Err(WalrusFetchError::Unavailable { source }) => {
                return Err(WalrusFetchError::Unavailable { source });
            }
            Err(WalrusFetchError::DigestMismatch {
                resolved: value,
                mismatch: current,
            }) => {
                resolved.push(VecMapEntry { key, value });
                mismatch.get_or_insert(current);
            }
        }
    }
    match mismatch {
        Some(mismatch) => Err(WalrusFetchError::DigestMismatch { resolved, mismatch }),
        None => Ok(resolved),
    }
}

fn validate_entries<P>(contents: &[VecMapEntry<P, NexusData>]) -> anyhow::Result<()> {
    anyhow::ensure!(
        contents.iter().all(|entry| entry.value.is_well_formed()),
        "cannot process malformed NexusData map",
    );
    Ok(())
}

fn walrus_reader(conf: &StorageConf) -> anyhow::Result<WalrusClient> {
    let aggregator_url = conf
        .walrus_aggregator_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Walrus aggregator URL is not set in storage config"))?;
    Ok(WalrusClient::builder()
        .with_aggregator_url(aggregator_url)
        .build())
}

fn walrus_writer(conf: &StorageConf) -> anyhow::Result<(WalrusClient, u8)> {
    conf.validate_walrus_upload()?;
    let publisher_url = conf
        .walrus_publisher_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Walrus publisher URL is not set in storage config"))?;
    let aggregator_url = conf
        .walrus_aggregator_url
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Walrus aggregator URL is not set in storage config"))?;
    let store_for_epochs = conf
        .walrus_save_for_epochs
        .ok_or_else(|| anyhow::anyhow!("Walrus save for epochs is not set in storage config"))?;
    if store_for_epochs > WALRUS_MAX_EPOCHS {
        anyhow::bail!("Walrus save for epochs exceeds maximum allowed ({WALRUS_MAX_EPOCHS})");
    }
    Ok((
        WalrusClient::builder()
            .with_publisher_url(publisher_url)
            .with_aggregator_url(aggregator_url)
            .build(),
        store_for_epochs,
    ))
}

fn blob_id_from_bytes(bytes: &[u8]) -> anyhow::Result<String> {
    canonical_walrus_blob_id(bytes).map(str::to_owned)
}

fn blob_id_from_storage_info(info: StorageInfo) -> anyhow::Result<String> {
    info.newly_created
        .map(|created| created.blob_object.blob_id)
        .or_else(|| info.already_certified.map(|certified| certified.blob_id))
        .ok_or_else(|| anyhow::anyhow!("Failed to store data on Walrus: no committed blob info"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::walrus::{AlreadyCertified, BlobObject, BlobStorage, NewlyCreated, SuiEvent},
        mockito::{Server, ServerGuard},
    };

    const BLOB_ID: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    async fn setup_mock_server_and_conf() -> anyhow::Result<(ServerGuard, StorageConf)> {
        let server = Server::new_async().await;
        let server_url = server.url();
        Ok((
            server,
            StorageConf {
                walrus_publisher_url: Some(server_url.clone()),
                walrus_aggregator_url: Some(server_url),
                walrus_save_for_epochs: Some(2),
            },
        ))
    }

    #[tokio::test]
    async fn typed_inline_commit_and_fetch_are_noops() {
        let storage_conf = StorageConf::default();
        let data = NexusData::inline_data(b"payload").unwrap();
        let fetched = data
            .clone()
            .commit(&storage_conf)
            .await
            .unwrap()
            .fetch(&storage_conf)
            .await
            .unwrap();

        assert!(matches!(
            fetched.as_slice(),
            [NexusValue::InlineData { bytes }] if bytes == b"payload"
        ));
    }

    #[tokio::test]
    async fn empty_many_is_rejected_before_walrus_configuration() {
        let value = NexusData::new(b"nexus_value".to_vec(), Vec::new(), Vec::new());

        assert!(value
            .clone()
            .upload_data_fields(&StorageConf::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot upload malformed NexusData"));
        assert!(value
            .clone()
            .commit(&StorageConf::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot commit malformed NexusData"));
        assert!(value
            .fetch(&StorageConf::default())
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot resolve malformed NexusData"));
    }

    #[tokio::test]
    async fn malformed_map_entry_precedes_all_walrus_requests() {
        let (mut server, storage_conf) = setup_mock_server_and_conf().await.unwrap();
        let get = server
            .mock("GET", format!("/v1/blobs/{BLOB_ID}").as_str())
            .expect(0)
            .create_async()
            .await;
        let valid = NexusData::walrus_data(BLOB_ID.as_bytes(), vec![0; 32]).unwrap();
        let malformed = NexusData::new(b"nexus_value".to_vec(), Vec::new(), Vec::new());
        let map = || VecMap {
            contents: vec![
                VecMapEntry {
                    key: InputPort::new("valid"),
                    value: valid.clone(),
                },
                VecMapEntry {
                    key: InputPort::new("malformed"),
                    value: malformed.clone(),
                },
            ],
        };

        assert!(map()
            .commit_all(&storage_conf)
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot process malformed NexusData map"));
        assert!(map()
            .resolve_all(&storage_conf)
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot process malformed NexusData map"));
        get.assert_async().await;
    }

    #[tokio::test]
    async fn typed_upload_reads_back_and_commits_digest() {
        let (mut server, storage_conf) = setup_mock_server_and_conf().await.unwrap();
        let response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: BLOB_ID.to_string(),
                    id: "raw_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };
        let put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&response).unwrap())
            .create_async()
            .await;
        let get = server
            .mock("GET", format!("/v1/blobs/{BLOB_ID}").as_str())
            .with_status(200)
            .with_body("payload")
            .create_async()
            .await;

        let committed = NexusData::inline_data(b"payload")
            .unwrap()
            .upload_data_fields(&storage_conf)
            .await
            .unwrap();

        assert!(matches!(
            committed.values().unwrap().as_slice(),
            [NexusValue::WalrusData { storage_key, .. }]
                if storage_key == BLOB_ID.as_bytes()
        ));
        put.assert_async().await;
        get.assert_async().await;
    }

    #[tokio::test]
    async fn typed_fetch_rejects_digest_mismatch() {
        let (mut server, storage_conf) = setup_mock_server_and_conf().await.unwrap();
        let get = server
            .mock("GET", format!("/v1/blobs/{BLOB_ID}").as_str())
            .with_status(200)
            .with_body("different")
            .create_async()
            .await;
        let value = NexusData::walrus_data(BLOB_ID.as_bytes(), Sha256::digest(b"payload").to_vec())
            .unwrap();

        let error = value.fetch(&storage_conf).await.unwrap_err();

        assert!(error
            .downcast_ref::<WalrusContentDigestMismatch>()
            .is_some());
        get.assert_async().await;
    }

    #[tokio::test]
    async fn typed_fetch_accepts_content_larger_than_onchain_inline_data() {
        let (mut server, storage_conf) = setup_mock_server_and_conf().await.unwrap();
        let bytes = vec![b'x'; 61_441];
        let get = server
            .mock("GET", format!("/v1/blobs/{BLOB_ID}").as_str())
            .with_status(200)
            .with_body(bytes.clone())
            .create_async()
            .await;
        let value =
            NexusData::walrus_data(BLOB_ID.as_bytes(), Sha256::digest(&bytes).to_vec()).unwrap();

        let resolved = value.fetch(&storage_conf).await.unwrap();

        assert!(matches!(
            resolved.as_slice(),
            [NexusValue::InlineData { bytes: resolved_bytes }] if resolved_bytes == &bytes
        ));
        get.assert_async().await;
    }

    #[tokio::test]
    async fn typed_resolve_preserves_fetched_bytes_for_digest_mismatch() {
        let (mut server, storage_conf) = setup_mock_server_and_conf().await.unwrap();
        let get = server
            .mock("GET", format!("/v1/blobs/{BLOB_ID}").as_str())
            .with_status(200)
            .with_body("different")
            .create_async()
            .await;
        let value = NexusData::walrus_data(BLOB_ID.as_bytes(), Sha256::digest(b"payload").to_vec())
            .unwrap();

        let WalrusFetchError::DigestMismatch { resolved, mismatch } =
            value.resolve(&storage_conf).await.unwrap_err()
        else {
            panic!("successful fetch with unequal bytes must be an integrity mismatch");
        };

        assert_eq!(mismatch.storage_key, BLOB_ID);
        assert!(matches!(
            resolved.as_slice(),
            [NexusValue::InlineData { bytes }] if bytes == b"different"
        ));
        get.assert_async().await;
    }

    #[tokio::test]
    async fn typed_resolve_classifies_unavailable_content_separately() {
        let (mut server, storage_conf) = setup_mock_server_and_conf().await.unwrap();
        let get = server
            .mock("GET", format!("/v1/blobs/{BLOB_ID}").as_str())
            .with_status(404)
            .create_async()
            .await;
        let value = NexusData::walrus_data(BLOB_ID.as_bytes(), Sha256::digest(b"payload").to_vec())
            .unwrap();

        assert!(matches!(
            value.resolve(&storage_conf).await.unwrap_err(),
            WalrusFetchError::Unavailable { .. }
        ));
        get.assert_async().await;
    }

    #[test]
    fn certified_response_returns_existing_blob_id() {
        let info = StorageInfo {
            newly_created: None,
            already_certified: Some(AlreadyCertified {
                blob_id: "existing".to_string(),
                event: SuiEvent {
                    tx_digest: "0x1".to_string(),
                },
                end_epoch: 1,
            }),
        };

        assert_eq!(blob_id_from_storage_info(info).unwrap(), "existing");
    }
}
