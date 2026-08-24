//! Tool inspection, owner configuration, and cashier operations.

use {
    crate::{
        events::NexusEventKind,
        move_bindings::{
            interface::{payment::PaymentSourceKind, verifier::ToolVerifierSupport},
            move_std::{ascii, type_name::TypeName},
            registry::network_auth::IdentityKey,
            sui_framework::{clock::Clock as SuiClock, linked_table::Node, object::ID},
            tool::{
                external_verifier::ExternalVerifier,
                finite_credits,
                invocation::Invocation,
                time_pass,
                tool_cashier::{CashierDeposit, PolicyKey, ToolCashier, ToolCashierInnerV1},
                tool_registry::{
                    Tool as ToolAnchor,
                    ToolInnerV1,
                    ToolRegistry,
                    ToolRegistryInnerV1,
                },
            },
            FiniteCredits,
            TimePass,
        },
        move_boundary,
        nexus::{
            client::NexusClient,
            error::NexusError,
            registry::preflight_external_verifier_registration,
        },
        sui,
        transactions::{tool, tool_cashier},
        types::{NexusContext, PackageRole, ToolState},
        ToolFqn,
    },
    std::{collections::HashSet, sync::Arc, time::Duration},
};

/// Compatibility of one Tool with this SDK and the current Registry authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ToolCompatibility {
    /// The Tool selects the same package graph as the current Registry.
    Current,
    /// The SDK understands the Tool, and no live Registry mutation is required.
    LegacyUnderstood,
    /// The SDK understands the Tool, but live Registry use requires migration.
    MigrationRequired,
    /// The observed era and inner pair has no adapter in this SDK.
    Unsupported,
    /// The Tool could not be observed or decoded independently.
    Unavailable,
}

/// Confirmed entitlement purchase with both discoverable object IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntitlementPurchaseResult {
    pub tx_digest: sui::types::Digest,
    pub entitlement_id: sui::types::Address,
    pub deposit_id: sui::types::Address,
}

/// Confirmed entitlement state change with its canonical object ID.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntitlementIssueResult {
    pub tx_digest: sui::types::Digest,
    pub entitlement_id: sui::types::Address,
}

/// Current sale terms for canonical finite Tool invocation credits.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FiniteCreditOffer {
    pub issuance_enabled: bool,
    pub price_per_credit: u64,
    pub minimum_credits: u64,
    pub maximum_credits: u64,
}

impl FiniteCreditOffer {
    fn purchase_price(&self, credits: u64) -> Result<u64, NexusError> {
        if !self.issuance_enabled {
            return Err(NexusError::Configuration(
                "Finite credit purchases are closed".to_owned(),
            ));
        }
        if credits < self.minimum_credits || credits > self.maximum_credits {
            return Err(NexusError::Configuration(format!(
                "Credit count '{credits}' must be between '{}' and '{}'",
                self.minimum_credits, self.maximum_credits,
            )));
        }
        self.price_per_credit.checked_mul(credits).ok_or_else(|| {
            NexusError::Configuration(
                "Finite credit purchase price exceeds the maximum MIST value".to_owned(),
            )
        })
    }
}

/// Current sale terms for canonical time based Tool access.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TimePassOffer {
    pub issuance_enabled: bool,
    pub price_per_ms: u64,
    pub minimum_duration_ms: u64,
    pub maximum_duration_ms: u64,
}

impl TimePassOffer {
    fn purchase_price(&self, duration_ms: u64) -> Result<u64, NexusError> {
        if !self.issuance_enabled {
            return Err(NexusError::Configuration(
                "Time pass purchases are closed".to_owned(),
            ));
        }
        if duration_ms < self.minimum_duration_ms || duration_ms > self.maximum_duration_ms {
            return Err(NexusError::Configuration(format!(
                "Duration '{duration_ms}' must be between '{}' and '{}' milliseconds",
                self.minimum_duration_ms, self.maximum_duration_ms,
            )));
        }
        self.price_per_ms.checked_mul(duration_ms).ok_or_else(|| {
            NexusError::Configuration(
                "Time pass purchase price exceeds the maximum MIST value".to_owned(),
            )
        })
    }
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
    pub policies: Vec<TypeName>,
    pub fixed_price_mist: u64,
    pub finite_credits: Option<FiniteCreditOffer>,
    pub time_pass: Option<TimePassOffer>,
}

/// User facing finite credit state for one Tool and beneficiary.
///
/// This combines the shared account with refunded child [Invocation] objects,
/// which no single Move value exposes as one serializable view.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct FiniteCreditAccess {
    pub account_id: sui::types::Address,
    pub remaining: u64,
    pub refunded_invocations: Vec<sui::types::Address>,
}

/// User facing time pass state for one Tool and beneficiary.
///
/// The active flag is evaluated against the onchain clock used by admission,
/// so clients do not need to compare local time with Move state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct TimePassAccess {
    pub account_id: sui::types::Address,
    pub valid_from_ms: u64,
    pub valid_until_ms: u64,
    pub active: bool,
}

/// Complete user facing access for one Tool and payment beneficiary.
///
/// The account IDs are derived from canonical Move keys. This stable view
/// combines those identities with typed state for RPC, CLI, and index users.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ToolAccess {
    pub tool_id: sui::types::Address,
    pub cashier_id: sui::types::Address,
    pub beneficiary: PaymentSourceKind,
    pub observed_at_ms: u64,
    pub finite_credits: Option<FiniteCreditAccess>,
    pub time_pass: Option<TimePassAccess>,
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
    /// Requested fully qualified name.
    pub fqn: ToolFqn,
    /// Stable Tool object ID.
    pub tool_id: sui::types::Address,
    /// Stable cashier object ID.
    pub tool_cashier_id: sui::types::Address,
    /// Whether the stable Tool object exists.
    pub exists: bool,
    /// Current owner when the Tool was observed.
    pub owner: Option<sui::types::Owner>,
    /// Exact era type observed below the Tool anchor.
    pub witness_type: Option<sui::types::StructTag>,
    /// Exact inner type observed below the Tool anchor.
    pub inner_type: Option<sui::types::StructTag>,
    /// Compatibility classification isolated to this Tool.
    pub compatibility: ToolCompatibility,
    /// Complete supported Tool state.
    pub tool: Option<ToolState>,
    /// Current Registry verifier support while the Tool is registered.
    pub verifier_support: Option<ToolVerifierSupport>,
    /// Current external verifier record, when configured.
    pub external_verifier: Option<ExternalVerifier>,
    /// Current Registry timeout while the Tool is registered.
    pub timeout_ms: Option<u64>,
    /// Current invocation price while the Tool is registered.
    pub invocation_cost_mist: Option<u64>,
    /// Diagnostic detail for unsupported or unavailable state.
    pub detail: Option<String>,
}

/// Result of a Tool owner or cashier transaction.
pub struct ToolActionResult {
    /// Digest of the submitted transaction.
    pub tx_digest: sui::types::Digest,
}

/// Compatibility preserving alias for cashier callers.
pub type ToolCashierActionResult = ToolActionResult;

/// Operations over Tool state, Registry projections, and payment state.
pub struct ToolActions {
    pub(super) client: NexusClient,
}

fn canonical_policy_accepted(
    context: &NexusContext,
    policies: &[TypeName],
    module: &str,
) -> Result<bool, NexusError> {
    let origin = context
        .type_origin(PackageRole::Tool, module, "Policy")
        .map_err(NexusError::Parsing)?;
    let expected = format!("{origin}::{module}::Policy");
    Ok(policies
        .iter()
        .any(|policy| policy.matches_qualified_name(&expected)))
}

impl ToolActions {
    async fn fetch_finite_credit_offer(
        client: &NexusClient,
        context: &NexusContext,
        cashier_id: sui::types::Address,
        tool_fqn: &ToolFqn,
    ) -> Result<FiniteCreditOffer, NexusError> {
        let config = client
            .crawler()
            .get_dynamic_field_by_key::<PolicyKey<finite_credits::Policy>, finite_credits::Config>(
                cashier_id,
                PolicyKey::new(false),
                &crate::move_bindings::type_tag::<PolicyKey<finite_credits::Policy>>(context),
            )
            .await
            .map_err(NexusError::Rpc)?
            .ok_or_else(|| {
                NexusError::Configuration(format!(
                    "Finite credits policy for Tool '{tool_fqn}' has no config"
                ))
            })?;
        Ok(FiniteCreditOffer {
            issuance_enabled: config.issuance_enabled,
            price_per_credit: config.price_per_credit,
            minimum_credits: config.minimum_credits,
            maximum_credits: config.maximum_credits,
        })
    }

    async fn fetch_time_pass_offer(
        client: &NexusClient,
        context: &NexusContext,
        cashier_id: sui::types::Address,
        tool_fqn: &ToolFqn,
    ) -> Result<TimePassOffer, NexusError> {
        let config = client
            .crawler()
            .get_dynamic_field_by_key::<PolicyKey<time_pass::Policy>, time_pass::Config>(
                cashier_id,
                PolicyKey::new(false),
                &crate::move_bindings::type_tag::<PolicyKey<time_pass::Policy>>(context),
            )
            .await
            .map_err(NexusError::Rpc)?
            .ok_or_else(|| {
                NexusError::Configuration(format!(
                    "Time pass policy for Tool '{tool_fqn}' has no config"
                ))
            })?;
        Ok(TimePassOffer {
            issuance_enabled: config.issuance_enabled,
            price_per_ms: config.price_per_ms,
            minimum_duration_ms: config.minimum_duration_ms,
            maximum_duration_ms: config.maximum_duration_ms,
        })
    }

    async fn resolve_tool_cashier_and_cap(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<
        (
            Arc<NexusContext>,
            sui::types::ObjectReference,
            sui::types::ObjectReference,
        ),
        NexusError,
    > {
        let tool_cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let context = client.context_for_object(*tool_cashier.object_id()).await?;
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
        Ok((context, tool_cashier, cashier_admin))
    }

    /// Lists every currently registered Tool.
    ///
    /// A Tool that cannot be decoded remains in the result with its own
    /// compatibility status. One bad Tool cannot hide the rest of the Registry.
    pub async fn list_tools(&self) -> Result<Vec<ToolInspection>, NexusError> {
        let (context, registry) = self.registry_state().await?;
        let entries = self.tool_directory(&context, &registry).await?;
        let mut tools = Vec::with_capacity(entries.len());
        for (fqn, tool_id) in entries {
            tools.push(
                self.inspect_tool_id(&context, &registry, fqn, tool_id)
                    .await,
            );
        }
        Ok(tools)
    }

    /// Inspects one stable Tool ID derived from its FQN.
    ///
    /// Missing, unsupported, and unavailable Tool state is represented in the
    /// returned value so callers can make compatibility decisions explicitly.
    pub async fn inspect_tool(&self, fqn: &ToolFqn) -> Result<ToolInspection, NexusError> {
        let (context, registry) = self.registry_state().await?;
        let tool_id = crate::move_bindings::derive_tool_id(context.tool_registry.object_id(), fqn)
            .map_err(NexusError::Parsing)?;
        Ok(self
            .inspect_tool_id(&context, &registry, fqn.clone(), tool_id)
            .await)
    }

    /// Updates the execution timeout for a registered Tool.
    pub async fn update_timeout(
        &self,
        fqn: &ToolFqn,
        timeout: Duration,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) = self.current_tool_inputs(fqn, owner_cap, true).await?;
        let transaction = tool::update_tool_timeout_ptb(&context, &tool_ref, &owner_cap, timeout)
            .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Updates the HTTP endpoint for a registered off chain Tool.
    pub async fn update_url(
        &self,
        fqn: &ToolFqn,
        url: &str,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) =
            self.current_tool_inputs(fqn, owner_cap, false).await?;
        let transaction = tool::update_off_chain_tool_url_ptb(&context, &tool_ref, &owner_cap, url)
            .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Updates the description for a registered Tool.
    pub async fn update_metadata(
        &self,
        fqn: &ToolFqn,
        description: &str,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) =
            self.current_tool_inputs(fqn, owner_cap, false).await?;
        let transaction =
            tool::update_tool_metadata_ptb(&context, &tool_ref, &owner_cap, description)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Updates the invocation price for a registered Tool.
    pub async fn set_invocation_cost(
        &self,
        fqn: &ToolFqn,
        cost_mist: u64,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let tool_ref = self.client.fetch_tool(fqn).await?;
        let context = self
            .client
            .context_for_object_with_roots(
                *tool_ref.object_id(),
                std::slice::from_ref(&self.client.nexus_objects.tool_registry),
            )
            .await?;
        let cashier_admin = self.client.object_reference(cashier_admin).await?;
        let transaction =
            tool::set_invocation_cost_ptb(&context, &tool_ref, &cashier_admin, cost_mist)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Enables registered key verification for a registered off chain Tool.
    pub async fn configure_registered_key_verifier(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let tool_ref = self.client.fetch_tool(fqn).await?;
        let tool_id = *tool_ref.object_id();
        let binding_id = self
            .client
            .network_auth()
            .binding_object_id(&IdentityKey::tool(tool_id))
            .await?;
        let objects = self.client.get_nexus_objects();
        let context = self
            .client
            .context_for_object_with_roots(
                objects.network_auth.object_id(),
                std::slice::from_ref(&objects.tool_registry),
            )
            .await?;
        let (owner_cap, tool_key_binding) = tokio::try_join!(
            self.client.object_reference(owner_cap),
            self.client.object_reference(binding_id),
        )?;
        let transaction = tool::configure_registered_key_verifier_ptb(
            &context,
            &tool_ref,
            &owner_cap,
            &tool_key_binding,
        )
        .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Validates and installs one external verifier for an off chain Tool.
    pub async fn configure_external_verifier(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
        package_id: sui::types::Address,
        module_name: &str,
        function_name: &str,
        verifier_object_ids: &[sui::types::Address],
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) = self.current_tool_inputs(fqn, owner_cap, true).await?;
        let registration = preflight_external_verifier_registration(
            self.client.crawler(),
            &context,
            package_id,
            module_name,
            function_name,
            verifier_object_ids,
        )
        .await
        .map_err(|error| NexusError::Configuration(error.to_string()))?;
        let transaction =
            tool::register_external_verifier_ptb(&context, &tool_ref, &owner_cap, &registration)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Points a registered on chain Tool at a new package.
    pub async fn migrate_on_chain_package(
        &self,
        fqn: &ToolFqn,
        target_package: sui::types::Address,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) =
            self.current_tool_inputs(fqn, owner_cap, false).await?;
        let transaction = tool::migrate_on_chain_tool_package_ptb(
            &context,
            &tool_ref,
            &owner_cap,
            target_package,
        )
        .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Unregisters a Tool from every live Registry lookup.
    pub async fn unregister(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) = self.unregister_inputs(fqn, owner_cap).await?;
        let transaction = tool::unregister_ptb(&context, &tool_ref, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Claims unlocked US collateral after Tool unregistration.
    pub async fn claim_collateral(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool_ref, owner_cap) =
            self.current_tool_inputs(fqn, owner_cap, false).await?;
        let transaction = tool::claim_collateral_for_self_ptb(&context, &tool_ref, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
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
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_time_pass_ptb(
            &context,
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

    /// Closes time pass issuance for a [`Tool`] without invalidating existing passes.
    pub async fn close_time_pass_issuance(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(client, fqn, cashier_admin).await?;
        let transaction =
            tool_cashier::close_time_pass_issuance_ptb(&context, &tool_cashier, &cashier_admin)
                .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolCashierActionResult {
            tx_digest: response.digest,
        })
    }

    /// Opens time pass issuance for a [`Tool`] using its current terms.
    pub async fn open_time_pass_issuance(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(client, fqn, cashier_admin).await?;
        let transaction =
            tool_cashier::open_time_pass_issuance_ptb(&context, &tool_cashier, &cashier_admin)
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
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::update_time_pass_terms_ptb(
            &context,
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

    /// Buys time access for the signer beneficiary from its SUI address balance.
    pub async fn buy_time_pass(
        &self,
        tool_fqn: &ToolFqn,
        duration_ms: u64,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = &self.client;
        let beneficiary = PaymentSourceKind::user_funded(client.owner()?);
        self.buy_time_pass_with(&client, tool_fqn, duration_ms, beneficiary)
            .await
    }

    /// Buys time access for an explicit beneficiary from the signer address balance.
    pub async fn buy_time_pass_for(
        &self,
        tool_fqn: &ToolFqn,
        duration_ms: u64,
        beneficiary: PaymentSourceKind,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = &self.client;
        self.buy_time_pass_with(&client, tool_fqn, duration_ms, beneficiary)
            .await
    }

    async fn buy_time_pass_with(
        &self,
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        duration_ms: u64,
        beneficiary: PaymentSourceKind,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        if duration_ms == 0 {
            return Err(NexusError::Configuration(
                "Time pass duration must be greater than zero".to_owned(),
            ));
        }
        let address = client.owner()?;
        let tool_cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let context = client.context_for_object(*tool_cashier.object_id()).await?;
        let price =
            Self::fetch_time_pass_offer(client, &context, *tool_cashier.object_id(), tool_fqn)
                .await?
                .purchase_price(duration_ms)?;
        let entitlement_id = crate::move_bindings::derive_time_pass_id(
            &context,
            *tool_cashier.object_id(),
            beneficiary.clone(),
        )
        .map_err(NexusError::Parsing)?;
        let pass = client
            .crawler()
            .get_optional_object::<TimePass>(entitlement_id)
            .await
            .map_err(NexusError::Rpc)?;
        let account = match pass {
            Some(pass) => {
                if pass.data.cashier.bytes != *tool_cashier.object_id()
                    || pass.data.beneficiary != beneficiary
                    || !pass.is_shared()
                {
                    return Err(NexusError::Configuration(format!(
                        "Time pass '{entitlement_id}' does not match its canonical account"
                    )));
                }
                Some(pass.object_ref())
            }
            None => None,
        };
        let transaction = tool_cashier::buy_time_pass_from_balance_ptb(
            &context,
            &tool_cashier,
            account.as_ref(),
            beneficiary,
            duration_ms,
            price,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
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

    /// Sets the canonical time pass window under Tool owner authority.
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
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let entitlement_id = crate::move_bindings::derive_time_pass_id(
            &context,
            *tool_cashier.object_id(),
            beneficiary.clone(),
        )
        .map_err(NexusError::Parsing)?;
        let pass = client
            .crawler()
            .get_optional_object::<TimePass>(entitlement_id)
            .await
            .map_err(NexusError::Rpc)?;
        let transaction = match pass {
            Some(pass) => {
                if pass.data.cashier.bytes != *tool_cashier.object_id()
                    || pass.data.beneficiary != beneficiary
                    || !pass.is_shared()
                {
                    return Err(NexusError::Configuration(format!(
                        "Time pass '{entitlement_id}' does not match its canonical account"
                    )));
                }
                tool_cashier::update_time_pass_window_ptb(
                    &context,
                    &tool_cashier,
                    &pass.object_ref(),
                    &cashier_admin,
                    valid_from_ms,
                    valid_until_ms,
                )
            }
            None => tool_cashier::issue_time_pass_ptb(
                &context,
                &tool_cashier,
                &cashier_admin,
                beneficiary,
                valid_from_ms,
                valid_until_ms,
            ),
        }
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
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
        let client = &self.client;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_finite_credits_ptb(
            &context,
            &tool_cashier,
            &cashier_admin,
            price_per_credit,
            minimum_credits,
            maximum_credits,
        )
        .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Closes finite credit issuance for a [`Tool`] without invalidating existing credits.
    pub async fn close_finite_credit_issuance(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(client, fqn, cashier_admin).await?;
        let transaction =
            tool_cashier::close_finite_credit_issuance_ptb(&context, &tool_cashier, &cashier_admin)
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
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction =
            tool_cashier::open_finite_credit_issuance_ptb(&context, &tool_cashier, &cashier_admin)
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
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let transaction = tool_cashier::update_finite_credit_terms_ptb(
            &context,
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

    /// Buys finite credits for the signer from its SUI address balance.
    pub async fn buy_finite_credits(
        &self,
        tool_fqn: &ToolFqn,
        credits: u64,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = &self.client;
        let address = client.owner()?;
        self.buy_finite_credits_with(
            &client,
            tool_fqn,
            credits,
            PaymentSourceKind::user_funded(address),
        )
        .await
    }

    /// Buys finite credits for an explicit beneficiary from the signer address balance.
    pub async fn buy_finite_credits_for(
        &self,
        tool_fqn: &ToolFqn,
        credits: u64,
        beneficiary: PaymentSourceKind,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        let client = &self.client;
        self.buy_finite_credits_with(&client, tool_fqn, credits, beneficiary)
            .await
    }

    async fn buy_finite_credits_with(
        &self,
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        credits: u64,
        beneficiary: PaymentSourceKind,
    ) -> Result<EntitlementPurchaseResult, NexusError> {
        if credits == 0 {
            return Err(NexusError::Configuration(
                "Credits must be at least one".to_owned(),
            ));
        }
        let address = client.owner()?;
        let tool_cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let context = client.context_for_object(*tool_cashier.object_id()).await?;
        let price =
            Self::fetch_finite_credit_offer(client, &context, *tool_cashier.object_id(), tool_fqn)
                .await?
                .purchase_price(credits)?;
        let entitlement_id = crate::move_bindings::derive_finite_credits_id(
            &context,
            *tool_cashier.object_id(),
            beneficiary.clone(),
        )
        .map_err(NexusError::Parsing)?;
        let credit_account = client
            .crawler()
            .get_optional_object::<FiniteCredits>(entitlement_id)
            .await
            .map_err(NexusError::Rpc)?;
        let account = match credit_account {
            Some(account) => {
                if account.data.cashier.bytes != *tool_cashier.object_id()
                    || account.data.beneficiary != beneficiary
                    || !account.is_shared()
                {
                    return Err(NexusError::Configuration(format!(
                        "Finite credit account '{entitlement_id}' does not match its canonical account"
                    )));
                }
                Some(account.object_ref())
            }
            None => None,
        };
        let transaction = tool_cashier::buy_finite_credits_from_balance_ptb(
            &context,
            &tool_cashier,
            account.as_ref(),
            beneficiary,
            credits,
            price,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
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
        credits: u64,
    ) -> Result<EntitlementIssueResult, NexusError> {
        if credits == 0 {
            return Err(NexusError::Configuration(
                "Credits must be at least one".to_owned(),
            ));
        }
        let client = &self.client;
        let address = client.owner()?;
        let (context, tool_cashier, cashier_admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let entitlement_id = crate::move_bindings::derive_finite_credits_id(
            &context,
            *tool_cashier.object_id(),
            beneficiary.clone(),
        )
        .map_err(NexusError::Parsing)?;
        let credit_account = client
            .crawler()
            .get_optional_object::<FiniteCredits>(entitlement_id)
            .await
            .map_err(NexusError::Rpc)?;
        let transaction = match credit_account {
            Some(account) => {
                if account.data.cashier.bytes != *tool_cashier.object_id()
                    || account.data.beneficiary != beneficiary
                    || !account.is_shared()
                {
                    return Err(NexusError::Configuration(format!(
                        "Finite credit account '{entitlement_id}' does not match its canonical account"
                    )));
                }
                tool_cashier::issue_more_finite_credits_ptb(
                    &context,
                    &tool_cashier,
                    &account.object_ref(),
                    &cashier_admin,
                    credits,
                )
            }
            None => tool_cashier::issue_finite_credits_ptb(
                &context,
                &tool_cashier,
                &cashier_admin,
                beneficiary,
                credits,
            ),
        }
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(EntitlementIssueResult {
            tx_digest: response.digest,
            entitlement_id,
        })
    }

    /// Restores one refunded Invocation to its exact finite credit account.
    pub async fn restore_finite_credit_refund(
        &self,
        tool_fqn: &ToolFqn,
        invocation_id: sui::types::Address,
    ) -> Result<EntitlementIssueResult, NexusError> {
        let client = &self.client;
        let sender = client.owner()?;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let context = client.context_for_object(*cashier.object_id()).await?;
        let refunded = client
            .crawler()
            .get_object::<Invocation>(invocation_id)
            .await
            .map_err(NexusError::Rpc)?;
        let expected_policy =
            crate::transactions::invocation::InvocationPolicyCall::finite_credits_policy(&context)
                .map_err(NexusError::Parsing)?;
        if refunded.data.cashier_id.bytes != *cashier.object_id()
            || refunded.data.policy != expected_policy
        {
            return Err(NexusError::Configuration(format!(
                "Invocation '{invocation_id}' is not a finite credit refund for this Tool"
            )));
        }
        let [credits_source] = refunded.data.sources.as_slice() else {
            return Err(NexusError::Configuration(format!(
                "Invocation '{invocation_id}' does not name one finite credit account"
            )));
        };
        let credits_id = credits_source.bytes;
        if refunded.owner != sui::types::Owner::Address(credits_id)
            || refunded.data.refund_to.copied_option() != Some(credits_id)
        {
            return Err(NexusError::Configuration(format!(
                "Invocation '{invocation_id}' was not refunded to credit account '{credits_id}'"
            )));
        }
        let credits = client
            .crawler()
            .get_object::<FiniteCredits>(credits_id)
            .await
            .map_err(NexusError::Rpc)?;
        if credits.data.cashier.bytes != *cashier.object_id() || !credits.is_shared() {
            return Err(NexusError::Configuration(format!(
                "Finite credit account '{credits_id}' does not match this Tool"
            )));
        }
        let transaction = tool_cashier::restore_finite_credit_refund_ptb(
            &context,
            &credits.object_ref(),
            &refunded.object_ref(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, sender).await?;
        Ok(EntitlementIssueResult {
            tx_digest: response.digest,
            entitlement_id: credits_id,
        })
    }

    /// Reads the canonical access accounts for one Tool and beneficiary.
    ///
    /// Account IDs are derived from the Tool cashier, beneficiary, and policy.
    /// The returned finite credit refunds are exact [Invocation] objects that
    /// can be passed to [`ToolActions::restore_finite_credit_refund`].
    pub async fn inspect_access(
        &self,
        tool_fqn: &ToolFqn,
        beneficiary: PaymentSourceKind,
    ) -> Result<ToolAccess, NexusError> {
        let client = &self.client;
        let context = client
            .context_for_root(&client.nexus_objects.tool_registry)
            .await?;
        let crawler = client.crawler();
        let tool_id = crate::move_bindings::derive_tool_id(
            client.nexus_objects.tool_registry.object_id(),
            tool_fqn,
        )
        .map_err(NexusError::Parsing)?;
        let cashier_origin = context
            .type_origin(PackageRole::Tool, "tool_cashier", "ToolCashierKey")
            .map_err(NexusError::Parsing)?;
        let cashier_id = crate::move_bindings::derive_tool_cashier_id(cashier_origin, tool_id)
            .map_err(NexusError::Parsing)?;
        crawler
            .get_object_metadata(cashier_id)
            .await
            .map_err(NexusError::Rpc)?;

        let observed_at_ms = crawler
            .get_object::<SuiClock>(move_boundary::CLOCK_OBJECT_ID)
            .await
            .map_err(NexusError::Rpc)?
            .data
            .timestamp_ms;
        let credits_id = crate::move_bindings::derive_finite_credits_id(
            &context,
            cashier_id,
            beneficiary.clone(),
        )
        .map_err(NexusError::Parsing)?;
        let pass_id =
            crate::move_bindings::derive_time_pass_id(&context, cashier_id, beneficiary.clone())
                .map_err(NexusError::Parsing)?;

        let finite_credits = match crawler
            .get_optional_object::<FiniteCredits>(credits_id)
            .await
            .map_err(NexusError::Rpc)?
        {
            Some(credits) => {
                if !credits.is_shared()
                    || credits.data.cashier.bytes != cashier_id
                    || credits.data.beneficiary != beneficiary
                {
                    return Err(NexusError::Configuration(format!(
                        "Finite credit account '{credits_id}' does not match Tool '{tool_fqn}' and its beneficiary"
                    )));
                }
                let policy =
                    crate::transactions::invocation::InvocationPolicyCall::finite_credits_policy(
                        &context,
                    )
                    .map_err(NexusError::Parsing)?;
                let mut refunded_invocations = crawler
                    .get_owned_objects::<Invocation>(
                        credits_id,
                        crate::move_bindings::struct_tag::<Invocation>(&context),
                    )
                    .await
                    .map_err(NexusError::Rpc)?
                    .into_iter()
                    .filter(|invocation| {
                        invocation.data.cashier_id.bytes == cashier_id
                            && invocation.data.policy == policy
                            && invocation.data.sources.as_slice()
                                == [crate::move_bindings::sui_framework::object::ID::new(
                                    credits_id,
                                )]
                            && invocation.data.refund_to.copied_option() == Some(credits_id)
                    })
                    .map(|invocation| invocation.object_id)
                    .collect::<Vec<_>>();
                refunded_invocations.sort_unstable();
                Some(FiniteCreditAccess {
                    account_id: credits_id,
                    remaining: credits.data.state.remaining,
                    refunded_invocations,
                })
            }
            None => None,
        };

        let time_pass = match crawler
            .get_optional_object::<TimePass>(pass_id)
            .await
            .map_err(NexusError::Rpc)?
        {
            Some(pass) => {
                if !pass.is_shared()
                    || pass.data.cashier.bytes != cashier_id
                    || pass.data.beneficiary != beneficiary
                {
                    return Err(NexusError::Configuration(format!(
                        "Time pass account '{pass_id}' does not match Tool '{tool_fqn}' and its beneficiary"
                    )));
                }
                let valid_from_ms = pass.data.state.valid_from_ms;
                let valid_until_ms = pass.data.state.valid_until_ms;
                Some(TimePassAccess {
                    account_id: pass_id,
                    valid_from_ms,
                    valid_until_ms,
                    active: observed_at_ms >= valid_from_ms && observed_at_ms < valid_until_ms,
                })
            }
            None => None,
        };

        Ok(ToolAccess {
            tool_id,
            cashier_id,
            beneficiary,
            observed_at_ms,
            finite_credits,
            time_pass,
        })
    }

    /// Reads every accepted policy and the canonical offers for one [`Tool`].
    ///
    /// Custom policy witness types remain visible in [`ToolEconomy::policies`]
    /// even when this SDK does not know how to decode their private configs.
    pub async fn inspect_economy(&self, tool_fqn: &ToolFqn) -> Result<ToolEconomy, NexusError> {
        let client = &self.client;
        Self::inspect_economy_with(&client, tool_fqn).await
    }

    async fn inspect_economy_with(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
    ) -> Result<ToolEconomy, NexusError> {
        let crawler = client.crawler();
        let cashier_ref = client.fetch_tool_cashier(tool_fqn).await?;
        let cashier_id = *cashier_ref.object_id();
        let context = client.context_for_object(cashier_id).await?;
        let cashier = client
            .state_resolver()
            .load_inner_for_supported_witness::<ToolCashier, ToolCashierInnerV1>(
                cashier_id, &context,
            )
            .await?;
        let tool_id = crate::move_bindings::derive_tool_id(
            client.nexus_objects.tool_registry.object_id(),
            tool_fqn,
        )
        .map_err(NexusError::Parsing)?;
        if cashier.data.tool.bytes != tool_id
            || cashier.data.tool_fqn.as_str() != tool_fqn.to_string()
        {
            return Err(NexusError::Configuration(format!(
                "Tool cashier '{cashier_id}' does not describe Tool '{tool_fqn}'"
            )));
        }

        let policies = cashier.data.policies.contents;
        if !canonical_policy_accepted(&context, &policies, "fixed_price")? {
            return Err(NexusError::Configuration(format!(
                "Tool '{tool_fqn}' is missing its mandatory fixed price policy"
            )));
        }
        let registry_root = &client.nexus_objects.tool_registry;
        let registry_context = client.context_for_root(registry_root).await?;
        let registry = client
            .state_resolver()
            .load_inner_for_supported_witness::<ToolRegistry, ToolRegistryInnerV1>(
                registry_root.object_id(),
                &registry_context,
            )
            .await?;
        let fqn_key = ascii::String::from(tool_fqn.to_string());
        let fqn_type = crate::move_bindings::type_tag::<ascii::String>(&registry_context);
        let fixed_price_mist = crawler
            .get_dynamic_field_by_key::<ascii::String, u64>(
                registry.data.invocation_costs_mist.id(),
                fqn_key,
                &fqn_type,
            )
            .await
            .map_err(NexusError::Rpc)?
            .ok_or_else(|| {
                NexusError::Configuration(format!(
                    "Fixed price policy for Tool '{tool_fqn}' has no price"
                ))
            })?;
        let finite_credits = if canonical_policy_accepted(&context, &policies, "finite_credits")? {
            Some(Self::fetch_finite_credit_offer(client, &context, cashier_id, tool_fqn).await?)
        } else {
            None
        };
        let time_pass = if canonical_policy_accepted(&context, &policies, "time_pass")? {
            Some(Self::fetch_time_pass_offer(client, &context, cashier_id, tool_fqn).await?)
        } else {
            None
        };

        Ok(ToolEconomy {
            tool_id,
            cashier_id,
            policies,
            fixed_price_mist,
            finite_credits,
            time_pass,
        })
    }

    /// Lists finalized Invocations and prepaid deposits waiting for collection.
    ///
    /// Transfer to Object indexes these objects under the [`ToolCashier`] ID as
    /// an address owner. Discovery therefore needs neither a mutable on chain
    /// registry nor a global usage counter.
    pub async fn inspect_cashier_inbox(
        &self,
        tool_fqn: &ToolFqn,
    ) -> Result<ToolCashierInbox, NexusError> {
        let client = &self.client;
        let cashier = client.fetch_tool_cashier(tool_fqn).await?;
        let cashier_id = *cashier.object_id();
        let context = client.context_for_object(cashier_id).await?;
        let invocations = client
            .crawler()
            .get_owned_objects::<Invocation>(
                cashier_id,
                crate::move_bindings::struct_tag::<Invocation>(&context),
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
            .get_owned_objects::<CashierDeposit>(
                cashier_id,
                crate::move_bindings::struct_tag::<CashierDeposit>(&context),
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
        let client = &self.client;
        let address = client.owner()?;
        let (context, cashier, admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let tool_id = crate::move_bindings::derive_tool_id(
            client.nexus_objects.tool_registry.object_id(),
            tool_fqn,
        )
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
                if response.owner != sui::types::Owner::Address(*cashier.object_id())
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
            &context,
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
        let client = &self.client;
        let (context, cashier, admin) =
            Self::resolve_tool_cashier_and_cap(&client, tool_fqn, cashier_admin).await?;
        let references = client
            .crawler()
            .get_objects_metadata(deposit_ids)
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .map(|response| {
                if response.owner != sui::types::Owner::Address(*cashier.object_id()) {
                    return Err(NexusError::Configuration(format!(
                        "Deposit '{}' is not in Tool cashier '{}'",
                        response.object_id,
                        cashier.object_id()
                    )));
                }
                Ok(response.object_ref())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let transaction =
            tool_cashier::collect_deposits_ptb(&context, &cashier, &admin, &references, recipient)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    async fn registry_state(&self) -> Result<(Arc<NexusContext>, ToolRegistryInnerV1), NexusError> {
        let root = &self.client.nexus_objects.tool_registry;
        let context = self.client.context_for_root(root).await?;
        let state = self
            .client
            .state_resolver()
            .load_inner_for_supported_witness::<ToolRegistry, ToolRegistryInnerV1>(
                root.object_id(),
                &context,
            )
            .await?;
        Ok((context, state.data))
    }

    async fn tool_directory(
        &self,
        context: &NexusContext,
        registry: &ToolRegistryInnerV1,
    ) -> Result<Vec<(ToolFqn, sui::types::Address)>, NexusError> {
        let mut next = registry.tool_ids.head.cloned_option();
        let mut seen = HashSet::with_capacity(registry.tool_ids.size());
        let mut entries = Vec::with_capacity(registry.tool_ids.size());
        let key_type = crate::move_bindings::type_tag::<ascii::String>(context);

        while let Some(key) = next {
            let raw = std::str::from_utf8(&key.bytes).map_err(|error| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Tool Registry contains a non UTF8 name: {error}"
                ))
            })?;
            let fqn = raw.parse::<ToolFqn>().map_err(|error| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Tool Registry contains invalid FQN '{raw}': {error}"
                ))
            })?;
            if !seen.insert(key.clone()) {
                return Err(NexusError::InvalidObjectState {
                    object: context.tool_registry.object_id(),
                    reason: "Tool directory contains a cycle".to_owned(),
                });
            }
            let node = self
                .client
                .crawler()
                .get_dynamic_field_by_key::<ascii::String, Node<ascii::String, ID>>(
                    registry.tool_ids.id(),
                    key,
                    &key_type,
                )
                .await
                .map_err(NexusError::Rpc)?
                .ok_or_else(|| NexusError::InvalidObjectState {
                    object: context.tool_registry.object_id(),
                    reason: format!("Tool directory node for '{fqn}' is missing"),
                })?;
            entries.push((fqn, node.value.bytes));
            next = node.next.into_option();
        }

        if entries.len() != registry.tool_ids.size() {
            return Err(NexusError::InvalidObjectState {
                object: context.tool_registry.object_id(),
                reason: format!(
                    "Tool directory reports {} entries but contains {}",
                    registry.tool_ids.size(),
                    entries.len()
                ),
            });
        }
        Ok(entries)
    }

    async fn inspect_tool_id(
        &self,
        registry_context: &NexusContext,
        registry: &ToolRegistryInnerV1,
        fqn: ToolFqn,
        tool_id: sui::types::Address,
    ) -> ToolInspection {
        let current_package = registry_context.packages().get(PackageRole::Tool);
        let fallback_origin = current_package
            .and_then(|package| package.type_origin("tool_cashier", "ToolCashierKey").ok())
            .unwrap_or(sui::types::Address::ZERO);
        let mut inspection = ToolInspection {
            fqn,
            tool_id,
            tool_cashier_id: crate::move_bindings::derive_tool_cashier_id(fallback_origin, tool_id)
                .unwrap_or(sui::types::Address::ZERO),
            exists: false,
            owner: None,
            witness_type: None,
            inner_type: None,
            compatibility: ToolCompatibility::Unavailable,
            tool: None,
            verifier_support: None,
            external_verifier: None,
            timeout_ms: None,
            invocation_cost_mist: None,
            detail: None,
        };

        match self
            .client
            .crawler()
            .get_optional_object::<ToolAnchor>(tool_id)
            .await
        {
            Ok(Some(anchor)) => {
                inspection.exists = true;
                inspection.owner = Some(anchor.owner);
            }
            Ok(None) => return inspection,
            Err(error) => {
                inspection.detail = Some(format!("Tool object is unavailable: {error}"));
                return inspection;
            }
        }

        let observed = match self.client.state_resolver().observe(tool_id).await {
            Ok(observed) => observed,
            Err(error) => {
                inspection.detail = Some(error.to_string());
                return inspection;
            }
        };
        inspection.witness_type = Some(observed.witness_type().clone());
        inspection.inner_type = Some(observed.inner_type().clone());

        let packages = match self
            .client
            .state_resolver()
            .resolve_package_graph(&observed)
            .await
        {
            Ok(packages) => packages,
            Err(error) => {
                inspection.compatibility = ToolCompatibility::Unsupported;
                inspection.detail = Some(error.to_string());
                return inspection;
            }
        };
        let context = NexusContext::new(self.client.get_nexus_objects(), packages);
        if let Ok(origin) = context
            .require_package(PackageRole::Tool)
            .and_then(|package| package.type_origin("tool_cashier", "ToolCashierKey"))
        {
            if let Ok(cashier_id) = crate::move_bindings::derive_tool_cashier_id(origin, tool_id) {
                inspection.tool_cashier_id = cashier_id;
            }
        }

        let inner = match self
            .client
            .state_resolver()
            .load_inner_for_supported_witness::<ToolAnchor, ToolInnerV1>(tool_id, &context)
            .await
        {
            Ok(inner) => inner.data,
            Err(error) => {
                inspection.compatibility = ToolCompatibility::Unsupported;
                inspection.detail = Some(error.to_string());
                return inspection;
            }
        };

        let registered = inner.unregistered_at_ms.vec.is_empty();
        let selected = context.packages().get(PackageRole::Tool);
        let is_current = current_package
            .zip(selected)
            .is_some_and(|(current, selected)| {
                current.storage_id == selected.storage_id && current.version == selected.version
            });
        inspection.compatibility = if is_current {
            ToolCompatibility::Current
        } else if registered {
            ToolCompatibility::MigrationRequired
        } else {
            ToolCompatibility::LegacyUnderstood
        };
        inspection.tool = Some(ToolState::new(tool_id, inner));

        if registered {
            if let Err(error) = self
                .load_registry_projections(registry_context, registry, &mut inspection)
                .await
            {
                inspection.detail = Some(error.to_string());
            }
        }
        inspection
    }

    async fn load_registry_projections(
        &self,
        context: &NexusContext,
        registry: &ToolRegistryInnerV1,
        inspection: &mut ToolInspection,
    ) -> Result<(), NexusError> {
        let fqn_key = ascii::String::from(inspection.fqn.to_string());
        let fqn_type = crate::move_bindings::type_tag::<ascii::String>(context);
        inspection.timeout_ms = self
            .client
            .crawler()
            .get_dynamic_field_by_key::<ascii::String, Node<ascii::String, u64>>(
                registry.timeouts.id(),
                fqn_key.clone(),
                &fqn_type,
            )
            .await
            .map_err(NexusError::Rpc)?
            .map(|node| node.value);
        inspection.invocation_cost_mist = self
            .client
            .crawler()
            .get_dynamic_field_by_key::<ascii::String, u64>(
                registry.invocation_costs_mist.id(),
                fqn_key,
                &fqn_type,
            )
            .await
            .map_err(NexusError::Rpc)?;

        let id_type = crate::move_bindings::type_tag::<ID>(context);
        inspection.verifier_support = self
            .client
            .crawler()
            .get_dynamic_field_by_key::<ID, ToolVerifierSupport>(
                registry.verifier_support.id(),
                ID::new(inspection.tool_id),
                &id_type,
            )
            .await
            .map_err(NexusError::Rpc)?;
        inspection.external_verifier = self
            .client
            .crawler()
            .get_dynamic_field_by_key::<ID, ExternalVerifier>(
                registry.external_verifiers.id(),
                ID::new(inspection.tool_id),
                &id_type,
            )
            .await
            .map_err(NexusError::Rpc)?;
        Ok(())
    }

    async fn current_tool_inputs(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
        uses_registry: bool,
    ) -> Result<
        (
            Arc<NexusContext>,
            sui::types::ObjectReference,
            sui::types::ObjectReference,
        ),
        NexusError,
    > {
        let tool = self.client.fetch_tool(fqn).await?;
        let context = if uses_registry {
            self.client
                .context_for_object_with_roots(
                    *tool.object_id(),
                    std::slice::from_ref(&self.client.nexus_objects.tool_registry),
                )
                .await?
        } else {
            self.client.context_for_object(*tool.object_id()).await?
        };
        let owner_cap = self.client.object_reference(owner_cap).await?;
        Ok((context, tool, owner_cap))
    }

    /// Resolves [`Self::unregister`] from stable IDs and current Registry authority.
    ///
    /// Unregistration is the recovery path for a Tool whose inner value this
    /// SDK cannot decode. The transaction therefore reads only object metadata
    /// for the Tool and owner capability. The current [`ToolRegistry`] selects
    /// the call target, and the Move transition remains authoritative for the
    /// Tool state and ownership checks.
    async fn unregister_inputs(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<
        (
            Arc<NexusContext>,
            sui::types::ObjectReference,
            sui::types::ObjectReference,
        ),
        NexusError,
    > {
        let registry = &self.client.nexus_objects.tool_registry;
        let context = self.client.context_for_root(registry).await?;
        let tool_id = crate::move_bindings::derive_tool_id(registry.object_id(), fqn)
            .map_err(NexusError::Parsing)?;
        let tool = self.client.object_reference(tool_id).await?;
        let owner_cap = self.client.object_reference(owner_cap).await?;
        Ok((context, tool, owner_cap))
    }

    async fn submit_action(
        &self,
        transaction: sui::types::ProgrammableTransaction,
    ) -> Result<ToolActionResult, NexusError> {
        let response = self
            .client
            .submit_transaction(transaction, self.client.owner()?)
            .await?;
        Ok(ToolActionResult {
            tx_digest: response.digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                interface::meta_schema::MetaSchema,
                move_std::option::Option as MoveOption,
                sui_framework::{
                    balance::Balance,
                    linked_table::LinkedTable,
                    object::UID,
                    table::Table,
                    vec_set::VecSet,
                },
                tool::{era::V1 as ToolWitnessV1, tool_registry::ToolRef},
            },
            test_utils::{nexus_mocks, sui_mocks},
        },
        std::collections::HashMap,
    };

    fn registry_inner(
        directory_id: sui::types::Address,
        keys: &[ascii::String],
    ) -> ToolRegistryInnerV1 {
        let mut directory = LinkedTable::new(directory_id, keys.len() as u64);
        directory.head = MoveOption::from_option(keys.first().cloned());
        directory.tail = MoveOption::from_option(keys.last().cloned());
        ToolRegistryInnerV1::new(
            directory,
            Table::new(sui::types::Address::from_static("0xd2"), keys.len() as u64),
            Table::new(sui::types::Address::from_static("0xd3"), keys.len() as u64),
            LinkedTable::new(sui::types::Address::from_static("0xd4"), keys.len() as u64),
            Table::new(sui::types::Address::from_static("0xd5"), 0),
            Table::new(sui::types::Address::from_static("0xd6"), 0),
            Table::new(sui::types::Address::from_static("0xd7"), keys.len() as u64),
            LinkedTable::new(sui::types::Address::from_static("0xd8"), 0),
            LinkedTable::new(sui::types::Address::from_static("0xd9"), 0),
            0,
            0,
        )
    }

    fn tool_inner(registry_id: sui::types::Address, fqn: &ToolFqn) -> ToolInnerV1 {
        ToolInnerV1::new(
            ID::new(registry_id),
            ascii::String::from(fqn.to_string()),
            ToolRef::Http {
                url: b"https://example.com/tool".to_vec(),
            },
            b"Compatibility fixture".to_vec(),
            MetaSchema::new(vec![], vec![]),
            false,
            Balance {
                value: 0,
                phantom_t0: std::marker::PhantomData,
            },
            false,
            0,
            0,
            MoveOption::from(Some(1)),
        )
    }

    struct EconomyActionFixture {
        client: NexusClient,
        fqn: ToolFqn,
        cashier_admin: sui::types::Address,
    }

    async fn economy_action_fixture_with<F>(configure: F) -> EconomyActionFixture
    where
        F: FnOnce(
            &NexusContext,
            sui::types::Address,
            &mut sui_mocks::grpc::MockLedgerService,
            &mut sui_mocks::grpc::MockStateService,
        ),
    {
        let context = sui_mocks::mock_nexus_context();
        let registry_id = context.tool_registry.object_id();
        let fqn = "xyz.taluslabs.economy.action@1"
            .parse::<ToolFqn>()
            .expect("Tool FQN parses");
        let tool_id =
            crate::move_bindings::derive_tool_id(registry_id, &fqn).expect("Tool ID derives");
        let cashier_origin = context
            .type_origin(PackageRole::Tool, "tool_cashier", "ToolCashierKey")
            .expect("cashier origin resolves");
        let cashier_id = crate::move_bindings::derive_tool_cashier_id(cashier_origin, tool_id)
            .expect("cashier ID derives");
        let cashier_admin = sui::types::Address::from_static("0xca");
        let policies = vec![
            crate::transactions::invocation::InvocationPolicyCall::fixed_price(&context)
                .expect("fixed price policy resolves")
                .policy,
            crate::transactions::invocation::InvocationPolicyCall::finite_credits_policy(&context)
                .expect("finite credit policy resolves"),
            crate::transactions::invocation::InvocationPolicyCall::time_pass(
                &context,
                sui::types::Address::ZERO,
                1,
            )
            .expect("time pass policy resolves")
            .policy,
        ];
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut packages = sui_mocks::grpc::MockMovePackageService::new();
        let mut state = sui_mocks::grpc::MockStateService::new();
        mock_registry(
            &mut ledger,
            &mut state,
            &context,
            registry_inner(sui::types::Address::from_static("0xd1"), &[]),
        );
        sui_mocks::grpc::mock_object_state::<ToolAnchor, ToolWitnessV1, ToolInnerV1>(
            &mut ledger,
            &mut state,
            &context,
            sui_mocks::object_ref_for_id(tool_id),
            sui::types::Owner::Shared(1),
            ToolAnchor::new(UID::new(tool_id)),
            tool_inner(registry_id, &fqn),
        );
        sui_mocks::grpc::mock_nexus_package_graph(&mut ledger, &mut packages, context.packages());
        sui_mocks::grpc::mock_object_state::<ToolCashier, ToolWitnessV1, ToolCashierInnerV1>(
            &mut ledger,
            &mut state,
            &context,
            sui_mocks::object_ref_for_id(cashier_id),
            sui::types::Owner::Shared(2),
            ToolCashier::new(UID::new(cashier_id)),
            ToolCashierInnerV1::new(
                ID::new(tool_id),
                ascii::String::from(fqn.to_string()),
                VecSet { contents: policies },
            ),
        );
        sui_mocks::grpc::mock_get_object_metadata_exact(
            &mut ledger,
            sui_mocks::object_ref_for_id(cashier_admin),
            sui::types::Owner::Address(sui::types::Address::from_static("0xa")),
            None,
        );
        configure(&context, cashier_id, &mut ledger, &mut state);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(packages),
            state_service_mock: Some(state),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;

        EconomyActionFixture {
            client,
            fqn,
            cashier_admin,
        }
    }

    async fn economy_action_fixture() -> EconomyActionFixture {
        economy_action_fixture_with(|_, _, _, _| {}).await
    }

    fn assert_missing_gas<T>(result: Result<T, NexusError>) {
        assert!(matches!(result, Err(NexusError::Configuration(message)) if
            message.contains("a gas source is required")));
    }

    fn mock_registry(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        state_service: &mut sui_mocks::grpc::MockStateService,
        context: &NexusContext,
        registry: ToolRegistryInnerV1,
    ) {
        let registry_id = context.tool_registry.object_id();
        sui_mocks::grpc::mock_object_state::<ToolRegistry, ToolWitnessV1, ToolRegistryInnerV1>(
            ledger_service,
            state_service,
            context,
            sui_mocks::object_ref_for_id(registry_id),
            sui::types::Owner::Shared(context.tool_registry.initial_shared_version),
            ToolRegistry::new(UID::new(registry_id)),
            registry,
        );
    }

    #[test]
    fn compatibility_states_are_distinct() {
        let states = [
            ToolCompatibility::Current,
            ToolCompatibility::LegacyUnderstood,
            ToolCompatibility::MigrationRequired,
            ToolCompatibility::Unsupported,
            ToolCompatibility::Unavailable,
        ];
        assert_eq!(states.into_iter().collect::<HashSet<_>>().len(), 5);
    }

    #[test]
    fn canonical_offer_prices_enforce_sale_terms_and_overflow() {
        let finite = FiniteCreditOffer {
            issuance_enabled: true,
            price_per_credit: 3,
            minimum_credits: 2,
            maximum_credits: 4,
        };
        assert_eq!(finite.purchase_price(3).unwrap(), 9);
        assert!(finite.purchase_price(1).is_err());
        assert!(finite.purchase_price(5).is_err());
        assert!(FiniteCreditOffer {
            issuance_enabled: false,
            ..finite.clone()
        }
        .purchase_price(3)
        .is_err());
        assert!(FiniteCreditOffer {
            price_per_credit: u64::MAX,
            ..finite
        }
        .purchase_price(2)
        .is_err());

        let pass = TimePassOffer {
            issuance_enabled: true,
            price_per_ms: 3,
            minimum_duration_ms: 2,
            maximum_duration_ms: 4,
        };
        assert_eq!(pass.purchase_price(3).unwrap(), 9);
        assert!(pass.purchase_price(1).is_err());
        assert!(pass.purchase_price(5).is_err());
        assert!(TimePassOffer {
            issuance_enabled: false,
            ..pass.clone()
        }
        .purchase_price(3)
        .is_err());
        assert!(TimePassOffer {
            price_per_ms: u64::MAX,
            ..pass
        }
        .purchase_price(2)
        .is_err());
    }

    #[tokio::test]
    async fn policy_administration_reaches_submission_after_resolving_live_inputs() {
        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .enable_time_passes(&fixture.fqn, fixture.cashier_admin, 2, 1, 10)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .close_time_pass_issuance(&fixture.fqn, fixture.cashier_admin)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .open_time_pass_issuance(&fixture.fqn, fixture.cashier_admin)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .update_time_pass_terms(&fixture.fqn, fixture.cashier_admin, 3, 2, 20)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .enable_finite_credits(&fixture.fqn, fixture.cashier_admin, 2, 1, 10)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .close_finite_credit_issuance(&fixture.fqn, fixture.cashier_admin)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .open_finite_credit_issuance(&fixture.fqn, fixture.cashier_admin)
                .await,
        );

        let fixture = economy_action_fixture().await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .update_finite_credit_terms(&fixture.fqn, fixture.cashier_admin, 3, 2, 20)
                .await,
        );
    }

    #[tokio::test]
    async fn economy_actions_reject_invalid_values_before_chain_reads() {
        let context = sui_mocks::mock_nexus_context();
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;
        let fqn = "xyz.taluslabs.validation@1"
            .parse::<ToolFqn>()
            .expect("Tool FQN parses");
        let admin = sui::types::Address::from_static("0xca");
        let beneficiary = PaymentSourceKind::user_funded(client.owner().unwrap());

        assert!(client.tool().buy_time_pass(&fqn, 0).await.is_err());
        assert!(client.tool().buy_finite_credits(&fqn, 0).await.is_err());
        assert!(client
            .tool()
            .issue_time_pass(&fqn, admin, beneficiary.clone(), 10, 10)
            .await
            .is_err());
        assert!(client
            .tool()
            .issue_finite_credits(&fqn, admin, beneficiary, 0)
            .await
            .is_err());
        assert!(client
            .tool()
            .enable_finite_credits(&fqn, admin, 1, 0, 10)
            .await
            .is_err());
        assert!(client
            .tool()
            .enable_finite_credits(&fqn, admin, 1, 10, 1)
            .await
            .is_err());
        assert!(client
            .tool()
            .update_finite_credit_terms(&fqn, admin, 1, 0, 10)
            .await
            .is_err());
        assert!(client
            .tool()
            .update_finite_credit_terms(&fqn, admin, 1, 10, 1)
            .await
            .is_err());
        assert!(client
            .tool()
            .collect_invocations(&fqn, admin, &[], sui::types::Address::from_static("0xcb"),)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn entitlement_purchase_and_issuance_reach_submission_with_canonical_accounts() {
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb1"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                cashier_id,
                &crate::move_bindings::type_tag::<PolicyKey<time_pass::Policy>>(context),
                PolicyKey::<time_pass::Policy>::new(false),
                time_pass::Config::new(true, 2, 1, 100),
            );
            let pass_id =
                crate::move_bindings::derive_time_pass_id(context, cashier_id, expected.clone())
                    .expect("pass ID derives");
            sui_mocks::grpc::mock_get_object_not_found(ledger, pass_id);
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .buy_time_pass_for(&fixture.fqn, 10, beneficiary)
                .await,
        );

        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb2"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                cashier_id,
                &crate::move_bindings::type_tag::<PolicyKey<finite_credits::Policy>>(context),
                PolicyKey::<finite_credits::Policy>::new(false),
                finite_credits::Config::new(true, 2, 1, 100),
            );
            let credits_id = crate::move_bindings::derive_finite_credits_id(
                context,
                cashier_id,
                expected.clone(),
            )
            .expect("credit ID derives");
            sui_mocks::grpc::mock_get_object_not_found(ledger, credits_id);
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .buy_finite_credits_for(&fixture.fqn, 10, beneficiary)
                .await,
        );

        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb3"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            let pass_id =
                crate::move_bindings::derive_time_pass_id(context, cashier_id, expected.clone())
                    .expect("pass ID derives");
            sui_mocks::grpc::mock_get_object_not_found(ledger, pass_id);
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .issue_time_pass(&fixture.fqn, fixture.cashier_admin, beneficiary, 10, 20)
                .await,
        );

        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb4"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            let credits_id = crate::move_bindings::derive_finite_credits_id(
                context,
                cashier_id,
                expected.clone(),
            )
            .expect("credit ID derives");
            sui_mocks::grpc::mock_get_object_not_found(ledger, credits_id);
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .issue_finite_credits(&fixture.fqn, fixture.cashier_admin, beneficiary, 10)
                .await,
        );
    }

    #[tokio::test]
    async fn existing_entitlements_are_extended_without_replacing_their_accounts() {
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb5"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                cashier_id,
                &crate::move_bindings::type_tag::<PolicyKey<time_pass::Policy>>(context),
                PolicyKey::<time_pass::Policy>::new(false),
                time_pass::Config::new(true, 2, 1, 100),
            );
            let pass_id =
                crate::move_bindings::derive_time_pass_id(context, cashier_id, expected.clone())
                    .expect("pass ID derives");
            let pass = TimePass::new(
                UID::new(pass_id),
                ID::new(cashier_id),
                expected,
                time_pass::State::new(10, 20),
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger,
                sui_mocks::object_ref_for_id(pass_id),
                sui::types::Owner::Shared(3),
                bcs::to_bytes(&pass).expect("pass serializes"),
            );
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .buy_time_pass_for(&fixture.fqn, 10, beneficiary)
                .await,
        );

        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb6"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                cashier_id,
                &crate::move_bindings::type_tag::<PolicyKey<finite_credits::Policy>>(context),
                PolicyKey::<finite_credits::Policy>::new(false),
                finite_credits::Config::new(true, 2, 1, 100),
            );
            let credits_id = crate::move_bindings::derive_finite_credits_id(
                context,
                cashier_id,
                expected.clone(),
            )
            .expect("credit ID derives");
            let credits = FiniteCredits::new(
                UID::new(credits_id),
                ID::new(cashier_id),
                expected,
                finite_credits::State::new(7),
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger,
                sui_mocks::object_ref_for_id(credits_id),
                sui::types::Owner::Shared(4),
                bcs::to_bytes(&credits).expect("credits serialize"),
            );
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .buy_finite_credits_for(&fixture.fqn, 10, beneficiary)
                .await,
        );

        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb7"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            let pass_id =
                crate::move_bindings::derive_time_pass_id(context, cashier_id, expected.clone())
                    .expect("pass ID derives");
            let pass = TimePass::new(
                UID::new(pass_id),
                ID::new(cashier_id),
                expected,
                time_pass::State::new(10, 20),
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger,
                sui_mocks::object_ref_for_id(pass_id),
                sui::types::Owner::Shared(5),
                bcs::to_bytes(&pass).expect("pass serializes"),
            );
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .issue_time_pass(&fixture.fqn, fixture.cashier_admin, beneficiary, 20, 30)
                .await,
        );

        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb8"));
        let expected = beneficiary.clone();
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            let credits_id = crate::move_bindings::derive_finite_credits_id(
                context,
                cashier_id,
                expected.clone(),
            )
            .expect("credit ID derives");
            let credits = FiniteCredits::new(
                UID::new(credits_id),
                ID::new(cashier_id),
                expected,
                finite_credits::State::new(7),
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger,
                sui_mocks::object_ref_for_id(credits_id),
                sui::types::Owner::Shared(6),
                bcs::to_bytes(&credits).expect("credits serialize"),
            );
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .issue_finite_credits(&fixture.fqn, fixture.cashier_admin, beneficiary, 10)
                .await,
        );
    }

    #[tokio::test]
    async fn finite_credit_refund_restoration_uses_the_exact_recorded_account() {
        let invocation_id = sui::types::Address::from_static("0x91");
        let credits_id = sui::types::Address::from_static("0x92");
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0xb9"));
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            let policy =
                crate::transactions::invocation::InvocationPolicyCall::finite_credits_policy(
                    context,
                )
                .expect("finite credit policy resolves");
            let invocation = Invocation::new(
                UID::new(invocation_id),
                sui::types::Address::from_static("0xe1"),
                b"vertex".to_vec(),
                ID::new(sui::types::Address::from_static("0xe2")),
                ID::new(cashier_id),
                beneficiary.clone(),
                policy,
                vec![ID::new(credits_id)],
                0,
                MoveOption::from_option(Some(credits_id)),
                Balance {
                    value: 0,
                    phantom_t0: std::marker::PhantomData,
                },
            );
            let credits = FiniteCredits::new(
                UID::new(credits_id),
                ID::new(cashier_id),
                beneficiary,
                finite_credits::State::new(0),
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger,
                sui_mocks::object_ref_for_id(invocation_id),
                sui::types::Owner::Address(credits_id),
                bcs::to_bytes(&invocation).expect("Invocation serializes"),
            );
            sui_mocks::grpc::mock_get_object_bcs(
                ledger,
                sui_mocks::object_ref_for_id(credits_id),
                sui::types::Owner::Shared(7),
                bcs::to_bytes(&credits).expect("credits serialize"),
            );
        })
        .await;

        assert_missing_gas(
            fixture
                .client
                .tool()
                .restore_finite_credit_refund(&fixture.fqn, invocation_id)
                .await,
        );
    }

    #[tokio::test]
    async fn economy_inspection_decodes_every_canonical_offer() {
        let fixture = economy_action_fixture_with(|context, cashier_id, ledger, _state| {
            let fqn = "xyz.taluslabs.economy.action@1"
                .parse::<ToolFqn>()
                .expect("Tool FQN parses");
            let fqn_key = ascii::String::from(fqn.to_string());
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                sui::types::Address::from_static("0xd7"),
                &crate::move_bindings::type_tag::<ascii::String>(context),
                fqn_key,
                17_u64,
            );
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                cashier_id,
                &crate::move_bindings::type_tag::<PolicyKey<finite_credits::Policy>>(context),
                PolicyKey::<finite_credits::Policy>::new(false),
                finite_credits::Config::new(true, 2, 1, 100),
            );
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                ledger,
                cashier_id,
                &crate::move_bindings::type_tag::<PolicyKey<time_pass::Policy>>(context),
                PolicyKey::<time_pass::Policy>::new(false),
                time_pass::Config::new(true, 3, 4, 200),
            );
        })
        .await;

        let economy = fixture
            .client
            .tool()
            .inspect_economy(&fixture.fqn)
            .await
            .expect("economy inspection succeeds");

        assert_eq!(economy.fixed_price_mist, 17);
        assert_eq!(
            economy.finite_credits,
            Some(FiniteCreditOffer {
                issuance_enabled: true,
                price_per_credit: 2,
                minimum_credits: 1,
                maximum_credits: 100,
            })
        );
        assert_eq!(
            economy.time_pass,
            Some(TimePassOffer {
                issuance_enabled: true,
                price_per_ms: 3,
                minimum_duration_ms: 4,
                maximum_duration_ms: 200,
            })
        );
    }

    #[tokio::test]
    async fn empty_cashier_inbox_is_discovered_without_a_mutable_index() {
        let fixture = economy_action_fixture_with(|_, _, _ledger, state| {
            state.expect_list_owned_objects().times(2).returning(|_| {
                Ok(tonic::Response::new(
                    sui::grpc::ListOwnedObjectsResponse::default(),
                ))
            });
        })
        .await;

        let inbox = fixture
            .client
            .tool()
            .inspect_cashier_inbox(&fixture.fqn)
            .await
            .expect("cashier inbox inspection succeeds");

        assert!(inbox.invocations.is_empty());
        assert!(inbox.deposits.is_empty());
    }

    #[tokio::test]
    async fn owner_collection_validates_exact_cashier_objects_before_submission() {
        let invocation_id = sui::types::Address::from_static("0x93");
        let fixture = economy_action_fixture_with(move |context, cashier_id, ledger, _state| {
            let fqn = "xyz.taluslabs.economy.action@1"
                .parse::<ToolFqn>()
                .expect("Tool FQN parses");
            let tool_id =
                crate::move_bindings::derive_tool_id(context.tool_registry.object_id(), &fqn)
                    .expect("Tool ID derives");
            let policy =
                crate::transactions::invocation::InvocationPolicyCall::fixed_price(context)
                    .expect("fixed price policy resolves")
                    .policy;
            let invocation = Invocation::new(
                UID::new(invocation_id),
                sui::types::Address::from_static("0xe1"),
                b"vertex".to_vec(),
                ID::new(tool_id),
                ID::new(cashier_id),
                PaymentSourceKind::user_funded(sui::types::Address::from_static("0xba")),
                policy,
                vec![ID::new(sui::types::Address::from_static("0xe3"))],
                9,
                MoveOption::from_option(None),
                Balance {
                    value: 9,
                    phantom_t0: std::marker::PhantomData,
                },
            );
            sui_mocks::grpc::mock_get_objects_bcs(
                ledger,
                vec![(
                    sui_mocks::object_ref_for_id(invocation_id),
                    sui::types::Owner::Address(cashier_id),
                    bcs::to_bytes(&invocation).expect("Invocation serializes"),
                    crate::move_bindings::struct_tag::<Invocation>(context),
                )],
            );
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .collect_invocations(
                    &fixture.fqn,
                    fixture.cashier_admin,
                    &[invocation_id],
                    sui::types::Address::from_static("0xbb"),
                )
                .await,
        );

        let deposit_id = sui::types::Address::from_static("0x94");
        let fixture = economy_action_fixture_with(move |_, cashier_id, ledger, _state| {
            sui_mocks::grpc::mock_get_objects_metadata(
                ledger,
                vec![(
                    sui_mocks::object_ref_for_id(deposit_id),
                    sui::types::Owner::Address(cashier_id),
                    None,
                )],
            );
        })
        .await;
        assert_missing_gas(
            fixture
                .client
                .tool()
                .collect_deposits(
                    &fixture.fqn,
                    fixture.cashier_admin,
                    &[deposit_id],
                    sui::types::Address::from_static("0xbb"),
                )
                .await,
        );
    }

    #[tokio::test]
    async fn inspect_tool_reports_missing_when_neither_object_exists() {
        let context = sui_mocks::mock_nexus_context();
        let fqn = "xyz.taluslabs.missing.tool@1".parse::<ToolFqn>().unwrap();
        let tool_id =
            crate::move_bindings::derive_tool_id(context.tool_registry.object_id(), &fqn).unwrap();
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_registry(
            &mut ledger_service,
            &mut state_service,
            &context,
            registry_inner(sui::types::Address::from_static("0xd1"), &[]),
        );
        sui_mocks::grpc::mock_get_object_not_found(&mut ledger_service, tool_id);
        sui_mocks::grpc::mock_package_versions(
            &mut ledger_service,
            &mut package_service,
            context.packages().all().cloned(),
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;

        let inspection = client.tool().inspect_tool(&fqn).await.unwrap();

        assert!(!inspection.exists);
        assert!(inspection.tool.is_none());
        assert_eq!(inspection.tool_id, tool_id);
    }

    #[tokio::test]
    async fn mixed_tool_versions_are_classified_without_aborting_the_inventory() {
        let context = sui_mocks::mock_nexus_context();
        let registry_id = context.tool_registry.object_id();
        let directory_id = sui::types::Address::from_static("0xd1");
        let entries = [
            (
                "xyz.taluslabs.current.tool@1".parse::<ToolFqn>().unwrap(),
                sui::types::Address::from_static("0xc1"),
            ),
            (
                "xyz.taluslabs.unavailable.tool@1"
                    .parse::<ToolFqn>()
                    .unwrap(),
                sui::types::Address::from_static("0xf1"),
            ),
        ];
        let keys = entries
            .iter()
            .map(|(fqn, _)| ascii::String::from(fqn.to_string()))
            .collect::<Vec<_>>();
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_registry(
            &mut ledger_service,
            &mut state_service,
            &context,
            registry_inner(directory_id, &keys),
        );
        let key_type = crate::move_bindings::type_tag::<ascii::String>(&context);
        for (index, ((_, tool_id), key)) in entries.iter().zip(&keys).enumerate() {
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                &mut ledger_service,
                directory_id,
                &key_type,
                key.clone(),
                Node::<ascii::String, ID>::new(
                    MoveOption::from_option(index.checked_sub(1).map(|i| keys[i].clone())),
                    MoveOption::from_option(keys.get(index + 1).cloned()),
                    ID::new(*tool_id),
                ),
            );
        }
        let current_ref = sui_mocks::object_ref_for_id(entries[0].1);
        sui_mocks::grpc::mock_object_state::<ToolAnchor, ToolWitnessV1, ToolInnerV1>(
            &mut ledger_service,
            &mut state_service,
            &context,
            current_ref,
            sui::types::Owner::Shared(1),
            ToolAnchor::new(UID::new(entries[0].1)),
            tool_inner(registry_id, &entries[0].0),
        );
        sui_mocks::grpc::mock_get_object_not_found(&mut ledger_service, entries[1].1);
        sui_mocks::grpc::mock_package_versions(
            &mut ledger_service,
            &mut package_service,
            context.packages().all().cloned(),
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;

        let inspections = client.tool().list_tools().await.unwrap();
        let by_fqn = inspections
            .iter()
            .map(|inspection| (inspection.fqn.to_string(), inspection))
            .collect::<HashMap<_, _>>();

        assert_eq!(inspections.len(), 2);
        assert_eq!(
            by_fqn["xyz.taluslabs.current.tool@1"].compatibility,
            ToolCompatibility::Current
        );
        assert!(by_fqn["xyz.taluslabs.current.tool@1"].tool.is_some());
        assert_eq!(
            by_fqn["xyz.taluslabs.unavailable.tool@1"].compatibility,
            ToolCompatibility::Unavailable
        );
        assert!(by_fqn["xyz.taluslabs.unavailable.tool@1"].tool.is_none());
    }

    #[tokio::test]
    async fn access_inspection_derives_and_reads_canonical_accounts() {
        let context = sui_mocks::mock_nexus_context();
        let tool_fqn = "xyz.taluslabs.access@1"
            .parse::<ToolFqn>()
            .expect("Tool FQN parses");
        let tool_id =
            crate::move_bindings::derive_tool_id(context.tool_registry.object_id(), &tool_fqn)
                .expect("tool id derives");
        let cashier_origin = context
            .type_origin(PackageRole::Tool, "tool_cashier", "ToolCashierKey")
            .expect("cashier origin resolves");
        let cashier_id = crate::move_bindings::derive_tool_cashier_id(cashier_origin, tool_id)
            .expect("cashier id derives");
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0x31"));
        let credits_id = crate::move_bindings::derive_finite_credits_id(
            &context,
            cashier_id,
            beneficiary.clone(),
        )
        .expect("credit account id derives");
        let pass_id =
            crate::move_bindings::derive_time_pass_id(&context, cashier_id, beneficiary.clone())
                .expect("time pass id derives");
        let credits = FiniteCredits::new(
            UID::new(credits_id),
            ID::new(cashier_id),
            beneficiary.clone(),
            finite_credits::State::new(7),
        );
        let pass = TimePass::new(
            UID::new(pass_id),
            ID::new(cashier_id),
            beneficiary.clone(),
            time_pass::State::new(10, 90),
        );

        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut packages = sui_mocks::grpc::MockMovePackageService::new();
        let mut state = sui_mocks::grpc::MockStateService::new();
        mock_registry(
            &mut ledger,
            &mut state,
            &context,
            registry_inner(sui::types::Address::from_static("0xd1"), &[]),
        );
        sui_mocks::grpc::mock_nexus_package_graph(&mut ledger, &mut packages, context.packages());
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger,
            sui_mocks::object_ref_for_id(cashier_id),
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger,
            sui_mocks::object_ref_for_id(move_boundary::CLOCK_OBJECT_ID),
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&SuiClock::new(move_boundary::CLOCK_OBJECT_ID, 50))
                .expect("clock serializes"),
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger,
            sui_mocks::object_ref_for_id(credits_id),
            sui::types::Owner::Shared(4),
            bcs::to_bytes(&credits).expect("credits serialize"),
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger,
            sui_mocks::object_ref_for_id(pass_id),
            sui::types::Owner::Shared(5),
            bcs::to_bytes(&pass).expect("pass serializes"),
        );
        state
            .expect_list_owned_objects()
            .times(1)
            .return_once(move |request| {
                assert_eq!(
                    request.get_ref().owner.as_deref(),
                    Some(credits_id.to_string().as_str())
                );
                Ok(tonic::Response::new(
                    sui::grpc::ListOwnedObjectsResponse::default(),
                ))
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(packages),
            state_service_mock: Some(state),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;

        let access = client
            .tool()
            .inspect_access(&tool_fqn, beneficiary.clone())
            .await
            .expect("access inspection succeeds");

        assert_eq!(access.tool_id, tool_id);
        assert_eq!(access.cashier_id, cashier_id);
        assert_eq!(access.beneficiary, beneficiary);
        assert_eq!(access.observed_at_ms, 50);
        assert_eq!(
            access.finite_credits,
            Some(FiniteCreditAccess {
                account_id: credits_id,
                remaining: 7,
                refunded_invocations: vec![],
            })
        );
        assert_eq!(
            access.time_pass,
            Some(TimePassAccess {
                account_id: pass_id,
                valid_from_ms: 10,
                valid_until_ms: 90,
                active: true,
            })
        );
    }

    #[tokio::test]
    async fn unregister_inputs_do_not_decode_tool_state() {
        let context = sui_mocks::mock_nexus_context();
        let fqn = "xyz.taluslabs.legacy.tool@1".parse::<ToolFqn>().unwrap();
        let tool_id =
            crate::move_bindings::derive_tool_id(context.tool_registry.object_id(), &fqn).unwrap();
        let tool_ref = sui_mocks::object_ref_for_id(tool_id);
        let owner_cap_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0xc4"));
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_registry(
            &mut ledger_service,
            &mut state_service,
            &context,
            registry_inner(sui::types::Address::from_static("0xd1"), &[]),
        );
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            context.packages(),
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service,
            tool_ref.clone(),
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service,
            owner_cap_ref.clone(),
            sui::types::Owner::Address(sui::types::Address::from_static("0xa")),
            None,
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;

        let (selected, selected_tool, selected_cap) = client
            .tool()
            .unregister_inputs(&fqn, *owner_cap_ref.object_id())
            .await
            .unwrap();

        assert_eq!(selected_tool, tool_ref);
        assert_eq!(selected_cap, owner_cap_ref);
        assert_eq!(
            selected.packages().get(PackageRole::Tool),
            context.packages().get(PackageRole::Tool),
        );
    }
}
