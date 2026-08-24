//! Tool inspection, owner configuration, and cashier operations.

use {
    crate::{
        move_bindings::{
            interface::verifier::ToolVerifierSupport,
            move_std::ascii,
            registry::network_auth::IdentityKey,
            sui_framework::{linked_table::Node, object::ID},
            tool::{
                external_verifier::ExternalVerifier,
                tool_registry::{
                    Tool as ToolAnchor,
                    ToolInnerV1,
                    ToolRegistry,
                    ToolRegistryInnerV1,
                },
            },
        },
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

/// Stable identity, state, and Registry projections for one Tool.
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

impl ToolActions {
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

    /// Drains settled SUI from a Tool cashier to the transaction sender.
    pub async fn drain_cashier(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let cashier = self.client.fetch_tool_cashier(fqn).await?;
        let context = self.client.context_for_object(*cashier.object_id()).await?;
        let owner_cap = self.client.object_reference(owner_cap).await?;
        let recipient = self.client.owner()?;
        let transaction =
            tool_cashier::drain_for_self_ptb(&context, &cashier, &owner_cap, recipient)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Enables expiry tickets for a Tool cashier.
    pub async fn enable_expiry_tickets(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        cost_per_minute: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let (context, cashier, cashier_admin) = self.cashier_inputs(fqn, cashier_admin).await?;
        let transaction =
            tool_cashier::enable_expiry_ptb(&context, &cashier, &cashier_admin, cost_per_minute)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Disables expiry tickets for a Tool cashier.
    pub async fn disable_expiry_tickets(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let (context, cashier, cashier_admin) = self.cashier_inputs(fqn, cashier_admin).await?;
        let transaction = tool_cashier::disable_expiry_ptb(&context, &cashier, &cashier_admin)
            .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Buys one expiry ticket from a Tool cashier.
    pub async fn buy_expiry_ticket(
        &self,
        fqn: &ToolFqn,
        minutes: u64,
        pay_with: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if minutes == 0 {
            return Err(NexusError::Configuration(
                "ticket duration must be at least one minute".to_owned(),
            ));
        }
        let cashier = self.client.fetch_tool_cashier(fqn).await?;
        let context = self.client.context_for_object(*cashier.object_id()).await?;
        let pay_with = self.client.object_reference(pay_with).await?;
        let transaction =
            tool_cashier::buy_expiry_payment_ticket_ptb(&context, &cashier, &pay_with, minutes)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Enables limited invocation tickets for a Tool cashier.
    pub async fn enable_limited_invocation_tickets(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
        cost_per_invocation: u64,
        min_invocations: u64,
        max_invocations: u64,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if min_invocations == 0 || min_invocations > max_invocations {
            return Err(NexusError::Configuration(
                "invocation ticket limits are invalid".to_owned(),
            ));
        }
        let (context, cashier, cashier_admin) = self.cashier_inputs(fqn, cashier_admin).await?;
        let transaction = tool_cashier::enable_limited_invocations_ptb(
            &context,
            &cashier,
            &cashier_admin,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        )
        .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Disables limited invocation tickets for a Tool cashier.
    pub async fn disable_limited_invocation_tickets(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let (context, cashier, cashier_admin) = self.cashier_inputs(fqn, cashier_admin).await?;
        let transaction =
            tool_cashier::disable_limited_invocations_ptb(&context, &cashier, &cashier_admin)
                .map_err(NexusError::TransactionBuilding)?;
        self.submit_action(transaction).await
    }

    /// Buys one limited invocation ticket from a Tool cashier.
    pub async fn buy_limited_invocation_ticket(
        &self,
        fqn: &ToolFqn,
        invocations: u64,
        pay_with: sui::types::Address,
    ) -> Result<ToolCashierActionResult, NexusError> {
        if invocations == 0 {
            return Err(NexusError::Configuration(
                "ticket invocations must be at least one".to_owned(),
            ));
        }
        let cashier = self.client.fetch_tool_cashier(fqn).await?;
        let context = self.client.context_for_object(*cashier.object_id()).await?;
        let pay_with = self.client.object_reference(pay_with).await?;
        let transaction = tool_cashier::buy_limited_invocations_payment_ticket_ptb(
            &context,
            &cashier,
            &pay_with,
            invocations,
        )
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

    async fn cashier_inputs(
        &self,
        fqn: &ToolFqn,
        cashier_admin: sui::types::Address,
    ) -> Result<
        (
            Arc<NexusContext>,
            sui::types::ObjectReference,
            sui::types::ObjectReference,
        ),
        NexusError,
    > {
        let cashier = self.client.fetch_tool_cashier(fqn).await?;
        let context = self.client.context_for_object(*cashier.object_id()).await?;
        let cashier_admin = self.client.object_reference(cashier_admin).await?;
        Ok((context, cashier, cashier_admin))
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
