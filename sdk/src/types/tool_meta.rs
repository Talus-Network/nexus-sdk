use {
    crate::{move_bindings::interface::meta_schema::MetaSchema, ToolFqn},
    std::time::Duration,
};

/// Byte owned tool metadata used to register an off chain tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolMeta {
    pub fqn: ToolFqn,
    pub url: String,
    pub description: String,
    pub timeout: Duration,
    pub input_schema: Vec<u8>,
    pub output_schema: Vec<u8>,
}

impl ToolMeta {
    /// Validates metadata that must be safe and useful before registration.
    ///
    /// # Errors
    ///
    /// Returns an error when the description contains no meaningful text.
    pub fn validate_registration(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.description.trim().is_empty(),
            "Tool description must not be empty"
        );
        Ok(())
    }

    /// Converts human-readable Schemars documents into the immutable on-chain schema.
    pub fn meta_schema(&self) -> anyhow::Result<MetaSchema> {
        MetaSchema::from_offchain_json_schemas(&self.input_schema, &self.output_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(description: &str) -> ToolMeta {
        ToolMeta {
            fqn: "xyz.taluslabs.example@1".parse().unwrap(),
            url: "https://example.invalid/tool".to_owned(),
            description: description.to_owned(),
            timeout: Duration::from_secs(10),
            input_schema: br#"{"type":"object","properties":{}}"#.to_vec(),
            output_schema: br#"{"oneOf":[{"type":"object","properties":{}}]}"#.to_vec(),
        }
    }

    #[test]
    fn registration_rejects_an_empty_description() {
        let error = metadata(" \n\t")
            .validate_registration()
            .expect_err("blank Tool descriptions must be rejected");

        assert_eq!(error.to_string(), "Tool description must not be empty");
    }

    #[test]
    fn registration_accepts_a_meaningful_description() {
        metadata("Returns an example result.")
            .validate_registration()
            .expect("meaningful Tool metadata must be accepted");
    }
}
