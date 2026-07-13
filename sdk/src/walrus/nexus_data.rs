//! Walrus helpers for raw `NexusData` payloads.

use {
    crate::{
        move_bindings::{
            interface::graph::{InputPort, OutputPort},
            primitives::data::NexusData,
            sui_framework::vec_map::{Entry as VecMapEntry, VecMap},
        },
        walrus::{StorageInfo, WalrusClient, WALRUS_MAX_EPOCHS},
    },
    futures_util::future::try_join_all,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageConf {
    pub walrus_publisher_url: Option<String>,
    pub walrus_aggregator_url: Option<String>,
    pub walrus_save_for_epochs: Option<u8>,
}

impl NexusData {
    pub async fn fetch(mut self, conf: &StorageConf) -> anyhow::Result<Self> {
        if !self.is_walrus() {
            return Ok(self);
        }

        let walrus_aggregator_url = conf
            .walrus_aggregator_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus aggregator URL is not set in storage config"))?;

        let client = WalrusClient::builder()
            .with_aggregator_url(walrus_aggregator_url)
            .build();

        if !self.one.is_empty() && !self.many.is_empty() {
            anyhow::bail!("NexusData cannot contain both one and many payloads");
        }

        if !self.one.is_empty() {
            let blob_id = blob_id_from_bytes(&self.one)?;
            self.one = client.read_file(&blob_id).await?;
            return Ok(self);
        }

        let mut data = Vec::with_capacity(self.many.len());
        for key in &self.many {
            let blob_id = blob_id_from_bytes(key)?;
            data.push(client.read_file(&blob_id).await?);
        }
        self.many = data;
        Ok(self)
    }

    pub async fn commit(mut self, conf: &StorageConf) -> anyhow::Result<Self> {
        if !self.is_walrus() {
            return Ok(self);
        }

        let walrus_publisher_url = conf
            .walrus_publisher_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Walrus publisher URL is not set in storage config"))?;

        let store_for_epochs = conf.walrus_save_for_epochs.ok_or_else(|| {
            anyhow::anyhow!("Walrus save for epochs is not set in storage config")
        })?;

        if store_for_epochs > WALRUS_MAX_EPOCHS {
            anyhow::bail!("Walrus save for epochs exceeds maximum allowed ({WALRUS_MAX_EPOCHS})");
        }

        let client = WalrusClient::builder()
            .with_publisher_url(walrus_publisher_url)
            .build();

        if !self.one.is_empty() && !self.many.is_empty() {
            anyhow::bail!("NexusData cannot contain both one and many payloads");
        }

        if !self.one.is_empty() {
            let response = client
                .upload_bytes(std::mem::take(&mut self.one), store_for_epochs, None)
                .await?;
            self.one = blob_id_from_storage_info(response)?.into_bytes();
            return Ok(self);
        }

        let payloads = std::mem::take(&mut self.many);
        let mut keys = Vec::with_capacity(payloads.len());
        for payload in payloads {
            let response = client.upload_bytes(payload, store_for_epochs, None).await?;
            keys.push(blob_id_from_storage_info(response)?.into_bytes());
        }
        self.many = keys;
        Ok(self)
    }
}

impl VecMap<InputPort, NexusData> {
    pub async fn commit_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        commit_entries(self.contents, storage_conf)
            .await
            .map(|contents| Self { contents })
    }

    pub async fn fetch_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        fetch_entries(self.contents, storage_conf)
            .await
            .map(|contents| Self { contents })
    }
}

impl VecMap<OutputPort, NexusData> {
    pub async fn commit_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        commit_entries(self.contents, storage_conf)
            .await
            .map(|contents| Self { contents })
    }

    pub async fn fetch_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        fetch_entries(self.contents, storage_conf)
            .await
            .map(|contents| Self { contents })
    }
}

async fn commit_entries<P>(
    contents: Vec<VecMapEntry<P, NexusData>>,
    storage_conf: &StorageConf,
) -> anyhow::Result<Vec<VecMapEntry<P, NexusData>>> {
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

async fn fetch_entries<P>(
    contents: Vec<VecMapEntry<P, NexusData>>,
    storage_conf: &StorageConf,
) -> anyhow::Result<Vec<VecMapEntry<P, NexusData>>> {
    try_join_all(contents.into_iter().map(|entry| async move {
        entry
            .value
            .fetch(storage_conf)
            .await
            .map(|value| VecMapEntry {
                key: entry.key,
                value,
            })
    }))
    .await
}

fn blob_id_from_bytes(bytes: &[u8]) -> anyhow::Result<String> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|e| anyhow::anyhow!("Walrus blob id must be UTF-8: {e}"))
}

fn blob_id_from_storage_info(info: StorageInfo) -> anyhow::Result<String> {
    info.newly_created
        .map(|created| created.blob_object.blob_id)
        .ok_or_else(|| {
            anyhow::anyhow!("Failed to store data on Walrus: no newly created blob info")
        })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                move_std::ascii::String as MoveString,
                sui_framework::vec_map::Entry as VecMapEntry,
            },
            walrus::{BlobObject, BlobStorage, NewlyCreated},
        },
        mockito::{Server, ServerGuard},
        std::collections::HashMap,
    };

    async fn setup_mock_server_and_conf() -> anyhow::Result<(ServerGuard, StorageConf)> {
        let server = Server::new_async().await;
        let server_url = server.url();

        let storage_conf = StorageConf {
            walrus_publisher_url: Some(server_url.clone()),
            walrus_aggregator_url: Some(server_url),
            walrus_save_for_epochs: Some(2),
        };

        Ok((server, storage_conf))
    }

    fn inline_bytes(value: &'static [u8]) -> NexusData {
        NexusData::inline_one(value.to_vec())
    }

    #[tokio::test]
    async fn inline_commit_and_fetch_are_noops_for_bytes() {
        let storage_conf = StorageConf::default();
        let data = NexusData::inline_one(b"payload".to_vec());

        let committed = data
            .clone()
            .commit(&storage_conf)
            .await
            .expect("inline commit should succeed");
        assert_eq!(committed, data);

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("inline fetch should succeed");
        assert_eq!(fetched, data);
    }

    #[tokio::test]
    async fn walrus_one_commits_and_fetches_raw_bytes() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("mock server should start");
        let mock_put_response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: "raw_blob_id".to_string(),
                    id: "raw_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };

        let mock_put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_put_response).expect("must serialize"))
            .create_async()
            .await;

        let mock_get = server
            .mock("GET", "/v1/blobs/raw_blob_id")
            .with_status(200)
            .with_body("payload")
            .create_async()
            .await;

        let committed = NexusData::walrus_one(b"payload".to_vec())
            .commit(&storage_conf)
            .await
            .expect("walrus commit should succeed");
        assert_eq!(committed, NexusData::walrus_one(b"raw_blob_id".to_vec()));

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("walrus fetch should succeed");
        assert_eq!(fetched, NexusData::walrus_one(b"payload".to_vec()));

        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[tokio::test]
    async fn walrus_many_commits_and_fetches_each_payload() {
        let (mut server, storage_conf) = setup_mock_server_and_conf()
            .await
            .expect("mock server should start");
        let mock_put_response = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: "raw_blob_id".to_string(),
                    id: "raw_object_id".to_string(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };

        let mock_put = server
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(2)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_put_response).expect("must serialize"))
            .create_async()
            .await;

        let mock_get = server
            .mock("GET", "/v1/blobs/raw_blob_id")
            .expect(2)
            .with_status(200)
            .with_body("payload")
            .create_async()
            .await;

        let committed = NexusData::walrus_many([b"left".to_vec(), b"right".to_vec()])
            .commit(&storage_conf)
            .await
            .expect("walrus commit should succeed");
        assert_eq!(
            committed,
            NexusData::walrus_many([b"raw_blob_id".to_vec(), b"raw_blob_id".to_vec()])
        );

        let fetched = committed
            .fetch(&storage_conf)
            .await
            .expect("walrus fetch should succeed");
        assert_eq!(
            fetched,
            NexusData::walrus_many([b"payload".to_vec(), b"payload".to_vec()])
        );

        mock_put.assert_async().await;
        mock_get.assert_async().await;
    }

    #[tokio::test]
    async fn input_ports_commit_and_fetch_preserve_keys() {
        let storage_conf = StorageConf::default();
        let mut values = HashMap::new();
        values.insert("port1".to_string(), inline_bytes(b"port-value"));

        let committed = VecMap::<InputPort, NexusData>::from_map(values)
            .commit_all(&storage_conf)
            .await
            .expect("inline ports should commit");
        let fetched = committed
            .fetch_all(&storage_conf)
            .await
            .expect("inline ports should fetch");

        let map = fetched.into_map();
        assert_eq!(map.len(), 1);
        assert!(map["port1"].is_inline());
        assert_eq!(map["port1"], inline_bytes(b"port-value"));
    }

    #[tokio::test]
    async fn empty_input_ports_commit_and_fetch() {
        let storage_conf = StorageConf::default();
        let committed = VecMap::<InputPort, NexusData>::from_map(HashMap::new())
            .commit_all(&storage_conf)
            .await
            .expect("empty ports should commit");

        let fetched = committed
            .fetch_all(&storage_conf)
            .await
            .expect("empty ports should fetch");
        assert!(fetched.contents.is_empty());
    }

    #[tokio::test]
    async fn output_ports_fetch_preserves_keys() {
        let storage_conf = StorageConf::default();
        let ports_data = VecMap::<OutputPort, NexusData> {
            contents: vec![VecMapEntry {
                key: OutputPort {
                    name: MoveString::from("output1"),
                },
                value: inline_bytes(b"output-value"),
            }],
        };

        let result = ports_data
            .fetch_all(&storage_conf)
            .await
            .expect("inline output ports should fetch");

        let map = result.into_map();
        assert_eq!(map.len(), 1);
        assert!(map["output1"].is_inline());
        assert_eq!(map["output1"], inline_bytes(b"output-value"));
    }
}
