//! Read-only helpers and high-level actions for standard TAP.

#[cfg(feature = "move_publish")]
use crate::types::TapSkillConfig;
use {
    crate::{
        events::NexusEventKind,
        idents::sui_framework,
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
            resolve_default_tap_dag_executor,
            AgentId,
            DefaultDagExecutor,
            DefaultDagExecutorFieldKey,
            DefaultDagExecutorRecord,
            DefaultDagExecutorValue,
            InterfaceRevision,
            NexusObjects,
            RuntimeVertex,
            SkillId,
            TapActiveSkillExecutionTarget,
            TapAgentPaymentVault,
            TapAgentVaultFieldKey,
            TapConfigDigestInput,
            TapEndpointKey,
            TapEndpointRecord,
            TapEndpointResolutionError,
            TapExecutionPayment,
            TapExecutionPaymentFinalState,
            TapExecutionPaymentHistoryFieldKey,
            TapExecutionPaymentHistoryList,
            TapExecutionPaymentReceipt,
            TapExecutionPaymentReceiptFieldKey,
            TapPublishArtifact,
            TapRegistry,
            TapRegistryObject,
            TapSchedulePolicy,
            TapScheduledAuthorizationGrantTemplate,
            TapScheduledSkillTask,
            TapSkillRecord,
            TapSkillRequirements,
            WorkflowVertexAuthorizationGrant,
            WorkflowVertexAuthorizationGrantAccess,
            WorkflowVertexAuthorizationGrantFieldKey,
        },
    },
    std::time::{Duration, Instant},
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
    pub prepay_amount: u64,
    pub refund_recipient: Option<sui::types::Address>,
    pub payment_source: Vec<u8>,
    pub occurrence_budget: u64,
    pub refund_mode: u8,
    pub schedule_policy: TapSchedulePolicy,
    pub refill_policy_commitment: Vec<u8>,
    pub schedule_entries_commitment: Vec<u8>,
    pub first_after_ms: u64,
    pub grant_templates: Vec<TapScheduledAuthorizationGrantTemplate>,
}

/// Parameters for creating an agent-vault-funded TAP schedule and linking it
/// to an existing workflow scheduler task.
#[derive(Clone, Debug)]
pub struct ScheduleSkillExecutionFromAgentVaultParams {
    pub scheduler_task: sui::types::ObjectReference,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub prepay_amount: u64,
    pub occurrence_budget: u64,
    pub refund_mode: u8,
    pub schedule_policy: TapSchedulePolicy,
    pub refill_policy_commitment: Vec<u8>,
    pub schedule_entries_commitment: Vec<u8>,
    pub first_after_ms: u64,
    pub grant_templates: Vec<TapScheduledAuthorizationGrantTemplate>,
}

/// Parameters for creating a durable address-funded TAP schedule for the
/// registry-owned default DAG executor and linking it to a scheduler task.
#[derive(Clone, Debug)]
pub struct ScheduleDefaultDagExecutorSkillExecutionAddressFundedParams {
    pub scheduler_task: sui::types::ObjectReference,
    pub prepay_amount: u64,
    pub refund_recipient: Option<sui::types::Address>,
    pub payment_source: Vec<u8>,
    pub occurrence_budget: u64,
    pub refund_mode: u8,
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

/// Result returned by the composed TAP skill publishing workflow.
#[cfg(feature = "move_publish")]
#[derive(Clone, Debug)]
pub struct PublishSkillResult {
    pub tap_package: TapPackagePublishResult,
    pub dag: crate::nexus::workflow::PublishResult,
    pub artifact: TapPublishArtifact,
}

/// Endpoint object metadata. In the current TAP model, endpoint revisions
/// live on the agent registry keyed by (agent_id, skill_id, interface_revision)
/// without a back-reference to the standalone `StandardEndpoint` object, so
/// this struct surfaces the on-chain object ref alone. Use
/// `nexus tap registry show` to inspect revisions and active endpoints.
#[derive(Clone, Debug)]
pub struct EndpointInspection {
    /// On-chain object ref of the endpoint itself.
    pub object_ref: sui::types::ObjectReference,
}

/// Inputs to [`TapActions::bind_agent_skill`].
#[derive(Clone, Debug)]
pub struct BindAgentSkillParams {
    pub operator: sui::types::Address,
    pub artifact: TapPublishArtifact,
}

/// Result returned by [`TapActions::bind_agent_skill`].
#[derive(Clone, Debug)]
pub struct BindAgentSkillResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub agent_id: AgentId,
    pub agent_object: sui::types::ObjectReference,
    pub skill_id: SkillId,
    pub config_digest: Vec<u8>,
    pub config_digest_input: TapConfigDigestInput,
}

/// Result returned by [`TapActions::wait_for_payment_settled`].
#[derive(Clone, Debug)]
pub struct WaitForPaymentResult {
    pub payment: TapExecutionPayment,
    pub terminal: bool,
    pub elapsed_ms: u64,
    pub timed_out: bool,
}

/// Parameters for [`TapActions::deposit_agent_payment_vault`].
#[derive(Clone, Debug)]
pub struct DepositAgentVaultParams {
    pub agent_id: AgentId,
    pub amount: u64,
}

/// Result returned by [`TapActions::deposit_agent_payment_vault`].
#[derive(Clone, Debug)]
pub struct DepositAgentVaultResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub agent_id: AgentId,
    pub amount: u64,
}

/// Whether a [`TapExecutionPayment`] has reached an irrecoverable final state.
pub fn payment_is_terminal(payment: &TapExecutionPayment) -> bool {
    if payment.accomplished || payment.refunded {
        return true;
    }
    matches!(
        payment.final_state,
        Some(TapExecutionPaymentFinalState::Accomplished)
            | Some(TapExecutionPaymentFinalState::Refunded)
    )
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
        let artifact =
            TapPublishArtifact::from_config(config, dag.dag_object_id, tap_package.package_id)
                .map_err(NexusError::TransactionBuilding)?;

        Ok(PublishSkillResult {
            tap_package,
            dag,
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
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
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
    ) -> Result<RegisterSkillResult, NexusError> {
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
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let config_digest = artifact
            .endpoint_config_digest()
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            true,
        ));

        if artifact
            .requirements
            .vertex_authorization_schema
            .is_default()
        {
            tap_tx::register_skill(
                &mut tx,
                nexus_objects,
                registry,
                agent,
                artifact.dag_id,
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
                artifact.shared_objects.clone(),
                config_digest,
            )
            .map_err(NexusError::TransactionBuilding)?;
        } else {
            tap_tx::register_skill_with_vertex_authorization_schema(
                &mut tx,
                nexus_objects,
                registry,
                agent,
                artifact.dag_id,
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
                &artifact.requirements.vertex_authorization_schema,
                artifact.shared_objects.clone(),
                config_digest,
            )
            .map_err(NexusError::TransactionBuilding)?;
        }

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
    ) -> Result<AnnounceEndpointRevisionResult, NexusError> {
        let config_digest_input = artifact.endpoint_config_digest_input();
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
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            true,
        ));

        tap_tx::announce_endpoint_revision(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            skill_id,
            artifact.interface_revision,
            artifact.shared_objects.clone(),
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
            artifact
                .requirements
                .vertex_authorization_schema
                .schema_commitment
                .clone(),
            config_digest.clone(),
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
            config_digest,
            config_digest_input,
        })
    }

    /// Deposit `amount` MIST into the agent's payment vault, splitting from
    /// the transaction gas coin. The vault is shared so any address can
    /// deposit; withdrawal stays gated on the agent owner or operator.
    pub async fn deposit_agent_payment_vault(
        &self,
        params: DepositAgentVaultParams,
    ) -> Result<DepositAgentVaultResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(params.agent_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            true,
        ));
        let amount_input =
            crate::idents::pure_arg(&params.amount).map_err(NexusError::TransactionBuilding)?;
        let amount_arg = tx.input(amount_input);
        let deposit_coin = tx
            .split_coins(tx.gas(), vec![amount_arg])
            .nested(0)
            .ok_or_else(|| {
                NexusError::TransactionBuilding(anyhow::anyhow!(
                    "failed to split agent vault deposit coin"
                ))
            })?;
        tap_tx::deposit_agent_payment_vault(&mut tx, nexus_objects, agent, deposit_coin);

        let response = self.submit_tap_transaction(tx, address).await?;
        Ok(DepositAgentVaultResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: params.agent_id,
            amount: params.amount,
        })
    }

    /// Schedule a standard TAP skill execution.
    #[allow(clippy::too_many_arguments)]
    pub async fn schedule_skill_execution(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        long_term_gas_coin_id: sui::types::Address,
        refill_policy_commitment: Vec<u8>,
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
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, false)
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
            long_term_gas_coin_id,
            refill_policy_commitment,
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
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, false)
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

        let scheduled_task = tap_tx::schedule_skill_execution_address_funded_with_grants(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            *params.scheduler_task.object_id(),
            params.skill_id,
            prepayment_coin,
            refund_recipient,
            params.payment_source,
            params.occurrence_budget,
            params.refund_mode,
            params.schedule_policy,
            params.refill_policy_commitment,
            params.schedule_entries_commitment,
            params.first_after_ms,
            params.grant_templates,
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

    /// Create an agent-vault-funded durable scheduled TAP task, attach it to
    /// the workflow scheduler task, and share the TAP scheduled task.
    pub async fn schedule_skill_execution_from_agent_vault(
        &self,
        params: ScheduleSkillExecutionFromAgentVaultParams,
    ) -> Result<ScheduleSkillExecutionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(params.agent_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, false)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tx.input(sui::tx::Input::shared(
            *agent_object.object_id(),
            agent_object.version(),
            true,
        ));
        let scheduler_task = tx.input(sui::tx::Input::shared(
            *params.scheduler_task.object_id(),
            params.scheduler_task.version(),
            true,
        ));

        let scheduled_task = tap_tx::schedule_skill_execution_from_agent_vault_with_grants(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            *params.scheduler_task.object_id(),
            params.skill_id,
            params.prepay_amount,
            params.occurrence_budget,
            params.refund_mode,
            params.schedule_policy,
            params.refill_policy_commitment,
            params.schedule_entries_commitment,
            params.first_after_ms,
            params.grant_templates,
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
                "ScheduledSkillExecutionCreatedEvent not found in agent-vault TAP schedule response"
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

    /// Create an address-funded durable scheduled TAP task for the
    /// registry-owned default DAG executor, attach it to the workflow scheduler
    /// task, and share the TAP scheduled task.
    pub async fn schedule_default_dag_executor_skill_execution_address_funded(
        &self,
        params: ScheduleDefaultDagExecutorSkillExecutionAddressFundedParams,
    ) -> Result<ScheduleSkillExecutionResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let refund_recipient = params.refund_recipient.unwrap_or(address);

        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tx.input(sui::tx::Input::shared(
            *nexus_objects.agent_registry.object_id(),
            nexus_objects.agent_registry.version(),
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
                    "failed to split default scheduled TAP prepayment coin"
                ))
            })?;

        let scheduled_task = tap_tx::schedule_default_dag_executor_skill_execution_address_funded(
            &mut tx,
            nexus_objects,
            registry,
            *params.scheduler_task.object_id(),
            prepayment_coin,
            refund_recipient,
            params.payment_source,
            params.occurrence_budget,
            params.refund_mode,
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
                "ScheduledSkillExecutionCreatedEvent not found in default TAP schedule response"
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

    /// Read the on-chain metadata of an endpoint object and return its object
    /// ref, so callers do not need to walk raw Sui object internals. In the
    /// current TAP model endpoint revisions live on the agent registry keyed by
    /// (agent_id, skill_id, interface_revision) without a back-reference to the
    /// standalone object, so use `nexus tap registry show` to inspect revisions
    /// and active endpoints. See [`EndpointInspection`].
    pub async fn inspect_endpoint(
        &self,
        endpoint_object_id: sui::types::Address,
    ) -> Result<EndpointInspection, NexusError> {
        let object_meta = self
            .client
            .crawler()
            .get_object_metadata(endpoint_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        Ok(EndpointInspection {
            object_ref: object_meta.object_ref(),
        })
    }

    /// Create a standard Talus agent and register its first skill atomically.
    pub async fn bind_agent_skill(
        &self,
        params: BindAgentSkillParams,
    ) -> Result<BindAgentSkillResult, NexusError> {
        let BindAgentSkillParams { operator, artifact } = params;

        let config_digest_input = artifact.endpoint_config_digest_input();
        let config_digest = config_digest_input
            .digest()
            .map_err(NexusError::TransactionBuilding)?;

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tap_tx::create_agent(&mut tx, nexus_objects, registry, operator)
            .map_err(NexusError::TransactionBuilding)?;

        if artifact
            .requirements
            .vertex_authorization_schema
            .is_default()
        {
            tap_tx::register_skill(
                &mut tx,
                nexus_objects,
                registry,
                agent,
                artifact.dag_id,
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
                artifact.shared_objects.clone(),
                config_digest.clone(),
            )
            .map_err(NexusError::TransactionBuilding)?;
        } else {
            tap_tx::register_skill_with_vertex_authorization_schema(
                &mut tx,
                nexus_objects,
                registry,
                agent,
                artifact.dag_id,
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
                &artifact.requirements.vertex_authorization_schema,
                artifact.shared_objects.clone(),
                config_digest.clone(),
            )
            .map_err(NexusError::TransactionBuilding)?;
        }

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

        let agent_event = find_event(&response, |kind| match kind {
            NexusEventKind::AgentCreated(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "AgentCreatedEvent not found in TAP bind response"
            ))
        })?;
        let skill_event = find_event(&response, |kind| match kind {
            NexusEventKind::SkillRegistered(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "SkillRegisteredEvent not found in TAP bind response"
            ))
        })?;

        let agent_object = response
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
                    "Standard Talus Agent object not found in TAP bind response"
                ))
            })?;

        Ok(BindAgentSkillResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: agent_event.agent_id,
            agent_object,
            skill_id: skill_event.skill_id,
            config_digest,
            config_digest_input,
        })
    }

    /// Poll a [`TapExecutionPayment`] until it reaches a terminal state
    /// (accomplished, refunded, or a non-pending [`TapExecutionPaymentFinalState`])
    /// or `timeout` elapses.
    ///
    /// `poll_interval` must be non-zero: a zero interval would turn the loop
    /// into a busy-wait that hammers the RPC endpoint and pins a CPU, so it is
    /// rejected up front with [`NexusError::Configuration`] rather than trusted
    /// to the caller.
    pub async fn wait_for_payment_settled(
        &self,
        payment_id: sui::types::Address,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<WaitForPaymentResult, NexusError> {
        if poll_interval.is_zero() {
            return Err(NexusError::Configuration(
                "poll_interval must be greater than zero".to_string(),
            ));
        }

        let crawler = self.client.crawler();
        let started_at = Instant::now();

        loop {
            let payment = fetch_tap_execution_payment(crawler, payment_id)
                .await
                .map_err(NexusError::Rpc)?
                .data;

            let terminal = payment_is_terminal(&payment);
            let elapsed_ms = started_at.elapsed().as_millis().min(u64::MAX as u128) as u64;

            if terminal {
                return Ok(WaitForPaymentResult {
                    payment,
                    terminal: true,
                    elapsed_ms,
                    timed_out: false,
                });
            }

            if started_at.elapsed() >= timeout {
                return Ok(WaitForPaymentResult {
                    payment,
                    terminal: false,
                    elapsed_ms,
                    timed_out: true,
                });
            }

            tokio::time::sleep(poll_interval).await;
        }
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
pub async fn fetch_agent_registry(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Response<TapRegistry>> {
    let mut registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    registry.data.default_executor = fetch_default_dag_executor(crawler, registry.data.id).await?;

    Ok(registry)
}

async fn fetch_agent_registry_tables(
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
            default_executor: None,
        },
    })
}

async fn fetch_default_dag_executor(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Option<DefaultDagExecutor>> {
    let default_executor = match crawler
        .get_dynamic_fields_bcs::<DefaultDagExecutorFieldKey, DefaultDagExecutorValue>(
            registry_id,
            0,
        )
        .await
    {
        Ok(mut fields) => fields
            .remove(&DefaultDagExecutorFieldKey {})
            .map(|value| value.target()),
        Err(key_error) => {
            let values = match crawler
                .get_dynamic_field_values_bcs::<DefaultDagExecutorValue>(registry_id)
                .await
            {
                Ok(values) => values
                    .into_iter()
                    .map(|(_value_type, value)| value)
                    .collect::<Vec<_>>(),
                Err(value_error) => crawler
                    .get_dynamic_field_object_values_bcs::<
                        DefaultDagExecutorFieldKey,
                        DefaultDagExecutorValue,
                    >(registry_id)
                    .await
                    .map_err(|object_error| {
                        anyhow::anyhow!(
                            "Could not fetch default DAG executor dynamic field for AgentRegistry {}: key decode failed: {key_error}; value decode failed: {value_error}; field object decode failed: {object_error}",
                            registry_id
                        )
                    })?,
            };
            if values.len() > 1 {
                anyhow::bail!(
                    "TapRegistry {} has multiple default DAG executor dynamic fields",
                    registry_id
                );
            }
            values.into_iter().next().map(|value| value.target())
        }
    };

    Ok(default_executor)
}

/// Fetch a pinned TAP endpoint from the real `AgentRegistry` vector layout.
pub async fn fetch_tap_endpoint(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    let record = registry.data.endpoint_record(TapEndpointKey {
        agent_id,
        skill_id,
        interface_revision,
    })?;

    Ok(registry_response_with_data(registry, record))
}

/// Resolve a fresh execution endpoint through the active revision stored on the skill.
pub async fn fetch_active_tap_endpoint(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    let record = registry.data.active_endpoint_record(agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, record))
}

/// Fetch the shared TAP registry named by `NexusObjects`.
pub async fn fetch_configured_agent_registry(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<TapRegistry>> {
    fetch_agent_registry(crawler, *objects.agent_registry.object_id()).await
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
        *objects.agent_registry.object_id(),
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
    let registry = fetch_configured_agent_registry(crawler, objects).await?;
    let target = resolve_active_tap_skill_execution_target(&registry.data, agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, target))
}

/// Resolve the configured default TAP DAG executor from the configured registry.
pub async fn fetch_configured_default_tap_dag_executor(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<DefaultDagExecutorRecord>> {
    let registry = fetch_configured_agent_registry(crawler, objects).await?;
    let target = resolve_default_tap_dag_executor(&registry.data)?;

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
    let key = WorkflowVertexAuthorizationGrantFieldKey { vertex };
    let mut matches = crawler
        .get_dynamic_object_field_refs_matching_key::<WorkflowVertexAuthorizationGrantFieldKey>(
            execution_id,
        )
        .await?;
    let field = matches
        .drain(..)
        .find(|field| field.name == key)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "workflow vertex authorization grant not found for execution '{execution_id}'"
            )
        })?;

    crawler
        .get_object::<WorkflowVertexAuthorizationGrant>(field.child_id)
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
pub fn resolve_active_endpoint_record<'a>(
    records: &'a [TapEndpointRecord],
    skills: &[TapSkillRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a TapEndpointRecord, TapEndpointResolutionError> {
    resolve_active_tap_endpoint(records, skills, agent_id, skill_id)
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
            idents::{primitives, registry::AgentRegistry},
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                DefaultDagExecutor,
                InterfaceRevision,
                MoveTable,
                NexusObjects,
                TapAgentRecord,
                TapDagBinding,
                TapEndpointKey,
                TapEndpointRevision,
                TapEndpointRevisionKey,
                TapPaymentPolicy,
                TapRegistryObject,
                TapSchedulePolicy,
                TapSharedObjectRef,
                TapSkillConfig,
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

    fn endpoint(revision: u64) -> TapEndpointRecord {
        TapEndpointRecord {
            key: TapEndpointKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(revision),
            },
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
        }
    }

    fn endpoint_revision(revision: u64) -> TapEndpointRevision {
        let endpoint = endpoint(revision);
        TapEndpointRevision {
            agent_id: endpoint.key.agent_id,
            skill_id: endpoint.key.skill_id,
            interface_revision: InterfaceRevision(revision),
            shared_objects: endpoint.shared_objects,
            requirements: endpoint.requirements,
            config_digest: endpoint.config_digest,
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
            }],
            skills: vec![TapSkillRecord {
                agent_id: agent,
                skill_id,
                dag_id: sui::types::Address::from_static("0x3"),
                dag_binding: TapDagBinding::pinned(sui::types::Address::from_static("0x3")),
                workflow_commitment: requirements.workflow_commitment.clone(),
                requirements_commitment: requirements.input_schema_commitment.clone(),
                metadata_commitment: requirements.metadata_commitment.clone(),
                payment_policy: requirements.payment_policy.clone(),
                schedule_policy: requirements.schedule_policy.clone(),
                capability_schema_commitment: vec![5],
                active_interface_revision: InterfaceRevision(2),
                active: true,
            }],
            endpoints: vec![
                endpoint_revision(1),
                TapEndpointRevision {
                    requirements,
                    ..endpoint_revision(2)
                },
            ],
            default_executor: Some(DefaultDagExecutor {
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
        default_executor_field_ref: Option<sui::types::ObjectReference>,
        default_executor_value: Option<DefaultDagExecutorValue>,
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
            .active_endpoint_record(skill_record.agent_id, skill_record.skill_id)
            .ok()
            .and_then(|active| {
                registry.endpoints.iter().find(|endpoint| {
                    endpoint.agent_id == active.key.agent_id
                        && endpoint.skill_id == active.key.skill_id
                        && endpoint.interface_revision == active.key.interface_revision
                })
            })
            .or_else(|| registry.endpoints.first())
            .expect("endpoint selected")
            .clone();
        let agent_field_ref = sui_mocks::mock_sui_object_ref();
        let skill_field_ref = sui_mocks::mock_sui_object_ref();
        let endpoint_field_ref = sui_mocks::mock_sui_object_ref();
        let default_executor_field_ref = registry
            .default_executor
            .map(|_| sui_mocks::mock_sui_object_ref());
        let default_executor_value =
            registry
                .default_executor
                .map(|default_executor| DefaultDagExecutorValue {
                    agent: crate::types::TapAgentObject {
                        id: default_executor.agent_id,
                        next_skill_index: agent.next_skill_index,
                        owner: agent.owner,
                        registry_id: Some(registry.id).into(),
                    },
                    skill_id: default_executor.skill_id,
                });

        RegistryObjectMock {
            registry_object: TapRegistryObject {
                id: registry.id,
                agents: MoveTable::new(sui::types::Address::from_static("0x9000"), 1),
            },
            agent_field_ref,
            skill_field_ref,
            endpoint_field_ref,
            default_executor_field_ref,
            default_executor_value,
            agent_record: agent,
            skill_record,
            endpoint_record,
        }
    }

    fn mock_fetch_registry_table_data(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &TapRegistry,
    ) -> RegistryObjectMock {
        let mock = registry_object_mock(registry);
        sui_mocks::grpc::mock_get_object_bcs_for(
            ledger_service_mock,
            registry_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&mock.registry_object).expect("raw registry bcs"),
            sui::types::StructTag::new(
                nexus_objects.registry_pkg_id,
                crate::idents::tap::STANDARD_TAP_MODULE,
                sui::types::Identifier::from_static("AgentRegistry"),
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
                mock.agent_field_ref.clone(),
                sui::types::Owner::Shared(1),
                mock.agent_record.agent_id,
                mock.agent_record.clone(),
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
                mock.skill_field_ref.clone(),
                sui::types::Owner::Shared(1),
                mock.skill_record.skill_id,
                mock.skill_record.clone(),
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
                mock.endpoint_field_ref.clone(),
                sui::types::Owner::Shared(1),
                TapEndpointRevisionKey::new(
                    mock.endpoint_record.skill_id,
                    mock.endpoint_record.interface_revision,
                ),
                mock.endpoint_record.clone(),
            )],
        );
        mock
    }

    fn mock_fetch_registry_from_tables(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &TapRegistry,
    ) {
        let mock = mock_fetch_registry_table_data(
            ledger_service_mock,
            state_service_mock,
            nexus_objects,
            registry_ref,
            registry,
        );
        if let (Some(field_ref), Some(value)) =
            (mock.default_executor_field_ref, mock.default_executor_value)
        {
            sui_mocks::grpc::mock_list_dynamic_fields(
                state_service_mock,
                vec![(DefaultDagExecutorFieldKey {}, *field_ref.object_id())],
            );
            sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
                ledger_service_mock,
                vec![(
                    field_ref,
                    sui::types::Owner::Shared(1),
                    DefaultDagExecutorFieldKey {},
                    value,
                )],
            );
        } else {
            sui_mocks::grpc::mock_list_dynamic_fields::<DefaultDagExecutorFieldKey>(
                state_service_mock,
                vec![],
            );
            sui_mocks::grpc::mock_get_dynamic_table_values_bcs::<
                DefaultDagExecutorFieldKey,
                DefaultDagExecutorValue,
            >(ledger_service_mock, vec![]);
        }
    }

    fn mock_fetch_registry_tables_only(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &TapRegistry,
    ) {
        mock_fetch_registry_table_data(
            ledger_service_mock,
            state_service_mock,
            nexus_objects,
            registry_ref,
            registry,
        );
    }

    #[test]
    fn resolve_active_endpoint_record_reuses_sdk_fail_closed_rule() {
        let records = vec![endpoint(1), endpoint(2)];
        let skills = vec![registry().skills[0].clone()];
        let resolved = resolve_active_endpoint_record(
            &records,
            &skills,
            sui::types::Address::from_static("0xa"),
            11,
        )
        .expect("one active endpoint");

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(2));
    }

    #[test]
    fn registry_active_resolution_uses_skill_active_revision() {
        let registry = registry();
        let records = registry.endpoint_records().expect("endpoint records");

        assert_eq!(records.len(), 2);

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
    fn configured_default_executor_reads_nexus_objects_metadata() {
        let objects = NexusObjects {
            default_tap_executor: DefaultDagExecutor {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            },
            ..crate::test_utils::sui_mocks::mock_nexus_objects()
        };

        assert_eq!(
            objects.default_tap_executor,
            DefaultDagExecutor {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            }
        );
    }

    #[tokio::test]
    async fn fetch_tap_endpoint_does_not_decode_default_executor() {
        let registry = registry();
        let registry_ref = sui_mocks::object_ref_for_id(registry.id);
        let nexus_objects = NexusObjects {
            agent_registry: registry_ref.clone(),
            ..sui_mocks::mock_nexus_objects()
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        mock_fetch_registry_tables_only(
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
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(std::sync::Arc::new(tokio::sync::Mutex::new(client)));

        let response = fetch_tap_endpoint(
            &crawler,
            registry.id,
            sui::types::Address::from_static("0xa"),
            11,
            InterfaceRevision(2),
        )
        .await
        .expect("endpoint recovery should not require default executor decoding");

        assert_eq!(response.data.key.interface_revision, InterfaceRevision(2));
    }

    #[tokio::test]
    async fn fetch_agent_registry_still_decodes_default_executor() {
        let registry = registry();
        let registry_ref = sui_mocks::object_ref_for_id(registry.id);
        let nexus_objects = NexusObjects {
            agent_registry: registry_ref.clone(),
            ..sui_mocks::mock_nexus_objects()
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
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
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(std::sync::Arc::new(tokio::sync::Mutex::new(client)));

        let response = fetch_agent_registry(&crawler, registry.id)
            .await
            .expect("full registry recovery decodes the default executor");

        assert_eq!(
            response.data.default_executor,
            Some(DefaultDagExecutor {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            })
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

    fn artifact() -> TapPublishArtifact {
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
        };

        TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xc"),
        )
        .expect("artifact")
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
    async fn tap_actions_register_skill_extracts_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let artifact = artifact();
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
        let expected_registry_pkg_id = nexus_objects.registry_pkg_id;
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint_matching(
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
            move |request| {
                let transaction = request.transaction.as_ref().expect("submitted transaction");
                let transaction = sui::types::Transaction::try_from(transaction)
                    .expect("submitted transaction decodes");
                let sui::types::TransactionKind::ProgrammableTransaction(
                    sui::types::ProgrammableTransaction { commands, .. },
                ) = &transaction.kind
                else {
                    panic!("expected programmable transaction");
                };
                assert!(commands.iter().any(|command| matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == expected_registry_pkg_id
                            && call.function
                                == AgentRegistry::REGISTER_SKILL.name
                )));
            },
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
            .register_skill(*agent_ref.object_id(), &artifact)
            .await
            .expect("register skill succeeds");

        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.tx_digest, digest);
    }

    /// Build an artifact with a non-default vertex authorization schema. This
    /// pushes [`TapVertexAuthorizationSchema::is_default`] to `false` so the
    /// SDK branches into `register_skill_with_vertex_authorization_schema`
    /// instead of the simpler `register_skill`. The shape must include the
    /// 0x0-sentinel fixed tool to keep the digest stable across the
    /// [`TapPublishArtifact::from_config`] substitution path.
    fn cap_gated_artifact() -> TapPublishArtifact {
        let config = TapSkillConfig {
            name: "cap-gated weather".to_string(),
            tap_package_name: "weather_tap".to_string(),
            dag_path: std::path::PathBuf::from("dag.json"),
            tap_package_path: std::path::PathBuf::from("tap"),
            requirements: TapSkillRequirements {
                input_schema_commitment: vec![1],
                workflow_commitment: vec![2],
                metadata_commitment: vec![3],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema {
                    schema_commitment: vec![9],
                    fixed_tools: vec![crate::types::TapAuthorizedTool {
                        package_id: sui::types::Address::ZERO,
                        module: "weather_tap".to_string(),
                        function: "execute".to_string(),
                        operation_commitment: vec![],
                    }],
                    requires_payment: false,
                },
            },
            shared_objects: vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0xe"),
            )],
            interface_revision: InterfaceRevision(1),
        };

        TapPublishArtifact::from_config(
            &config,
            sui::types::Address::from_static("0xd"),
            sui::types::Address::from_static("0xc"),
        )
        .expect("cap-gated artifact")
    }

    #[tokio::test]
    async fn tap_actions_register_skill_routes_through_cap_gated_entrypoint_when_schema_non_default(
    ) {
        // When `TapVertexAuthorizationSchema::is_default` is false the SDK
        // must route through `register_skill_with_vertex_authorization_schema`
        // — the on-chain digest assertion would otherwise reject the artifact
        // because the cap-gated chain entrypoint reconstructs the
        // requirements digest with the full schema baked in. We mock the
        // submitted PTB and assert the move-call shape; the simpler
        // `register_skill` entry must not appear in the tx.
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let artifact = cap_gated_artifact();
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
        let expected_registry_pkg_id = nexus_objects.registry_pkg_id;
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint_matching(
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
            move |request| {
                let transaction = request.transaction.as_ref().expect("submitted transaction");
                let transaction = sui::types::Transaction::try_from(transaction)
                    .expect("submitted transaction decodes");
                let sui::types::TransactionKind::ProgrammableTransaction(
                    sui::types::ProgrammableTransaction { commands, .. },
                ) = &transaction.kind
                else {
                    panic!("expected programmable transaction");
                };
                let mut saw_cap_gated = false;
                let mut saw_plain = false;
                for command in commands {
                    if let sui::types::Command::MoveCall(call) = command {
                        if call.package == expected_registry_pkg_id {
                            if call.function
                                == AgentRegistry::REGISTER_SKILL_WITH_VERTEX_AUTHORIZATION_SCHEMA
                                    .name
                            {
                                saw_cap_gated = true;
                            } else if call.function == AgentRegistry::REGISTER_SKILL.name {
                                saw_plain = true;
                            }
                        }
                    }
                }
                assert!(
                    saw_cap_gated,
                    "cap-gated artifact must route through register_skill_with_vertex_authorization_schema"
                );
                assert!(
                    !saw_plain,
                    "cap-gated artifact must not also call the simpler register_skill"
                );
            },
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
            .register_skill(*agent_ref.object_id(), &artifact)
            .await
            .expect("cap-gated register skill succeeds");

        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_bind_agent_skill_extracts_agent_and_skill_events() {
        // Default-schema happy path for `bind_agent_skill`: a single PTB
        // creates the agent, registers the skill (default branch), and
        // shares the Agent object. The mock returns both AgentCreatedEvent
        // and SkillRegisteredEvent plus the shared Agent object so the
        // helper can recover the agent ref. A regression that drops the
        // share call or events would fail the assertions below.
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let artifact = artifact();

        let agent_addr = sui::types::Address::from_static("0xa");
        let agent_ref =
            sui::types::ObjectReference::new(agent_addr, 1, sui::types::Digest::generate(&mut rng));
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
                .expect("agent object contents"),
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
            vec![
                wrapped_event(
                    &nexus_objects,
                    nexus_objects.interface_pkg_id,
                    "tap",
                    "AgentCreatedEvent",
                    bcs::to_bytes(&Wrapper {
                        event: events::AgentCreatedEvent {
                            agent_id: agent_addr,
                            vault_id: sui::types::Address::from_static("0xb"),
                            owner: sui::types::Address::from_static("0x1"),
                            operator: sui::types::Address::from_static("0x2"),
                        },
                    })
                    .unwrap(),
                ),
                wrapped_event(
                    &nexus_objects,
                    nexus_objects.interface_pkg_id,
                    "tap",
                    "SkillRegisteredEvent",
                    bcs::to_bytes(&Wrapper {
                        event: events::SkillRegisteredEvent {
                            agent_id: agent_addr,
                            skill_id: 17,
                            dag_id: artifact.dag_id,
                            dag_binding: TapDagBinding::pinned(artifact.dag_id),
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
                ),
            ],
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
            .bind_agent_skill(BindAgentSkillParams {
                operator: sui::types::Address::from_static("0x2"),
                artifact: artifact.clone(),
            })
            .await
            .expect("bind agent + skill");

        assert_eq!(result.agent_id, agent_addr);
        assert_eq!(result.skill_id, 17);
        assert_eq!(result.agent_object.object_id(), &agent_addr);
        assert_eq!(result.tx_digest, digest);
        // The bind result carries the digest input used to compute the
        // config_digest, so callers can re-derive it for evidence.
        assert_eq!(
            result.config_digest_input,
            artifact.endpoint_config_digest_input()
        );
        let expected_digest = result
            .config_digest_input
            .digest()
            .expect("recompute config digest");
        assert_eq!(result.config_digest, expected_digest);
    }

    #[tokio::test]
    async fn tap_actions_announce_endpoint_revision_extracts_event() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            3,
            sui::types::Digest::generate(&mut rng),
        );
        let artifact = artifact();
        let config_digest = artifact.endpoint_config_digest().expect("endpoint digest");
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
                nexus_objects.registry_pkg_id,
                "tap",
                "EndpointRevisionAnnouncedEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::EndpointRevisionAnnouncedEvent {
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        interface_revision: artifact.interface_revision,
                        shared_objects: artifact.shared_objects.clone(),
                        requirements: artifact.requirements.clone(),
                        config_digest: config_digest.clone(),
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
            .announce_endpoint_revision(*agent_ref.object_id(), 11, &artifact)
            .await
            .expect("announce succeeds");

        assert_eq!(result.endpoint_key.agent_id, *agent_ref.object_id());
        assert_eq!(result.endpoint_key.skill_id, 11);
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
                prepay_amount: 25,
                refund_recipient: None,
                payment_source: vec![2],
                occurrence_budget: 5,
                refund_mode: 0,
                schedule_policy: TapSchedulePolicy::default(),
                refill_policy_commitment: vec![4],
                schedule_entries_commitment: vec![5],
                first_after_ms: 10,
                grant_templates: vec![TapScheduledAuthorizationGrantTemplate {
                    dag_id: sui::types::Address::from_static("0xda6"),
                    vertex: "demo_delayed_fire_vertex".to_string(),
                    tool_package: sui::types::Address::from_static("0xde0"),
                    tool_module: "demo_delayed_fire_vertex".to_string(),
                    tool_function: "execute".to_string(),
                    operation_commitment: b"demo-tap-delayed-fire".to_vec(),
                    constraints_commitment: Vec::new(),
                }],
            })
            .await
            .expect("durable schedule succeeds");

        assert_eq!(result.scheduled_task_id, scheduled_task_id);
        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_deposit_agent_payment_vault_calls_tap_deposit() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            7,
            sui::types::Digest::generate(&mut rng),
        );
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
        let expected_interface_pkg_id = nexus_objects.interface_pkg_id;
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint_matching(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref,
            vec![],
            vec![],
            vec![],
            move |request| {
                let transaction = request.transaction.as_ref().expect("submitted transaction");
                let transaction = sui::types::Transaction::try_from(transaction)
                    .expect("submitted transaction decodes");
                let sui::types::TransactionKind::ProgrammableTransaction(
                    sui::types::ProgrammableTransaction { commands, .. },
                ) = &transaction.kind
                else {
                    panic!("expected programmable transaction");
                };
                assert!(commands.iter().any(|command| matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == expected_interface_pkg_id
                            && call.function
                                == crate::idents::tap::TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name
                )));
            },
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
            .deposit_agent_payment_vault(DepositAgentVaultParams {
                agent_id: *agent_ref.object_id(),
                amount: 1500,
            })
            .await
            .expect("vault deposit succeeds");

        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.amount, 1500);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_schedule_skill_execution_from_agent_vault_extracts_created_task_event() {
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
        let expected_interface_pkg_id = nexus_objects.interface_pkg_id;
        let expected_registry_pkg_id = nexus_objects.registry_pkg_id;
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint_matching(
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
                        skill_id: 12,
                        long_term_gas_coin_id: *agent_ref.object_id(),
                        schedule_entries_commitment: vec![4],
                        first_after_ms: 10,
                        max_occurrences: 1,
                        source_kind: crate::types::TapPaymentSourceKind::AgentVault,
                        source_identity: *agent_ref.object_id(),
                        prepaid_amount: 25,
                        occurrence_budget: 5,
                        refund_mode: 0,
                    },
                })
                .unwrap(),
            )],
            move |request| {
                let transaction = request.transaction.as_ref().expect("submitted transaction");
                let transaction = sui::types::Transaction::try_from(transaction)
                    .expect("submitted transaction decodes");
                let sui::types::TransactionKind::ProgrammableTransaction(
                    sui::types::ProgrammableTransaction { commands, .. },
                ) = &transaction.kind
                else {
                    panic!("expected programmable transaction");
                };
                assert!(commands.iter().any(|command| matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == expected_interface_pkg_id
                            && call.function
                                == crate::idents::tap::TapStandard::SCHEDULED_AUTHORIZATION_GRANT_TEMPLATE.name
                )));
                assert!(commands.iter().any(|command| matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == expected_registry_pkg_id
                            && call.function
                                == AgentRegistry::SCHEDULE_SKILL_EXECUTION_FROM_AGENT_VAULT_WITH_GRANTS.name
                )));
            },
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
            .schedule_skill_execution_from_agent_vault(ScheduleSkillExecutionFromAgentVaultParams {
                scheduler_task: scheduler_task_ref,
                agent_id: *agent_ref.object_id(),
                skill_id: 12,
                prepay_amount: 25,
                occurrence_budget: 5,
                refund_mode: 0,
                schedule_policy: TapSchedulePolicy::default(),
                refill_policy_commitment: vec![4],
                schedule_entries_commitment: vec![5],
                first_after_ms: 10,
                grant_templates: vec![TapScheduledAuthorizationGrantTemplate {
                    dag_id: sui::types::Address::from_static("0xda6"),
                    vertex: "demo_delayed_fire_vertex".to_string(),
                    tool_package: sui::types::Address::from_static("0xde0"),
                    tool_module: "demo_delayed_fire_vertex".to_string(),
                    tool_function: "execute".to_string(),
                    operation_commitment: b"demo-tap-delayed-fire".to_vec(),
                    constraints_commitment: Vec::new(),
                }],
            })
            .await
            .expect("agent-vault durable schedule succeeds");

        assert_eq!(result.scheduled_task_id, scheduled_task_id);
        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 12);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_schedule_default_executor_address_funded_does_not_fetch_agent() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let scheduler_task_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0x66"),
            5,
            sui::types::Digest::generate(&mut rng),
        );
        let default_agent_id = sui::types::Address::from_static("0xad");
        let scheduled_task_id = sui::types::Address::from_static("0x77");
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
                        agent_id: default_agent_id,
                        skill_id: 0,
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
            .schedule_default_dag_executor_skill_execution_address_funded(
                ScheduleDefaultDagExecutorSkillExecutionAddressFundedParams {
                    scheduler_task: scheduler_task_ref,
                    prepay_amount: 25,
                    refund_recipient: None,
                    payment_source: vec![2],
                    occurrence_budget: 5,
                    refund_mode: 0,
                    schedule_policy: TapSchedulePolicy::default(),
                    refill_policy_commitment: vec![4],
                    schedule_entries_commitment: vec![5],
                    first_after_ms: 10,
                },
            )
            .await
            .expect("default durable schedule succeeds");

        assert_eq!(result.scheduled_task_id, scheduled_task_id);
        assert_eq!(result.agent_id, default_agent_id);
        assert_eq!(result.skill_id, 0);
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_get_skill_requirements_resolves_active_endpoint() {
        let registry = registry();
        let registry_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = NexusObjects {
            agent_registry: registry_ref.clone(),
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

    fn baseline_payment(
        accomplished: bool,
        refunded: bool,
        final_state: Option<TapExecutionPaymentFinalState>,
    ) -> TapExecutionPayment {
        TapExecutionPayment {
            id: sui::types::Address::from_static("0x1"),
            execution_id: sui::types::Address::from_static("0x2"),
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            interface_revision: InterfaceRevision(1),
            payer: sui::types::Address::from_static("0xc"),
            payment_mode: crate::types::TapPaymentMode::UserFunded,
            source_kind: None,
            source_identity: None,
            max_budget: 1_000_000,
            locked_budget: 0,
            consumed: 0,
            refund_mode: 0,
            payment_source_hash: vec![],
            accomplished,
            refunded,
            final_state,
            locked_vertices: vec![],
        }
    }

    #[test]
    fn payment_is_terminal_recognizes_each_settled_form() {
        assert!(!payment_is_terminal(&baseline_payment(false, false, None)));
        assert!(payment_is_terminal(&baseline_payment(true, false, None)));
        assert!(payment_is_terminal(&baseline_payment(false, true, None)));
        assert!(payment_is_terminal(&baseline_payment(
            false,
            false,
            Some(TapExecutionPaymentFinalState::Accomplished)
        )));
        assert!(payment_is_terminal(&baseline_payment(
            false,
            false,
            Some(TapExecutionPaymentFinalState::Refunded)
        )));
        assert!(!payment_is_terminal(&baseline_payment(
            false,
            false,
            Some(TapExecutionPaymentFinalState::Pending)
        )));
    }

    #[tokio::test]
    async fn wait_for_payment_settled_rejects_zero_poll_interval() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        // A zero poll interval would busy-loop the poller; it must be rejected
        // before any RPC traffic is generated.
        let error = client
            .tap()
            .wait_for_payment_settled(
                sui::types::Address::from_static("0xa"),
                Duration::from_secs(5),
                Duration::ZERO,
            )
            .await
            .expect_err("zero poll interval must be rejected");

        assert!(
            matches!(&error, NexusError::Configuration(msg) if msg.contains("poll_interval")),
            "unexpected error: {error:?}"
        );
    }
}
