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
                witness_id,
            } => (
                Some(*package_address),
                Some(module_name.clone()),
                Some(*witness_id),
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
        crate::{
            fqn,
            sui,
            test_utils::{nexus_mocks, sui_mocks},
        },
        std::time::Duration,
    };

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
