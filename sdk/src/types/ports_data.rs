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

// TODO:
// #[cfg(test)]
// mod tests {
//     use {super::*, serde_json::json};

//     fn sample_ports_data() -> PortsData {
//         let mut values = HashMap::new();
//         values.insert(
//             TypeName::new("port1"),
//             NexusData::new_inline(json!("value1")),
//         );
//         values.insert(TypeName::new("port2"), NexusData::new_inline(json!(42)));
//         PortsData { values }
//     }

//     #[test]
//     fn test_serialize_ports_data() {
//         let ports_data = sample_ports_data();
//         let json = serde_json::to_string(&ports_data).unwrap();
//         assert!(json.contains("port1"));
//         assert!(json.contains("value1"));
//         assert!(json.contains("port2"));
//         assert!(json.contains("42"));
//     }

//     #[test]
//     fn test_deserialize_ports_data() {
//         let json = r#"
//         {
//             "contents": [
//                 { "key": "port1", "value": { "String": "value1" } },
//                 { "key": "port2", "value": { "Int": 42 } }
//             ]
//         }
//         "#;
//         let ports_data: PortsData = serde_json::from_str(json).unwrap();
//         assert_eq!(ports_data.values.len(), 2);
//         assert_eq!(
//             ports_data.values.get(&TypeName::from("port1")),
//             Some(&NexusData::String("value1".to_string()))
//         );
//         assert_eq!(
//             ports_data.values.get(&TypeName::from("port2")),
//             Some(&NexusData::Int(42))
//         );
//     }

//     #[test]
//     fn test_roundtrip_ser_de() {
//         let ports_data = sample_ports_data();
//         let json = serde_json::to_string(&ports_data).unwrap();
//         let deserialized: PortsData = serde_json::from_str(&json).unwrap();
//         assert_eq!(ports_data.values, deserialized.values);
//     }

//     #[test]
//     fn test_empty_ports_data() {
//         let ports_data = PortsData {
//             values: HashMap::new(),
//         };
//         let json = serde_json::to_string(&ports_data).unwrap();
//         let deserialized: PortsData = serde_json::from_str(&json).unwrap();
//         assert!(deserialized.values.is_empty());
//     }
//     fn sample_ports_data() -> PortsData {
//         let mut values = HashMap::new();
//         values.insert(
//             TypeName::from("port1"),
//             NexusData::new_inline(serde_json::json!("value1")),
//         );
//         values.insert(TypeName::from("port2"), NexusData::Int(42));
//         PortsData { values }
//     }

//     #[test]
//     fn test_serialize_ports_data() {
//         let ports_data = sample_ports_data();
//         let json = serde_json::to_string(&ports_data).unwrap();
//         assert!(json.contains("port1"));
//         assert!(json.contains("value1"));
//         assert!(json.contains("port2"));
//         assert!(json.contains("42"));
//     }

//     #[test]
//     fn test_deserialize_ports_data() {
//         let json = r#"
//     {
//         "contents": [
//             { "key": "port1", "value": { "Inline": "value1" } },
//             { "key": "port2", "value": { "Int": 42 } }
//         ]
//     }
//     "#;
//         let ports_data: PortsData = serde_json::from_str(json).unwrap();
//         assert_eq!(ports_data.values.len(), 2);
//         assert_eq!(
//             ports_data.values.get(&TypeName::from("port1")),
//             Some(&NexusData::new_inline(serde_json::json!("value1")))
//         );
//         assert_eq!(
//             ports_data.values.get(&TypeName::from("port2")),
//             Some(&NexusData::Int(42))
//         );
//     }

//     #[test]
//     fn test_roundtrip_ser_de() {
//         let ports_data = sample_ports_data();
//         let json = serde_json::to_string(&ports_data).unwrap();
//         let deserialized: PortsData = serde_json::from_str(&json).unwrap();
//         assert_eq!(ports_data.values, deserialized.values);
//     }

//     #[test]
//     fn test_empty_ports_data() {
//         let ports_data = PortsData {
//             values: HashMap::new(),
//         };
//         let json = serde_json::to_string(&ports_data).unwrap();
//         let deserialized: PortsData = serde_json::from_str(&json).unwrap();
//         assert!(deserialized.values.is_empty());
//     }
// }
