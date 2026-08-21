//! Tool inspection, lifecycle, and cashier operations.

#[cfg(test)]
use crate::move_bindings::tool::era::V1 as ToolWitnessV1;
use {
    crate::{
        move_bindings::{
            move_std::ascii,
            sui_framework::{linked_table::Node, object::ID},
            tool::tool_registry::{
                Tool as ToolAnchor,
                ToolDefinition,
                ToolInnerV1,
                ToolLifecycle,
                ToolRegistry,
                ToolRegistryInnerV1,
            },
        },
        nexus::{client::NexusClient, error::NexusError},
        sui,
        transactions::{tool, tool_cashier},
        types::{NexusContext, PackageRole, ToolState},
        ToolFqn,
    },
    std::{collections::HashSet, sync::Arc},
};

/// Compatibility of one Tool with this SDK and the current Registry authority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ToolCompatibility {
    /// The Tool uses the package accepted by the current [`ToolRegistry`].
    Current,
    /// The SDK understands the Tool and its terminal cleanup remains valid.
    LegacyUnderstood,
    /// The SDK understands the Tool, but it must migrate before governed use.
    MigrationRequired,
    /// The witness and inner pair has no adapter in this SDK.
    Unsupported,
    /// The Tool could not be observed or decoded independently.
    Unavailable,
}

/// Stable identity and compatibility details for one Tool inventory item.
#[derive(Clone, Debug)]
pub struct ToolInspection {
    /// Permanent fully qualified name recorded by the [`ToolRegistry`].
    pub fqn: ToolFqn,
    /// Stable [`ToolAnchor`] object ID.
    pub tool_id: sui::types::Address,
    /// Stable Tool cashier object ID derived from the selected Tool package.
    pub tool_cashier_id: sui::types::Address,
    /// Current owner when the Tool anchor was observed.
    pub owner: Option<sui::types::Owner>,
    /// Exact witness type observed below the Tool anchor.
    pub witness_type: Option<sui::types::StructTag>,
    /// Exact inner type observed below the Tool anchor.
    pub inner_type: Option<sui::types::StructTag>,
    /// Compatibility classification isolated to this Tool.
    pub compatibility: ToolCompatibility,
    /// Current lifecycle when the inner layout is understood.
    pub lifecycle: Option<ToolLifecycle>,
    /// Registry controlled endorsement when the current Registry value is available.
    pub endorsed: Option<bool>,
    /// Immutable definition retained by the current [`ToolRegistry`].
    pub definition: Option<ToolDefinition>,
    /// Complete supported view when both definition and inner layout decode.
    pub tool: Option<ToolState>,
    /// Diagnostic detail for unsupported or unavailable items.
    pub detail: Option<String>,
}

/// Result of a Tool lifecycle or cashier transaction.
pub struct ToolActionResult {
    /// Digest of the submitted transaction.
    pub tx_digest: sui::types::Digest,
}

/// Compatibility preserving alias for cashier callers.
pub type ToolCashierActionResult = ToolActionResult;

/// Operations over Tool definitions, lifecycle, and payment state.
pub struct ToolActions {
    pub(super) client: NexusClient,
}

impl ToolActions {
    /// Lists every permanent Tool directory entry.
    ///
    /// Failure to decode one Tool produces an isolated inventory status and
    /// does not discard other entries. Failure to read the canonical Registry
    /// itself still fails the request because no authoritative directory is
    /// then available.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when Registry state or its permanent directory
    /// cannot be read.
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

    /// Inspects one Tool without allowing its compatibility to affect other
    /// inventory entries.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] only when the canonical [`ToolRegistry`] cannot
    /// be resolved or decoded.
    pub async fn inspect_tool(&self, fqn: &ToolFqn) -> Result<ToolInspection, NexusError> {
        let (context, registry) = self.registry_state().await?;
        let tool_id = crate::move_bindings::derive_tool_id(context.tool_registry.object_id(), fqn)
            .map_err(NexusError::Parsing)?;
        Ok(self
            .inspect_tool_id(&context, &registry, fqn.clone(), tool_id)
            .await)
    }

    /// Closes a current Tool so it accepts no new invocations.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when authority resolution, transaction building,
    /// or submission fails.
    pub async fn close(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool, owner_cap) = self.current_tool_inputs(fqn, owner_cap, true).await?;
        let transaction = tool::close_ptb(&context, &tool, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;
        let response = self
            .client
            .submit_transaction(transaction, self.client.owner()?)
            .await?;
        Ok(ToolActionResult {
            tx_digest: response.digest,
        })
    }

    /// Claims unlocked US collateral from a closed Tool.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when authority resolution, transaction building,
    /// or submission fails.
    pub async fn claim_collateral(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool, owner_cap) = self.current_tool_inputs(fqn, owner_cap, false).await?;
        let transaction = tool::claim_collateral_for_self_ptb(&context, &tool, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;
        let response = self
            .client
            .submit_transaction(transaction, self.client.owner()?)
            .await?;
        Ok(ToolActionResult {
            tx_digest: response.digest,
        })
    }

    /// Drains settled SUI from a Tool cashier to the transaction sender.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when authority resolution, transaction building,
    /// or submission fails.
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
        let response = self
            .client
            .submit_transaction(transaction, recipient)
            .await?;
        Ok(ToolActionResult {
            tx_digest: response.digest,
        })
    }

    /// Retires a closed Tool after its collateral, cashier, and tickets drain.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError`] when authority resolution, transaction building,
    /// or submission fails.
    pub async fn retire(
        &self,
        fqn: &ToolFqn,
        owner_cap: sui::types::Address,
    ) -> Result<ToolActionResult, NexusError> {
        let (context, tool, owner_cap) = self.current_tool_inputs(fqn, owner_cap, true).await?;
        let cashier = self.client.fetch_tool_cashier(fqn).await?;
        let transaction = tool::retire_ptb(&context, &tool, &cashier, &owner_cap)
            .map_err(NexusError::TransactionBuilding)?;
        let response = self
            .client
            .submit_transaction(transaction, self.client.owner()?)
            .await?;
        Ok(ToolActionResult {
            tx_digest: response.digest,
        })
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
        self.submit_cashier(transaction).await
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
        self.submit_cashier(transaction).await
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
        self.submit_cashier(transaction).await
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
        self.submit_cashier(transaction).await
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
        self.submit_cashier(transaction).await
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
        self.submit_cashier(transaction).await
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
            owner: None,
            witness_type: None,
            inner_type: None,
            compatibility: ToolCompatibility::Unavailable,
            lifecycle: None,
            endorsed: None,
            definition: None,
            tool: None,
            detail: None,
        };

        let id_type = crate::move_bindings::type_tag::<ID>(registry_context);
        match self
            .client
            .crawler()
            .get_dynamic_field_by_key::<ID, ToolDefinition>(
                registry.definitions.id(),
                ID::new(tool_id),
                &id_type,
            )
            .await
        {
            Ok(Some(definition)) => inspection.definition = Some(definition),
            Ok(None) => {
                inspection.detail = Some("Tool definition is missing".to_owned());
                return inspection;
            }
            Err(error) => {
                inspection.detail = Some(format!("Tool definition is unavailable: {error}"));
                return inspection;
            }
        }
        match self
            .client
            .crawler()
            .get_dynamic_field_by_key::<ID, bool>(
                registry.endorsements.id(),
                ID::new(tool_id),
                &id_type,
            )
            .await
        {
            Ok(endorsed) => inspection.endorsed = endorsed,
            Err(error) => {
                inspection.detail = Some(format!("Tool endorsement is unavailable: {error}"));
            }
        }

        let observed = match self.client.state_resolver().observe(tool_id).await {
            Ok(observed) => observed,
            Err(error) => {
                inspection.detail = Some(error.to_string());
                return inspection;
            }
        };
        inspection.owner = Some(observed.owner);
        inspection.witness_type = Some(observed.witness_type().clone());
        inspection.inner_type = Some(observed.inner_type().clone());

        let packages = match self
            .client
            .state_resolver()
            .resolve_package_graph(&observed)
            .await
        {
            Ok(packages) => packages,
            Err(NexusError::ClientUpgradeRequired(error)) => {
                inspection.compatibility = ToolCompatibility::Unsupported;
                inspection.detail = Some(error.to_string());
                return inspection;
            }
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
        inspection.lifecycle = Some(inner.lifecycle);

        let selected = context.packages().get(PackageRole::Tool);
        let is_current = current_package
            .zip(selected)
            .is_some_and(|(current, selected)| {
                current.storage_id == selected.storage_id && current.version == selected.version
            });
        inspection.compatibility = if is_current {
            ToolCompatibility::Current
        } else if matches!(inner.lifecycle, ToolLifecycle::Open) {
            ToolCompatibility::MigrationRequired
        } else {
            ToolCompatibility::LegacyUnderstood
        };
        inspection.tool = inspection
            .definition
            .clone()
            .map(|definition| ToolState::new(tool_id, definition, inner));
        inspection
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

    async fn submit_cashier(
        &self,
        transaction: sui::types::ProgrammableTransaction,
    ) -> Result<ToolCashierActionResult, NexusError> {
        let response = self
            .client
            .submit_transaction(transaction, self.client.owner()?)
            .await?;
        Ok(ToolCashierActionResult {
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
                    table::Table as MoveTable,
                },
                tool::tool_registry::{ToolRef, ToolVerifierContract},
            },
            test_utils::{nexus_mocks, sui_mocks},
            types::NexusPackages,
        },
        std::{collections::HashMap, sync::Arc},
    };

    fn tool_definition(fqn: &ToolFqn) -> ToolDefinition {
        ToolDefinition::new(
            ascii::String::from(fqn.to_string()),
            ToolRef::Http {
                url: b"https://example.com/tool".to_vec(),
            },
            b"Compatibility fixture".to_vec(),
            MetaSchema::new(vec![], vec![]),
            30_000,
            ToolVerifierContract::None,
            true,
            0,
        )
    }

    fn tool_inner(registry_id: sui::types::Address) -> ToolInnerV1 {
        ToolInnerV1::new(
            ID::new(registry_id),
            Balance {
                value: 0,
                phantom_t0: std::marker::PhantomData,
            },
            0,
            0,
            ToolLifecycle::Open,
        )
    }

    fn mock_unknown_tool_state(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        state_service: &mut sui_mocks::grpc::MockStateService,
        context: &NexusContext,
        tool_ref: sui::types::ObjectReference,
        package: sui::types::Address,
    ) {
        let object_id = *tool_ref.object_id();
        let anchor_type = sui::types::StructTag::new(
            package,
            sui::types::Identifier::from_static("tool_registry"),
            sui::types::Identifier::from_static("Tool"),
            vec![],
        );
        let witness_type = sui::types::StructTag::new(
            package,
            sui::types::Identifier::from_static("witness"),
            sui::types::Identifier::from_static("V2"),
            vec![],
        );
        let inner_type = sui::types::StructTag::new(
            package,
            sui::types::Identifier::from_static("tool_registry"),
            sui::types::Identifier::from_static("ToolInnerV2"),
            vec![],
        );
        let primitives = context
            .require_package(PackageRole::Primitives)
            .expect("test context contains Primitives");
        let state_key = |name: &'static str| {
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                primitives
                    .type_origin("object_state", name)
                    .expect("test context contains object state keys"),
                sui::types::Identifier::from_static("object_state"),
                sui::types::Identifier::from_static(name),
                vec![],
            )))
        };
        let dynamic_field_type = |key: sui::types::TypeTag, value: sui::types::StructTag| {
            sui::types::StructTag::new(
                sui::types::Address::from_static("0x2"),
                sui::types::Identifier::from_static("dynamic_field"),
                sui::types::Identifier::from_static("Field"),
                vec![key, sui::types::TypeTag::Struct(Box::new(value))],
            )
        };
        let witness_key = state_key("Witness");
        let inner_key = state_key("Inner");
        let witness_field_type = dynamic_field_type(witness_key, witness_type.clone());
        let inner_field_type = dynamic_field_type(inner_key, inner_type.clone());

        let expected_id = object_id.to_string();
        let anchor = ToolAnchor::new(UID::new(object_id));
        let anchor_contents = bcs::to_bytes(&anchor).expect("Tool anchor serializes");
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request.get_ref().object_id.as_deref() == Some(expected_id.as_str())
            })
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut object = sui::grpc::Object::default();
                object.set_object_id(object_id);
                object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Shared(1)));
                object.set_version(tool_ref.version());
                object.set_digest(*tool_ref.digest());
                object.set_object_type(anchor_type.to_string());
                let mut contents = sui::grpc::Bcs::default();
                contents.set_name(anchor_type.to_string());
                contents.set_value(anchor_contents.clone());
                object.set_contents(contents);
                response.set_object(object);
                Ok(tonic::Response::new(response))
            });

        let expected_parent = object_id.to_string();
        state_service
            .expect_list_dynamic_fields()
            .withf(move |request| request.get_ref().parent_opt() == Some(expected_parent.as_str()))
            .times(1)
            .returning(move |_request| {
                let mut witness = sui::grpc::DynamicField::default();
                witness.set_field_id(sui_mocks::mock_sui_address());
                witness.set_value_type(witness_type.to_string());
                let mut witness_object = sui::grpc::Object::default();
                witness_object.set_object_type(witness_field_type.to_string());
                witness.set_field_object(witness_object);

                let mut inner = sui::grpc::DynamicField::default();
                inner.set_field_id(sui_mocks::mock_sui_address());
                inner.set_value_type(inner_type.to_string());
                let mut inner_object = sui::grpc::Object::default();
                inner_object.set_object_type(inner_field_type.to_string());
                inner.set_field_object(inner_object);

                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                response.set_dynamic_fields(vec![witness, inner]);
                Ok(tonic::Response::new(response))
            });
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
    async fn mixed_tool_versions_are_classified_without_aborting_the_inventory() {
        let context = sui_mocks::mock_nexus_context();
        let registry_id = context.tool_registry.object_id();
        let registry_ref = sui_mocks::object_ref_for_id(registry_id);
        let directory_id = sui::types::Address::from_static("0xd1");
        let definitions_id = sui::types::Address::from_static("0xd2");
        let endorsements_id = sui::types::Address::from_static("0xd4");
        let entries = [
            (
                "xyz.taluslabs.future.tool@1".parse::<ToolFqn>().unwrap(),
                sui::types::Address::from_static("0xf1"),
            ),
            (
                "xyz.taluslabs.current.tool@1".parse::<ToolFqn>().unwrap(),
                sui::types::Address::from_static("0xc1"),
            ),
            (
                "xyz.taluslabs.legacy.tool@1".parse::<ToolFqn>().unwrap(),
                sui::types::Address::from_static("0xb1"),
            ),
        ];
        let keys = entries
            .iter()
            .map(|(fqn, _)| ascii::String::from(fqn.to_string()))
            .collect::<Vec<_>>();
        let mut directory = LinkedTable::new(directory_id, entries.len() as u64);
        directory.head = MoveOption::from_option(keys.first().cloned());
        directory.tail = MoveOption::from_option(keys.last().cloned());
        let registry = ToolRegistryInnerV1::new(
            directory,
            MoveTable::new(
                sui::types::Address::from_static("0xd3"),
                entries.len() as u64,
            ),
            MoveTable::new(definitions_id, entries.len() as u64),
            MoveTable::new(endorsements_id, entries.len() as u64),
            0,
            0,
        );

        let mut legacy_tool = context
            .packages()
            .tool
            .clone()
            .expect("test context contains Tool");
        let legacy_package = sui::types::Address::from_static("0xb7");
        legacy_tool.initial_id = legacy_package;
        legacy_tool.storage_id = legacy_package;
        for origins in legacy_tool.type_origins.values_mut() {
            for origin in origins.values_mut() {
                *origin = legacy_package;
            }
        }
        let legacy_context = NexusContext::new(
            Arc::new(context.objects().clone()),
            NexusPackages {
                primitives: context.packages().primitives.clone(),
                interface: context.packages().interface.clone(),
                tool: Some(legacy_tool.clone()),
                ..Default::default()
            },
        );

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_object_state::<ToolRegistry, ToolWitnessV1, ToolRegistryInnerV1>(
            &mut ledger_service,
            &mut state_service,
            &context,
            registry_ref,
            sui::types::Owner::Shared(context.tool_registry.initial_shared_version),
            ToolRegistry::new(UID::new(registry_id)),
            registry,
        );

        let directory_key_type = crate::move_bindings::type_tag::<ascii::String>(&context);
        let definition_key_type = crate::move_bindings::type_tag::<ID>(&context);
        for (index, ((fqn, tool_id), key)) in entries.iter().zip(&keys).enumerate() {
            let previous = index.checked_sub(1).map(|index| keys[index].clone());
            let next = keys.get(index + 1).cloned();
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                &mut ledger_service,
                directory_id,
                &directory_key_type,
                key.clone(),
                Node::<ascii::String, ID>::new(
                    MoveOption::from_option(previous),
                    MoveOption::from_option(next),
                    ID::new(*tool_id),
                ),
            );
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                &mut ledger_service,
                definitions_id,
                &definition_key_type,
                ID::new(*tool_id),
                tool_definition(fqn),
            );
            sui_mocks::grpc::mock_get_dynamic_field_by_key(
                &mut ledger_service,
                endorsements_id,
                &definition_key_type,
                ID::new(*tool_id),
                index == 1,
            );
        }

        let current_ref = sui_mocks::object_ref_for_id(entries[1].1);
        sui_mocks::grpc::mock_object_state::<ToolAnchor, ToolWitnessV1, ToolInnerV1>(
            &mut ledger_service,
            &mut state_service,
            &context,
            current_ref,
            sui::types::Owner::Shared(1),
            ToolAnchor::new(UID::new(entries[1].1)),
            tool_inner(registry_id),
        );
        let legacy_ref = sui_mocks::object_ref_for_id(entries[2].1);
        sui_mocks::grpc::mock_object_state::<ToolAnchor, ToolWitnessV1, ToolInnerV1>(
            &mut ledger_service,
            &mut state_service,
            &legacy_context,
            legacy_ref,
            sui::types::Owner::Shared(1),
            ToolAnchor::new(UID::new(entries[2].1)),
            tool_inner(registry_id),
        );
        mock_unknown_tool_state(
            &mut ledger_service,
            &mut state_service,
            &context,
            sui_mocks::object_ref_for_id(entries[0].1),
            sui::types::Address::from_static("0xf7"),
        );

        let mut packages = context.packages().all().cloned().collect::<Vec<_>>();
        packages.push(legacy_tool);
        sui_mocks::grpc::mock_package_versions(&mut ledger_service, &mut package_service, packages);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&context, &rpc_url).await;

        let inspections = client.tool().list_tools().await.expect("inventory loads");
        let by_fqn = inspections
            .iter()
            .map(|inspection| (inspection.fqn.to_string(), inspection))
            .collect::<HashMap<_, _>>();

        assert_eq!(inspections.len(), 3);
        let future = by_fqn
            .get("xyz.taluslabs.future.tool@1")
            .expect("future Tool remains in inventory");
        assert_eq!(future.compatibility, ToolCompatibility::Unsupported);
        assert!(future.tool.is_none());
        let current = by_fqn
            .get("xyz.taluslabs.current.tool@1")
            .expect("current Tool remains in inventory");
        assert_eq!(current.compatibility, ToolCompatibility::Current);
        assert_eq!(current.endorsed, Some(true));
        assert!(current.tool.is_some());
        let legacy = by_fqn
            .get("xyz.taluslabs.legacy.tool@1")
            .expect("legacy Tool remains in inventory");
        assert_eq!(legacy.compatibility, ToolCompatibility::MigrationRequired);
        assert!(legacy.tool.is_some());
    }
}
