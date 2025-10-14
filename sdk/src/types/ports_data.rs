//! Module defines [`PortsData`] - struct that represents data stored on-chain
//! in relation to their variants and ports. This can deserialize directly to
//! [`crate::nexus_data::NexusData`]

use {
    crate::types::{NexusData, TypeName},
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortsData {
    pub values: HashMap<TypeName, NexusData>,
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
                .map(|entry| (entry.key, entry.value))
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
        struct VecMapEntry<'a> {
            key: &'a TypeName,
            value: &'a NexusData,
        }

        #[derive(Serialize)]
        struct VecMapWrapper<'a> {
            contents: Vec<VecMapEntry<'a>>,
        }

        let contents: Vec<VecMapEntry> = self
            .values
            .iter()
            .map(|(key, value)| VecMapEntry { key, value })
            .collect();

        VecMapWrapper { contents }.serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    fn sample_ports_data() -> PortsData {
        let mut values = HashMap::new();
        values.insert(
            TypeName::new("port1"),
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
}
