//! Helpers for generated input/output port data maps.

use {
    crate::types::{
        interface::graph::{InputPort, OutputPort},
        primitives::data::NexusData,
        sui_framework::vec_map::{Entry as VecMapEntry, VecMap as MoveVecMap},
        MoveString,
        StorageConf,
    },
    futures::future::try_join_all,
    std::collections::HashMap,
};

pub type PortsData = MoveVecMap<InputPort, NexusData>;
pub type OutputPortsData = MoveVecMap<OutputPort, NexusData>;

impl PortsData {
    pub fn into_map(self) -> HashMap<String, NexusData> {
        self.contents
            .into_iter()
            .map(|entry| (String::from(entry.key.name), entry.value))
            .collect()
    }

    pub fn from_map(values: HashMap<String, NexusData>) -> Self {
        Self {
            contents: values
                .into_iter()
                .map(|(key, value)| VecMapEntry {
                    key: InputPort {
                        name: MoveString::from(key),
                    },
                    value,
                })
                .collect(),
        }
    }

    pub async fn commit_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        let commit_futures = self.contents.into_iter().map(|entry| {
            let storage_conf = storage_conf.clone();
            async move {
                entry
                    .value
                    .commit(&storage_conf)
                    .await
                    .map(|value| VecMapEntry {
                        key: entry.key,
                        value,
                    })
            }
        });

        try_join_all(commit_futures)
            .await
            .map(|contents| Self { contents })
    }

    pub async fn fetch_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        let fetch_futures = self.contents.into_iter().map(|entry| {
            let storage_conf = storage_conf.clone();
            async move {
                entry
                    .value
                    .fetch(&storage_conf)
                    .await
                    .map(|value| VecMapEntry {
                        key: entry.key,
                        value,
                    })
            }
        });

        try_join_all(fetch_futures)
            .await
            .map(|contents| Self { contents })
    }
}

impl OutputPortsData {
    pub fn into_map(self) -> HashMap<String, NexusData> {
        self.contents
            .into_iter()
            .map(|entry| (String::from(entry.key.name), entry.value))
            .collect()
    }

    pub async fn commit_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        let commit_futures = self.contents.into_iter().map(|entry| {
            let storage_conf = storage_conf.clone();
            async move {
                entry
                    .value
                    .commit(&storage_conf)
                    .await
                    .map(|value| VecMapEntry {
                        key: entry.key,
                        value,
                    })
            }
        });

        try_join_all(commit_futures)
            .await
            .map(|contents| Self { contents })
    }

    pub async fn fetch_all(self, storage_conf: &StorageConf) -> anyhow::Result<Self> {
        let fetch_futures = self.contents.into_iter().map(|entry| {
            let storage_conf = storage_conf.clone();
            async move {
                entry
                    .value
                    .fetch(&storage_conf)
                    .await
                    .map(|value| VecMapEntry {
                        key: entry.key,
                        value,
                    })
            }
        });

        try_join_all(fetch_futures)
            .await
            .map(|contents| Self { contents })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::types::StorageKind, serde_json::json};

    fn sample_ports_data() -> PortsData {
        let mut values = HashMap::new();
        values.insert(
            "port1".to_string(),
            NexusData::new_inline(json!({ "key": "value" })),
        );
        PortsData::from_map(values)
    }

    #[test]
    fn test_ser_deser_ports_data() {
        let ports_data = sample_ports_data();
        let json = serde_json::to_string(&ports_data).unwrap();
        let data_bytes = serde_json::to_vec(&json!({ "key": "value" })).unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&json).unwrap(),
            json!({
                "contents": [{
                    "key": { "name": "port1" },
                    "value": {
                        "storage": b"inline",
                        "one": data_bytes,
                        "many": []
                    }
                }]
            })
        );

        let deserialized: PortsData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ports_data);
    }

    #[test]
    fn test_into_map_returns_inner_hashmap() {
        let ports_data = sample_ports_data();
        let map = ports_data.clone().into_map();

        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get("port1").unwrap(),
            &NexusData::new_inline(json!({ "key": "value" }))
        );
    }

    #[test]
    fn test_into_map_empty_ports_data() {
        let ports_data = PortsData::from_map(HashMap::new());
        let map = ports_data.into_map();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn test_commit_all_success() {
        let ports_data = sample_ports_data();
        let storage_conf = StorageConf::default();

        let result = ports_data.commit_all(&storage_conf).await;
        assert!(result.is_ok());

        let map = result.unwrap().into_map();
        assert_eq!(map.len(), 1);
        assert_eq!(map["port1"].storage_kind(), StorageKind::Inline);
        assert_eq!(map["port1"].as_json(), json!({ "key": "value" }));
    }

    #[tokio::test]
    async fn test_fetch_all_success() {
        let ports_data = sample_ports_data();
        let storage_conf = StorageConf::default();

        let result = ports_data.fetch_all(&storage_conf).await;
        assert!(result.is_ok());

        let map = result.unwrap().into_map();
        assert_eq!(map.len(), 1);
        assert_eq!(map["port1"].storage_kind(), StorageKind::Inline);
        assert_eq!(map["port1"].as_json(), json!({ "key": "value" }));
    }

    #[tokio::test]
    async fn test_commit_all_empty_ports_data() {
        let ports_data = PortsData::from_map(HashMap::new());
        let storage_conf = StorageConf::default();

        let result = ports_data.commit_all(&storage_conf).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contents.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_all_empty_ports_data() {
        let ports_data = PortsData::from_map(HashMap::new());
        let storage_conf = StorageConf::default();

        let result = ports_data.fetch_all(&storage_conf).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contents.is_empty());
    }

    #[tokio::test]
    async fn test_output_ports_data_fetch_all_success() {
        let ports_data = OutputPortsData {
            contents: vec![VecMapEntry {
                key: OutputPort {
                    name: MoveString::from("output1"),
                },
                value: NexusData::new_inline(json!({ "key": "value" })),
            }],
        };
        let storage_conf = StorageConf::default();

        let result = ports_data.fetch_all(&storage_conf).await;
        assert!(result.is_ok());

        let map = result.unwrap().into_map();
        assert_eq!(map.len(), 1);
        assert_eq!(map["output1"].storage_kind(), StorageKind::Inline);
        assert_eq!(map["output1"].as_json(), json!({ "key": "value" }));
    }
}
