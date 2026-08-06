//! Commands related to Nexus tool management.
//!
//! - [`ToolActions::update_timeout`] to update a tool's timeout.

use {
    crate::{
        move_bindings::{
            interface::verifier::ToolVerifierSupport,
            tool::external_verifier::ExternalVerifier,
        },
        nexus::{
            client::NexusClient,
            error::NexusError,
            registry::{
                fetch_current_tool_registration,
                fetch_external_verifier_record,
                fetch_tool_invocation_cost,
                preflight_external_verifier_registration,
            },
        },
        sui,
        transactions::{tool, tool_payment},
        types::{Tool, ToolAnchor, ToolRef, ToolStateV1},
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

/// Result of a [`Tool`] payment configuration or ticket transaction.
pub struct ToolPaymentActionResult {
    pub tx_digest: sui::types::Digest,
}

/// Result of [`ToolActions::inspect_tool`].
///
/// The object IDs are derived locally even when the Tool does not exist.
/// An existing Tool includes its complete [`ToolStateV1`] record.
#[derive(Clone, Debug)]
pub struct ToolInspection {
    pub fqn: ToolFqn,
    pub tool_id: sui::types::Address,
    pub tool_payment_id: sui::types::Address,
    pub exists: bool,
    pub tool: Option<ToolStateV1>,
    pub verifier_support: Option<ToolVerifierSupport>,
    pub external_verifier: Option<ExternalVerifier>,
    pub invocation_cost_mist: Option<u64>,
}

pub struct ToolActions {
    pub(super) client: NexusClient,
}

impl ToolActions {
    async fn resolve_tool_payment_and_cap(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
    ) -> Result<(sui::types::ObjectReference, sui::types::ObjectReference), NexusError> {
        let tool_payment = client.fetch_tool_payment(tool_fqn).await?;
        let payment_admin = client
            .crawler()
            .get_object_metadata(payment_admin)
            .await
            .map_err(|error| {
                NexusError::Configuration(format!(
                    "Tool '{tool_fqn}' payment admin capability '{payment_admin}' could not be resolved: {error}"
                ))
            })?
            .object_ref();
        Ok((tool_payment, payment_admin))
    }

    async fn resolve_tool_and_payment_admin(
        client: &NexusClient,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
    ) -> Result<(sui::types::ObjectReference, sui::types::ObjectReference), NexusError> {
        let tool = client.fetch_tool(tool_fqn).await?;
        let payment_admin = client
            .crawler()
            .get_object_metadata(payment_admin)
            .await
            .map_err(|error| {
                NexusError::Configuration(format!(
                    "Tool '{tool_fqn}' payment admin capability '{payment_admin}' could not be resolved: {error}"
                ))
            })?
            .object_ref();
        Ok((tool, payment_admin))
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

    /// Enable expiry payment tickets for a [`Tool`].
    pub async fn enable_expiry_tickets(
        &self,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
        cost_per_minute: u64,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_payment, payment_admin) =
            Self::resolve_tool_payment_and_cap(&client, tool_fqn, payment_admin).await?;
        let transaction = tool_payment::enable_expiry_ptb(
            &client.nexus_objects,
            &tool_payment,
            &payment_admin,
            cost_per_minute,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Set the invocation price for a [`Tool`] in MIST.
    pub async fn set_invocation_cost(
        &self,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
        invocation_cost_mist: u64,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool, payment_admin) =
            Self::resolve_tool_and_payment_admin(&client, tool_fqn, payment_admin).await?;
        let transaction = tool::set_invocation_cost_ptb(
            &client.nexus_objects,
            &tool,
            &payment_admin,
            invocation_cost_mist,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Disable expiry payment tickets for a [`Tool`].
    pub async fn disable_expiry_tickets(
        &self,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_payment, payment_admin) =
            Self::resolve_tool_payment_and_cap(&client, tool_fqn, payment_admin).await?;
        let transaction =
            tool_payment::disable_expiry_ptb(&client.nexus_objects, &tool_payment, &payment_admin)
                .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Buy an expiry payment ticket for a [`Tool`].
    pub async fn buy_expiry_ticket(
        &self,
        tool_fqn: &ToolFqn,
        minutes: u64,
        pay_with: sui::types::Address,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        if minutes == 0 {
            return Err(NexusError::Configuration(
                "Ticket duration must be at least one minute".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let tool_payment = client.fetch_tool_payment(tool_fqn).await?;
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
        let transaction = tool_payment::buy_expiry_payment_ticket_ptb(
            &client.nexus_objects,
            &tool_payment,
            &pay_with,
            minutes,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Enable limited invocation payment tickets for a [`Tool`].
    pub async fn enable_limited_invocation_tickets(
        &self,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
        cost_per_invocation: u64,
        min_invocations: u64,
        max_invocations: u64,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        if min_invocations == 0 {
            return Err(NexusError::Configuration(
                "Minimum invocations must be at least one".to_owned(),
            ));
        }
        if min_invocations > max_invocations {
            return Err(NexusError::Configuration(format!(
                "Minimum invocations '{min_invocations}' cannot exceed maximum invocations '{max_invocations}'"
            )));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_payment, payment_admin) =
            Self::resolve_tool_payment_and_cap(&client, tool_fqn, payment_admin).await?;
        let transaction = tool_payment::enable_limited_invocations_ptb(
            &client.nexus_objects,
            &tool_payment,
            &payment_admin,
            cost_per_invocation,
            min_invocations,
            max_invocations,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Disable limited invocation payment tickets for a [`Tool`].
    pub async fn disable_limited_invocation_tickets(
        &self,
        tool_fqn: &ToolFqn,
        payment_admin: sui::types::Address,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let (tool_payment, payment_admin) =
            Self::resolve_tool_payment_and_cap(&client, tool_fqn, payment_admin).await?;
        let transaction = tool_payment::disable_limited_invocations_ptb(
            &client.nexus_objects,
            &tool_payment,
            &payment_admin,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Buy a limited invocation payment ticket for a [`Tool`].
    pub async fn buy_limited_invocation_ticket(
        &self,
        tool_fqn: &ToolFqn,
        invocations: u64,
        pay_with: sui::types::Address,
    ) -> Result<ToolPaymentActionResult, NexusError> {
        if invocations == 0 {
            return Err(NexusError::Configuration(
                "Ticket invocations must be at least one".to_owned(),
            ));
        }
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let tool_payment = client.fetch_tool_payment(tool_fqn).await?;
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
        let transaction = tool_payment::buy_limited_invocations_payment_ticket_ptb(
            &client.nexus_objects,
            &tool_payment,
            &pay_with,
            invocations,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        Ok(ToolPaymentActionResult {
            tx_digest: response.digest,
        })
    }

    /// Derive the Tool and ToolPayment object IDs for `fqn` and probe the Tool
    /// object. Returns `exists: false` when neither object is present yet,
    /// and the full onchain `Tool` record when both exist. The same shape
    /// works for HTTP and Sui tools. Callers can inspect the generated
    /// `Tool::r#ref` field or use [`ToolRef`] helper
    /// methods for ergonomic projections.
    ///
    /// Returns [`NexusError::Configuration`] when only one of Tool/ToolPayment
    /// exists — that combination indicates corrupt registry state and
    /// requires operator intervention (e.g. a localnet reset).
    pub async fn inspect_tool(&self, fqn: &ToolFqn) -> Result<ToolInspection, NexusError> {
        let client = self.client.operation_client().await?;
        let crawler = client.crawler();
        let nexus_objects = &client.nexus_objects;
        let tool_registry_id = *nexus_objects.tool_registry.object_id();

        let tool_id = crate::move_bindings::derive_tool_id(tool_registry_id, fqn)
            .map_err(NexusError::Parsing)?;
        let tool_payment_id = crate::move_bindings::derive_tool_payment_id(
            nexus_objects.tool_type_origin_pkg_id(),
            tool_id,
        )
        .map_err(NexusError::Parsing)?;

        let tool_exists = crawler.get_object_metadata(tool_id).await.is_ok();
        let tool_payment_exists = crawler.get_object_metadata(tool_payment_id).await.is_ok();

        if tool_exists ^ tool_payment_exists {
            return Err(NexusError::Configuration(format!(
                "Tool '{fqn}' has inconsistent state: Tool exists={tool_exists}, \
                 ToolPayment exists={tool_payment_exists}. Reset the deployment or recreate the missing \
                 object before retrying."
            )));
        }

        if !tool_exists {
            return Ok(ToolInspection {
                fqn: fqn.clone(),
                tool_id,
                tool_payment_id,
                exists: false,
                tool: None,
                verifier_support: None,
                external_verifier: None,
                invocation_cost_mist: None,
            });
        }

        let tool = crawler
            .get_versioned_object::<ToolAnchor, ToolStateV1>(tool_id, 1)
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
            tool_payment_id,
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
                sui_framework::{
                    self,
                    linked_table::LinkedTable,
                    object::{ID, UID},
                    table::Table,
                    versioned::Versioned,
                },
                tool::{
                    external_verifier::ExternalVerifier,
                    tool_registry::{ToolRegistry, ToolRegistryStateV1},
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
        tool_payment_id: sui::types::Address,
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
            let tool_payment_id = crate::move_bindings::derive_tool_payment_id(
                nexus_objects.tool_type_origin_pkg_id(),
                tool_id,
            )
            .expect("tool payment id derives");
            Self {
                nexus_objects,
                fqn,
                tool_id,
                tool_payment_id,
            }
        }
    }

    fn ascii(value: &str) -> ascii::String {
        ascii::String::from(value)
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
    ) -> ToolStateV1 {
        ToolStateV1 {
            minimum_protocol_version: 1,
            registry: crate::move_bindings::sui_framework::object::ID::new(
                *fixture.nexus_objects.tool_registry.object_id(),
            ),
            fqn: ascii(&fixture.fqn.to_string()),
            r#ref: reference,
            description: b"demo".to_vec(),
            input_schema: b"{}".to_vec(),
            output_schema: b"{}".to_vec(),
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
            let tool_registry_state = ToolRegistryStateV1::new(
                ID::new(sui::types::Address::ZERO),
                1,
                LinkedTable::<ascii::String, ID>::new(id("0x101"), 0),
                Table::<ID, bool>::new(id("0x102"), 0),
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
        assert_eq!(inspection.tool_payment_id, fixture.tool_payment_id);
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
        // Second probe (ToolPayment) fails -> inconsistent.
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
        let tool_payment_ref = sui::types::ObjectReference::new(
            fixture.tool_payment_id,
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
            tool_payment_ref,
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
        assert_eq!(inspection.tool_payment_id, fixture.tool_payment_id);
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
    async fn inspect_tool_rejects_inconsistent_state_when_only_tool_payment_present() {
        let fixture = InspectionFixture::new();
        let tool_payment_ref = sui::types::ObjectReference::new(
            fixture.tool_payment_id,
            5,
            sui::types::Digest::from([2u8; 32]),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        // First probe (Tool) fails.
        mock_get_object_not_found(&mut ledger_service_mock);
        // Second probe (ToolPayment) succeeds -> the XOR triggers the other branch.
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_payment_ref,
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
                && error_string.contains("ToolPayment exists=true"),
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
        let tool_payment_ref = sui::types::ObjectReference::new(
            fixture.tool_payment_id,
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
            tool_payment_ref,
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
}
