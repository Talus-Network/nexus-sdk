//! This module provides a model for on-chain representation of a tool with some
//! added logic like object ID derivation and input/output validation.

use {
    crate::{
        idents::move_std,
        sui,
        types::serde_parsers::{
            deserialize_bytes_to_json_value,
            deserialize_bytes_to_string,
            deserialize_bytes_to_url,
            deserialize_string_to_datetime,
            serialize_datetime_to_string,
            serialize_json_value_to_bytes,
            serialize_string_to_bytes,
            serialize_url_to_bytes,
        },
        ToolFqn,
    },
    serde::{Deserialize, Serialize},
    sui::traits::ToBcs,
};

/// A [`ToolRef`] is the differentiating enum between HTTP and Sui hosted tools.
///
/// HTTP tools are represented by their URL, while Sui tools are represented by
/// a Sui package address, module name, and witness ID.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "_variant_name")]
pub enum ToolRef {
    /// An off-chain tool represented by an HTTP URL.
    Http {
        #[serde(
            serialize_with = "serialize_url_to_bytes",
            deserialize_with = "deserialize_bytes_to_url"
        )]
        url: reqwest::Url,
    },
    /// An on-chain tool represented by a Sui package address, module name, and witness ID.
    Sui {
        package_address: sui::types::Address,
        module_name: sui::types::Identifier,
        witness_id: sui::types::Address,
    },
}

/// A [`Tool`] represents a tool that can be either on-chain or off-chain. This
/// structure matches the on-chain representation of a tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Tool {
    pub id: sui::types::Address,
    pub fqn: ToolFqn,
    #[serde(rename = "ref")]
    pub reference: ToolRef,
    #[serde(
        serialize_with = "serialize_string_to_bytes",
        deserialize_with = "deserialize_bytes_to_string"
    )]
    pub description: String,
    #[serde(
        serialize_with = "serialize_json_value_to_bytes",
        deserialize_with = "deserialize_bytes_to_json_value"
    )]
    pub input_schema: serde_json::Value,
    #[serde(
        serialize_with = "serialize_json_value_to_bytes",
        deserialize_with = "deserialize_bytes_to_json_value"
    )]
    pub output_schema: serde_json::Value,
    #[serde(
        serialize_with = "serialize_datetime_to_string",
        deserialize_with = "deserialize_string_to_datetime"
    )]
    pub registered_at_ms: chrono::DateTime<chrono::Utc>,
}

impl Tool {
    /// Validate the provided [serde_json::Value] against the tool's input schema.
    pub fn validate_input(&self, data: &serde_json::Value) -> Result<(), anyhow::Error> {
        match jsonschema::draft202012::validate(&self.input_schema, data) {
            Ok(()) => Ok(()),
            Err(e) => anyhow::bail!("Input data does not match the input schema: {e}"),
        }
    }

    /// Validate the provided [serde_json::Value] against the tool's output schema.
    pub fn validate_output(&self, data: &serde_json::Value) -> Result<(), anyhow::Error> {
        match jsonschema::draft202012::validate(&self.output_schema, data) {
            Ok(()) => Ok(()),
            Err(e) => anyhow::bail!("Output data does not match the output schema: {e}"),
        }
    }

    /// Derive a Tool's ID from the ToolRegistry ID and ToolFqn.
    pub fn derive_id(
        registry_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::Address> {
        let key_type = move_std::into_type_tag(move_std::Ascii::STRING_TYPE);
        let key_bcs = fqn.to_bcs()?;

        Ok(registry_id.derive_object_id(&key_type, &key_bcs))
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::fqn};

    #[test]
    fn test_tool_derive_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        // 1
        let fqn = fqn!("xyz.taluslabs.math.i64.add@1");
        let expected_id = sui::types::Address::from_static(
            "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35",
        );
        let derived_id = Tool::derive_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);

        // 2
        let fqn = fqn!("xyz.taluslabs.math.i64.mul@1");
        let expected_id = sui::types::Address::from_static(
            "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da",
        );
        let derived_id = Tool::derive_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);
    }
}
