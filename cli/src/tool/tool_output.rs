use {
    nexus_sdk::{
        move_bindings::tool::{
            external_verifier::ExternalVerifier,
            tool_registry::{ToolLifecycle, ToolVerifierContract},
        },
        sui,
        types::{ToolRef, ToolState},
    },
    serde::Serialize,
};

/// Semantic CLI view of [`ToolState`].
#[derive(Debug, Serialize)]
pub(super) struct ToolOutput {
    pub(super) tool_id: sui::types::Address,
    pub(super) registry_id: sui::types::Address,
    pub(super) fqn: String,
    pub(super) reference: ToolReferenceOutput,
    pub(super) description: String,
    pub(super) meta_schema: serde_json::Value,
    pub(super) timeout_ms: u64,
    pub(super) verifier_contract: ToolVerifierContractOutput,
    pub(super) workflow_authorization_cap_first: bool,
    pub(super) invocation_cost_mist: u64,
    pub(super) vault_balance: u64,
    pub(super) lock_duration_ms: u64,
    pub(super) registered_at: chrono::DateTime<chrono::Utc>,
    pub(super) lifecycle: ToolLifecycleOutput,
}

impl ToolOutput {
    pub(super) fn try_from_state(tool: &ToolState) -> anyhow::Result<Self> {
        Ok(Self {
            tool_id: tool.object_id,
            registry_id: tool.registry_id(),
            fqn: tool.fqn_string()?,
            reference: ToolReferenceOutput::try_from(tool.reference())?,
            description: tool.description_string()?,
            meta_schema: tool.meta_schema.to_json_value()?,
            timeout_ms: tool.timeout_ms,
            verifier_contract: ToolVerifierContractOutput::try_from(&tool.verifier_contract)?,
            workflow_authorization_cap_first: tool.workflow_authorization_cap_first,
            invocation_cost_mist: tool.invocation_cost_mist,
            vault_balance: tool.inner.vault.value,
            lock_duration_ms: tool.inner.lock_duration_ms,
            registered_at: tool.registered_at_datetime()?,
            lifecycle: ToolLifecycleOutput::from(&tool.inner.lifecycle),
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

/// Semantic CLI view of [`ToolVerifierContract`].
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ToolVerifierContractOutput {
    None,
    RegisteredKey,
    External { verifier: ExternalVerifierOutput },
}

impl TryFrom<&ToolVerifierContract> for ToolVerifierContractOutput {
    type Error = anyhow::Error;

    fn try_from(contract: &ToolVerifierContract) -> Result<Self, Self::Error> {
        match contract {
            ToolVerifierContract::None => Ok(Self::None),
            ToolVerifierContract::RegisteredKey => Ok(Self::RegisteredKey),
            ToolVerifierContract::External { pos0 } => Ok(Self::External {
                verifier: ExternalVerifierOutput::try_from(pos0)?,
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
struct VerifierMethodOutput {
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

/// Stable CLI lifecycle representation.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum ToolLifecycleOutput {
    Open,
    Closed { at_ms: u64 },
    Retired { at_ms: u64 },
}

impl From<&ToolLifecycle> for ToolLifecycleOutput {
    fn from(lifecycle: &ToolLifecycle) -> Self {
        match lifecycle {
            ToolLifecycle::Open => Self::Open,
            ToolLifecycle::Closed { at_ms } => Self::Closed { at_ms: *at_ms },
            ToolLifecycle::Retired { at_ms } => Self::Retired { at_ms: *at_ms },
        }
    }
}
