//! Commands related to Nexus tool management.
//!
//! - [`ToolActions::update_timeout`] to update a tool's timeout.

use {
    crate::{
        events::NexusEventKind,
        move_bindings::{
            interface::payment::PaymentSourceKind,
            interface::verifier::ToolVerifierSupport,
            move_std::type_name::TypeName,
            tool::{
                external_verifier::ExternalVerifier,
                finite_credits,
                invocation::Invocation,
                time_pass,
                tool_cashier::{CashierDeposit, PolicyKey, ToolCashier, ToolCashierStateV1},
            },
        },
        nexus::{
            client::NexusClient,
            error::NexusError,
            registry::{
                fetch_current_tool_registration, fetch_external_verifier_record,
                fetch_tool_invocation_cost, preflight_external_verifier_registration,
            },
        },
        sui,
        transactions::{tool, tool_cashier},
        types::{Tool, ToolAnchor, ToolRef, ToolState},
        ToolFqn,
    },
    std::time::Duration,
};

pub struct UpdateToolTimeoutResult {
    pub tx_digest: sui::types::Digest,
}

pub struct ConfigureToolVerifierResult {
    pub tx_digest: sui::types::Digest,
    pub tool_id: sui::types::Address,
}

/// Result of a [`Tool`] cashier policy transaction.
pub struct ToolCashierActionResult {
    pub tx_digest: sui::types::Digest,
}

/// Confirmed entitlement purchase with both discoverable object IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntitlementPurchaseResult {
    pub tx_digest: sui::types::Digest,
    pub entitlement_id: sui::types::Address,
    pub deposit_id: sui::types::Address,
}

/// Confirmed owner issuance of one discoverable entitlement object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntitlementIssueResult {
    pub tx_digest: sui::types::Digest,
    pub entitlement_id: sui::types::Address,
}

/// Result of creating one independently usable finite credit object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiniteCreditsResult {
    pub tx_digest: sui::types::Digest,
    pub credits_id: sui::types::Address,
}

/// One refunded finite credit Invocation waiting to be claimed.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FiniteCreditRefund {
    pub object_ref: sui::types::ObjectReference,
    pub sources: Vec<sui::types::Address>,
}

/// Current sale terms for canonical finite Tool invocation credits.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FiniteCreditOffer {
    pub issuance_enabled: bool,
    pub price_per_credit: u64,
    pub minimum_credits: u64,
    pub maximum_credits: u64,
}

/// Current sale terms for canonical time based Tool access.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TimePassOffer {
    pub issuance_enabled: bool,
    pub price_per_ms: u64,
    pub minimum_duration_ms: u64,
    pub maximum_duration_ms: u64,
}

/// Discoverable economic policies accepted by one [`ToolCashier`].
///
/// [`ToolEconomy::policies`] preserves every owner supplied policy type.
/// Canonical offers are decoded into stable SDK values for direct use by
/// clients and command line tools.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ToolEconomy {
    pub tool_id: sui::types::Address,
    pub cashier_id: sui::types::Address,
    pub minimum_protocol_version: u64,
    pub policies: Vec<TypeName>,
    pub fixed_price_mist: Option<u64>,
    pub free_invocations: bool,
    pub finite_credits: Option<FiniteCreditOffer>,
    pub time_pass: Option<TimePassOffer>,
}

/// One finalized Invocation waiting in a [`ToolCashier`] inbox.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CollectableInvocation {
    pub object_ref: sui::types::ObjectReference,
    pub policy: TypeName,
    pub sources: Vec<sui::types::Address>,
    pub amount_mist: u64,
}

/// One prepaid sale deposit waiting in a [`ToolCashier`] inbox.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CollectableDeposit {
    pub object_ref: sui::types::ObjectReference,
    pub amount_mist: u64,
}

/// Indexed economic objects currently owned by one [`ToolCashier`].
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ToolCashierInbox {
    pub invocations: Vec<CollectableInvocation>,
    pub deposits: Vec<CollectableDeposit>,
}

/// Result of [`ToolActions::inspect_tool`].
///
/// The object IDs are derived locally even when the Tool does not exist.
/// An existing Tool includes its complete [`ToolState`] record.
#[derive(Clone, Debug)]
pub struct ToolInspection {
    pub fqn: ToolFqn,
    pub tool_id: sui::types::Address,
    pub tool_cashier_id: sui::types::Address,
    pub exists: bool,
    pub tool: Option<ToolState>,
    pub verifier_support: Option<ToolVerifierSupport>,
    pub external_verifier: Option<ExternalVerifier>,
    pub invocation_cost_mist: Option<u64>,
}

pub struct ToolActions {
    pub(super) client: NexusClient,
}

fn canonical_policy_accepted(
    objects: &crate::types::NexusObjects,
    policies: &[TypeName],
    module: &str,
) -> bool {
    let origin = objects.packages.tool.type_origin(module, "Policy");
    let expected = format!("{origin}::{module}::Policy");
    policies
        .iter()
        .any(|policy| policy.matches_qualified_name(&expected))
}

impl ToolActions {
    async fn resolve_tool_cashier_and_cap(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<(sui::types::ObjectReference, sui::types::ObjectReference), NexusError> {
        let tool_cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let cashier_admin = client
            .crawler()
            .get_object_metadata(cashier_admin)
            .await
            .map_err(|error| {
                NexusError::Configuration(format!(
                    "Tool '{tool_fqn}' cashier admin capability '{cashier_admin}' could not be resolved: {error}"
                ))
            })?
            .object_ref();
        Ok((tool_cashier, cashier_admin))
    }

    async fn resolve_tool_and_cashier_admin(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<(sui::types::ObjectReference, sui::types::ObjectReference), NexusError> {
        let tool = client.fetch_tool(tool_fqn).await?;
        let cashier_admin = client
            .crawler()
            .get_object_metadata(cashier_admin)
            .await
            .map_err(|error| {
                NexusError::Configuration(format!(
                    "Tool '{tool_fqn}' cashier admin capability '{cashier_admin}' could not be resolved: {error}"
                ))
            })?
            .object_ref();
        Ok((tool, cashier_admin))
    }

    async fn resolve_tool_and_owner_cap(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<
        (
            sui::types::Address,
            sui::types::ObjectReference,
            sui::types::ObjectReference,
        ),
        NexusError,
    > {
        let objects = &client.nexus_objects;
        let tool_id = Tool::derive_id(*objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool = client
            .crawler()
            .get_object_metadata(tool_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let owner_cap = client
            .crawler()
            .get_object_metadata(owner_cap)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        Ok((tool_id, tool, owner_cap))
    }

    /// Update a tool's timeout.
    pub async fn update_timeout(
        &self,
        tool_fqn: &ToolFqn,
        new_timeout: Duration,
        owner_cap: sui::types::Address,
    ) -> Result<UpdateToolTimeoutResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let nexus_objects = &client.nexus_objects;

        let owner_cap = client
            .crawler()
            .get_object_metadata(owner_cap)
            .await
            .map_err(NexusError::Rpc)?;

        // Derive and fetch the Tool object.
        let tool_ref = client.fetch_tool(tool_fqn).await?;

        let tx = tool::update_tool_timeout_ptb(
            nexus_objects,
            &tool_ref,
            &owner_cap.object_ref(),
            new_timeout,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = client.submit_transaction(tx, address).await?;

        Ok(UpdateToolTimeoutResult {
            tx_digest: response.digest,
        })
    }

    /// Configure an offchain Tool for the built-in RegisteredKey verifier.
    pub async fn configure_registered_key_verifier(
        &self,
        tool_fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ConfigureToolVerifierResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let objects = &client.nexus_objects;
        let (tool_id, tool, owner_cap) =
            Self::resolve_tool_and_owner_cap(&client, tool_fqn, owner_cap).await?;
        let binding_id = client
            .network_auth()
            .binding_object_id(
                &crate::move_bindings::registry::network_auth::IdentityKey::tool(tool_id),
            )
            .await?;
        let tool_key_binding = client
            .crawler()
            .get_object_metadata(binding_id)
            .await
            .map_err(|e| {
                NexusError::Configuration(format!(
                    "Tool '{tool_fqn}' has no NetworkAuth key binding at '{binding_id}': {e}"
                ))
            })?
            .object_ref();
        let tx = tool::configure_registered_key_verifier_ptb(
            objects,
            &tool,
            &owner_cap,
            &tool_key_binding,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(tx, address).await?;
        Ok(ConfigureToolVerifierResult {
            tx_digest: response.digest,
            tool_id,
        })
    }

    /// Preflight and register one Tool-bound External verifier.
    pub async fn configure_external_verifier(
        &self,
        tool_fqn: &ToolFqn,
        owner_cap: sui::types::Address,
        package_id: sui::types::Address,
        module_name: &str,
        function_name: &str,
        verifier_object_ids: &[sui::types::Address],
    ) -> Result<ConfigureToolVerifierResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let objects = &client.nexus_objects;
        let (tool_id, tool, owner_cap) =
            Self::resolve_tool_and_owner_cap(&client, tool_fqn, owner_cap).await?;
        let registration = preflight_external_verifier_registration(
            client.crawler(),
            objects,
            package_id,
            module_name,
            function_name,
            verifier_object_ids,
        )
        .await
        .map_err(|e| NexusError::Configuration(e.to_string()))?;
        let tx = tool::register_external_verifier_ptb(objects, &tool, &owner_cap, &registration)
            .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(tx, address).await?;
        Ok(ConfigureToolVerifierResult {
            tx_digest: response.digest,
            tool_id,
        })
    }

    /// Enables fixed price Invocation admission for a [`Tool`].
    pub async fn enable_fixed_price(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_fixed_price_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Disables fixed price admission without changing existing Invocations.
    pub async fn disable_fixed_price(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::disable_fixed_price_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Enables sponsored free Invocation admission for a [`Tool`].
    pub async fn enable_free_invocations(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_free_invocation_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Disables sponsored free admission without changing existing Invocations.
    pub async fn disable_free_invocations(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::disable_free_invocation_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Enables time pass sales and admission for a [`Tool`].
    pub async fn enable_time_passes(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        price_per_ms: u64,
        minimum_duration_ms: u64,
        maximum_duration_ms: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_time_pass_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
            price_per_ms,
            minimum_duration_ms,
            maximum_duration_ms,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Set the invocation price for a [`Tool`] in MIST.
    pub async fn set_invocation_cost(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        invocation_cost_mist: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool, cashier_admin) =
            Self::resolve_tool_and_cashier_admin(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool::set_invocation_cost_ptb(
            &client.nexus_objects,
            &tool,
            &cashier_admin,
            invocation_cost_mist,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Closes time pass issuance for a [`Tool`] without invalidating existing passes.
    pub async fn close_time_pass_issuance(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::close_time_pass_issuance_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Opens time pass issuance for a [`Tool`] using its current terms.
    pub async fn open_time_pass_issuance(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::open_time_pass_issuance_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Updates time pass terms for a [`Tool`] without invalidating existing passes.
    pub async fn update_time_pass_terms(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        price_per_ms: u64,
        minimum_duration_ms: u64,
        maximum_duration_ms: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::update_time_pass_terms_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
            price_per_ms,
            minimum_duration_ms,
            maximum_duration_ms,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Buys and freezes a time pass for a [`Tool`].
    pub async fn buy_time_pass(
        &self,
        tool_fqn: &ToolFqn,
        duration_ms: u64,
        pay_with: sui::types::Address,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = self.client.operation_client().await?;
        let beneficiary = PaymentSourceKind::user_funded(client.owner()?);
        self.buy_time_pass_with(&client, tool_fqn, duration_ms, pay_with, beneficiary)
            .await
    }

    /// Buys and freezes a time pass for an explicit user or Agent beneficiary.
    pub async fn buy_time_pass_for(
        &self,
        tool_fqn: &ToolFqn,
        duration_ms: u64,
        pay_with: sui::types::Address,
        beneficiary: PaymentSourceKind,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = self.client.operation_client().await?;
        self.buy_time_pass_with(&client, tool_fqn, duration_ms, pay_with, beneficiary)
            .await
    }

    async fn buy_time_pass_with(
        &self,
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        duration_ms: u64,
        pay_with: sui::types::Address,
        beneficiary: PaymentSourceKind,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        if duration_ms == 0 {
            return Err(NexusError::Configuration(
                "Time pass duration must be greater than zero".to_owned(),
            ));
        }
        let address = client.owner()?;
        let tool_id = Tool::derive_id(*client.nexus_objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool_cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let pay_with = client
            .crawler()
            .get_object_metadata(pay_with)
            .await
            .map_err(|error| {
                NexusError::Configuration(format!(
                    "Payment coin '{pay_with}' could not be resolved: {error}"
                ))
            })?
            .object_ref();
        let transaction = tool_cashier::buy_time_pass_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &pay_with,
            beneficiary,
            duration_ms,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        let entitlement_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::TimePassCreated(created)
                    if created.tool.bytes == tool_id
                        && created.cashier.bytes == *tool_cashier.object_id() =>
                {
                    Some(created.pass.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Time pass purchase '{}' emitted no TimePassCreatedEvent",
                    response.digest
                ))
            })?;
        let deposit_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::CashierDepositCreated(created)
                    if created.cashier.bytes == *tool_cashier.object_id() =>
                {
                    Some(created.deposit.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Time pass purchase '{}' emitted no CashierDepositCreatedEvent",
                    response.digest
                ))
            })?;
        Ok(EntitlementPurchaseResult {
            tx_digest: response.digest,
            entitlement_id,
            deposit_id,
        })
    }

    /// Issues and freezes a time pass under Tool owner authority.
    pub async fn issue_time_pass(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        beneficiary: PaymentSourceKind,
        valid_from_ms: u64,
        valid_until_ms: u64,
    ) -> Result<EntitlementIssueResult, NexusError> {
        if valid_from_ms >= valid_until_ms {
            return Err(NexusError::Configuration(
                "Time pass end must be after its start".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let tool_id = Tool::derive_id(*client.nexus_objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::issue_time_pass_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
            beneficiary,
            valid_from_ms,
            valid_until_ms,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        let entitlement_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::TimePassCreated(created)
                    if created.tool.bytes == tool_id
                        && created.cashier.bytes == *tool_cashier.object_id() =>
                {
                    Some(created.pass.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Time pass issuance '{}' emitted no TimePassCreatedEvent",
                    response.digest
                ))
            })?;
        Ok(EntitlementIssueResult {
            tx_digest: response.digest,
            entitlement_id,
        })
    }

    /// Enables finite credit sales and admission for a [`Tool`].
    pub async fn enable_finite_credits(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        price_per_credit: u64,
        minimum_credits: u64,
        maximum_credits: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if minimum_credits == 0 {
            return Err(NexusError::Configuration(
                "Minimum credits must be at least one".to_owned(),
            ));
        }
        if minimum_credits > maximum_credits {
            return Err(NexusError::Configuration(format!(
                "Minimum credits '{minimum_credits}' cannot exceed maximum credits '{maximum_credits}'"
            )));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_finite_credits_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
            price_per_credit,
            minimum_credits,
            maximum_credits,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Closes finite credit issuance for a [`Tool`] without invalidating existing credits.
    pub async fn close_finite_credit_issuance(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::close_finite_credit_issuance_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Opens finite credit issuance for a [`Tool`] using its current terms.
    pub async fn open_finite_credit_issuance(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::open_finite_credit_issuance_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Updates finite credit terms for a [`Tool`] without invalidating issued credits.
    pub async fn update_finite_credit_terms(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        price_per_credit: u64,
        minimum_credits: u64,
        maximum_credits: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if minimum_credits == 0 {
            return Err(NexusError::Configuration(
                "Minimum credits must be at least one".to_owned(),
            ));
        }
        if minimum_credits > maximum_credits {
            return Err(NexusError::Configuration(format!(
                "Minimum credits '{minimum_credits}' cannot exceed maximum credits '{maximum_credits}'"
            )));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::update_finite_credit_terms_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
            price_per_credit,
            minimum_credits,
            maximum_credits,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Buys shared finite credits for a [`Tool`].
    pub async fn buy_finite_credits(
        &self,
        tool_fqn: &ToolFqn,
        credits: u64,
        pay_with: sui::types::Address,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        self.buy_finite_credits_with(
            &client,
            tool_fqn,
            credits,
            pay_with,
            PaymentSourceKind::user_funded(address),
            address,
        )
        .await
    }

    /// Buys finite credits for an explicit beneficiary and refund recipient.
    pub async fn buy_finite_credits_for(
        &self,
        tool_fqn: &ToolFqn,
        credits: u64,
        pay_with: sui::types::Address,
        beneficiary: PaymentSourceKind,
        refund_to: sui::types::Address,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = self.client.operation_client().await?;
        self.buy_finite_credits_with(&client, tool_fqn, credits, pay_with, beneficiary, refund_to)
            .await
    }

    async fn buy_finite_credits_with(
        &self,
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        credits: u64,
        pay_with: sui::types::Address,
        beneficiary: PaymentSourceKind,
        refund_to: sui::types::Address,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        if credits == 0 {
            return Err(NexusError::Configuration(
                "Credits must be at least one".to_owned(),
            ));
        }
        let address = client.owner()?;
        let tool_id = Tool::derive_id(*client.nexus_objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool_cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let pay_with = client
            .crawler()
            .get_object_metadata(pay_with)
            .await
            .map_err(|error| {
                NexusError::Configuration(format!(
                    "Payment coin '{pay_with}' could not be resolved: {error}"
                ))
            })?
            .object_ref();
        let transaction = tool_cashier::buy_finite_credits_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &pay_with,
            beneficiary,
            refund_to,
            credits,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        let entitlement_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::CreditsCreated(created)
                    if created.tool.bytes == tool_id
                        && created.cashier.bytes == *tool_cashier.object_id() =>
                {
                    Some(created.credits.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Finite credit purchase '{}' emitted no CreditsCreatedEvent",
                    response.digest
                ))
            })?;
        let deposit_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::CashierDepositCreated(created)
                    if created.cashier.bytes == *tool_cashier.object_id() =>
                {
                    Some(created.deposit.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Finite credit purchase '{}' emitted no CashierDepositCreatedEvent",
                    response.digest
                ))
            })?;
        Ok(EntitlementPurchaseResult {
            tx_digest: response.digest,
            entitlement_id,
            deposit_id,
        })
    }

    /// Issues shared finite credits under Tool owner authority.
    pub async fn issue_finite_credits(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        beneficiary: PaymentSourceKind,
        refund_to: sui::types::Address,
        credits: u64,
    ) -> Result<EntitlementIssueResult, NexusError> {
        if credits == 0 {
            return Err(NexusError::Configuration(
                "Credits must be at least one".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let tool_id = Tool::derive_id(*client.nexus_objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let (tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::issue_finite_credits_ptb(
            &client.nexus_objects,
            &tool_cashier,
            &cashier_admin,
            beneficiary,
            refund_to,
            credits,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        let entitlement_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::CreditsCreated(created)
                    if created.tool.bytes == tool_id
                        && created.cashier.bytes == *tool_cashier.object_id() =>
                {
                    Some(created.credits.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Finite credit issuance '{}' emitted no CreditsCreatedEvent",
                    response.digest
                ))
            })?;
        Ok(EntitlementIssueResult {
            tx_digest: response.digest,
            entitlement_id,
        })
    }

    /// Splits shared finite credits into a second independently usable object.
    pub async fn split_finite_credits(
        &self,
        tool_fqn: &ToolFqn,
        credits_id: sui::types::Address,
        amount: u64,
    ) -> Result<FiniteCreditsResult, NexusError> {
        if amount == 0 {
            return Err(NexusError::Configuration(
                "Split credits must be at least one".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let sender = client.owner()?;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let credits = client
            .crawler()
            .get_object::<finite_credits::Credits>(credits_id)
            .await
            .map_err(NexusError::Rpc)?;
        if credits.data.cashier.bytes != *cashier.object_id() {
            return Err(NexusError::Configuration(format!(
                "Finite credits '{credits_id}' belong to another Tool"
            )));
        }
        if credits.data.refund_to != sender {
            return Err(NexusError::Configuration(format!(
                "Signer '{sender}' does not manage finite credits '{credits_id}'"
            )));
        }
        if amount > credits.data.remaining {
            return Err(NexusError::Configuration(format!(
                "Cannot split '{amount}' from '{}' remaining credits",
                credits.data.remaining
            )));
        }
        if !credits.is_shared() {
            return Err(NexusError::Configuration(format!(
                "Finite credits '{credits_id}' must be shared"
            )));
        }
        let transaction = tool_cashier::split_finite_credits_ptb(
            &client.nexus_objects,
            &credits.object_ref(),
            amount,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, sender).await?;
        let created = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::CreditsCreated(created)
                    if created.cashier.bytes == *cashier.object_id()
                        && created.credits.bytes != credits_id =>
                {
                    Some(created.credits.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Finite credit split '{}' emitted no new Credits object",
                    response.digest
                ))
            })?;
        Ok(FiniteCreditsResult {
            tx_digest: response.digest,
            credits_id: created,
        })
    }

    /// Joins one compatible shared finite credit object into another.
    pub async fn join_finite_credits(
        &self,
        tool_fqn: &ToolFqn,
        credits_id: sui::types::Address,
        other_credits_id: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if credits_id == other_credits_id {
            return Err(NexusError::Configuration(
                "Finite credit objects to join must be distinct".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let sender = client.owner()?;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let mut credits = client
            .crawler()
            .get_objects::<finite_credits::Credits>(&[credits_id, other_credits_id])
            .await
            .map_err(NexusError::Rpc)?;
        let other = credits.pop().ok_or_else(|| {
            NexusError::Configuration(format!(
                "Finite credits '{other_credits_id}' could not be resolved"
            ))
        })?;
        let credits = credits.pop().ok_or_else(|| {
            NexusError::Configuration(format!(
                "Finite credits '{credits_id}' could not be resolved"
            ))
        })?;
        if credits.object_id != credits_id || other.object_id != other_credits_id {
            return Err(NexusError::Configuration(
                "Finite credit object response order did not match the request".to_owned(),
            ));
        }
        let compatible = credits.data.tool == other.data.tool
            && credits.data.cashier == other.data.cashier
            && credits.data.beneficiary == other.data.beneficiary
            && credits.data.refund_to == other.data.refund_to;
        if credits.data.cashier.bytes != *cashier.object_id() || !compatible {
            return Err(NexusError::Configuration(
                "Finite credit objects are not compatible with this Tool".to_owned(),
            ));
        }
        if credits.data.refund_to != sender {
            return Err(NexusError::Configuration(format!(
                "Signer '{sender}' does not manage these finite credits"
            )));
        }
        if !credits.is_shared() || !other.is_shared() {
            return Err(NexusError::Configuration(
                "Finite credit objects to join must be shared".to_owned(),
            ));
        }
        let transaction = tool_cashier::join_finite_credits_ptb(
            &client.nexus_objects,
            &credits.object_ref(),
            &other.object_ref(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, sender).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Restores one refunded finite credit Invocation as a shared credit object.
    pub async fn claim_finite_credit_refund(
        &self,
        tool_fqn: &ToolFqn,
        invocation_id: sui::types::Address,
    ) -> Result<FiniteCreditsResult, NexusError> {
        let client = self.client.operation_client().await?;
        let sender = client.owner()?;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let refunded = client
            .crawler()
            .get_object::<Invocation>(invocation_id)
            .await
            .map_err(NexusError::Rpc)?;
        let expected_policy =
            crate::transactions::invocation::InvocationPolicyCall::finite_credits_policy(
                &client.nexus_objects,
            );
        if refunded.data.cashier_id.bytes != *cashier.object_id()
            || refunded.data.policy != expected_policy
        {
            return Err(NexusError::Configuration(format!(
                "Invocation '{invocation_id}' is not a finite credit refund for this Tool"
            )));
        }
        if refunded.owner != sui::types::Owner::Address(sender) {
            return Err(NexusError::Configuration(format!(
                "Invocation '{invocation_id}' is not owned by signer '{sender}'"
            )));
        }
        let transaction = tool_cashier::claim_finite_credit_refund_ptb(
            &client.nexus_objects,
            &refunded.object_ref(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, sender).await?;
        let credits_id = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::CreditsCreated(created)
                    if created.cashier.bytes == *cashier.object_id() =>
                {
                    Some(created.credits.bytes)
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Finite credit refund claim '{}' emitted no Credits object",
                    response.digest
                ))
            })?;
        Ok(FiniteCreditsResult {
            tx_digest: response.digest,
            credits_id,
        })
    }

    /// Lists finite credit refunds owned by `owner` for one [`Tool`].
    pub async fn inspect_finite_credit_refunds(
        &self,
        tool_fqn: &ToolFqn,
        owner: sui::types::Address,
    ) -> Result<Vec<FiniteCreditRefund>, NexusError> {
        let client = self.client.operation_client().await?;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let expected_policy =
            crate::transactions::invocation::InvocationPolicyCall::finite_credits_policy(
                &client.nexus_objects,
            );
        let refunds = client
            .crawler()
            .get_owned_objects::<Invocation>(
                owner,
                crate::move_bindings::struct_tag::<Invocation>(&client.nexus_objects),
            )
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .filter(|invocation| {
                invocation.data.cashier_id.bytes == *cashier.object_id()
                    && invocation.data.policy == expected_policy
            })
            .map(|invocation| FiniteCreditRefund {
                object_ref: invocation.object_ref(),
                sources: invocation
                    .data
                    .sources
                    .into_iter()
                    .map(|source| source.bytes)
                    .collect(),
            })
            .collect();
        Ok(refunds)
    }

    /// Reads every accepted policy and the canonical offers for one [`Tool`].
    ///
    /// Custom policy witness types remain visible in [`ToolEconomy::policies`]
    /// even when this SDK does not know how to decode their private configs.
    pub async fn inspect_economy(&self, tool_fqn: &ToolFqn) -> Result<ToolEconomy, NexusError> {
        let client = self.client.operation_client().await?;
        let objects = &client.nexus_objects;
        let crawler = client.crawler();
        let tool_id =
            crate::move_bindings::derive_tool_id(*objects.tool_registry.object_id(), tool_fqn)
                .map_err(NexusError::Parsing)?;
        let cashier_id = crate::move_bindings::derive_tool_cashier_id(
            objects.tool_cashier_type_origin_pkg_id(),
            tool_id,
        )
        .map_err(NexusError::Parsing)?;
        let cashier = crawler
            .get_versioned_object::<ToolCashier, ToolCashierStateV1>(cashier_id, 1)
            .await
            .map_err(NexusError::Rpc)?;
        if cashier.data.tool.bytes != tool_id
            || cashier.data.tool_fqn.as_str() != tool_fqn.to_string()
        {
            return Err(NexusError::Configuration(format!(
                "Tool cashier '{cashier_id}' does not describe Tool '{tool_fqn}'"
            )));
        }

        let policies = cashier.data.policies.contents;
        let fixed_price_mist = if canonical_policy_accepted(objects, &policies, "fixed_price") {
            Some(
                fetch_tool_invocation_cost(crawler, &objects.tool_registry, tool_fqn)
                    .await
                    .map_err(NexusError::Rpc)?
                    .ok_or_else(|| {
                        NexusError::Configuration(format!(
                            "Fixed price policy for Tool '{tool_fqn}' has no price"
                        ))
                    })?,
            )
        } else {
            None
        };
        let free_invocations = canonical_policy_accepted(objects, &policies, "free_invocation");
        let finite_credits = if canonical_policy_accepted(objects, &policies, "finite_credits") {
            let config = crawler
                .get_dynamic_field_by_key::<
                    PolicyKey<finite_credits::Policy>,
                    finite_credits::Config,
                >(
                    cashier_id,
                    PolicyKey::new(false),
                    &crate::move_bindings::type_tag::<PolicyKey<finite_credits::Policy>>(objects),
                )
                .await
                .map_err(NexusError::Rpc)?
                .ok_or_else(|| {
                    NexusError::Configuration(format!(
                        "Finite credits policy for Tool '{tool_fqn}' has no config"
                    ))
                })?;
            Some(FiniteCreditOffer {
                issuance_enabled: config.issuance_enabled,
                price_per_credit: config.price_per_credit,
                minimum_credits: config.minimum_credits,
                maximum_credits: config.maximum_credits,
            })
        } else {
            None
        };
        let time_pass = if canonical_policy_accepted(objects, &policies, "time_pass") {
            let config = crawler
                .get_dynamic_field_by_key::<PolicyKey<time_pass::Policy>, time_pass::Config>(
                    cashier_id,
                    PolicyKey::new(false),
                    &crate::move_bindings::type_tag::<PolicyKey<time_pass::Policy>>(objects),
                )
                .await
                .map_err(NexusError::Rpc)?
                .ok_or_else(|| {
                    NexusError::Configuration(format!(
                        "Time pass policy for Tool '{tool_fqn}' has no config"
                    ))
                })?;
            Some(TimePassOffer {
                issuance_enabled: config.issuance_enabled,
                price_per_ms: config.price_per_ms,
                minimum_duration_ms: config.minimum_duration_ms,
                maximum_duration_ms: config.maximum_duration_ms,
            })
        } else {
            None
        };

        Ok(ToolEconomy {
            tool_id,
            cashier_id,
            minimum_protocol_version: cashier.data.minimum_protocol_version,
            policies,
            fixed_price_mist,
            free_invocations,
            finite_credits,
            time_pass,
        })
    }

    /// Lists finalized Invocations and prepaid deposits waiting for collection.
    ///
    /// Sui indexes these objects by their [`ToolCashier`] owner, so discovery
    /// does not require a mutable on chain registry or a global usage counter.
    pub async fn inspect_cashier_inbox(
        &self,
        tool_fqn: &ToolFqn,
    ) -> Result<ToolCashierInbox, NexusError> {
        let client = self.client.operation_client().await?;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let cashier_id = *cashier.object_id();
        let invocations = client
            .crawler()
            .get_object_owned_objects::<Invocation>(
                cashier_id,
                crate::move_bindings::struct_tag::<Invocation>(&client.nexus_objects),
            )
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .map(|response| CollectableInvocation {
                object_ref: response.object_ref(),
                policy: response.data.policy,
                sources: response
                    .data
                    .sources
                    .into_iter()
                    .map(|source| source.bytes)
                    .collect(),
                amount_mist: response.data.amount,
            })
            .collect();
        let deposits = client
            .crawler()
            .get_object_owned_objects::<CashierDeposit>(
                cashier_id,
                crate::move_bindings::struct_tag::<CashierDeposit>(&client.nexus_objects),
            )
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .map(|response| CollectableDeposit {
                object_ref: response.object_ref(),
                amount_mist: response.data.funds.value,
            })
            .collect();
        Ok(ToolCashierInbox {
            invocations,
            deposits,
        })
    }

    /// Collects one same policy batch of finalized Invocations.
    pub async fn collect_invocations(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        invocation_ids: &[sui::types::Address],
        recipient: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if invocation_ids.is_empty() {
            return Err(NexusError::Configuration(
                "Invocation collection requires at least one object ID".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (cashier, admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let tool_id = Tool::derive_id(*client.nexus_objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let invocations = client
            .crawler()
            .get_objects::<Invocation>(invocation_ids)
            .await
            .map_err(NexusError::Rpc)?;
        let policy = invocations
            .first()
            .map(|invocation| invocation.data.policy.clone())
            .ok_or_else(|| {
                NexusError::Configuration(
                    "Invocation collection requires at least one object".to_owned(),
                )
            })?;
        let references = invocations
            .into_iter()
            .map(|response| {
                if response.owner != sui::types::Owner::Object(*cashier.object_id())
                    || response.data.cashier_id.bytes != *cashier.object_id()
                    || response.data.tool_id.bytes != tool_id
                {
                    return Err(NexusError::Configuration(format!(
                        "Invocation '{}' is not in Tool cashier '{}'",
                        response.object_id,
                        cashier.object_id()
                    )));
                }
                if response.data.policy != policy {
                    return Err(NexusError::Configuration(format!(
                        "Invocation '{}' uses policy '{}', not '{}'",
                        response.object_id, response.data.policy, policy
                    )));
                }
                Ok(response.object_ref())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = tool_cashier::collect_invocations_ptb(
            &client.nexus_objects,
            &cashier,
            &admin,
            &policy,
            &references,
            recipient,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Collects one batch of prepaid sale deposits.
    pub async fn collect_deposits(
        &self,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        deposit_ids: &[sui::types::Address],
        recipient: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (cashier, admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let references = client
            .crawler()
            .get_objects_metadata(deposit_ids)
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .map(|response| {
                if response.owner != sui::types::Owner::Object(*cashier.object_id()) {
                    return Err(NexusError::Configuration(format!(
                        "Deposit '{}' is not in Tool cashier '{}'",
                        response.object_id,
                        cashier.object_id()
                    )));
                }
                Ok(response.object_ref())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transaction = tool_cashier::collect_deposits_ptb(
            &client.nexus_objects,
            &cashier,
            &admin,
            &references,
            recipient,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Derive the [`Tool`] and [`crate::move_bindings::tool::tool_cashier::ToolCashier`]
    /// object IDs for `fqn` and probe the [`Tool`] object. Returns
    /// `exists: false` when neither object is present yet, and the full
    /// [`ToolState`] record when both exist. The same shape
    /// works for HTTP and Sui tools. Callers can inspect the generated
    /// `Tool::r#ref` field or use [`ToolRef`] helper
    /// methods for ergonomic projections.
    ///
    /// Returns [`NexusError::Configuration`] when only one object exists;
    /// that combination indicates corrupt registry state and requires
    /// operator intervention such as a localnet reset.
    pub async fn inspect_tool(&self, fqn: &ToolFqn) -> Result<ToolInspection, NexusError> {
        let client = self.client.operation_client().await?;
        let crawler = client.crawler();
        let nexus_objects = &client.nexus_objects;
        let tool_registry_id = *nexus_objects.tool_registry.object_id();

        let tool_id = crate::move_bindings::derive_tool_id(tool_registry_id, fqn)
            .map_err(NexusError::Parsing)?;
        let tool_cashier_id = crate::move_bindings::derive_tool_cashier_id(
            nexus_objects.tool_cashier_type_origin_pkg_id(),
            tool_id,
        )
        .map_err(NexusError::Parsing)?;

        let tool_exists = crawler.get_object_metadata(tool_id).await.is_ok();
        let tool_cashier_exists = crawler.get_object_metadata(tool_cashier_id).await.is_ok();

        if tool_exists ^ tool_cashier_exists {
            return Err(NexusError::Configuration(format!(
                "Tool '{fqn}' has inconsistent state: Tool exists={tool_exists}, \
                 ToolCashier exists={tool_cashier_exists}. Reset the deployment or recreate the missing \
                 object before retrying."
            )));
        }

        if !tool_exists {
            return Ok(ToolInspection {
                fqn: fqn.clone(),
                tool_id,
                tool_cashier_id,
                exists: false,
                tool: None,
                verifier_support: None,
                external_verifier: None,
                invocation_cost_mist: None,
            });
        }

        let tool = crawler
            .get_versioned_object::<ToolAnchor, ToolState>(tool_id, 1)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let invocation_cost_mist =
            fetch_tool_invocation_cost(crawler, &nexus_objects.tool_registry, fqn)
                .await
                .map_err(NexusError::Rpc)?;
        let (verifier_support, external_verifier) = match &tool.r#ref {
            ToolRef::Http { .. } => {
                let support =
                    fetch_current_tool_registration(crawler, &nexus_objects.tool_registry, tool_id)
                        .await
                        .map_err(NexusError::Rpc)?
                        .and_then(|registration| registration.verifier_support);
                let record =
                    fetch_external_verifier_record(crawler, &nexus_objects.tool_registry, tool_id)
                        .await
                        .map_err(NexusError::Rpc)?;
                (support, record)
            }
            ToolRef::Sui { .. } => (None, None),
        };
        match (&verifier_support, &external_verifier) {
            (Some(ToolVerifierSupport::External { method_id }), Some(record))
                if method_id == &record.method => {}
            (Some(ToolVerifierSupport::External { .. }), _) => {
                return Err(NexusError::Configuration(format!(
                    "Tool '{fqn}' has inconsistent External verifier state"
                )));
            }
            (_, Some(_)) => {
                return Err(NexusError::Configuration(format!(
                    "Tool '{fqn}' has an External verifier record without External support"
                )));
            }
            _ => {}
        }

        Ok(ToolInspection {
            fqn: fqn.clone(),
            tool_id,
            tool_cashier_id,
            exists: true,
            tool: Some(tool),
            verifier_support,
            external_verifier,
            invocation_cost_mist,
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fqn,
            move_bindings::{
                move_std::{ascii, option::Option as MoveOption},
                primitives::{data::NexusData, event::EventWrapper},
                sui_framework::{
                    self,
                    linked_table::LinkedTable,
                    object::{ID, UID},
                    table::Table,
                    versioned::Versioned,
                },
                tool::{
                    external_verifier::ExternalVerifier,
                    finite_credits::CreditsCreatedEvent,
                    time_pass::TimePassCreatedEvent,
                    tool_cashier::CashierDepositCreatedEvent,
                    tool_registry::{ToolRegistry, ToolRegistryState},
                },
            },
            test_utils::{nexus_mocks, sui_mocks},
        },
        tonic::Status,
    };

    /// Test fixture for the inspection mocks. Captures the derived ids and a
    /// preconstructed gRPC server to drive the crawler against.
    struct InspectionFixture {
        nexus_objects: crate::types::NexusObjects,
        fqn: crate::ToolFqn,
        tool_id: sui::types::Address,
        tool_cashier_id: sui::types::Address,
    }

    impl InspectionFixture {
        fn new() -> Self {
            let nexus_objects = sui_mocks::mock_nexus_objects();
            let fqn = fqn!("xyz.taluslabs.example@1");
            let tool_id = crate::move_bindings::derive_tool_id(
                *nexus_objects.tool_registry.object_id(),
                &fqn,
            )
            .expect("tool id derives");
            let tool_cashier_id = crate::move_bindings::derive_tool_cashier_id(
                nexus_objects.tool_cashier_type_origin_pkg_id(),
                tool_id,
            )
            .expect("tool cashier id derives");
            Self {
                nexus_objects,
                fqn,
                tool_id,
                tool_cashier_id,
            }
        }
    }

    fn ascii(value: &str) -> ascii::String {
        ascii::String::from(value)
    }

    #[test]
    fn canonical_policy_detection_uses_defining_type_identity() {
        let objects = sui_mocks::mock_nexus_objects();
        let origin = objects.packages.tool.type_origin("fixed_price", "Policy");
        let policies = vec![crate::move_bindings::move_std::type_name::TypeName::new(
            &format!("{origin}::fixed_price::Policy"),
        )];

        assert!(canonical_policy_accepted(
            &objects,
            &policies,
            "fixed_price"
        ));
        assert!(!canonical_policy_accepted(
            &objects,
            &policies,
            "free_invocation"
        ));
    }

    fn sui_tool_ref(
        package_address: sui::types::Address,
        module_name: sui::types::Identifier,
        tool_witness_id: sui::types::Address,
    ) -> ToolRef {
        ToolRef::Sui {
            package_address,
            module_name: ascii(module_name.as_str()),
            tool_witness_id: crate::move_bindings::sui_framework::object::ID::new(tool_witness_id),
        }
    }

    fn fixture_tool(
        fixture: &InspectionFixture,
        reference: ToolRef,
        workflow_authorization_cap_first: bool,
    ) -> ToolState {
        ToolState {
            minimum_protocol_version: 1,
            registry: crate::move_bindings::sui_framework::object::ID::new(
                *fixture.nexus_objects.tool_registry.object_id(),
            ),
            fqn: ascii(&fixture.fqn.to_string()),
            r#ref: reference,
            description: b"demo".to_vec(),
            meta_schema: crate::move_bindings::interface::meta_schema::MetaSchema::new(
                vec![],
                vec![],
            ),
            verified: false,
            vault: sui_framework::balance::Balance {
                value: 0,
                phantom_t0: std::marker::PhantomData,
            },
            workflow_authorization_cap_first,
            lock_duration_ms: 0,
            registered_at_ms: 0,
            unregistered_at_ms: MoveOption::from(None),
        }
    }

    fn mock_empty_tool_registry_state(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        fixture: &InspectionFixture,
        reads: usize,
    ) {
        use crate::move_bindings::interface::verifier::ToolVerifierSupport;

        let id = sui::types::Address::from_static;
        let tool_registry_state_id = id("0x109");
        let tool_registry = ToolRegistry::new(
            UID::new(*fixture.nexus_objects.tool_registry.object_id()),
            Versioned::new(UID::new(tool_registry_state_id), 1),
        );
        for _ in 0..reads {
            let tool_registry_state = ToolRegistryState::new(
                ID::new(sui::types::Address::ZERO),
                1,
                LinkedTable::<ascii::String, ID>::new(id("0x101"), 0),
                Table::<ID, bool>::new(id("0x102"), 0),
                Table::<ID, crate::move_bindings::interface::meta_schema::MetaSchema>::new(
                    id("0x110"),
                    0,
                ),
                LinkedTable::<ascii::String, u64>::new(id("0x103"), 0),
                Table::<ID, ToolVerifierSupport>::new(id("0x104"), 0),
                Table::<ID, ExternalVerifier>::new(id("0x107"), 0),
                Table::<ascii::String, u64>::new(id("0x108"), 0),
                LinkedTable::<ascii::String, ID>::new(id("0x105"), 0),
                LinkedTable::<ascii::String, bool>::new(id("0x106"), 0),
                0,
                0,
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger_service,
                fixture.nexus_objects.tool_registry.clone(),
                sui::types::Owner::Shared(fixture.nexus_objects.tool_registry.version()),
                bcs::to_bytes(&tool_registry).unwrap(),
            );
            sui_mocks::grpc::mock_versioned_payload(
                ledger_service,
                tool_registry_state_id,
                1,
                tool_registry_state,
            );
        }
    }

    /// Expect a `get_object` call and reply with a tonic NotFound error so the
    /// crawler treats the object as missing.
    fn mock_get_object_not_found(ledger_service: &mut sui_mocks::grpc::MockLedgerService) {
        ledger_service
            .expect_get_object()
            .times(1)
            .returning(|_request| Err(Status::not_found("object not present")));
    }

    #[tokio::test]
    async fn inspect_tool_reports_missing_when_neither_object_exists() {
        let fixture = InspectionFixture::new();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        mock_get_object_not_found(&mut ledger_service_mock);
        mock_get_object_not_found(&mut ledger_service_mock);

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(&fixture.nexus_objects, &rpc_url).await;

        let inspection = client
            .tool()
            .inspect_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds when both objects missing");

        assert!(!inspection.exists);
        assert_eq!(inspection.tool_id, fixture.tool_id);
        assert_eq!(inspection.tool_cashier_id, fixture.tool_cashier_id);
        assert!(inspection.tool.is_none());
    }

    #[tokio::test]
    async fn inspect_tool_rejects_inconsistent_state() {
        let fixture = InspectionFixture::new();
        let tool_ref = sui::types::ObjectReference::new(
            fixture.tool_id,
            5,
            sui::types::Digest::from([1u8; 32]),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        // First probe (Tool) succeeds.
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            None,
        );
        // Second probe (ToolCashier) fails -> inconsistent.
        mock_get_object_not_found(&mut ledger_service_mock);

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let error = client
            .tool()
            .inspect_tool(&fixture.fqn)
            .await
            .expect_err("inconsistent state should error");

        let error_string = error.to_string();
        assert!(
            matches!(error, NexusError::Configuration(_)),
            "unexpected error variant: {error_string}"
        );
        assert!(
            error_string.contains("inconsistent state"),
            "unexpected error message: {error_string}"
        );
    }

    #[tokio::test]
    async fn inspect_tool_decodes_existing_sui_tool() {
        let mut rng = rand::thread_rng();
        let fixture = InspectionFixture::new();
        let package_address = sui::types::Address::generate(&mut rng);
        let tool_witness_id = sui::types::Address::generate(&mut rng);
        let module_name = sui::types::Identifier::from_static("demo_onchain_vertex");

        let tool_ref = sui::types::ObjectReference::new(
            fixture.tool_id,
            7,
            sui::types::Digest::from([3u8; 32]),
        );
        let tool_cashier_ref = sui::types::ObjectReference::new(
            fixture.tool_cashier_id,
            7,
            sui::types::Digest::from([4u8; 32]),
        );
        let tool_state = fixture_tool(
            &fixture,
            sui_tool_ref(package_address, module_name.clone(), tool_witness_id),
            true,
        );
        let tool_state_id = sui::types::Address::from_static("0x2010");
        let tool = ToolAnchor::new(
            UID::new(fixture.tool_id),
            Versioned::new(UID::new(tool_state_id), 1),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_cashier_ref,
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&tool).expect("Tool anchor serializes to BCS"),
        );
        sui_mocks::grpc::mock_versioned_payload(
            &mut ledger_service_mock,
            tool_state_id,
            1,
            tool_state,
        );
        mock_empty_tool_registry_state(&mut ledger_service_mock, &fixture, 1);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let inspection = client
            .tool()
            .inspect_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds when Tool present");

        assert!(inspection.exists);
        assert_eq!(inspection.tool_id, fixture.tool_id);
        assert_eq!(inspection.tool_cashier_id, fixture.tool_cashier_id);
        let decoded = inspection.tool.expect("Tool decoded");
        assert!(decoded.workflow_authorization_cap_first);
        let Some((decoded_package, decoded_module, decoded_witness)) =
            decoded.r#ref.sui_parts().expect("Sui tool ref decodes")
        else {
            panic!("expected Sui-variant tool");
        };
        assert_eq!(decoded_package, package_address);
        assert_eq!(decoded_module, module_name.as_str());
        assert_eq!(decoded_witness, tool_witness_id);
    }

    #[tokio::test]
    async fn inspect_tool_rejects_inconsistent_state_when_only_tool_cashier_present() {
        let fixture = InspectionFixture::new();
        let tool_cashier_ref = sui::types::ObjectReference::new(
            fixture.tool_cashier_id,
            5,
            sui::types::Digest::from([2u8; 32]),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        // First probe (Tool) fails.
        mock_get_object_not_found(&mut ledger_service_mock);
        // Second probe (ToolCashier) succeeds -> the XOR triggers the other branch.
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_cashier_ref,
            sui::types::Owner::Shared(1),
            None,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let error = client
            .tool()
            .inspect_tool(&fixture.fqn)
            .await
            .expect_err("inconsistent state should error");

        let error_string = error.to_string();
        assert!(
            matches!(error, NexusError::Configuration(_)),
            "unexpected error variant: {error_string}"
        );
        assert!(
            error_string.contains("Tool exists=false")
                && error_string.contains("ToolCashier exists=true"),
            "unexpected error message: {error_string}"
        );
    }

    #[tokio::test]
    async fn inspect_tool_decodes_existing_http_tool() {
        let fixture = InspectionFixture::new();

        let tool_ref = sui::types::ObjectReference::new(
            fixture.tool_id,
            11,
            sui::types::Digest::from([7u8; 32]),
        );
        let tool_cashier_ref = sui::types::ObjectReference::new(
            fixture.tool_cashier_id,
            11,
            sui::types::Digest::from([8u8; 32]),
        );
        let http_tool_state = fixture_tool(
            &fixture,
            ToolRef::Http {
                url: b"https://example.com/tool".to_vec(),
            },
            false,
        );
        let tool_state_id = sui::types::Address::from_static("0x2020");
        let http_tool = ToolAnchor::new(
            UID::new(fixture.tool_id),
            Versioned::new(UID::new(tool_state_id), 1),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_cashier_ref,
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&http_tool).expect("Tool anchor serializes to BCS"),
        );
        sui_mocks::grpc::mock_versioned_payload(
            &mut ledger_service_mock,
            tool_state_id,
            1,
            http_tool_state,
        );
        mock_empty_tool_registry_state(&mut ledger_service_mock, &fixture, 3);

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let inspection = client
            .tool()
            .inspect_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds for HTTP tool");

        assert!(inspection.exists);
        let decoded = inspection.tool.expect("Tool decoded");
        assert_eq!(
            decoded.r#ref.http_url_string().unwrap().unwrap().as_str(),
            "https://example.com/tool"
        );
    }

    #[tokio::test]
    async fn test_tool_actions_update_tool_timeout() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.example@1");
        let tool_ref = sui_mocks::mock_sui_object_ref();
        let owner_cap_id = sui::types::Address::generate(&mut rng);
        let owner_cap_object_ref = sui::types::ObjectReference::new(owner_cap_id, 0, tx_digest);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock owner cap object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            owner_cap_object_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0x3")),
            None,
        );

        // Mock tool object metadata
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );

        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tool()
            .update_timeout(&tool_fqn, Duration::from_secs(1000), owner_cap_id)
            .await
            .expect("Failed to update tool timeout");

        assert_eq!(result.tx_digest, submitted.digest());
    }

    #[derive(Clone, Copy)]
    enum PaymentAction {
        EnableTimePass,
        SetInvocationCost,
        CloseTimePassIssuance,
        BuyTimePass,
        EnableFiniteCredits,
        CloseFiniteCreditIssuance,
        BuyFiniteCredits,
    }

    #[derive(serde::Serialize)]
    struct EventWrapperValue<T> {
        event: T,
    }

    fn wrapped_tool_event<T>(objects: &crate::types::NexusObjects, event: T) -> sui::types::Event
    where
        T: serde::Serialize + sui_move::MoveStruct,
    {
        let inner = crate::move_bindings::struct_tag::<T>(objects);
        let wrapper = crate::move_bindings::struct_tag::<EventWrapper<NexusData>>(objects);
        let wrapper = sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        );
        sui_mocks::mock_sui_event(
            objects.tool_pkg_id(),
            wrapper,
            bcs::to_bytes(&EventWrapperValue { event }).expect("Tool event serializes"),
        )
    }

    async fn assert_payment_action_succeeds(action: PaymentAction) {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.payment@1");
        let tool_id = crate::move_bindings::derive_tool_id(
            *nexus_objects.tool_registry.object_id(),
            &tool_fqn,
        )
        .expect("tool id derives");
        let tool_cashier_id = crate::move_bindings::derive_tool_cashier_id(
            nexus_objects.tool_cashier_type_origin_pkg_id(),
            tool_id,
        )
        .expect("tool cashier id derives");
        let primary_id = match action {
            PaymentAction::SetInvocationCost => tool_id,
            _ => tool_cashier_id,
        };
        let primary_ref = sui_mocks::object_ref_for_id(primary_id);
        let auxiliary_id = sui::types::Address::from_static("0x402");
        let auxiliary_ref = sui_mocks::object_ref_for_id(auxiliary_id);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut transaction_service = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service = sui_mocks::grpc::MockSubscriptionService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 1_000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service,
            primary_ref,
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service,
            auxiliary_ref,
            sui::types::Owner::Address(sui::types::Address::from_static("0x403")),
            None,
        );
        let entitlement_id = sui::types::Address::from_static("0x501");
        let deposit_id = sui::types::Address::from_static("0x502");
        let beneficiary = crate::move_bindings::interface::payment::PaymentSourceKind::user_funded(
            sui::types::Address::from_static("0x3"),
        );
        let events = match action {
            PaymentAction::BuyTimePass => vec![
                wrapped_tool_event(
                    &nexus_objects,
                    TimePassCreatedEvent::new(
                        ID::new(tool_id),
                        ID::new(tool_cashier_id),
                        ID::new(entitlement_id),
                        beneficiary,
                        1,
                        4,
                    ),
                ),
                wrapped_tool_event(
                    &nexus_objects,
                    CashierDepositCreatedEvent::new(
                        ID::new(tool_cashier_id),
                        ID::new(deposit_id),
                        21,
                    ),
                ),
            ],
            PaymentAction::BuyFiniteCredits => vec![
                wrapped_tool_event(
                    &nexus_objects,
                    CreditsCreatedEvent::new(
                        ID::new(tool_id),
                        ID::new(tool_cashier_id),
                        ID::new(entitlement_id),
                        beneficiary,
                        sui::types::Address::from_static("0x3"),
                        4,
                    ),
                ),
                wrapped_tool_event(
                    &nexus_objects,
                    CashierDepositCreatedEvent::new(
                        ID::new(tool_cashier_id),
                        ID::new(deposit_id),
                        52,
                    ),
                ),
            ],
            _ => vec![],
        };
        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut transaction_service,
            &mut subscription_service,
            &mut ledger_service,
            gas_coin_ref,
            vec![],
            vec![],
            events,
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            execution_service_mock: Some(transaction_service),
            subscription_service_mock: Some(subscription_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;
        let actions = client.tool();

        let tx_digest = match action {
            PaymentAction::EnableTimePass => actions
                .enable_time_passes(&tool_fqn, auxiliary_id, 7, 1, 100)
                .await
                .map(|result| result.tx_digest),
            PaymentAction::SetInvocationCost => actions
                .set_invocation_cost(&tool_fqn, auxiliary_id, 11)
                .await
                .map(|result| result.tx_digest),
            PaymentAction::CloseTimePassIssuance => actions
                .close_time_pass_issuance(&tool_fqn, auxiliary_id)
                .await
                .map(|result| result.tx_digest),
            PaymentAction::BuyTimePass => actions
                .buy_time_pass(&tool_fqn, 3, auxiliary_id)
                .await
                .map(|result| result.tx_digest),
            PaymentAction::EnableFiniteCredits => actions
                .enable_finite_credits(&tool_fqn, auxiliary_id, 13, 2, 9)
                .await
                .map(|result| result.tx_digest),
            PaymentAction::CloseFiniteCreditIssuance => actions
                .close_finite_credit_issuance(&tool_fqn, auxiliary_id)
                .await
                .map(|result| result.tx_digest),
            PaymentAction::BuyFiniteCredits => actions
                .buy_finite_credits(&tool_fqn, 4, auxiliary_id)
                .await
                .map(|result| result.tx_digest),
        }
        .expect("Tool payment action succeeds");

        assert_eq!(tx_digest, submitted.digest());
    }

    #[tokio::test]
    async fn tool_cashier_actions_resolve_objects_and_submit() {
        for action in [
            PaymentAction::EnableTimePass,
            PaymentAction::SetInvocationCost,
            PaymentAction::CloseTimePassIssuance,
            PaymentAction::BuyTimePass,
            PaymentAction::EnableFiniteCredits,
            PaymentAction::CloseFiniteCreditIssuance,
            PaymentAction::BuyFiniteCredits,
        ] {
            assert_payment_action_succeeds(action).await;
        }
    }

    #[tokio::test]
    async fn registered_key_verifier_resolves_binding_and_submits() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let tool_fqn = fqn!("xyz.taluslabs.verified@1");
        let tool_id = crate::move_bindings::derive_tool_id(
            *nexus_objects.tool_registry.object_id(),
            &tool_fqn,
        )
        .expect("tool id derives");
        let owner_cap_id = sui::types::Address::from_static("0x411");
        let derivation_client = NexusClient::builder()
            .with_rpc_url("http://127.0.0.1:1")
            .with_nexus_objects(nexus_objects.clone())
            .build()
            .await
            .expect("derivation client builds");
        let binding_id = derivation_client
            .network_auth()
            .binding_object_id(
                &crate::move_bindings::registry::network_auth::IdentityKey::tool(tool_id),
            )
            .await
            .expect("binding id derives");

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut transaction_service = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service = sui_mocks::grpc::MockSubscriptionService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 1_000);
        for (object_ref, owner) in [
            (
                sui_mocks::object_ref_for_id(tool_id),
                sui::types::Owner::Shared(1),
            ),
            (
                sui_mocks::object_ref_for_id(owner_cap_id),
                sui::types::Owner::Address(sui::types::Address::from_static("0x412")),
            ),
            (
                sui_mocks::object_ref_for_id(binding_id),
                sui::types::Owner::Shared(1),
            ),
        ] {
            sui_mocks::grpc::mock_get_object_metadata(&mut ledger_service, object_ref, owner, None);
        }
        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut transaction_service,
            &mut subscription_service,
            &mut ledger_service,
            sui_mocks::mock_sui_object_ref(),
            vec![],
            vec![],
            vec![],
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            execution_service_mock: Some(transaction_service),
            subscription_service_mock: Some(subscription_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tool()
            .configure_registered_key_verifier(&tool_fqn, owner_cap_id)
            .await
            .expect("registered key verifier configuration succeeds");

        assert_eq!(result.tx_digest, submitted.digest());
        assert_eq!(result.tool_id, tool_id);
    }
}
