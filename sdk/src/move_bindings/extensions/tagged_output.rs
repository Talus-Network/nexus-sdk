//! Readable projections for the published ordered `TaggedOutput` storage layout.

use {
    crate::move_bindings::primitives::tagged_output::TaggedOutput,
    anyhow::Context as _,
    serde_json::{json, Value},
};

impl TaggedOutput {
    /// Encodes readable JSON while retaining raw payload order and duplicate names.
    pub fn to_json_value(&self) -> anyhow::Result<Value> {
        let tag = std::str::from_utf8(&self.tag).context("output tag is not UTF-8")?;
        let named_payload = self
            .named_payload
            .contents
            .iter()
            .map(|entry| {
                let port_name =
                    std::str::from_utf8(&entry.key).context("raw output port name is not UTF-8")?;
                Ok(json!({
                    "port_name": port_name,
                    "value": entry.value.to_json_value()?,
                }))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(json!({ "tag": tag, "named_payload": named_payload }))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                primitives::data::{NexusData as StoredNexusData, NexusValue},
                sui_framework::vec_map::{Entry, VecMap},
            },
            types::NexusData,
        },
    };

    fn stored_inline(value: &'static [u8]) -> StoredNexusData {
        let value = NexusValue::inline_data(value).expect("fixture is bounded");
        StoredNexusData::new(
            b"nexus_value".to_vec(),
            bcs::to_bytes(&value).expect("fixture should encode"),
            Vec::new(),
        )
    }

    #[test]
    fn raw_output_json_projection_preserves_duplicate_payload_order() {
        let output = TaggedOutput::new(
            b"Ok".to_vec(),
            VecMap::new(vec![
                Entry::new(b"result".to_vec(), stored_inline(b"one")),
                Entry::new(b"result".to_vec(), stored_inline(b"one")),
            ]),
        );
        let json = output.to_json_value().expect("output should encode");

        assert_eq!(json["named_payload"][0]["port_name"], "result");
        assert_eq!(
            json["named_payload"][0]["value"],
            NexusData::inline_data(b"one")
                .expect("fixture is bounded")
                .to_json_value()
                .expect("fixture should project")
        );
    }
}
