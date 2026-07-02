//! Helpers for generated input/output port data maps.

use {
    crate::move_bindings::{
        interface::graph::{InputPort, OutputPort},
        move_std::ascii::String as MoveString,
        primitives::data::NexusData,
        sui_framework::vec_map::{Entry as VecMapEntry, VecMap},
    },
    std::collections::HashMap,
};

impl VecMap<InputPort, NexusData> {
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
}

impl VecMap<OutputPort, NexusData> {
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
                    key: OutputPort {
                        name: MoveString::from(key),
                    },
                    value,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inline_bytes(value: &'static [u8]) -> NexusData {
        NexusData::inline_one(value.to_vec())
    }

    fn sample_ports_data() -> VecMap<InputPort, NexusData> {
        let mut values = HashMap::new();
        values.insert("port1".to_string(), inline_bytes(b"port-value"));
        VecMap::<InputPort, NexusData>::from_map(values)
    }

    #[test]
    fn test_bcs_roundtrip_ports_data() {
        let ports_data = sample_ports_data();
        let bytes = bcs::to_bytes(&ports_data).unwrap();
        let decoded: VecMap<InputPort, NexusData> = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, ports_data);
        assert_eq!(decoded.contents[0].value.one, b"port-value");
    }

    #[test]
    fn test_into_map_returns_inner_hashmap() {
        let ports_data = sample_ports_data();
        let map = ports_data.clone().into_map();

        assert_eq!(map.len(), 1);
        assert_eq!(map.get("port1").unwrap(), &inline_bytes(b"port-value"));
    }

    #[test]
    fn test_into_map_empty_ports_data() {
        let ports_data = VecMap::<InputPort, NexusData>::from_map(HashMap::new());
        let map = ports_data.into_map();
        assert!(map.is_empty());
    }
}
