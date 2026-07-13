//! Nexus helpers for the generated on chain [`crate::move_bindings::registry::tool_registry::Tool`]
//! representation.
//!
//! The persisted object shape is generated from Move. This module must not
//! duplicate that shape; it only adds SDK projections that are not part of the
//! ABI itself.

pub use tool_registry::{Tool, ToolRef};
use {
    crate::{
        move_bindings::{move_std::ascii, registry::tool_registry},
        sui,
        ToolFqn,
    },
    anyhow::{anyhow, bail, Context as _},
    chrono::{DateTime, Utc},
};

impl Tool {
    /// Derive a `Tool` object ID from the `ToolRegistry` ID and tool FQN.
    pub fn derive_id(
        registry_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::Address> {
        crate::move_bindings::derive_tool_id(registry_id, fqn)
    }

    pub fn object_id(&self) -> sui::types::Address {
        self.id.address()
    }

    pub fn registry_id(&self) -> sui::types::Address {
        self.registry.address()
    }

    pub fn fqn_string(&self) -> anyhow::Result<String> {
        ascii_string(&self.fqn).context("Tool FQN is not UTF-8")
    }

    pub fn parsed_fqn(&self) -> anyhow::Result<ToolFqn> {
        let value = self.fqn_string()?;
        value
            .parse()
            .map_err(|error| anyhow!("Tool FQN '{value}' did not parse: {error}"))
    }

    pub fn reference(&self) -> &ToolRef {
        &self.r#ref
    }

    pub fn input_schema_json(&self) -> anyhow::Result<serde_json::Value> {
        serde_json::from_slice(&self.input_schema).context("Tool input schema is not valid JSON")
    }

    pub fn output_schema_json(&self) -> anyhow::Result<serde_json::Value> {
        serde_json::from_slice(&self.output_schema).context("Tool output schema is not valid JSON")
    }

    /// Validate the provided JSON input against the tool's input schema.
    pub fn validate_input(&self, data: &serde_json::Value) -> anyhow::Result<()> {
        let schema = self.input_schema_json()?;
        match jsonschema::draft202012::validate(&schema, data) {
            Ok(()) => Ok(()),
            Err(error) => anyhow::bail!("Input data does not match the input schema: {error}"),
        }
    }

    /// Validate the provided JSON output against the tool's output schema.
    pub fn validate_output(&self, data: &serde_json::Value) -> anyhow::Result<()> {
        let schema = self.output_schema_json()?;
        match jsonschema::draft202012::validate(&schema, data) {
            Ok(()) => Ok(()),
            Err(error) => anyhow::bail!("Output data does not match the output schema: {error}"),
        }
    }

    pub fn description_string(&self) -> anyhow::Result<String> {
        std::str::from_utf8(&self.description)
            .map(str::to_owned)
            .context("Tool description is not UTF-8")
    }

    pub fn registered_at_datetime(&self) -> anyhow::Result<DateTime<Utc>> {
        timestamp_millis_to_datetime(self.registered_at_ms, "registered_at_ms")
    }

    pub fn unregistered_at_datetime(&self) -> anyhow::Result<Option<DateTime<Utc>>> {
        match self.unregistered_at_ms.vec.as_slice() {
            [] => Ok(None),
            [millis] => timestamp_millis_to_datetime(*millis, "unregistered_at_ms").map(Some),
            values => bail!(
                "Tool unregistered_at_ms is not a valid Move option: {} values",
                values.len()
            ),
        }
    }

    pub fn unregistered_at_millis(&self) -> anyhow::Result<Option<u64>> {
        match self.unregistered_at_ms.vec.as_slice() {
            [] => Ok(None),
            [millis] => Ok(Some(*millis)),
            values => bail!(
                "Tool unregistered_at_ms is not a valid Move option: {} values",
                values.len()
            ),
        }
    }
}

impl ToolRef {
    pub fn http_url_string(&self) -> anyhow::Result<Option<String>> {
        let Self::Http { url, .. } = self else {
            return Ok(None);
        };
        bytes_string(url, "HTTP tool URL").map(Some)
    }

    pub fn sui_parts(
        &self,
    ) -> anyhow::Result<Option<(sui::types::Address, String, sui::types::Address)>> {
        let Self::Sui {
            package_address,
            module_name,
            tool_witness_id,
            ..
        } = self
        else {
            return Ok(None);
        };

        Ok(Some((
            *package_address,
            ascii_string(module_name).context("Sui tool module name is not UTF-8")?,
            tool_witness_id.address(),
        )))
    }

    pub fn display_string(&self) -> anyhow::Result<String> {
        match self {
            Self::Http { .. } => self
                .http_url_string()?
                .ok_or_else(|| anyhow!("expected HTTP tool reference")),
            Self::Sui { .. } => {
                let Some((package_address, module_name, tool_witness_id)) = self.sui_parts()?
                else {
                    unreachable!("matched Sui tool reference")
                };
                Ok(format!(
                    "{package_address}::{module_name}@{tool_witness_id}"
                ))
            }
        }
    }
}

impl std::fmt::Display for ToolRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.display_string() {
            Ok(value) => f.write_str(&value),
            Err(_) => f.write_str("<invalid tool reference>"),
        }
    }
}

fn ascii_string(value: &ascii::String) -> anyhow::Result<String> {
    bytes_string(&value.bytes, "Move ASCII string")
}

fn bytes_string(value: &[u8], label: &str) -> anyhow::Result<String> {
    std::str::from_utf8(value)
        .map(str::to_owned)
        .with_context(|| format!("{label} is not UTF-8"))
}

fn timestamp_millis_to_datetime(value: u64, field: &str) -> anyhow::Result<DateTime<Utc>> {
    let millis = i64::try_from(value).with_context(|| format!("{field} exceeds i64::MAX"))?;
    DateTime::<Utc>::from_timestamp_millis(millis)
        .ok_or_else(|| anyhow!("{field} is outside chrono's timestamp range"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fqn,
            move_bindings::{move_std::option::Option as MoveOption, sui_framework},
            test_utils::sui_mocks,
        },
    };

    fn ascii(value: &str) -> ascii::String {
        ascii::String::from(value)
    }

    fn fixture_tool(reference: ToolRef, input_schema: Vec<u8>, output_schema: Vec<u8>) -> Tool {
        Tool {
            id: crate::move_bindings::sui_framework::object::UID::new(sui_mocks::mock_sui_address()),
            registry: crate::move_bindings::sui_framework::object::ID::new(
                sui_mocks::mock_sui_address(),
            ),
            fqn: ascii("xyz.taluslabs.math.i64.add@1"),
            r#ref: reference,
            description: b"A test tool".to_vec(),
            input_schema,
            output_schema,
            verified: false,
            vault: sui_framework::balance::Balance {
                value: 0,
                phantom_t0: std::marker::PhantomData,
            },
            supported_verifier_methods: vec![],
            workflow_authorization_cap_first: true,
            lock_duration_ms: 0,
            registered_at_ms: 0,
            unregistered_at_ms: MoveOption::from(None),
        }
    }

    #[test]
    fn test_tool_derive_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        let fqn = fqn!("xyz.taluslabs.math.i64.add@1");
        let expected_id = sui::types::Address::from_static(
            "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35",
        );
        assert_eq!(Tool::derive_id(registry_id, &fqn).unwrap(), expected_id);

        let fqn = fqn!("xyz.taluslabs.math.i64.mul@1");
        let expected_id = sui::types::Address::from_static(
            "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da",
        );
        assert_eq!(Tool::derive_id(registry_id, &fqn).unwrap(), expected_id);
    }

    #[test]
    fn generated_tool_bcs_roundtrips_and_preserves_schema_bytes() {
        let tool = fixture_tool(
            ToolRef::Http {
                _variant_name: ascii("Http"),
                url: b"https://example.com/tool".to_vec(),
            },
            b"input schema bytes".to_vec(),
            b"output schema bytes".to_vec(),
        );

        let bytes = bcs::to_bytes(&tool).expect("generated Tool serializes as BCS");
        let decoded: Tool = bcs::from_bytes(&bytes).expect("generated Tool deserializes as BCS");

        assert_eq!(
            decoded.fqn_string().unwrap(),
            "xyz.taluslabs.math.i64.add@1"
        );
        assert_eq!(decoded.description_string().unwrap(), "A test tool");
        assert_eq!(
            decoded.registered_at_datetime().unwrap(),
            DateTime::<Utc>::from_timestamp(0, 0).unwrap()
        );
        assert_eq!(decoded.input_schema, b"input schema bytes");
        assert_eq!(decoded.output_schema, b"output schema bytes");
    }

    #[test]
    fn tool_ref_helpers_decode_http_and_sui() {
        let http_ref = ToolRef::Http {
            _variant_name: ascii("Http"),
            url: b"https://example.com/tool".to_vec(),
        };
        assert_eq!(
            http_ref.http_url_string().unwrap().unwrap().as_str(),
            "https://example.com/tool"
        );
        assert_eq!(http_ref.to_string(), "https://example.com/tool");

        let package_address = sui::types::Address::from_static(
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        );
        let tool_witness_id = sui::types::Address::from_static(
            "0xabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd",
        );
        let sui_ref = ToolRef::Sui {
            _variant_name: ascii("Sui"),
            package_address,
            module_name: ascii("my_tool_module"),
            tool_witness_id: crate::move_bindings::sui_framework::object::ID::new(tool_witness_id),
        };
        let (package, module, witness) = sui_ref.sui_parts().unwrap().unwrap();
        assert_eq!(package, package_address);
        assert_eq!(module, "my_tool_module");
        assert_eq!(witness, tool_witness_id);
        assert_eq!(
            sui_ref.to_string(),
            format!("{package_address}::my_tool_module@{tool_witness_id}")
        );
    }
}
