//! Read-only helpers and high-level actions for standard TAP.

#[cfg(feature = "move_publish")]
use crate::types::TapSkillConfig;
use crate::{
    events::NexusEventKind,
    idents::{sui_framework, tap::TapStandard},
    nexus::{
        client::NexusClient,
        crawler::{Crawler, Response},
        error::NexusError,
        signer::ExecutedTransaction,
    },
    sui,
    transactions::{scheduler as scheduler_tx, tap as tap_tx},
    types::{
        resolve_active_tap_endpoint,
        resolve_active_tap_skill_execution_target,
        resolve_default_tap_execution_target,
        AgentId,
        InterfaceRevision,
        NexusObjects,
        RuntimeVertex,
        SkillId,
        TapActiveSkillExecutionTarget,
        TapAgentPaymentVault,
        TapAgentVaultFieldKey,
        TapConfigDigestInput,
        TapDefaultExecutionTarget,
        TapDefaultExecutionTargetRecord,
        TapEndpointKey,
        TapEndpointRecord,
        TapEndpointResolutionError,
        TapExecutionPayment,
        TapExecutionPaymentHistoryFieldKey,
        TapExecutionPaymentHistoryList,
        TapExecutionPaymentReceipt,
        TapExecutionPaymentReceiptFieldKey,
        TapPublishArtifact,
        TapRegistry,
        TapRegistryObject,
        TapSchedulePolicy,
        TapScheduledSkillTask,
        TapSkillRecord,
        TapSkillRequirements,
        WorkflowVertexAuthorizationGrant,
        WorkflowVertexAuthorizationGrantAccess,
        WorkflowVertexAuthorizationGrantFieldKey,
    },
};
#[cfg(feature = "move_publish")]
use {std::path::PathBuf, sui_move_build::CompiledPackage};

/// High-level standard TAP actions exposed through [`NexusClient`].
#[derive(Clone)]
pub struct TapActions {
    pub(super) client: NexusClient,
}

/// Result returned after creating a standard Talus agent.
#[derive(Clone, Debug)]
pub struct CreateAgentResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub agent_id: AgentId,
    pub agent_object: sui::types::ObjectReference,
}

/// Result returned after registering a published DAG/TAP package as a skill.
#[derive(Clone, Debug)]
pub struct RegisterSkillResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
}

/// Result returned after announcing a TAP endpoint revision.
#[derive(Clone, Debug)]
pub struct AnnounceEndpointRevisionResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub endpoint_key: TapEndpointKey,
    pub endpoint_object: sui::types::ObjectReference,
    pub config_digest: Vec<u8>,
    pub config_digest_input: TapConfigDigestInput,
}

/// Result returned after resolving live skill requirements.
#[derive(Clone, Debug)]
pub struct GetSkillRequirementsResult {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub active_endpoint_key: TapEndpointKey,
    pub requirements: TapSkillRequirements,
}

/// Result returned after scheduling a standard TAP skill execution.
#[derive(Clone, Debug)]
pub struct ScheduleSkillExecutionResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub scheduled_task_id: sui::types::Address,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TapPaymentHistory {
    pub wallet_receipts: Vec<TapExecutionPaymentReceipt>,
    pub vault_receipts: Vec<TapExecutionPaymentReceipt>,
    pub unresolved_execution_ids: Vec<sui::types::Address>,
    pub resolved_execution_ids: Vec<sui::types::Address>,
}

/// Parameters for creating a durable address-funded TAP schedule and linking
/// it to an existing workflow scheduler task.
#[derive(Clone, Debug)]
pub struct ScheduleSkillExecutionAddressFundedParams {
    pub scheduler_task: sui::types::ObjectReference,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub input_commitment: Vec<u8>,
    pub prepay_amount: u64,
    pub refund_recipient: Option<sui::types::Address>,
    pub payment_source: Vec<u8>,
    pub occurrence_budget: u64,
    pub refund_mode: u8,
    pub authorization_plan_commitment: Option<Vec<u8>>,
    pub schedule_policy: TapSchedulePolicy,
    pub refill_policy_commitment: Vec<u8>,
    pub schedule_entries_commitment: Vec<u8>,
    pub first_after_ms: u64,
}

/// Options for publishing a TAP Move package through [`TapActions`].
#[cfg(feature = "move_publish")]
#[derive(Clone, Debug, Default)]
pub struct TapPackagePublishOptions {
    pub package_path: PathBuf,
    pub named_address_overrides: Vec<(String, sui::types::Address)>,
}

/// Result returned after publishing a TAP Move package.
#[cfg(feature = "move_publish")]
#[derive(Clone, Debug)]
pub struct TapPackagePublishResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub package_id: sui::types::Address,
}

/// Result returned after creating and sharing a standard TAP endpoint object.
#[derive(Clone, Debug)]
pub struct CreateStandardEndpointResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub endpoint_object: sui::types::ObjectReference,
}

/// Result returned by the composed TAP skill publishing workflow.
#[cfg(feature = "move_publish")]
#[derive(Clone, Debug)]
pub struct PublishSkillResult {
    pub tap_package: TapPackagePublishResult,
    pub dag: crate::nexus::workflow::PublishResult,
    pub endpoint: CreateStandardEndpointResult,
    pub artifact: TapPublishArtifact,
}

impl TapActions {
    #[cfg(feature = "move_publish")]
    pub async fn publish_tap_package(
        &self,
        options: TapPackagePublishOptions,
    ) -> Result<TapPackagePublishResult, NexusError> {
        let package = build_move_package(&options.package_path, &options.named_address_overrides)
            .map_err(NexusError::TransactionBuilding)?;
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let upgrade_cap = tx.publish(
            package.package.get_package_bytes(false),
            package
                .get_dependency_storage_package_ids()
                .iter()
                .map(|id| {
                    id.to_string()
                        .parse::<sui::types::Address>()
                        .expect("compiled package dependency id must parse as Sui address")
                })
                .collect(),
        );
        let sender_arg = sui_framework::Address::address_from_type(&mut tx, address)
            .map_err(NexusError::TransactionBuilding)?;
        tx.transfer_objects(vec![upgrade_cap], sender_arg);

        let response = self.submit_tap_transaction(tx, address).await?;
        let package_id = response
            .objects
            .iter()
            .find_map(|object| match object.data() {
                sui::types::ObjectData::Package(package) => Some(package.id),
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Published TAP package ID not found in publish response"
                ))
            })?;

        Ok(TapPackagePublishResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            package_id,
        })
    }

    pub async fn create_standard_endpoint(
        &self,
        package_id: sui::types::Address,
    ) -> Result<CreateStandardEndpointResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let mut tx = sui::tx::TransactionBuilder::new();
        let endpoint = tap_tx::create_standard_endpoint(&mut tx, nexus_objects, package_id)
            .map_err(NexusError::TransactionBuilding)?;
        tap_tx::share_standard_endpoint(&mut tx, nexus_objects, endpoint);

        let response = self.submit_tap_transaction(tx, address).await?;
        let endpoint_object = response
            .objects
            .iter()
            .find_map(|object| match object.object_type() {
                sui::types::ObjectType::Struct(tag)
                    if *tag.address() == nexus_objects.registry_pkg_id()
                        && *tag.module() == TapStandard::CREATE_STANDARD_ENDPOINT.module
                        && *tag.name()
                            == sui::types::Identifier::from_static("StandardEndpoint") =>
                {
                    Some(sui::types::ObjectReference::new(
                        object.object_id(),
                        object.version(),
                        object.digest(),
                    ))
                }
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "Created standard TAP endpoint object not found in response"
                ))
            })?;

        Ok(CreateStandardEndpointResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            endpoint_object,
        })
    }

    #[cfg(feature = "move_publish")]
    pub async fn publish_skill(
        &self,
        config: &TapSkillConfig,
        dag: crate::types::Dag,
        package_options: TapPackagePublishOptions,
    ) -> Result<PublishSkillResult, NexusError> {
        config
            .validate()
            .map_err(|error| NexusError::TransactionBuilding(anyhow::anyhow!(error)))?;

        let tap_package = self.publish_tap_package(package_options).await?;
        let dag = self.client.workflow().publish(dag).await?;
        let endpoint = self
            .create_standard_endpoint(tap_package.package_id)
            .await?;
        let artifact =
            TapPublishArtifact::from_config(config, dag.dag_object_id, tap_package.package_id)
                .map_err(NexusError::TransactionBuilding)?
                .with_endpoint_object(endpoint.endpoint_object.clone())
                .map_err(NexusError::TransactionBuilding)?;

        Ok(PublishSkillResult {
            tap_package,
            dag,
            endpoint,
            artifact,
        })
    }

    /// Create a standard Talus agent through the configured TAP registry.
    pub async fn create_agent(
        &self,
        operator: sui::types::Address,
    ) -> Result<CreateAgentResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::tap_registry_arg(&mut tx, nexus_objects)
            .map_err(NexusError::TransactionBuilding)?;

        let agent = tap_tx::create_agent(&mut tx, nexus_objects, registry, operator)
            .map_err(NexusError::TransactionBuilding)?;
        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
                vec![crate::idents::tap::agent_type(
                    nexus_objects.interface_pkg_id,
                )],
            ),
            vec![agent],
        );

        let response = self.submit_tap_transaction(tx, address).await?;
        let event = find_event(&response, |kind| match kind {
            NexusEventKind::AgentCreated(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "AgentCreatedEvent not found in TAP create-agent response"
            ))
        })?;

        Ok(CreateAgentResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: event.agent_id,
            agent_object: response
                .objects
                .iter()
                .find_map(|object| match object.object_type() {
                    sui::types::ObjectType::Struct(tag)
                        if *tag.address() == nexus_objects.interface_pkg_id
                            && *tag.module() == crate::idents::tap::STANDARD_TAP_MODULE
                            && *tag.name() == sui::types::Identifier::from_static("Agent") =>
                    {
                        Some(sui::types::ObjectReference::new(
                            object.object_id(),
                            object.version(),
                            object.digest(),
                        ))
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    NexusError::Parsing(anyhow::anyhow!(
                        "Created standard Talus Agent object not found in response"
                    ))
                })?,
        })
    }

    /// Register a skill from a publish artifact.
    pub async fn register_skill(
        &self,
        agent_id: AgentId,
        artifact: &TapPublishArtifact,
        endpoint_object_id: Option<sui::types::Address>,
    ) -> Result<RegisterSkillResult, NexusError> {
        let endpoint_object_id = artifact
            .endpoint_object_id_or(endpoint_object_id)
            .map_err(NexusError::TransactionBuilding)?;
        let endpoint_object = self
            .client
            .crawler()
            .get_object_metadata(endpoint_object_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::tap_registry_arg(&mut tx, nexus_objects)
            .map_err(NexusError::TransactionBuilding)?;
        let config_digest = artifact
            .endpoint_config_digest(endpoint_object_id)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            true,
        ));

        tap_tx::register_skill(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            artifact.dag_id,
            artifact.tap_package_id,
            artifact.requirements.workflow_commitment.clone(),
            artifact.requirements.input_schema_commitment.clone(),
            artifact.requirements.metadata_commitment.clone(),
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
            artifact
                .requirements
                .vertex_authorization_schema
                .schema_commitment
                .clone(),
            *endpoint_object.object_id(),
            endpoint_object.version(),
            endpoint_object.digest().inner().to_vec(),
            artifact.shared_objects.clone(),
            config_digest,
            true,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.submit_tap_transaction(tx, address).await?;
        let event = find_event(&response, |kind| match kind {
            NexusEventKind::SkillRegistered(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "SkillRegisteredEvent not found in TAP register-skill response"
            ))
        })?;

        Ok(RegisterSkillResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: event.agent_id,
            skill_id: event.skill_id,
        })
    }

    /// Fetch live skill requirements from the configured TAP registry.
    pub async fn get_skill_requirements(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
    ) -> Result<GetSkillRequirementsResult, NexusError> {
        let target = fetch_configured_active_tap_skill_execution_target(
            self.client.crawler(),
            &self.client.nexus_objects,
            agent_id,
            skill_id,
        )
        .await
        .map_err(NexusError::Rpc)?
        .data;

        Ok(GetSkillRequirementsResult {
            agent_id,
            skill_id,
            active_endpoint_key: target.endpoint.key,
            requirements: target.endpoint.requirements,
        })
    }

    /// Announce a new endpoint revision for a registered TAP skill.
    pub async fn announce_endpoint_revision(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        artifact: &TapPublishArtifact,
        endpoint_object_id: Option<sui::types::Address>,
        active_for_new_executions: bool,
    ) -> Result<AnnounceEndpointRevisionResult, NexusError> {
        let endpoint_object_id = artifact
            .endpoint_object_id_or(endpoint_object_id)
            .map_err(NexusError::TransactionBuilding)?;
        let endpoint_object = self
            .client
            .crawler()
            .get_object_metadata(endpoint_object_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let config_digest_input = artifact.endpoint_config_digest_input(endpoint_object_id);
        let config_digest = config_digest_input
            .digest()
            .map_err(NexusError::TransactionBuilding)?;

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::tap_registry_arg(&mut tx, nexus_objects)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            false,
        ));

        tap_tx::announce_endpoint_revision(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            skill_id,
            artifact.interface_revision,
            *endpoint_object.object_id(),
            endpoint_object.version(),
            endpoint_object.digest().inner().to_vec(),
            artifact.shared_objects.clone(),
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
            artifact
                .requirements
                .vertex_authorization_schema
                .schema_commitment
                .clone(),
            config_digest.clone(),
            active_for_new_executions,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.submit_tap_transaction(tx, address).await?;
        let event = find_event(&response, |kind| match kind {
            NexusEventKind::EndpointRevisionAnnounced(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "EndpointRevisionAnnouncedEvent not found in TAP announce response"
            ))
        })?;

        Ok(AnnounceEndpointRevisionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            endpoint_key: TapEndpointKey {
                agent_id: event.agent_id,
                skill_id: event.skill_id,
                interface_revision: event.interface_revision,
            },
            endpoint_object,
            config_digest,
            config_digest_input,
        })
    }

    /// Schedule a standard TAP skill execution.
    #[allow(clippy::too_many_arguments)]
    pub async fn schedule_skill_execution(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        long_term_gas_coin_id: sui::types::Address,
        input_commitment: Vec<u8>,
        refill_policy_commitment: Vec<u8>,
        authorization_plan_commitment: Option<Vec<u8>>,
        schedule_policy: TapSchedulePolicy,
        schedule_entries_commitment: Vec<u8>,
        first_after_ms: u64,
    ) -> Result<ScheduleSkillExecutionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::tap_registry_arg(&mut tx, nexus_objects)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            false,
        ));

        let scheduled_task = tap_tx::schedule_skill_execution(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            skill_id,
            input_commitment,
            long_term_gas_coin_id,
            refill_policy_commitment,
            authorization_plan_commitment,
            schedule_policy,
            schedule_entries_commitment,
            first_after_ms,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let scheduled_task_type =
            crate::idents::tap::scheduled_skill_task_type(nexus_objects.interface_pkg_id);
        tx.move_call(
            sui::tx::Function::new(
                crate::idents::sui_framework::PACKAGE_ID,
                crate::idents::sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
                crate::idents::sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
                vec![scheduled_task_type],
            ),
            vec![scheduled_task],
        );

        let response = self.submit_tap_transaction(tx, address).await?;
        let event = find_event(&response, |kind| match kind {
            NexusEventKind::ScheduledSkillExecutionCreated(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "ScheduledSkillExecutionCreatedEvent not found in TAP schedule response"
            ))
        })?;

        Ok(ScheduleSkillExecutionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            scheduled_task_id: event.scheduled_task_id,
            agent_id: event.agent_id,
            skill_id: event.skill_id,
        })
    }

    /// Create an address-funded durable scheduled TAP task, attach it to the
    /// workflow scheduler task, and share the TAP scheduled task.
    pub async fn schedule_skill_execution_address_funded(
        &self,
        params: ScheduleSkillExecutionAddressFundedParams,
    ) -> Result<ScheduleSkillExecutionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let refund_recipient = params.refund_recipient.unwrap_or(address);
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(params.agent_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::tap_registry_arg(&mut tx, nexus_objects)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            false,
        ));
        let scheduler_task = tx.input(sui::tx::Input::shared(
            *params.scheduler_task.object_id(),
            params.scheduler_task.version(),
            true,
        ));
        let prepay_amount_input = crate::idents::pure_arg(&params.prepay_amount)
            .map_err(NexusError::TransactionBuilding)?;
        let prepay_amount = tx.input(prepay_amount_input);
        let prepayment_coin = tx
            .split_coins(tx.gas(), vec![prepay_amount])
            .nested(0)
            .ok_or_else(|| {
                NexusError::TransactionBuilding(anyhow::anyhow!(
                    "failed to split scheduled TAP prepayment coin"
                ))
            })?;

        let scheduled_task = tap_tx::schedule_skill_execution_address_funded(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            *params.scheduler_task.object_id(),
            params.skill_id,
            params.input_commitment,
            prepayment_coin,
            refund_recipient,
            params.payment_source,
            params.occurrence_budget,
            params.refund_mode,
            params.authorization_plan_commitment,
            params.schedule_policy,
            params.refill_policy_commitment,
            params.schedule_entries_commitment,
            params.first_after_ms,
        )
        .map_err(NexusError::TransactionBuilding)?;
        scheduler_tx::attach_tap_scheduled_task_link(
            &mut tx,
            nexus_objects,
            scheduler_task,
            scheduled_task,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let scheduled_task_type =
            crate::idents::tap::scheduled_skill_task_type(nexus_objects.interface_pkg_id);
        tx.move_call(
            sui::tx::Function::new(
                crate::idents::sui_framework::PACKAGE_ID,
                crate::idents::sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
                crate::idents::sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
                vec![scheduled_task_type],
            ),
            vec![scheduled_task],
        );

        let response = self.submit_tap_transaction(tx, address).await?;
        let event = find_event(&response, |kind| match kind {
            NexusEventKind::ScheduledSkillExecutionCreated(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "ScheduledSkillExecutionCreatedEvent not found in durable TAP schedule response"
            ))
        })?;

        Ok(ScheduleSkillExecutionResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            scheduled_task_id: event.scheduled_task_id,
            agent_id: event.agent_id,
            skill_id: event.skill_id,
        })
    }

    async fn submit_tap_transaction(
        &self,
        mut tx: sui::tx::TransactionBuilder,
        address: sui::types::Address,
    ) -> Result<ExecutedTransaction, NexusError> {
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
            .map_err(|error| NexusError::TransactionBuilding(error.into()))?;
        let signature = self.client.signer.sign_tx(&tx).await?;
        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await;

        self.client.gas.release_gas_coin(gas_coin).await;
        response
    }
}

fn find_event<T>(
    response: &ExecutedTransaction,
    matcher: impl Fn(&NexusEventKind) -> Option<&T>,
) -> Option<T>
where
    T: Clone,
{
    response
        .events
        .iter()
        .find_map(|event| matcher(&event.data).cloned())
}

#[cfg(feature = "move_publish")]
fn build_move_package(
    package_path: &std::path::Path,
    named_address_overrides: &[(String, sui::types::Address)],
) -> anyhow::Result<CompiledPackage> {
    let mut build_config = crate::sui::build::BuildConfig::new_for_testing_replace_addresses(
        named_address_overrides
            .iter()
            .map(|(name, address)| (name.clone(), address.to_string().parse().unwrap()))
            .collect::<Vec<_>>(),
    );
    build_config.print_diags_to_stderr = false;
    build_config.build(package_path)
}

/// Fetch the shared standard TAP registry object from chain storage.
pub async fn fetch_tap_registry(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Response<TapRegistry>> {
    let raw = crawler
        .get_object_contents_bcs::<TapRegistryObject>(registry_id)
        .await?;
    let agent_records = crawler
        .get_dynamic_fields_bcs::<sui::types::Address, crate::types::TapAgentRecord>(
            raw.data.agents.id,
            raw.data.agents.size(),
        )
        .await?;

    let mut agents = Vec::with_capacity(agent_records.len());
    let mut skills = Vec::new();
    let mut endpoints = Vec::new();
    let mut active_endpoints = Vec::new();

    for (_, agent) in agent_records {
        let skill_records = crawler
            .get_dynamic_fields_bcs::<SkillId, TapSkillRecord>(agent.skills.id, agent.skills.size())
            .await?;
        let endpoint_records = crawler
            .get_dynamic_fields_bcs::<crate::types::TapEndpointRevisionKey, crate::types::TapEndpointRevision>(
                agent.endpoints.id,
                agent.endpoints.size(),
            )
            .await?;

        active_endpoints.extend(agent.active_endpoints.iter().cloned());
        skills.extend(skill_records.into_values());
        endpoints.extend(endpoint_records.into_values());
        agents.push(agent);
    }

    Ok(Response {
        object_id: raw.object_id,
        owner: raw.owner,
        version: raw.version,
        digest: raw.digest,
        balance: raw.balance,
        data: TapRegistry {
            id: raw.data.id,
            agents,
            skills,
            endpoints,
            active_endpoints,
            default_target: raw.data.default_target.0,
        },
    })
}

/// Fetch a pinned TAP endpoint from the real `TapRegistry` vector layout.
pub async fn fetch_tap_endpoint(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let registry = fetch_tap_registry(crawler, registry_id).await?;
    let record = registry.data.endpoint_record(TapEndpointKey {
        agent_id,
        skill_id,
        interface_revision,
    })?;

    Ok(registry_response_with_data(registry, record))
}

/// Resolve a fresh execution endpoint through `TapRegistry.active_endpoints`.
pub async fn fetch_active_tap_endpoint(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let registry = fetch_tap_registry(crawler, registry_id).await?;
    let record = registry.data.active_endpoint_record(agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, record))
}

/// Return the configured TAP registry object ID, failing clearly for
/// deployments created before the standard registry was added to metadata.
pub fn configured_tap_registry_id(objects: &NexusObjects) -> anyhow::Result<sui::types::Address> {
    objects
        .tap_registry()
        .map(|registry| *registry.object_id())
        .ok_or_else(|| anyhow::anyhow!("NexusObjects missing tap_registry object reference"))
}

/// Return the configured standard default TAP execution target from deployment metadata.
pub fn configured_default_tap_target(
    objects: &NexusObjects,
) -> anyhow::Result<TapDefaultExecutionTarget> {
    objects
        .default_tap_target()
        .ok_or_else(|| anyhow::anyhow!("NexusObjects missing default_tap_target metadata"))
}

/// Fetch the shared TAP registry named by `NexusObjects`.
pub async fn fetch_configured_tap_registry(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<TapRegistry>> {
    fetch_tap_registry(crawler, configured_tap_registry_id(objects)?).await
}

/// Resolve a fresh execution endpoint through the configured TAP registry.
pub async fn fetch_configured_active_tap_endpoint(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    fetch_active_tap_endpoint(
        crawler,
        configured_tap_registry_id(objects)?,
        agent_id,
        skill_id,
    )
    .await
}

/// Resolve the active skill registration plus endpoint from the configured TAP registry.
pub async fn fetch_configured_active_tap_skill_execution_target(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapActiveSkillExecutionTarget>> {
    let registry = fetch_configured_tap_registry(crawler, objects).await?;
    let target = resolve_active_tap_skill_execution_target(&registry.data, agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, target))
}

/// Resolve the configured default TAP execution target from the configured registry.
pub async fn fetch_configured_default_tap_execution_target(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<TapDefaultExecutionTargetRecord>> {
    let registry = fetch_configured_tap_registry(crawler, objects).await?;
    let target = resolve_default_tap_execution_target(&registry.data)?;

    Ok(registry_response_with_data(registry, target))
}

/// Fetch a shared standard TAP execution payment object by object ID.
pub async fn fetch_tap_execution_payment(
    crawler: &Crawler,
    payment_id: sui::types::Address,
) -> anyhow::Result<Response<TapExecutionPayment>> {
    crawler.get_object::<TapExecutionPayment>(payment_id).await
}

/// Fetch wallet-owned standard TAP execution payment receipts.
pub async fn fetch_wallet_execution_payment_receipts(
    crawler: &Crawler,
    objects: &NexusObjects,
    owner: sui::types::Address,
) -> anyhow::Result<Vec<Response<TapExecutionPaymentReceipt>>> {
    crawler
        .get_owned_objects(
            owner,
            sui::types::StructTag::new(
                objects.interface_pkg_id,
                crate::idents::tap::STANDARD_TAP_MODULE,
                sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
                vec![],
            ),
        )
        .await
}

/// Fetch one vault-owned payment receipt stored under a Talus agent.
pub async fn fetch_agent_vault_execution_payment_receipt(
    crawler: &Crawler,
    agent_id: AgentId,
    execution_id: sui::types::Address,
) -> anyhow::Result<Response<TapExecutionPaymentReceipt>> {
    crawler
        .get_dynamic_object_field::<TapExecutionPaymentReceiptFieldKey, TapExecutionPaymentReceipt>(
            agent_id,
            TapExecutionPaymentReceiptFieldKey { execution_id },
        )
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "execution payment receipt for execution '{execution_id}' not found under agent '{agent_id}': {e}"
            )
        })
}

/// Fetch vault-owned execution IDs from an agent's unresolved/resolved history lists.
pub async fn fetch_agent_execution_payment_history_ids(
    crawler: &Crawler,
    agent_id: AgentId,
) -> anyhow::Result<(Vec<sui::types::Address>, Vec<sui::types::Address>)> {
    let mut fields = crawler
        .get_dynamic_object_fields::<
            TapExecutionPaymentHistoryFieldKey,
            TapExecutionPaymentHistoryList,
        >(agent_id)
        .await?;
    let mut unresolved = Vec::new();
    let mut resolved = Vec::new();

    if let Some(list) = fields.remove(&TapExecutionPaymentHistoryFieldKey { resolved: false }) {
        unresolved = list.data.execution_ids;
    }
    if let Some(list) = fields.remove(&TapExecutionPaymentHistoryFieldKey { resolved: true }) {
        resolved = list.data.execution_ids;
    }

    Ok((unresolved, resolved))
}

/// Fetch wallet receipts and, when an agent is supplied, vault receipt history.
pub async fn fetch_execution_payment_history(
    crawler: &Crawler,
    objects: &NexusObjects,
    owner: sui::types::Address,
    agent_id: Option<AgentId>,
) -> anyhow::Result<TapPaymentHistory> {
    let wallet_receipts = fetch_wallet_execution_payment_receipts(crawler, objects, owner)
        .await?
        .into_iter()
        .map(|response| response.data)
        .collect::<Vec<_>>();
    let mut history = TapPaymentHistory {
        wallet_receipts,
        ..Default::default()
    };

    let Some(agent_id) = agent_id else {
        return Ok(history);
    };

    let (unresolved_execution_ids, resolved_execution_ids) =
        fetch_agent_execution_payment_history_ids(crawler, agent_id).await?;
    let mut vault_receipts = Vec::new();
    for execution_id in unresolved_execution_ids
        .iter()
        .chain(resolved_execution_ids.iter())
        .copied()
    {
        vault_receipts.push(
            fetch_agent_vault_execution_payment_receipt(crawler, agent_id, execution_id)
                .await?
                .data,
        );
    }

    history.vault_receipts = vault_receipts;
    history.unresolved_execution_ids = unresolved_execution_ids;
    history.resolved_execution_ids = resolved_execution_ids;
    Ok(history)
}

/// Fetch a standard Talus agent payment vault object by object ID.
pub async fn fetch_tap_agent_payment_vault(
    crawler: &Crawler,
    vault_id: sui::types::Address,
) -> anyhow::Result<Response<TapAgentPaymentVault>> {
    crawler.get_object::<TapAgentPaymentVault>(vault_id).await
}

/// Fetch the standard Talus agent payment vault stored as a child of the agent object.
pub async fn fetch_tap_agent_payment_vault_for_agent(
    crawler: &Crawler,
    agent_id: AgentId,
) -> anyhow::Result<Response<TapAgentPaymentVault>> {
    crawler
        .get_dynamic_object_field::<TapAgentVaultFieldKey, TapAgentPaymentVault>(
            agent_id,
            TapAgentVaultFieldKey {},
        )
        .await
}

pub async fn fetch_workflow_vertex_authorization_grant(
    crawler: &Crawler,
    execution_id: sui::types::Address,
    vertex: RuntimeVertex,
) -> anyhow::Result<Response<WorkflowVertexAuthorizationGrant>> {
    crawler
        .get_dynamic_object_field::<
            WorkflowVertexAuthorizationGrantFieldKey,
            WorkflowVertexAuthorizationGrant,
        >(execution_id, WorkflowVertexAuthorizationGrantFieldKey { vertex })
        .await
}

pub async fn resolve_current_workflow_vertex_authorization_grant(
    crawler: &Crawler,
    execution_id: sui::types::Address,
    vertex: &RuntimeVertex,
) -> anyhow::Result<Option<WorkflowVertexAuthorizationGrantAccess>> {
    match fetch_workflow_vertex_authorization_grant(crawler, execution_id, vertex.clone()).await {
        Ok(response) => Ok(Some(WorkflowVertexAuthorizationGrantAccess {
            object_ref: response.object_ref(),
            grant: response.data,
        })),
        Err(error) if error.to_string().contains("not found") => Ok(None),
        Err(error) => Err(error),
    }
}

/// Fetch a standard TAP scheduled skill task object by object ID.
pub async fn fetch_tap_scheduled_skill_task(
    crawler: &Crawler,
    scheduled_task_id: sui::types::Address,
) -> anyhow::Result<Response<TapScheduledSkillTask>> {
    crawler
        .get_object::<TapScheduledSkillTask>(scheduled_task_id)
        .await
}

/// Resolve a fresh execution endpoint from already fetched records.
pub fn resolve_active_endpoint_record(
    records: &[TapEndpointRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&TapEndpointRecord, TapEndpointResolutionError> {
    resolve_active_tap_endpoint(records, agent_id, skill_id)
}

fn registry_response_with_data<T>(registry: Response<TapRegistry>, data: T) -> Response<T> {
    Response {
        object_id: registry.object_id,
        owner: registry.owner,
        version: registry.version,
        data,
        digest: registry.digest,
        balance: registry.balance,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events,
            idents::primitives,
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                InterfaceRevision,
                MoveTable,
                NexusObjects,
                TapAgentRecord,
                TapDagBinding,
                TapDefaultExecutionTarget,
                TapEndpointActivation,
                TapEndpointKey,
                TapEndpointRevision,
                TapEndpointRevisionKey,
                TapPaymentPolicy,
                TapRegistryObject,
                TapSchedulePolicy,
                TapSharedObjectRef,
                TapSkillRecord,
                TapSkillRequirements,
                TapVertexAuthorizationSchema,
            },
        },
    };

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct Wrapper<T> {
        event: T,
    }

    fn endpoint(active: bool) -> TapEndpointRecord {
        TapEndpointRecord {
            key: TapEndpointKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(1),
            },
            package_id: sui::types::Address::from_static("0xc"),
            endpoint_object: sui::types::ObjectReference::new(
                sui::types::Address::from_static("0xd"),
                1,
                sui::types::Digest::from([7; 32]),
            ),
            shared_objects: vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0xe"),
            )],
            config_digest: vec![1],
            requirements: TapSkillRequirements {
                input_schema_commitment: vec![2],
                workflow_commitment: vec![3],
                metadata_commitment: vec![4],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            active_for_new_executions: active,
        }
    }

    fn endpoint_revision(revision: u64, active: bool) -> TapEndpointRevision {
        let endpoint = endpoint(active);
        TapEndpointRevision {
            agent_id: endpoint.key.agent_id,
            skill_id: endpoint.key.skill_id,
            interface_revision: InterfaceRevision(revision),
            package_id: endpoint.package_id,
            endpoint_object_id: *endpoint.endpoint_object.object_id(),
            endpoint_object_version: endpoint.endpoint_object.version(),
            endpoint_object_digest: endpoint.endpoint_object.digest().inner().to_vec(),
            shared_objects: endpoint.shared_objects,
            requirements: endpoint.requirements,
            config_digest: endpoint.config_digest,
            active_for_new_executions: active,
        }
    }

    fn registry() -> TapRegistry {
        let agent = sui::types::Address::from_static("0xa");
        let skill_id = 11;
        let requirements = TapSkillRequirements {
            input_schema_commitment: vec![2],
            workflow_commitment: vec![3],
            metadata_commitment: vec![4],
            payment_policy: TapPaymentPolicy::default(),
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        };

        TapRegistry {
            id: sui::types::Address::from_static("0xf"),
            agents: vec![TapAgentRecord {
                agent_id: agent,
                owner: sui::types::Address::from_static("0x1"),
                operator: sui::types::Address::from_static("0x2"),
                active: true,
                next_skill_index: 1,
                skills: MoveTable::new(sui::types::Address::from_static("0x90"), 1),
                endpoints: MoveTable::new(sui::types::Address::from_static("0x91"), 1),
                active_endpoints: vec![TapEndpointActivation {
                    agent_id: agent,
                    skill_id,
                    interface_revision: InterfaceRevision(2),
                }],
            }],
            skills: vec![TapSkillRecord {
                agent_id: agent,
                skill_id,
                dag_id: sui::types::Address::from_static("0x3"),
                dag_binding: TapDagBinding::pinned(sui::types::Address::from_static("0x3")),
                tap_package_id: sui::types::Address::from_static("0xc"),
                workflow_commitment: requirements.workflow_commitment.clone(),
                requirements_commitment: requirements.input_schema_commitment.clone(),
                metadata_commitment: requirements.metadata_commitment.clone(),
                payment_policy: requirements.payment_policy.clone(),
                schedule_policy: requirements.schedule_policy.clone(),
                capability_schema_commitment: vec![5],
                active: true,
            }],
            endpoints: vec![
                endpoint_revision(1, true),
                TapEndpointRevision {
                    requirements,
                    ..endpoint_revision(2, false)
                },
            ],
            active_endpoints: vec![TapEndpointActivation {
                agent_id: agent,
                skill_id,
                interface_revision: InterfaceRevision(2),
            }],
            default_target: Some(TapDefaultExecutionTarget {
                agent_id: agent,
                skill_id,
            }),
        }
    }

    #[derive(Clone)]
    struct RegistryObjectMock {
        registry_object: TapRegistryObject,
        agent_field_ref: sui::types::ObjectReference,
        skill_field_ref: sui::types::ObjectReference,
        endpoint_field_ref: sui::types::ObjectReference,
        agent_record: TapAgentRecord,
        skill_record: TapSkillRecord,
        endpoint_record: TapEndpointRevision,
    }

    fn registry_object_mock(registry: &TapRegistry) -> RegistryObjectMock {
        assert_eq!(registry.agents.len(), 1, "test registry has one agent");
        assert_eq!(registry.skills.len(), 1, "test registry has one skill");
        assert!(
            !registry.endpoints.is_empty(),
            "test registry has at least one endpoint"
        );

        let agent = registry.agents[0].clone();
        let skill_record = registry.skills[0].clone();
        let endpoint_record = registry
            .active_endpoints
            .iter()
            .find_map(|active| {
                registry.endpoints.iter().find(|endpoint| {
                    endpoint.agent_id == active.agent_id
                        && endpoint.skill_id == active.skill_id
                        && endpoint.interface_revision == active.interface_revision
                })
            })
            .or_else(|| registry.endpoints.first())
            .expect("endpoint selected")
            .clone();
        let agent_field_ref = sui_mocks::mock_sui_object_ref();
        let skill_field_ref = sui_mocks::mock_sui_object_ref();
        let endpoint_field_ref = sui_mocks::mock_sui_object_ref();

        RegistryObjectMock {
            registry_object: TapRegistryObject {
                id: registry.id,
                agents: MoveTable::new(sui::types::Address::from_static("0x9000"), 1),
                default_target: registry.default_target.into(),
            },
            agent_field_ref,
            skill_field_ref,
            endpoint_field_ref,
            agent_record: agent,
            skill_record,
            endpoint_record,
        }
    }

    fn mock_fetch_registry_from_tables(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &TapRegistry,
    ) {
        let mock = registry_object_mock(registry);
        sui_mocks::grpc::mock_get_object_bcs_for(
            ledger_service_mock,
            registry_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&mock.registry_object).expect("raw registry bcs"),
            sui::types::StructTag::new(
                nexus_objects.registry_pkg_id(),
                crate::idents::tap::STANDARD_TAP_MODULE,
                sui::types::Identifier::from_static("TapRegistry"),
                vec![],
            ),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                mock.agent_record.agent_id,
                mock.agent_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.agent_field_ref,
                sui::types::Owner::Shared(1),
                mock.agent_record.agent_id,
                mock.agent_record,
            )],
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                mock.skill_record.skill_id,
                mock.skill_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.skill_field_ref,
                sui::types::Owner::Shared(1),
                mock.skill_record.skill_id,
                mock.skill_record,
            )],
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                TapEndpointRevisionKey::new(
                    mock.endpoint_record.skill_id,
                    mock.endpoint_record.interface_revision,
                ),
                mock.endpoint_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.endpoint_field_ref,
                sui::types::Owner::Shared(1),
                TapEndpointRevisionKey::new(
                    mock.endpoint_record.skill_id,
                    mock.endpoint_record.interface_revision,
                ),
                mock.endpoint_record,
            )],
        );
    }

    #[test]
    fn resolve_active_endpoint_record_reuses_sdk_fail_closed_rule() {
        let records = vec![endpoint(false), endpoint(true)];
        let resolved =
            resolve_active_endpoint_record(&records, sui::types::Address::from_static("0xa"), 11)
                .expect("one active endpoint");

        assert!(resolved.active_for_new_executions);
    }

    #[test]
    fn registry_active_resolution_uses_activation_vector() {
        let registry = registry();
        let records = registry.endpoint_records().expect("endpoint records");

        assert_eq!(records.len(), 2);
        assert!(!records[0].active_for_new_executions);
        assert!(records[1].active_for_new_executions);

        let resolved = registry
            .active_endpoint_record(sui::types::Address::from_static("0xa"), 11)
            .expect("active endpoint");

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(2));
    }

    #[test]
    fn active_skill_execution_target_reuses_sdk_registry_resolution() {
        let registry = registry();
        let target = resolve_active_tap_skill_execution_target(
            &registry,
            sui::types::Address::from_static("0xa"),
            11,
        )
        .expect("active skill target");

        assert_eq!(target.skill.dag_id, sui::types::Address::from_static("0x3"));
        assert_eq!(
            target.skill.dag_binding,
            TapDagBinding::pinned(sui::types::Address::from_static("0x3"))
        );
        assert_eq!(target.endpoint.key.interface_revision, InterfaceRevision(2));
    }

    #[test]
    fn configured_default_target_reads_nexus_objects_metadata() {
        let objects = NexusObjects {
            default_tap_target: Some(TapDefaultExecutionTarget {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            }),
            ..crate::test_utils::sui_mocks::mock_nexus_objects()
        };

        assert_eq!(
            configured_default_tap_target(&objects).expect("configured default target"),
            TapDefaultExecutionTarget {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            }
        );
    }

    fn wrapped_event(
        objects: &NexusObjects,
        package: sui::types::Address,
        module: &'static str,
        name: &'static str,
        bytes: Vec<u8>,
    ) -> sui::types::Event {
        let inner = sui::types::StructTag::new(
            package,
            sui::types::Identifier::from_static(module),
            sui::types::Identifier::from_static(name),
            vec![],
        );

        sui_mocks::mock_sui_event(
            objects.primitives_pkg_id,
            sui::types::StructTag::new(
                objects.primitives_pkg_id,
                primitives::Event::EVENT_WRAPPER.module,
                primitives::Event::EVENT_WRAPPER.name,
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            bytes,
        )
    }

    fn standard_endpoint_object(
        objects: &NexusObjects,
        endpoint_ref: &sui::types::ObjectReference,
    ) -> sui::types::Object {
        sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        objects.registry_pkg_id(),
                        TapStandard::CREATE_STANDARD_ENDPOINT.module,
                        sui::types::Identifier::from_static("StandardEndpoint"),
                        vec![],
                    ),
                    true,
                    endpoint_ref.version(),
                    endpoint_ref.object_id().as_bytes().to_vec(),
                )
                .expect("endpoint object contents include id"),
            ),
            sui::types::Owner::Shared(endpoint_ref.version()),
            *endpoint_ref.digest(),
            0,
        )
    }

    fn artifact_with_endpoint(endpoint_ref: &sui::types::ObjectReference) -> TapPublishArtifact {
        let config = TapSkillConfig {
            name: "weather skill".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: std::path::PathBuf::from("dag.json"),
            tap_package_path: std::path::PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_commitment: vec![1],
                workflow_commitment: vec![2],
                metadata_commitment: vec![3],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            shared_objects: vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0xe"),
            )],
            interface_revision: InterfaceRevision(1),
            active_for_new_executions: true,
        };

        TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xc"),
        )
        .expect("artifact")
        .with_endpoint_object(endpoint_ref.clone())
        .expect("endpoint artifact")
    }

    #[tokio::test]
    async fn tap_actions_create_agent_extracts_agent_created_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            1,
            sui::types::Digest::generate(&mut rng),
        );
        let agent_object = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.interface_pkg_id,
                        crate::idents::tap::STANDARD_TAP_MODULE,
                        sui::types::Identifier::from_static("Agent"),
                        vec![],
                    ),
                    true,
                    agent_ref.version(),
                    agent_ref.object_id().as_bytes().to_vec(),
                )
                .expect("agent object contents include id"),
            ),
            sui::types::Owner::Shared(agent_ref.version()),
            digest,
            0,
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![agent_object],
            vec![],
            vec![wrapped_event(
                &nexus_objects,
                nexus_objects.interface_pkg_id,
                "tap",
                "AgentCreatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::AgentCreatedEvent {
                        agent_id: sui::types::Address::from_static("0xa"),
                        vault_id: sui::types::Address::from_static("0xb"),
                        owner: sui::types::Address::from_static("0x1"),
                        operator: sui::types::Address::from_static("0x2"),
                    },
                })
                .unwrap(),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tap()
            .create_agent(sui::types::Address::from_static("0x2"))
            .await
            .expect("create agent succeeds");

        assert_eq!(result.agent_id, sui::types::Address::from_static("0xa"));
        assert_eq!(
            result.agent_object.object_id(),
            &sui::types::Address::from_static("0xa")
        );
        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.tx_checkpoint, 1);
    }

    #[tokio::test]
    async fn tap_actions_create_standard_endpoint_extracts_shared_endpoint_object() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let endpoint_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xf"),
            7,
            sui::types::Digest::generate(&mut rng),
        );
        let endpoint_object = standard_endpoint_object(&nexus_objects, &endpoint_ref);
        let expected_endpoint_ref = sui::types::ObjectReference::new(
            endpoint_object.object_id(),
            endpoint_object.version(),
            endpoint_object.digest(),
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![endpoint_object],
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
            .tap()
            .create_standard_endpoint(sui::types::Address::from_static("0xc"))
            .await
            .expect("endpoint created");

        assert_eq!(result.endpoint_object, expected_endpoint_ref);
        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.tx_checkpoint, 1);
    }

    #[tokio::test]
    async fn tap_actions_register_skill_reads_metadata_and_extracts_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let endpoint_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xf"),
            7,
            sui::types::Digest::generate(&mut rng),
        );
        let artifact = artifact_with_endpoint(&endpoint_ref);
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            endpoint_ref.clone(),
            sui::types::Owner::Shared(endpoint_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref.clone(),
            sui::types::Owner::Shared(agent_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![],
            vec![],
            vec![wrapped_event(
                &nexus_objects,
                nexus_objects.interface_pkg_id,
                "tap",
                "SkillRegisteredEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::SkillRegisteredEvent {
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        dag_id: artifact.dag_id,
                        dag_binding: TapDagBinding::pinned(artifact.dag_id),
                        tap_package_id: artifact.tap_package_id,
                        workflow_commitment: artifact.requirements.workflow_commitment.clone(),
                        requirements_commitment: artifact
                            .requirements
                            .input_schema_commitment
                            .clone(),
                        capability_schema_commitment: artifact
                            .requirements
                            .vertex_authorization_schema
                            .schema_commitment
                            .clone(),
                    },
                })
                .unwrap(),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tap()
            .register_skill(*agent_ref.object_id(), &artifact, None)
            .await
            .expect("register skill succeeds");

        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_announce_endpoint_revision_reads_metadata_and_extracts_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let endpoint_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xf"),
            7,
            sui::types::Digest::generate(&mut rng),
        );
        let artifact = artifact_with_endpoint(&endpoint_ref);
        let config_digest = artifact
            .endpoint_config_digest(*endpoint_ref.object_id())
            .expect("endpoint digest");
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            endpoint_ref.clone(),
            sui::types::Owner::Shared(endpoint_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref.clone(),
            sui::types::Owner::Shared(agent_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![],
            vec![],
            vec![wrapped_event(
                &nexus_objects,
                nexus_objects.interface_pkg_id,
                "tap",
                "EndpointRevisionAnnouncedEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::EndpointRevisionAnnouncedEvent {
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        interface_revision: artifact.interface_revision,
                        package_id: artifact.tap_package_id,
                        endpoint_object_id: *endpoint_ref.object_id(),
                        endpoint_object_version: endpoint_ref.version(),
                        endpoint_object_digest: endpoint_ref.digest().inner().to_vec(),
                        shared_objects: artifact.shared_objects.clone(),
                        requirements: artifact.requirements.clone(),
                        config_digest: config_digest.clone(),
                        active_for_new_executions: true,
                    },
                })
                .unwrap(),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tap()
            .announce_endpoint_revision(*agent_ref.object_id(), 11, &artifact, None, true)
            .await
            .expect("announce succeeds");

        assert_eq!(result.endpoint_key.agent_id, *agent_ref.object_id());
        assert_eq!(result.endpoint_key.skill_id, 11);
        assert_eq!(result.endpoint_object, endpoint_ref);
        assert_eq!(result.config_digest, config_digest);
    }

    #[tokio::test]
    async fn tap_actions_schedule_skill_execution_extracts_created_task_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let scheduled_task_id = sui::types::Address::from_static("0x77");
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref.clone(),
            sui::types::Owner::Shared(agent_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![],
            vec![],
            vec![wrapped_event(
                &nexus_objects,
                nexus_objects.interface_pkg_id,
                "tap",
                "ScheduledSkillExecutionCreatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::ScheduledSkillExecutionCreatedEvent {
                        scheduled_task_id,
                        scheduler_task_id: sui::types::Address::ZERO,
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        long_term_gas_coin_id: sui::types::Address::from_static("0xc"),
                        schedule_entries_commitment: vec![4],
                        first_after_ms: 10,
                        max_occurrences: 1,
                        source_kind: crate::types::TapPaymentSourceKind::Invoker,
                        source_identity: sui::types::Address::from_static("0xc"),
                        prepaid_amount: 0,
                        occurrence_budget: 0,
                        refund_mode: 0,
                    },
                })
                .unwrap(),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tap()
            .schedule_skill_execution(
                *agent_ref.object_id(),
                11,
                sui::types::Address::from_static("0xc"),
                vec![1],
                vec![2],
                Some(vec![3]),
                TapSchedulePolicy::default(),
                vec![4],
                10,
            )
            .await
            .expect("schedule succeeds");

        assert_eq!(result.scheduled_task_id, scheduled_task_id);
        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
    }

    #[tokio::test]
    async fn tap_actions_schedule_skill_execution_address_funded_extracts_created_task_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let scheduler_task_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0x66"),
            5,
            sui::types::Digest::generate(&mut rng),
        );
        let scheduled_task_id = sui::types::Address::from_static("0x77");
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref.clone(),
            sui::types::Owner::Shared(agent_ref.version()),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![],
            vec![],
            vec![wrapped_event(
                &nexus_objects,
                nexus_objects.interface_pkg_id,
                "tap",
                "ScheduledSkillExecutionCreatedEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::ScheduledSkillExecutionCreatedEvent {
                        scheduled_task_id,
                        scheduler_task_id: *scheduler_task_ref.object_id(),
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        long_term_gas_coin_id: sui::types::Address::from_static("0xc"),
                        schedule_entries_commitment: vec![4],
                        first_after_ms: 10,
                        max_occurrences: 1,
                        source_kind: crate::types::TapPaymentSourceKind::Invoker,
                        source_identity: sui::types::Address::from_static("0xc"),
                        prepaid_amount: 25,
                        occurrence_budget: 5,
                        refund_mode: 0,
                    },
                })
                .unwrap(),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tap()
            .schedule_skill_execution_address_funded(ScheduleSkillExecutionAddressFundedParams {
                scheduler_task: scheduler_task_ref,
                agent_id: *agent_ref.object_id(),
                skill_id: 11,
                input_commitment: vec![1],
                prepay_amount: 25,
                refund_recipient: None,
                payment_source: vec![2],
                occurrence_budget: 5,
                refund_mode: 0,
                authorization_plan_commitment: Some(vec![3]),
                schedule_policy: TapSchedulePolicy::default(),
                refill_policy_commitment: vec![4],
                schedule_entries_commitment: vec![5],
                first_after_ms: 10,
            })
            .await
            .expect("durable schedule succeeds");

        assert_eq!(result.scheduled_task_id, scheduled_task_id);
        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_get_skill_requirements_resolves_active_endpoint() {
        let registry = registry();
        let registry_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = NexusObjects {
            tap_registry: Some(registry_ref.clone()),
            ..sui_mocks::mock_nexus_objects()
        };

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        mock_fetch_registry_from_tables(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &nexus_objects,
            registry_ref,
            &registry,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .tap()
            .get_skill_requirements(sui::types::Address::from_static("0xa"), 11)
            .await
            .expect("requirements fetch succeeds");

        assert_eq!(
            result.active_endpoint_key.interface_revision,
            InterfaceRevision(2)
        );
        assert_eq!(result.requirements.workflow_commitment, vec![3]);
    }
}
