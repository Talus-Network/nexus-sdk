use {
    nexus_sdk::{
        move_bindings::{
            interface::verifier::{ToolVerifierSupport, VerifierMethodId},
            tool::external_verifier::ExternalVerifier,
        },
        sui,
        types::{ToolRef, ToolState},
    },
    serde::Serialize,
};

/// Semantic CLI view of [`ToolState`].
#[derive(Debug, Serialize)]
pub(super) struct ToolOutput {
    pub(super) minimum_protocol_version: u64,
    pub(super) registry_id: sui::types::Address,
    pub(super) fqn: String,
    pub(super) reference: ToolReferenceOutput,
    pub(super) description: String,
    pub(super) meta_schema: serde_json::Value,
    pub(super) verified: bool,
    pub(super) vault_balance: u64,
    pub(super) workflow_authorization_cap_first: bool,
    pub(super) lock_duration_ms: u64,
    pub(super) timeout_ms: Option<u64>,
    pub(super) registered_at: chrono::DateTime<chrono::Utc>,
    pub(super) unregistered_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ToolOutput {
    pub(super) fn try_from_state(
        tool: &ToolState,
        timeout_ms: Option<u64>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            minimum_protocol_version: tool.minimum_protocol_version,
            registry_id: tool.registry_id(),
            fqn: tool.fqn_string()?,
            reference: ToolReferenceOutput::try_from(tool.reference())?,
            description: tool.description_string()?,
            meta_schema: tool.meta_schema.to_json_value()?,
            verified: tool.verified,
            vault_balance: tool.vault.value,
            workflow_authorization_cap_first: tool.workflow_authorization_cap_first,
            lock_duration_ms: tool.lock_duration_ms,
            timeout_ms,
            registered_at: tool.registered_at_datetime()?,
            unregistered_at: tool.unregistered_at_datetime()?,
        })
    }
}

/// Semantic CLI view of [`ToolRef`].
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ToolReferenceOutput {
    Http {
        url: String,
    },
    Sui {
        package_id: sui::types::Address,
        module: String,
        witness_id: sui::types::Address,
    },
}

impl TryFrom<&ToolRef> for ToolReferenceOutput {
    type Error = anyhow::Error;

    fn try_from(reference: &ToolRef) -> Result<Self, Self::Error> {
        match reference {
            ToolRef::Http { .. } => Ok(Self::Http {
                url: reference
                    .http_url_string()?
                    .expect("matched HTTP Tool reference"),
            }),
            ToolRef::Sui { .. } => {
                let (package_id, module, witness_id) =
                    reference.sui_parts()?.expect("matched Sui Tool reference");
                Ok(Self::Sui {
                    package_id,
                    module,
                    witness_id,
                })
            }
        }
    }
}

impl std::fmt::Display for ToolReferenceOutput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http { url } => formatter.write_str(url),
            Self::Sui {
                package_id,
                module,
                witness_id,
            } => write!(formatter, "{package_id}::{module}@{witness_id}"),
        }
    }
}

/// Semantic CLI view of [`ToolVerifierSupport`].
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ToolVerifierSupportOutput {
    RegisteredKey,
    External { method: VerifierMethodOutput },
}

impl TryFrom<&ToolVerifierSupport> for ToolVerifierSupportOutput {
    type Error = anyhow::Error;

    fn try_from(support: &ToolVerifierSupport) -> Result<Self, Self::Error> {
        match support {
            ToolVerifierSupport::RegisteredKey => Ok(Self::RegisteredKey),
            ToolVerifierSupport::External { method_id } => Ok(Self::External {
                method: VerifierMethodOutput::try_from(method_id)?,
            }),
        }
    }
}

/// Semantic CLI view of [`VerifierMethodId`].
#[derive(Debug, Serialize)]
pub(super) struct VerifierMethodOutput {
    tool_id: sui::types::Address,
    package_id: sui::types::Address,
    module: String,
    function: String,
}

impl TryFrom<&VerifierMethodId> for VerifierMethodOutput {
    type Error = anyhow::Error;

    fn try_from(method: &VerifierMethodId) -> Result<Self, Self::Error> {
        Ok(Self {
            tool_id: method.tool_id.address(),
            package_id: method.package_id.address(),
            module: std::str::from_utf8(&method.module_name.bytes)?.to_owned(),
            function: std::str::from_utf8(&method.function_name.bytes)?.to_owned(),
        })
    }
}

/// Semantic CLI view of [`ExternalVerifier`].
#[derive(Debug, Serialize)]
pub(super) struct ExternalVerifierOutput {
    method: VerifierMethodOutput,
    witness_id: sui::types::Address,
    immutable_shared_object_ids: Vec<sui::types::Address>,
}

impl TryFrom<&ExternalVerifier> for ExternalVerifierOutput {
    type Error = anyhow::Error;

    fn try_from(verifier: &ExternalVerifier) -> Result<Self, Self::Error> {
        Ok(Self {
            method: VerifierMethodOutput::try_from(&verifier.method)?,
            witness_id: verifier.witness.address(),
            immutable_shared_object_ids: verifier
                .immutable_shared_objects
                .iter()
                .map(|object| object.address())
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            move_bindings::{
                interface::meta_schema::{MetaSchema, OutputVariantSchema},
                move_std::{ascii, option::Option as MoveOption},
                sui_framework::{balance::Balance, object::ID},
            },
            types::{ToolRef, ToolState},
        },
    };

    fn fixture_tool() -> ToolState {
        ToolState {
            minimum_protocol_version: 1,
            registry: ID::new(nexus_sdk::sui::types::Address::from_static("0x42")),
            fqn: ascii::String::from("xyz.taluslabs.example@1"),
            r#ref: ToolRef::Http {
                url: b"https://example.com/tool".to_vec(),
            },
            description: b"Example tool".to_vec(),
            meta_schema: MetaSchema::new(
                vec![],
                vec![OutputVariantSchema::new(b"Ok".to_vec(), vec![])],
            ),
            verified: true,
            vault: Balance {
                value: 25,
                phantom_t0: std::marker::PhantomData,
            },
            workflow_authorization_cap_first: false,
            lock_duration_ms: 5_000,
            registered_at_ms: 0,
            unregistered_at_ms: MoveOption::from(None),
        }
    }

    #[test]
    fn tool_output_replaces_move_storage_values_with_domain_values() {
        let output = ToolOutput::try_from_state(&fixture_tool(), Some(10_000))
            .expect("valid Tool state should project");
        let value = serde_json::to_value(output).expect("Tool output should serialize");

        assert_eq!(
            value,
            serde_json::json!({
                "minimum_protocol_version": 1,
                "registry_id": nexus_sdk::sui::types::Address::from_static("0x42").to_string(),
                "fqn": "xyz.taluslabs.example@1",
                "reference": {
                    "kind": "http",
                    "url": "https://example.com/tool",
                },
                "description": "Example tool",
                "meta_schema": {
                    "input_ports": [],
                    "output_variants": [{
                        "variant_name": "Ok",
                        "ports": [],
                    }],
                },
                "verified": true,
                "vault_balance": 25,
                "workflow_authorization_cap_first": false,
                "lock_duration_ms": 5_000,
                "timeout_ms": 10_000,
                "registered_at": "1970-01-01T00:00:00Z",
                "unregistered_at": null,
            })
        );
        assert!(!value.to_string().contains("\"bytes\""));
    }
}
