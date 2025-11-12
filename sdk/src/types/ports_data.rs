//! Module defines [`PortsData`] - struct that represents data stored on-chain
//! in relation to their variants and ports. This can deserialize directly to
//! [`crate::types::NexusData`]

use {
    crate::{
        crypto::session::Session,
        types::{DataStorage, NexusData, StorageConf, TypeName},
    },
    futures::future::try_join_all,
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
    tokio::sync::Mutex,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortsData {
    values: HashMap<String, NexusData>,
}

impl PortsData {
    /// Consumes self and returns the inner [`HashMap`].
    pub fn into_map(self) -> HashMap<String, NexusData> {
        self.values
    }

    /// Creates a [`PortsData`] from a [`HashMap`].
    pub fn from_map(values: HashMap<String, NexusData>) -> Self {
        PortsData { values }
    }

    /// Function to commit all [`NexusData`] values to storage. This is done in
    /// parallel.
    pub async fn commit_all(
        self,
        storage_conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<HashMap<String, DataStorage>> {
        let commit_futures = self.values.into_iter().map(|(key, data)| {
            let storage_conf = storage_conf.clone();
            let session = Arc::clone(&session);

            async move {
                match data.commit(&storage_conf, session).await {
                    Ok(storage) => Ok((key, storage)),
                    Err(e) => Err(e),
                }
            }
        });

        try_join_all(commit_futures).await.map(|results| {
            results
                .into_iter()
                .collect::<HashMap<String, DataStorage>>()
        })
    }

    /// Function to fetch all [`NexusData`] values from storage. This is done in
    /// parallel.
    pub async fn fetch_all(
        self,
        storage_conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> anyhow::Result<HashMap<String, DataStorage>> {
        let fetch_futures = self.values.into_iter().map(|(key, data)| {
            let storage_conf = storage_conf.clone();
            let session = Arc::clone(&session);

            async move {
                match data.fetch(&storage_conf, session).await {
                    Ok(storage) => Ok((key, storage)),
                    Err(e) => Err(e),
                }
            }
        });

        try_join_all(fetch_futures).await.map(|results| {
            results
                .into_iter()
                .collect::<HashMap<String, DataStorage>>()
        })
    }
}

impl std::ops::Deref for PortsData {
    type Target = HashMap<String, NexusData>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl std::ops::DerefMut for PortsData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}

impl<'de> serde::Deserialize<'de> for PortsData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct VecMapWrapper {
            contents: Vec<VecMapEntry>,
        }

        #[derive(Deserialize)]
        struct VecMapEntry {
            key: TypeName,
            value: NexusData,
        }

        let values = VecMapWrapper::deserialize(deserializer)?;

        Ok(PortsData {
            values: values
                .contents
                .into_iter()
                .map(|entry| (entry.key.name, entry.value))
                .collect(),
        })
    }
}

impl serde::Serialize for PortsData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct VecMapEntry {
            key: TypeName,
            value: NexusData,
        }

        #[derive(Serialize)]
        struct VecMapWrapper {
            contents: Vec<VecMapEntry>,
        }

        let contents: Vec<VecMapEntry> = self
            .values
            .iter()
            .map(|(key, value)| VecMapEntry {
                key: TypeName::new(key),
                value: value.clone(),
            })
            .collect();

        VecMapWrapper { contents }.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::nexus_mocks, serde_json::json};

    fn sample_ports_data() -> PortsData {
        let mut values = HashMap::new();
        values.insert(
            "port1".to_string(),
            NexusData::new_inline(json!({ "key": "value" })),
        );
        PortsData { values }
    }

    #[test]
    fn test_ser_deser_ports_data() {
        let ports_data = sample_ports_data();
        let json = serde_json::to_string(&ports_data).unwrap();

        assert_eq!(
            json,
            r#"{"contents":[{"key":{"name":"port1"},"value":{"storage":[105,110,108,105,110,101],"one":[123,34,107,101,121,34,58,34,118,97,108,117,101,34,125],"many":[],"encrypted":false}}]}"#
        );

        let deserialized: PortsData = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ports_data);
    }

    #[test]
    fn test_into_map_returns_inner_hashmap() {
        let ports_data = sample_ports_data();
        let map = ports_data.clone().into_map();

        // The map should contain the same entries as the original PortsData
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("port1"));
        assert_eq!(map.get("port1"), ports_data.values.get("port1"));
    }

    #[test]
    fn test_into_map_empty_ports_data() {
        let ports_data = PortsData {
            values: HashMap::new(),
        };
        let map = ports_data.into_map();
        assert!(map.is_empty());
    }

    #[tokio::test]
    async fn test_commit_all_success() {
        let mut values = HashMap::new();
        values.insert(
            "port1".to_string(),
            NexusData::new_inline(json!({ "key": "value" })),
        );
        let ports_data = PortsData { values };

        let storage_conf = StorageConf::default();
        let (session, _) = nexus_mocks::mock_session();

        let result = ports_data
            .clone()
            .commit_all(&storage_conf, session.clone())
            .await;
        assert!(result.is_ok());
        let map = result.unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("port1"));
    }

    #[tokio::test]
    async fn test_fetch_all_success() {
        let mut values = HashMap::new();
        values.insert(
            "port1".to_string(),
            NexusData::new_inline(json!({ "key": "value" })),
        );
        let ports_data = PortsData { values };

        let storage_conf = StorageConf::default();
        let (session, _) = nexus_mocks::mock_session();

        let result = ports_data
            .clone()
            .fetch_all(&storage_conf, session.clone())
            .await;
        assert!(result.is_ok());
        let map = result.unwrap();
        assert_eq!(map.len(), 1);
        assert!(map.contains_key("port1"));
    }

    #[tokio::test]
    async fn test_commit_all_empty_ports_data() {
        let ports_data = PortsData {
            values: HashMap::new(),
        };
        let storage_conf = StorageConf::default();
        let (session, _) = nexus_mocks::mock_session();

        let result = ports_data.commit_all(&storage_conf, session).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fetch_all_empty_ports_data() {
        let ports_data = PortsData {
            values: HashMap::new(),
        };
        let storage_conf = StorageConf::default();
        let (session, _) = nexus_mocks::mock_session();

        let result = ports_data.fetch_all(&storage_conf, session).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
