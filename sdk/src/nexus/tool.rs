//! Commands related to Nexus tool management.
//!
//! - [`ToolActions::update_timeout`] to update a tool's timeout.

use {
    crate::{
        idents::{primitives, workflow as workflow_idents},
        nexus::{client::NexusClient, error::NexusError},
        sui,
        transactions::tool,
        types::{derive_tool_gas_id, derive_tool_id, Tool, ToolRef},
        ToolFqn,
    },
    std::time::Duration,
};

pub struct UpdateToolTimeoutResult {
    pub tx_digest: sui::types::Digest,
}

/// Result of [`ToolActions::inspect_on_chain_tool`]. When `exists == false`,
/// `tool` and the decoded Sui-tool fields are `None`, but `tool_id` and
/// `tool_gas_id` are still populated from local derivation.
#[derive(Clone, Debug)]
pub struct OnChainToolInspection {
    pub fqn: ToolFqn,
    pub tool_id: sui::types::Address,
    pub tool_gas_id: sui::types::Address,
    pub exists: bool,
    pub tool: Option<Tool>,
    pub package_address: Option<sui::types::Address>,
    pub module_name: Option<sui::types::Identifier>,
    pub witness_id: Option<sui::types::Address>,
}

/// Inputs for [`ToolActions::register_on_chain_or_reuse`].
#[derive(Clone, Debug)]
pub struct RegisterOnChainToolParams {
    pub package_address: sui::types::Address,
    pub module: sui::types::Identifier,
    pub fqn: ToolFqn,
    pub description: String,
    pub input_schema: String,
    pub output_schema: String,
    pub timeout: Duration,
    pub witness_id: sui::types::Address,
    pub collateral_coin: sui::types::ObjectReference,
    pub workflow_authorization_cap_first: bool,
}

/// Result of [`ToolActions::register_on_chain_or_reuse`]. Owner-cap and tx
/// fields are populated only on a fresh registration; reused registrations
/// return decoded refs and `reused: true`.
#[derive(Clone, Debug)]
pub struct RegisterOnChainToolResult {
    pub fqn: ToolFqn,
    pub tool_id: sui::types::Address,
    pub tool_gas_id: sui::types::Address,
    pub package_address: sui::types::Address,
    pub module_name: sui::types::Identifier,
    pub witness_id: sui::types::Address,
    pub owner_cap_over_tool: Option<sui::types::Address>,
    pub owner_cap_over_gas: Option<sui::types::Address>,
    pub workflow_authorization_cap_first: bool,
    pub reused: bool,
    pub tx_digest: Option<sui::types::Digest>,
    pub tx_checkpoint: Option<u64>,
}

pub struct ToolActions {
    pub(super) client: NexusClient,
}

impl ToolActions {
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

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = tool::update_tool_timeout(
            &mut tx,
            nexus_objects,
            &tool_ref,
            &owner_cap.object_ref(),
            new_timeout,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(UpdateToolTimeoutResult {
            tx_digest: response.digest,
        })
    }

    /// Derive the Tool and ToolGas object IDs for `fqn` and probe the Tool
    /// object. Returns `exists: false` when neither object is present yet.
    /// When the Tool exists, its `ToolRef::Sui` fields are decoded into
    /// `package_address`, `module_name`, and `witness_id` for direct use.
    ///
    /// Returns [`NexusError::Configuration`] when only one of Tool/ToolGas
    /// exists — that combination indicates corrupt registry state and
    /// requires operator intervention (e.g. a localnet reset).
    pub async fn inspect_on_chain_tool(
        &self,
        fqn: &ToolFqn,
    ) -> Result<OnChainToolInspection, NexusError> {
        let crawler = self.client.crawler();
        let nexus_objects = &self.client.nexus_objects;
        let tool_registry_id = *nexus_objects.tool_registry.object_id();
        let gas_service_id = *nexus_objects.gas_service.object_id();

        let tool_id = derive_tool_id(tool_registry_id, fqn).map_err(NexusError::Parsing)?;
        let tool_gas_id = derive_tool_gas_id(gas_service_id, fqn).map_err(NexusError::Parsing)?;

        let tool_exists = crawler.get_object_metadata(tool_id).await.is_ok();
        let tool_gas_exists = crawler.get_object_metadata(tool_gas_id).await.is_ok();

        if tool_exists ^ tool_gas_exists {
            return Err(NexusError::Configuration(format!(
                "On-chain tool '{fqn}' has inconsistent state: Tool exists={tool_exists}, \
                 ToolGas exists={tool_gas_exists}. Reset the deployment or recreate the missing \
                 object before retrying."
            )));
        }

        if !tool_exists {
            return Ok(OnChainToolInspection {
                fqn: fqn.clone(),
                tool_id,
                tool_gas_id,
                exists: false,
                tool: None,
                package_address: None,
                module_name: None,
                witness_id: None,
            });
        }

        let tool = crawler
            .get_object::<Tool>(tool_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;

        let (package_address, module_name, witness_id) = match &tool.reference {
            ToolRef::Sui {
                package_address,
                module_name,
                tool_witness_id,
            } => (
                Some(*package_address),
                Some(module_name.clone()),
                Some(*tool_witness_id),
            ),
            ToolRef::Http { .. } => (None, None, None),
        };

        Ok(OnChainToolInspection {
            fqn: fqn.clone(),
            tool_id,
            tool_gas_id,
            exists: true,
            tool: Some(tool),
            package_address,
            module_name,
            witness_id,
        })
    }

    /// Register a cap-gated on-chain tool, or return the existing Tool/ToolGas
    /// objects when both are already present for `params.fqn`. The reuse path
    /// returns `reused: true` and decoded refs from the on-chain Tool object;
    /// no transaction is submitted in that case.
    pub async fn register_on_chain_or_reuse(
        &self,
        params: RegisterOnChainToolParams,
    ) -> Result<RegisterOnChainToolResult, NexusError> {
        let inspection = self.inspect_on_chain_tool(&params.fqn).await?;
        if inspection.exists {
            let package_address = inspection.package_address.ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "On-chain tool '{}' exists but does not have a Sui reference; cannot reuse",
                    params.fqn
                ))
            })?;
            let module_name = inspection.module_name.ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "On-chain tool '{}' exists but does not have a module name; cannot reuse",
                    params.fqn
                ))
            })?;
            let witness_id = inspection.witness_id.ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "On-chain tool '{}' exists but does not have a witness id; cannot reuse",
                    params.fqn
                ))
            })?;
            return Ok(RegisterOnChainToolResult {
                fqn: params.fqn,
                tool_id: inspection.tool_id,
                tool_gas_id: inspection.tool_gas_id,
                package_address,
                module_name,
                witness_id,
                owner_cap_over_tool: None,
                owner_cap_over_gas: None,
                workflow_authorization_cap_first: inspection
                    .tool
                    .map(|t| t.workflow_authorization_cap_first)
                    .unwrap_or(params.workflow_authorization_cap_first),
                reused: true,
                tx_digest: None,
                tx_checkpoint: None,
            });
        }

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        let mut tx = sui::tx::TransactionBuilder::new();
        tool::register_on_chain_for_self_with_workflow_authorization_cap(
            &mut tx,
            nexus_objects,
            params.package_address,
            params.module.as_str(),
            &params.fqn,
            &params.description,
            &params.input_schema,
            &params.output_schema,
            params.timeout,
            params.witness_id,
            &params.collateral_coin,
            address,
            params.workflow_authorization_cap_first,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;
        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);
        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await;
        self.client.gas.release_gas_coin(gas_coin).await;
        let response = response?;

        let (owner_cap_over_tool, owner_cap_over_gas) =
            extract_owner_caps(&response.objects, nexus_objects)?;

        Ok(RegisterOnChainToolResult {
            fqn: params.fqn,
            tool_id: inspection.tool_id,
            tool_gas_id: inspection.tool_gas_id,
            package_address: params.package_address,
            module_name: params.module,
            witness_id: params.witness_id,
            owner_cap_over_tool: Some(owner_cap_over_tool),
            owner_cap_over_gas,
            workflow_authorization_cap_first: params.workflow_authorization_cap_first,
            reused: false,
            tx_digest: Some(response.digest),
            tx_checkpoint: Some(response.checkpoint),
        })
    }
}

/// Locate the `CloneableOwnerCap<OverTool>` and `CloneableOwnerCap<OverGas>`
/// objects in a register-on-chain transaction response. OverGas may be absent
/// for older registration paths; OverTool is always required.
fn extract_owner_caps(
    objects: &[sui::types::Object],
    nexus_objects: &crate::types::NexusObjects,
) -> Result<(sui::types::Address, Option<sui::types::Address>), NexusError> {
    let mut over_tool = None;
    let mut over_gas = None;

    for object in objects {
        let sui::types::ObjectType::Struct(tag) = object.object_type() else {
            continue;
        };
        if *tag.address() != nexus_objects.primitives_pkg_id
            || *tag.module() != primitives::OwnerCap::CLONEABLE_OWNER_CAP.module
            || *tag.name() != primitives::OwnerCap::CLONEABLE_OWNER_CAP.name
        {
            continue;
        }
        let Some(generic) = tag.type_params().first() else {
            continue;
        };
        let sui::types::TypeTag::Struct(inner) = generic else {
            continue;
        };
        if *inner.address() == nexus_objects.workflow_pkg_id
            && *inner.module() == workflow_idents::ToolRegistry::OVER_TOOL.module
            && *inner.name() == workflow_idents::ToolRegistry::OVER_TOOL.name
        {
            over_tool = Some(object.object_id());
        } else if *inner.address() == nexus_objects.workflow_pkg_id
            && *inner.module() == workflow_idents::Gas::OVER_GAS.module
            && *inner.name() == workflow_idents::Gas::OVER_GAS.name
        {
            over_gas = Some(object.object_id());
        }
    }

    let over_tool = over_tool.ok_or_else(|| {
        NexusError::Parsing(anyhow::anyhow!(
            "OwnerCap<OverTool> not found in register-on-chain response"
        ))
    })?;
    Ok((over_tool, over_gas))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            fqn,
            sui::traits::*,
            test_utils::{nexus_mocks, sui_mocks},
        },
        tonic::Status,
    };

    /// Construct a `CloneableOwnerCap<INNER_TAG>` Sui object that carries
    /// `owner_cap_id` as its on-chain id. Used by the `extract_owner_caps`
    /// tests to fake the post-publish object set.
    fn owner_cap_object(
        rng: &mut rand::rngs::ThreadRng,
        primitives_pkg_id: sui::types::Address,
        inner: sui::types::StructTag,
        owner_cap_id: sui::types::Address,
    ) -> sui::types::Object {
        let cap_tag = sui::types::StructTag::new(
            primitives_pkg_id,
            primitives::OwnerCap::CLONEABLE_OWNER_CAP.module,
            primitives::OwnerCap::CLONEABLE_OWNER_CAP.name,
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        );
        let owner_address = sui::types::Address::generate(&mut *rng);
        let digest = sui::types::Digest::generate(&mut *rng);
        sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(cap_tag, true, 0, owner_cap_id.to_bcs().unwrap())
                    .unwrap(),
            ),
            sui::types::Owner::Address(owner_address),
            digest,
            1000,
        )
    }

    fn over_tool_inner(workflow_pkg_id: sui::types::Address) -> sui::types::StructTag {
        sui::types::StructTag::new(
            workflow_pkg_id,
            workflow_idents::ToolRegistry::OVER_TOOL.module,
            workflow_idents::ToolRegistry::OVER_TOOL.name,
            vec![],
        )
    }

    fn over_gas_inner(workflow_pkg_id: sui::types::Address) -> sui::types::StructTag {
        sui::types::StructTag::new(
            workflow_pkg_id,
            workflow_idents::Gas::OVER_GAS.module,
            workflow_idents::Gas::OVER_GAS.name,
            vec![],
        )
    }

    #[test]
    fn extract_owner_caps_finds_both_caps() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let over_tool_id = sui::types::Address::generate(&mut rng);
        let over_gas_id = sui::types::Address::generate(&mut rng);

        let objects = vec![
            owner_cap_object(
                &mut rng,
                nexus_objects.primitives_pkg_id,
                over_tool_inner(nexus_objects.workflow_pkg_id),
                over_tool_id,
            ),
            owner_cap_object(
                &mut rng,
                nexus_objects.primitives_pkg_id,
                over_gas_inner(nexus_objects.workflow_pkg_id),
                over_gas_id,
            ),
        ];

        let (tool, gas) = extract_owner_caps(&objects, &nexus_objects).expect("both caps found");
        assert_eq!(tool, over_tool_id);
        assert_eq!(gas, Some(over_gas_id));
    }

    #[test]
    fn extract_owner_caps_allows_missing_over_gas() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let over_tool_id = sui::types::Address::generate(&mut rng);

        let objects = vec![owner_cap_object(
            &mut rng,
            nexus_objects.primitives_pkg_id,
            over_tool_inner(nexus_objects.workflow_pkg_id),
            over_tool_id,
        )];

        let (tool, gas) = extract_owner_caps(&objects, &nexus_objects).expect("over_tool found");
        assert_eq!(tool, over_tool_id);
        assert!(gas.is_none());
    }

    #[test]
    fn extract_owner_caps_fails_when_over_tool_missing() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let over_gas_id = sui::types::Address::generate(&mut rng);

        let objects = vec![owner_cap_object(
            &mut rng,
            nexus_objects.primitives_pkg_id,
            over_gas_inner(nexus_objects.workflow_pkg_id),
            over_gas_id,
        )];

        let error = extract_owner_caps(&objects, &nexus_objects).expect_err("over_tool required");
        assert!(
            error.to_string().contains("OwnerCap<OverTool>"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn extract_owner_caps_ignores_unrelated_struct_objects() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let unrelated_inner = sui::types::StructTag::new(
            nexus_objects.workflow_pkg_id,
            sui::types::Identifier::from_static("some_module"),
            sui::types::Identifier::from_static("SomeStruct"),
            vec![],
        );

        // Unrelated cap with the same CloneableOwnerCap wrapper but a different
        // generic; we don't want it to match either slot.
        let unrelated_id = sui::types::Address::generate(&mut rng);
        let objects = vec![owner_cap_object(
            &mut rng,
            nexus_objects.primitives_pkg_id,
            unrelated_inner,
            unrelated_id,
        )];

        let error =
            extract_owner_caps(&objects, &nexus_objects).expect_err("unrelated cap must not match");
        assert!(
            error.to_string().contains("OwnerCap<OverTool>"),
            "unexpected error: {error}"
        );
    }

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
            let tool_id = derive_tool_id(*nexus_objects.tool_registry.object_id(), &fqn)
                .expect("tool id derives");
            let tool_gas_id = derive_tool_gas_id(*nexus_objects.gas_service.object_id(), &fqn)
                .expect("tool gas id derives");
            Self {
                nexus_objects,
                fqn,
                tool_id,
                tool_gas_id,
            }
        }
    }

    fn fixture_tool(
        id: sui::types::Address,
        fqn: crate::ToolFqn,
        package_address: sui::types::Address,
        module_name: sui::types::Identifier,
        witness_id: sui::types::Address,
        workflow_authorization_cap_first: bool,
    ) -> Tool {
        Tool {
            id,
            fqn,
            reference: ToolRef::Sui {
                package_address,
                module_name,
                tool_witness_id: witness_id,
            },
            description: "demo".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            workflow_authorization_cap_first,
            registered_at: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
                .expect("epoch datetime"),
            unregistered_at: None,
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
    async fn inspect_on_chain_tool_reports_missing_when_neither_object_exists() {
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
            .inspect_on_chain_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds when both objects missing");

        assert!(!inspection.exists);
        assert_eq!(inspection.tool_id, fixture.tool_id);
        assert_eq!(inspection.tool_gas_id, fixture.tool_gas_id);
        assert!(inspection.tool.is_none());
        assert!(inspection.package_address.is_none());
        assert!(inspection.module_name.is_none());
        assert!(inspection.witness_id.is_none());
    }

    #[tokio::test]
    async fn inspect_on_chain_tool_rejects_inconsistent_state() {
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
            .inspect_on_chain_tool(&fixture.fqn)
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
    async fn inspect_on_chain_tool_decodes_existing_sui_tool() {
        let mut rng = rand::thread_rng();
        let fixture = InspectionFixture::new();
        let package_address = sui::types::Address::generate(&mut rng);
        let witness_id = sui::types::Address::generate(&mut rng);
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
            fixture.tool_id,
            fixture.fqn.clone(),
            package_address,
            module_name.clone(),
            witness_id,
            true,
        );
        let tool_json = serde_json::to_value(&tool).expect("Tool serializes to JSON");

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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            tool_json,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let inspection = client
            .tool()
            .inspect_on_chain_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds when Tool present");

        assert!(inspection.exists);
        assert_eq!(inspection.tool_id, fixture.tool_id);
        assert_eq!(inspection.tool_gas_id, fixture.tool_gas_id);
        assert_eq!(inspection.package_address, Some(package_address));
        assert_eq!(inspection.module_name.as_ref(), Some(&module_name));
        assert_eq!(inspection.witness_id, Some(witness_id));
        let decoded = inspection.tool.expect("Tool decoded");
        assert!(decoded.workflow_authorization_cap_first);
    }

    #[tokio::test]
    async fn register_on_chain_or_reuse_short_circuits_when_tool_exists() {
        let mut rng = rand::thread_rng();
        let fixture = InspectionFixture::new();
        let package_address = sui::types::Address::generate(&mut rng);
        let witness_id = sui::types::Address::generate(&mut rng);
        let module_name = sui::types::Identifier::from_static("demo_onchain_vertex");
        let collateral_coin = sui_mocks::mock_sui_object_ref();

        let tool_ref = sui::types::ObjectReference::new(
            fixture.tool_id,
            9,
            sui::types::Digest::from([5u8; 32]),
        );
        let tool_gas_ref = sui::types::ObjectReference::new(
            fixture.tool_gas_id,
            9,
            sui::types::Digest::from([6u8; 32]),
        );
        let tool = fixture_tool(
            fixture.tool_id,
            fixture.fqn.clone(),
            package_address,
            module_name.clone(),
            witness_id,
            true,
        );
        let tool_json = serde_json::to_value(&tool).expect("Tool serializes to JSON");

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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            tool_json,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let params = RegisterOnChainToolParams {
            package_address: sui::types::Address::generate(&mut rng),
            module: sui::types::Identifier::from_static("ignored_module"),
            fqn: fixture.fqn.clone(),
            description: "ignored".to_string(),
            input_schema: String::new(),
            output_schema: String::new(),
            timeout: Duration::from_secs(5),
            witness_id: sui::types::Address::generate(&mut rng),
            collateral_coin,
            workflow_authorization_cap_first: false,
        };

        let result = client
            .tool()
            .register_on_chain_or_reuse(params)
            .await
            .expect("reuse path succeeds");

        assert!(result.reused);
        assert!(result.tx_digest.is_none());
        assert!(result.tx_checkpoint.is_none());
        assert!(result.owner_cap_over_tool.is_none());
        assert!(result.owner_cap_over_gas.is_none());
        // Decoded refs come from the on-chain Tool, NOT from params.
        assert_eq!(result.package_address, package_address);
        assert_eq!(result.module_name, module_name);
        assert_eq!(result.witness_id, witness_id);
        // The stored `workflow_authorization_cap_first` from the on-chain Tool
        // is preferred over the params value.
        assert!(result.workflow_authorization_cap_first);
    }

    #[tokio::test]
    async fn inspect_on_chain_tool_rejects_inconsistent_state_when_only_tool_gas_present() {
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
            .inspect_on_chain_tool(&fixture.fqn)
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
    async fn inspect_on_chain_tool_returns_none_decoded_fields_for_http_tool() {
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
        let http_tool = Tool {
            id: fixture.tool_id,
            fqn: fixture.fqn.clone(),
            reference: ToolRef::Http {
                url: "https://example.com/tool".parse().expect("valid URL"),
            },
            description: "http tool".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            workflow_authorization_cap_first: false,
            registered_at: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
                .expect("epoch datetime"),
            unregistered_at: None,
        };
        let tool_json = serde_json::to_value(&http_tool).expect("Tool serializes to JSON");

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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            tool_json,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let inspection = client
            .tool()
            .inspect_on_chain_tool(&fixture.fqn)
            .await
            .expect("inspect succeeds for HTTP tool");

        assert!(inspection.exists);
        // HTTP-variant tools have no on-chain package/module/witness to decode.
        assert!(inspection.package_address.is_none());
        assert!(inspection.module_name.is_none());
        assert!(inspection.witness_id.is_none());
        // But the Tool itself must still come back so the caller can read other fields.
        let decoded = inspection.tool.expect("Tool decoded");
        assert!(matches!(decoded.reference, ToolRef::Http { .. }));
    }

    #[tokio::test]
    async fn register_on_chain_or_reuse_fails_when_existing_tool_is_http() {
        let mut rng = rand::thread_rng();
        let fixture = InspectionFixture::new();
        let collateral_coin = sui_mocks::mock_sui_object_ref();

        let tool_ref = sui::types::ObjectReference::new(
            fixture.tool_id,
            3,
            sui::types::Digest::from([9u8; 32]),
        );
        let tool_gas_ref = sui::types::ObjectReference::new(
            fixture.tool_gas_id,
            3,
            sui::types::Digest::from([10u8; 32]),
        );
        let http_tool = Tool {
            id: fixture.tool_id,
            fqn: fixture.fqn.clone(),
            reference: ToolRef::Http {
                url: "https://example.com/tool".parse().expect("valid URL"),
            },
            description: "http tool".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            workflow_authorization_cap_first: false,
            registered_at: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
                .expect("epoch datetime"),
            unregistered_at: None,
        };
        let tool_json = serde_json::to_value(&http_tool).expect("Tool serializes to JSON");

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
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            tool_ref,
            sui::types::Owner::Shared(1),
            tool_json,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let params = RegisterOnChainToolParams {
            package_address: sui::types::Address::generate(&mut rng),
            module: sui::types::Identifier::from_static("ignored_module"),
            fqn: fixture.fqn.clone(),
            description: "ignored".to_string(),
            input_schema: String::new(),
            output_schema: String::new(),
            timeout: Duration::from_secs(5),
            witness_id: sui::types::Address::generate(&mut rng),
            collateral_coin,
            workflow_authorization_cap_first: true,
        };

        let error = client
            .tool()
            .register_on_chain_or_reuse(params)
            .await
            .expect_err("reuse path rejects HTTP tools");

        let error_string = error.to_string();
        assert!(
            matches!(error, NexusError::Parsing(_)),
            "unexpected error variant: {error_string}"
        );
        assert!(
            error_string.contains("does not have a Sui reference"),
            "unexpected error message: {error_string}"
        );
    }

    #[tokio::test]
    async fn register_on_chain_or_reuse_submits_fresh_registration_and_extracts_owner_caps() {
        let mut rng = rand::thread_rng();
        let fixture = InspectionFixture::new();
        let package_address = sui::types::Address::generate(&mut rng);
        let witness_id = sui::types::Address::generate(&mut rng);
        let module_name = sui::types::Identifier::from_static("demo_onchain_vertex");
        let collateral_coin = sui_mocks::mock_sui_object_ref();
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let tx_digest = sui::types::Digest::generate(&mut rng);

        let over_tool_id = sui::types::Address::generate(&mut rng);
        let over_gas_id = sui::types::Address::generate(&mut rng);
        let response_objects = vec![
            owner_cap_object(
                &mut rng,
                fixture.nexus_objects.primitives_pkg_id,
                over_tool_inner(fixture.nexus_objects.workflow_pkg_id),
                over_tool_id,
            ),
            owner_cap_object(
                &mut rng,
                fixture.nexus_objects.primitives_pkg_id,
                over_gas_inner(fixture.nexus_objects.workflow_pkg_id),
                over_gas_id,
            ),
        ];

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        // Both probes fail → register_on_chain_or_reuse takes the fresh-registration branch.
        mock_get_object_not_found(&mut ledger_service_mock);
        mock_get_object_not_found(&mut ledger_service_mock);
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref,
            response_objects,
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&fixture.nexus_objects, &rpc_url).await;

        let params = RegisterOnChainToolParams {
            package_address,
            module: module_name.clone(),
            fqn: fixture.fqn.clone(),
            description: "fresh demo tool".to_string(),
            input_schema: "{}".to_string(),
            output_schema: "{}".to_string(),
            timeout: Duration::from_secs(5),
            witness_id,
            collateral_coin,
            workflow_authorization_cap_first: true,
        };

        let result = client
            .tool()
            .register_on_chain_or_reuse(params)
            .await
            .expect("fresh registration succeeds");

        assert!(!result.reused);
        assert_eq!(result.tx_digest, Some(tx_digest));
        assert!(result.tx_checkpoint.is_some());
        assert_eq!(result.owner_cap_over_tool, Some(over_tool_id));
        assert_eq!(result.owner_cap_over_gas, Some(over_gas_id));
        assert_eq!(result.package_address, package_address);
        assert_eq!(result.module_name, module_name);
        assert_eq!(result.witness_id, witness_id);
        assert_eq!(result.tool_id, fixture.tool_id);
        assert_eq!(result.tool_gas_id, fixture.tool_gas_id);
        assert!(result.workflow_authorization_cap_first);
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
