use {
    nexus_sdk::{
        move_bindings::{
            interface::verifier::ToolVerifierSupport,
            tool::external_verifier::ExternalVerifier,
        },
        nexus::tool::ToolInspection,
        sui,
        types::ToolRef,
    },
    serde::Serialize,
};

/// Semantic CLI view of one supported Tool inspection.
#[derive(Debug, Serialize)]
pub(super) struct ToolOutput {
    pub(super) tool_id: sui::types::Address,
    pub(super) registry_id: sui::types::Address,
    pub(super) fqn: String,
    pub(super) reference: ToolReferenceOutput,
    pub(super) description: String,
    pub(super) meta_schema: serde_json::Value,
    pub(super) verified: bool,
    pub(super) timeout_ms: Option<u64>,
    pub(super) verifier_support: Option<ToolVerifierSupportOutput>,
    pub(super) external_verifier: Option<ExternalVerifierOutput>,
    pub(super) workflow_authorization_cap_first: bool,
    pub(super) invocation_cost_mist: Option<u64>,
    pub(super) vault_balance: u64,
    pub(super) lock_duration_ms: u64,
    pub(super) registered_at: chrono::DateTime<chrono::Utc>,
    pub(super) registered: bool,
    pub(super) unregistered_at_ms: Option<u64>,
}

impl ToolOutput {
    pub(super) fn try_from_state(tool: &nexus_sdk::types::ToolState) -> anyhow::Result<Self> {
        let unregistered_at_ms = tool.unregistered_at_millis()?;
        Ok(Self {
            tool_id: tool.object_id,
            registry_id: tool.registry_id(),
            fqn: tool.fqn_string()?,
            reference: ToolReferenceOutput::try_from(tool.reference())?,
            description: tool.description_string()?,
            meta_schema: tool.inner.meta_schema.to_json_value()?,
            verified: tool.inner.verified,
            timeout_ms: None,
            verifier_support: None,
            external_verifier: None,
            workflow_authorization_cap_first: tool.inner.workflow_authorization_cap_first,
            invocation_cost_mist: None,
            vault_balance: tool.inner.vault.value,
            lock_duration_ms: tool.inner.lock_duration_ms,
            registered_at: tool.registered_at_datetime()?,
            registered: unregistered_at_ms.is_none(),
            unregistered_at_ms,
        })
    }

    pub(super) fn try_from_inspection(inspection: &ToolInspection) -> anyhow::Result<Self> {
        let tool = inspection
            .tool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Tool state is not available"))?;
        let unregistered_at_ms = tool.unregistered_at_millis()?;
        Ok(Self {
            tool_id: tool.object_id,
            registry_id: tool.registry_id(),
            fqn: tool.fqn_string()?,
            reference: ToolReferenceOutput::try_from(tool.reference())?,
            description: tool.description_string()?,
            meta_schema: tool.inner.meta_schema.to_json_value()?,
            verified: tool.inner.verified,
            timeout_ms: inspection.timeout_ms,
            verifier_support: inspection
                .verifier_support
                .as_ref()
                .map(ToolVerifierSupportOutput::try_from)
                .transpose()?,
            external_verifier: inspection
                .external_verifier
                .as_ref()
                .map(ExternalVerifierOutput::try_from)
                .transpose()?,
            workflow_authorization_cap_first: tool.inner.workflow_authorization_cap_first,
            invocation_cost_mist: inspection.invocation_cost_mist,
            vault_balance: tool.inner.vault.value,
            lock_duration_ms: tool.inner.lock_duration_ms,
            registered_at: tool.registered_at_datetime()?,
            registered: unregistered_at_ms.is_none(),
            unregistered_at_ms,
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

/// Semantic CLI view of current verifier support.
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

#[derive(Debug, Serialize)]
pub(super) struct VerifierMethodOutput {
    tool_id: sui::types::Address,
    package_id: sui::types::Address,
    module: String,
    function: String,
}

impl TryFrom<&nexus_sdk::move_bindings::interface::verifier::VerifierMethodId>
    for VerifierMethodOutput
{
    type Error = anyhow::Error;

    fn try_from(
        method: &nexus_sdk::move_bindings::interface::verifier::VerifierMethodId,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            tool_id: method.tool_id.address(),
            package_id: method.package_id.address(),
            module: std::str::from_utf8(&method.module_name.bytes)?.to_owned(),
            function: std::str::from_utf8(&method.function_name.bytes)?.to_owned(),
        })
    }
}
