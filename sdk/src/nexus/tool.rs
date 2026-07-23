//! Commands related to Nexus tool management.
//!
//! - [`ToolActions::update_timeout`] to update a tool's timeout.

use {
    crate::{
        move_bindings::{
            interface::verifier::ToolVerifierSupport,
            registry::verifier_registry::ExternalVerifierRecord,
        },
        nexus::{
            client::NexusClient,
            error::NexusError,
            registry::{
                fetch_current_tool_registration,
                fetch_external_verifier_record,
                preflight_external_verifier_registration,
            },
        },
        sui,
        transactions::tool,
        types::{Tool, ToolRef},
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

/// Result of [`ToolActions::inspect_tool`]. When `exists == false`, `tool` is
/// `None`, but `tool_id` and `tool_gas_id` are still populated from local
/// derivation so callers can pre-compute them. When `exists == true`, `tool`
/// carries the full on-chain `Tool` record (HTTP- or Sui-variant).
#[derive(Clone, Debug)]
pub struct ToolInspection {
    pub fqn: ToolFqn,
    pub tool_id: sui::types::Address,
    pub tool_gas_id: sui::types::Address,
    pub exists: bool,
    pub tool: Option<Tool>,
    pub verifier_support: Option<ToolVerifierSupport>,
    pub external_verifier: Option<ExternalVerifierRecord>,
}

pub struct ToolActions {
    pub(super) client: NexusClient,
}

impl ToolActions {
    async fn resolve_tool_and_owner_cap(
        &self,
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
        let objects = &self.client.nexus_objects;
        let tool_id = Tool::derive_id(*objects.tool_registry.object_id(), tool_fqn)
            .map_err(NexusError::Parsing)?;
        let tool = self
            .client
            .crawler()
            .get_object_metadata(tool_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let owner_cap = self
            .client
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
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        let owner_cap = self
            .client
            .crawler()
            .get_object_metadata(owner_cap)
            .await
            .map_err(NexusError::Rpc)?;

        // Derive and fetch the Tool object.
        let tool_ref = self.client.fetch_tool_gas(tool_fqn).await?;

        let tx = tool::update_tool_timeout_ptb(
            nexus_objects,
            &tool_ref,
            &owner_cap.object_ref(),
            new_timeout,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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
        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;
        let (tool_id, tool, owner_cap) =
            self.resolve_tool_and_owner_cap(tool_fqn, owner_cap).await?;
        let binding_id = self.client.network_auth().binding_object_id(
            &crate::move_bindings::registry::network_auth::IdentityKey::tool(tool_id),
        )?;
        let tool_key_binding = self
            .client
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
        let response = self.client.submit_transaction(tx, address).await?;
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
        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;
        let (tool_id, tool, owner_cap) =
            self.resolve_tool_and_owner_cap(tool_fqn, owner_cap).await?;
        let registration = preflight_external_verifier_registration(
            self.client.crawler(),
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
        let response = self.client.submit_transaction(tx, address).await?;
        Ok(ConfigureToolVerifierResult {
            tx_digest: response.digest,
            tool_id,
        })
    }

    /// Derive the Tool and ToolGas object IDs for `fqn` and probe the Tool
    /// object. Returns `exists: false` when neither object is present yet,
    /// and the full on-chain `Tool` record when both exist. The same shape
    /// works for HTTP and Sui tools. Callers can inspect the generated
    /// `Tool::r#ref` field or use [`ToolRef`] helper
    /// methods for ergonomic projections.
    ///
    /// Returns [`NexusError::Configuration`] when only one of Tool/ToolGas
    /// exists — that combination indicates corrupt registry state and
    /// requires operator intervention (e.g. a localnet reset).
    pub async fn inspect_tool(&self, fqn: &ToolFqn) -> Result<ToolInspection, NexusError> {
        let crawler = self.client.crawler();
        let nexus_objects = &self.client.nexus_objects;
        let tool_registry_id = *nexus_objects.tool_registry.object_id();
        let gas_service_id = *nexus_objects.gas_service.object_id();

        let tool_id = crate::move_bindings::derive_tool_id(tool_registry_id, fqn)
            .map_err(NexusError::Parsing)?;
        let tool_gas_id = crate::move_bindings::derive_tool_gas_id(gas_service_id, fqn)
            .map_err(NexusError::Parsing)?;

        let tool_exists = crawler.get_object_metadata(tool_id).await.is_ok();
        let tool_gas_exists = crawler.get_object_metadata(tool_gas_id).await.is_ok();

        if tool_exists ^ tool_gas_exists {
            return Err(NexusError::Configuration(format!(
                "Tool '{fqn}' has inconsistent state: Tool exists={tool_exists}, \
                 ToolGas exists={tool_gas_exists}. Reset the deployment or recreate the missing \
                 object before retrying."
            )));
        }

        if !tool_exists {
            return Ok(ToolInspection {
                fqn: fqn.clone(),
                tool_id,
                tool_gas_id,
                exists: false,
                tool: None,
                verifier_support: None,
                external_verifier: None,
            });
        }

        let tool = crawler
            .get_object::<Tool>(tool_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let (verifier_support, external_verifier) = match &tool.r#ref {
            ToolRef::Http { .. } => {
                let support =
                    fetch_current_tool_registration(crawler, &nexus_objects.tool_registry, tool_id)
                        .await
                        .map_err(NexusError::Rpc)?
                        .and_then(|registration| registration.verifier_support);
                let record = fetch_external_verifier_record(
                    crawler,
                    &nexus_objects.verifier_registry,
                    tool_id,
                )
                .await
                .map_err(NexusError::Rpc)?;
                (support, record)
            }
            ToolRef::Sui { .. } => (None, None),
        };
        match (&verifier_support, &external_verifier) {
            (Some(ToolVerifierSupport::External { method_id }), Some(record))
                if method_id == &record.method_id => {}
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
            tool_gas_id,
            exists: true,
            tool: Some(tool),
            verifier_support,
            external_verifier,
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
                registry::{
                    tool_registry::ToolRegistry,
                    verifier_registry::{ExternalVerifierRecord, VerifierRegistry},
                },
                sui_framework::{self, linked_table::LinkedTable, table::Table},
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
        tool_gas_id: sui::types::Address,
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
            let tool_gas_id = crate::move_bindings::derive_tool_gas_id(
                *nexus_objects.gas_service.object_id(),
                &fqn,
            )
            .expect("tool gas id derives");
            Self {
                nexus_objects,
                fqn,
                tool_id,
                tool_gas_id,
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
            _variant_name: ascii("Sui"),
            package_address,
            module_name: ascii(module_name.as_str()),
            tool_witness_id: crate::move_bindings::sui_framework::object::ID::new(tool_witness_id),
        }
    }

    fn fixture_tool(
        fixture: &InspectionFixture,
        reference: ToolRef,
        workflow_authorization_cap_first: bool,
    ) -> Tool {
        Tool {
            id: crate::move_bindings::sui_framework::object::UID::new(fixture.tool_id),
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

    fn mock_empty_verifier_state(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        fixture: &InspectionFixture,
    ) {
        use crate::move_bindings::{
            interface::verifier::ToolVerifierSupport,
            sui_framework::object::ID,
        };

        let id = sui::types::Address::from_static;
        let tool_registry = ToolRegistry::new(
            sui_framework::object::UID::new(*fixture.nexus_objects.tool_registry.object_id()),
            LinkedTable::<ascii::String, ID>::new(id("0x101"), 0),
            Table::<ID, bool>::new(id("0x102"), 0),
            LinkedTable::<ascii::String, u64>::new(id("0x103"), 0),
            Table::<ID, ToolVerifierSupport>::new(id("0x104"), 0),
            LinkedTable::<ascii::String, ID>::new(id("0x105"), 0),
            LinkedTable::<ascii::String, bool>::new(id("0x106"), 0),
            0,
            0,
        );
        let verifier_registry = VerifierRegistry::new(
            sui_framework::object::UID::new(*fixture.nexus_objects.verifier_registry.object_id()),
            sui_framework::object::UID::new(id("0x107")),
            Table::<ID, ExternalVerifierRecord>::new(id("0x108"), 0),
        );
        sui_mocks::grpc::mock_get_object_bcs(
            ledger_service,
            fixture.nexus_objects.tool_registry.clone(),
            sui::types::Owner::Shared(fixture.nexus_objects.tool_registry.version()),
            bcs::to_bytes(&tool_registry).unwrap(),
        );
        sui_mocks::grpc::mock_get_object_bcs(
            ledger_service,
            fixture.nexus_objects.verifier_registry.clone(),
            sui::types::Owner::Shared(fixture.nexus_objects.verifier_registry.version()),
            bcs::to_bytes(&verifier_registry).unwrap(),
        );
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
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        mock_get_object_not_found(&mut ledger_service_mock);
        mock_get_object_not_found(&mut ledger_service_mock);

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let inspection = client
            .tool()
            .inspect_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds when both objects missing");

        assert!(!inspection.exists);
        assert_eq!(inspection.tool_id, fixture.tool_id);
        assert_eq!(inspection.tool_gas_id, fixture.tool_gas_id);
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
        // Second probe (ToolGas) fails -> inconsistent.
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
        let tool_gas_ref = sui::types::ObjectReference::new(
            fixture.tool_gas_id,
            7,
            sui::types::Digest::from([4u8; 32]),
        );
        let tool = fixture_tool(
            &fixture,
            sui_tool_ref(package_address, module_name.clone(), tool_witness_id),
            true,
        );
        let tool_bcs = bcs::to_bytes(&tool).expect("Tool serializes to BCS");

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
            tool_gas_ref,
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            tool_bcs,
        );
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
        assert_eq!(inspection.tool_gas_id, fixture.tool_gas_id);
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
    async fn inspect_tool_rejects_inconsistent_state_when_only_tool_gas_present() {
        let fixture = InspectionFixture::new();
        let tool_gas_ref = sui::types::ObjectReference::new(
            fixture.tool_gas_id,
            5,
            sui::types::Digest::from([2u8; 32]),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        // First probe (Tool) fails.
        mock_get_object_not_found(&mut ledger_service_mock);
        // Second probe (ToolGas) succeeds -> the XOR triggers the other branch.
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            tool_gas_ref,
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
                && error_string.contains("ToolGas exists=true"),
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
        let tool_gas_ref = sui::types::ObjectReference::new(
            fixture.tool_gas_id,
            11,
            sui::types::Digest::from([8u8; 32]),
        );
        let http_tool = fixture_tool(
            &fixture,
            ToolRef::Http {
                _variant_name: ascii("Http"),
                url: b"https://example.com/tool".to_vec(),
            },
            false,
        );
        let tool_bcs = bcs::to_bytes(&http_tool).expect("Tool serializes to BCS");

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
            tool_gas_ref,
            sui::types::Owner::Shared(1),
            None,
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            tool_bcs,
        );
        mock_empty_verifier_state(&mut ledger_service_mock, &fixture);

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

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
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

        assert_eq!(result.tx_digest, tx_digest);
    }
}
