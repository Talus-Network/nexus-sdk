//! Read-only helpers and high-level actions for standard TAP.

#[cfg(feature = "move_publish")]
use crate::idents::publish_dependency_ids_or_framework_defaults;
#[cfg(feature = "move_publish")]
use crate::types::SkillConfig;
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
        transactions::{
            agent_input::AgentInput,
            dag as dag_tx,
            scheduler as scheduler_tx,
            tap as tap_tx,
        },
        types::{
            resolve_active_tap_skill_execution_target,
            resolve_active_tap_skill_revision,
            resolve_default_tap_dag_executor,
            ActiveSkillExecutionTarget,
            AgentId,
            AgentPaymentVault,
            AgentRegistry,
            AgentRegistryObject,
            AgentVaultFieldKey,
            AgentVertexAuthorizationTemplate,
            DataStorage,
            DefaultDagExecutor,
            DefaultDagExecutorFieldKey,
            DefaultDagExecutorRecord,
            DefaultDagExecutorValue,
            ExecutionPayment,
            ExecutionPaymentHistoryFieldKey,
            ExecutionPaymentHistoryList,
            ExecutionPaymentReceipt,
            ExecutionPaymentReceiptFieldKey,
            InterfaceRevision,
            NexusObjects,
            SkillId,
            SkillRecord,
            SkillRequirements,
            SkillRevisionKey,
            SkillRevisionRecord,
            SkillRevisionResolutionError,
            TapPublishArtifact,
        },
    },
    std::{
        collections::HashMap,
        time::{Duration, Instant},
    },
};
#[cfg(feature = "move_publish")]
use {std::path::PathBuf, sui_move_build::CompiledPackage};

/// High-level standard TAP actions exposed through [`NexusClient`].
#[derive(Clone)]
pub struct TapActions {
    pub(super) client: NexusClient,
}

pub(crate) fn agent_input_from_metadata(metadata: &Response<()>) -> anyhow::Result<AgentInput> {
    let object_ref = metadata.object_ref();
    match metadata.owner {
        sui::types::Owner::Shared(_) => Ok(AgentInput::Shared(object_ref)),
        sui::types::Owner::Immutable => Ok(AgentInput::Immutable(object_ref)),
        sui::types::Owner::Address(_) => Ok(AgentInput::Owned(object_ref)),
        ref owner => Err(anyhow::anyhow!(
            "agent '{}' has unsupported owner for transaction input: {owner:?}",
            metadata.object_id
        )),
    }
}

pub(crate) fn agent_argument_from_metadata(
    tx: &mut sui::tx::TransactionBuilder,
    metadata: &Response<()>,
    mutable: bool,
) -> anyhow::Result<sui::tx::Argument> {
    let input = agent_input_from_metadata(metadata)?;
    if mutable {
        input.mutable_argument(tx)
    } else {
        input.immutable_argument(tx)
    }
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

/// Result returned after updating an existing skill from a publish artifact.
#[derive(Clone, Debug)]
pub struct UpdateSkillResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub current_interface_revision: InterfaceRevision,
    pub dag_binding: crate::types::SkillDagBinding,
    pub requirements: SkillRequirements,
}

/// Result returned after resolving live skill requirements.
#[derive(Clone, Debug)]
pub struct GetSkillRequirementsResult {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub active_skill_revision_key: SkillRevisionKey,
    pub requirements: SkillRequirements,
}

/// Parameters required to create an explicit-agent scheduled task.
pub struct CreateAgentTaskParams {
    pub dag_id: sui::types::Address,
    pub entry_group: String,
    pub input_data: HashMap<String, HashMap<String, DataStorage>>,
    pub metadata: Vec<(String, String)>,
    pub execution_priority_fee_per_gas_unit: u64,
    pub initial_schedule: Option<crate::nexus::scheduler::OccurrenceRequest>,
    pub generator: crate::nexus::scheduler::GeneratorKind,
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub payment: AgentTaskPayment,
}

/// Actions supported when mutating an explicit-agent scheduled task.
#[derive(Clone, Copy, Debug)]
pub enum AgentTaskStateAction {
    Pause,
    Resume,
    Cancel,
}

pub struct SetAgentTaskStateResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub task_id: sui::types::Address,
    pub agent_id: AgentId,
    pub state: AgentTaskStateAction,
}

#[derive(Clone, Debug)]
pub enum AgentTaskPayment {
    AddressFunded {
        prepay_amount: u64,
        refund_recipient: Option<sui::types::Address>,
        occurrence_budget: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
    AgentVault {
        prepay_amount: u64,
        occurrence_budget: u64,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AgentVertexAuthorizationTemplate>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TapPaymentHistory {
    pub wallet_receipts: Vec<ExecutionPaymentReceipt>,
    pub vault_receipts: Vec<ExecutionPaymentReceipt>,
    pub unresolved_execution_ids: Vec<sui::types::Address>,
    pub resolved_execution_ids: Vec<sui::types::Address>,
}

/// Options for publishing a TAP Move package through [`TapActions`].
#[cfg(feature = "move_publish")]
#[derive(Clone, Debug, Default)]
pub struct TapPackagePublishOptions {
    pub package_path: PathBuf,
    pub named_address_overrides: Vec<(String, sui::types::Address)>,
    pub environment: Option<String>,
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

/// Inputs to [`TapActions::bind_agent_skill`].
#[derive(Clone, Debug)]
pub struct BindAgentSkillParams {
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
}

/// Result returned by [`TapActions::wait_for_payment_settled`].
#[derive(Clone, Debug)]
pub struct WaitForPaymentResult {
    pub payment: ExecutionPayment,
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

/// Parameters for [`TapActions::accomplish_execution_payment`].
#[derive(Clone, Debug)]
pub struct AccomplishExecutionPaymentParams {
    /// The shared `DAGExecution` object whose TAP payment should be settled.
    pub execution_id: sui::types::Address,
    /// When set, the SDK fetches the agent object and routes through
    /// `nexus_workflow::execution_settlement::accomplish_tap_execution_payment_from_agent_vault`
    /// — settling the payment out of the agent's vault rather than from the
    /// invoker-funded payment object. When `None`, the default
    /// `accomplish_tap_execution_payment` PTB is built.
    pub agent_id: Option<sui::types::Address>,
}

/// Result returned by [`TapActions::accomplish_execution_payment`].
#[derive(Clone, Debug)]
pub struct AccomplishExecutionPaymentResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub execution_id: sui::types::Address,
    /// Echoes the resolved `agent_id` when the from-vault PTB was used.
    /// `None` for the default invoker-funded path.
    pub agent_id: Option<sui::types::Address>,
}

/// Parameters for [`TapActions::refill_execution_payment`].
#[derive(Clone, Debug)]
pub struct RefillExecutionPaymentParams {
    /// The shared `DAGExecution` object whose live TAP payment should receive
    /// additional funds.
    pub execution_id: sui::types::Address,
    /// MIST amount split from the transaction gas coin and moved into the
    /// execution payment.
    pub amount: u64,
}

/// Parameters for [`TapActions::refill_execution_payment_from_agent_vault`].
#[derive(Clone, Debug)]
pub struct RefillExecutionPaymentFromAgentVaultParams {
    /// The shared `DAGExecution` object whose live TAP payment should receive
    /// additional funds.
    pub execution_id: sui::types::Address,
    /// Agent object whose vault is the refill source.
    pub agent_id: AgentId,
    /// MIST amount withdrawn from the agent vault.
    pub amount: u64,
}

/// Result returned by TAP execution-payment refill helpers.
#[derive(Clone, Debug)]
pub struct RefillExecutionPaymentResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub execution_id: sui::types::Address,
    pub agent_id: Option<AgentId>,
    pub amount: u64,
}

/// Whether a [`ExecutionPayment`] has reached an irrecoverable final state.
pub fn payment_is_terminal(payment: &ExecutionPayment) -> bool {
    if payment.accomplished || payment.refunded {
        return true;
    }
    matches!(
        payment.final_state,
        crate::types::ExecutionPaymentFinalState::Accomplished
            | crate::types::ExecutionPaymentFinalState::Refunded
    )
}

impl TapActions {
    #[cfg(feature = "move_publish")]
    pub async fn publish_tap_package(
        &self,
        options: TapPackagePublishOptions,
    ) -> Result<TapPackagePublishResult, NexusError> {
        let package = build_move_package(
            &options.package_path,
            &options.named_address_overrides,
            options.environment.clone(),
        )
        .map_err(NexusError::TransactionBuilding)?;
        let address = self.client.signer.get_active_address();
        let mut tx = sui::tx::TransactionBuilder::new();
        let upgrade_cap = tx.publish(
            package.package.get_package_bytes(false),
            publish_dependency_ids_or_framework_defaults(
                package
                    .get_dependency_storage_package_ids()
                    .iter()
                    .map(|id| {
                        id.to_string()
                            .parse::<sui::types::Address>()
                            .expect("compiled package dependency id must parse as Sui address")
                    }),
            ),
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
        config: &SkillConfig,
        dag: crate::types::Dag,
        package_options: TapPackagePublishOptions,
    ) -> Result<PublishSkillResult, NexusError> {
        config
            .validate()
            .map_err(|error| NexusError::TransactionBuilding(anyhow::anyhow!(error)))?;

        let tap_package = self.publish_tap_package(package_options).await?;
        let dag = self.client.workflow().publish(dag).await?;
        let artifact = TapPublishArtifact::from_config(config, dag.dag_object_id)
            .map_err(NexusError::TransactionBuilding)?;

        Ok(PublishSkillResult {
            tap_package,
            dag,
            artifact,
        })
    }

    /// Create a standard Talus agent through the configured TAP registry.
    pub async fn create_agent(&self) -> Result<CreateAgentResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;

        let agent = tap_tx::create_agent(&mut tx, nexus_objects, registry)
            .map_err(NexusError::TransactionBuilding)?;
        let recipient = sui_framework::Address::address_from_type(&mut tx, address)
            .map_err(NexusError::TransactionBuilding)?;
        tx.transfer_objects(vec![agent], recipient);

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
                            && *tag.module() == crate::idents::tap::STANDARD_AGENT_MODULE
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
                        "Created Talus Agent object not found in response"
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
            .map_err(NexusError::Rpc)?;
        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = agent_argument_from_metadata(&mut tx, &agent_object, true)
            .map_err(NexusError::TransactionBuilding)?;

        tap_tx::register_skill_with_fixed_tools(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            artifact.dag_id,
            artifact.skill_name.as_bytes().to_vec(),
            artifact.requirements.input_schema_commitment.clone(),
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
            artifact.requirements.fixed_tools.clone(),
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
            active_skill_revision_key: target.skill_revision.key,
            requirements: target.skill_revision.requirements,
        })
    }

    /// Update an existing skill's current contract from a publish artifact.
    pub async fn update_skill_from_artifact(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        artifact: &TapPublishArtifact,
    ) -> Result<UpdateSkillResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();
        let registry_for_dag = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let registry_for_policies = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let agent_for_dag = agent_argument_from_metadata(&mut tx, &agent_object, true)
            .map_err(NexusError::TransactionBuilding)?;
        let agent_for_policies = agent_argument_from_metadata(&mut tx, &agent_object, true)
            .map_err(NexusError::TransactionBuilding)?;

        tap_tx::update_dag(
            &mut tx,
            nexus_objects,
            registry_for_dag,
            agent_for_dag,
            skill_id,
            artifact.dag_id,
        )
        .map_err(NexusError::TransactionBuilding)?;

        tap_tx::update_skill_policies(
            &mut tx,
            nexus_objects,
            registry_for_policies,
            agent_for_policies,
            skill_id,
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.submit_tap_transaction(tx, address).await?;
        let event = response
            .events
            .iter()
            .rev()
            .find_map(|event| match &event.data {
                NexusEventKind::SkillContractRevisioned(event) => Some(event),
                _ => None,
            })
            .cloned()
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "SkillContractRevisionedEvent not found in TAP skill update response"
                ))
            })?;

        Ok(UpdateSkillResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: event.agent_id,
            skill_id: event.skill_id,
            current_interface_revision: event.current_interface_revision,
            dag_binding: event.dag_binding,
            requirements: event.requirements,
        })
    }

    /// Deposit `amount` MIST into the agent's payment vault, splitting from
    /// the transaction gas coin. The vault is shared so any address can
    /// deposit; withdrawal stays gated on mutable agent custody.
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
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent = agent_argument_from_metadata(&mut tx, &agent_object, true)
            .map_err(NexusError::TransactionBuilding)?;
        let amount_arg = tx.pure(&params.amount);
        let gas = tx.gas();
        let deposit_coin = tx
            .split_coins(gas, vec![amount_arg])
            .into_iter()
            .next()
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

    /// Wraps the on-chain `nexus_workflow::execution_settlement::accomplish_tap_execution_payment`
    /// PTB so the holder of the `DAGExecution` can settle its TAP payment
    /// directly — useful when the off-chain leader has not (yet) emitted the
    /// settlement transaction itself but the execution has reached a state
    /// that satisfies `assert_execution_can_accomplish_payment`. Returns
    /// the transaction digest, checkpoint, and the resolved execution id.
    ///
    /// When `params.agent_id` is supplied, the SDK additionally fetches the
    /// shared agent object and routes through
    /// `accomplish_tap_execution_payment_from_agent_vault` — settling the
    /// payment out of the agent's payment vault rather than from the
    /// invoker-funded payment object.
    pub async fn accomplish_execution_payment(
        &self,
        params: AccomplishExecutionPaymentParams,
    ) -> Result<AccomplishExecutionPaymentResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let crawler = self.client.crawler();

        let execution_ref = crawler
            .get_object_metadata(params.execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let execution = tx.object(sui::tx::ObjectInput::shared(
            *execution_ref.object_id(),
            execution_ref.version(),
            true,
        ));

        if let Some(agent_id) = params.agent_id {
            let agent_ref = crawler
                .get_object_metadata(agent_id)
                .await
                .map_err(NexusError::Rpc)?;
            let agent = agent_argument_from_metadata(&mut tx, &agent_ref, true)
                .map_err(NexusError::TransactionBuilding)?;
            dag_tx::accomplish_tap_execution_payment_from_agent_vault(
                &mut tx,
                &self.client.nexus_objects,
                agent,
                execution,
            );
        } else {
            dag_tx::accomplish_tap_execution_payment(
                &mut tx,
                &self.client.nexus_objects,
                execution,
            );
        }

        let response = self.submit_tap_transaction(tx, address).await?;
        Ok(AccomplishExecutionPaymentResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            execution_id: params.execution_id,
            agent_id: params.agent_id,
        })
    }

    /// Refill a live TAP execution payment by splitting MIST from the caller's
    /// transaction gas coin.
    pub async fn refill_execution_payment(
        &self,
        params: RefillExecutionPaymentParams,
    ) -> Result<RefillExecutionPaymentResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let execution_ref = self
            .client
            .crawler()
            .get_object_metadata(params.execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let mut tx = sui::tx::TransactionBuilder::new();
        let execution = tx.object(sui::tx::ObjectInput::shared(
            *execution_ref.object_id(),
            execution_ref.version(),
            true,
        ));
        let amount = tx.pure(&params.amount);
        let gas = tx.gas();
        let refill_coin = tx
            .split_coins(gas, vec![amount])
            .into_iter()
            .next()
            .ok_or_else(|| {
                NexusError::TransactionBuilding(anyhow::anyhow!(
                    "failed to split execution payment refill coin"
                ))
            })?;
        dag_tx::refill_tap_execution_payment(
            &mut tx,
            &self.client.nexus_objects,
            execution,
            refill_coin,
        );

        let response = self.submit_tap_transaction(tx, address).await?;
        Ok(RefillExecutionPaymentResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            execution_id: params.execution_id,
            agent_id: None,
            amount: params.amount,
        })
    }

    /// Refill a live TAP execution payment from an agent payment vault.
    pub async fn refill_execution_payment_from_agent_vault(
        &self,
        params: RefillExecutionPaymentFromAgentVaultParams,
    ) -> Result<RefillExecutionPaymentResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let crawler = self.client.crawler();
        let execution_ref = crawler
            .get_object_metadata(params.execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let agent_ref = crawler
            .get_object_metadata(params.agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();
        let agent = agent_argument_from_metadata(&mut tx, &agent_ref, true)
            .map_err(NexusError::TransactionBuilding)?;
        let execution = tx.object(sui::tx::ObjectInput::shared(
            *execution_ref.object_id(),
            execution_ref.version(),
            true,
        ));
        dag_tx::refill_tap_execution_payment_from_agent_vault(
            &mut tx,
            &self.client.nexus_objects,
            agent,
            execution,
            params.amount,
        );

        let response = self.submit_tap_transaction(tx, address).await?;
        Ok(RefillExecutionPaymentResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            execution_id: params.execution_id,
            agent_id: Some(params.agent_id),
            amount: params.amount,
        })
    }

    /// Create a scheduled task for an explicit standard agent skill.
    pub async fn create_agent_task(
        &self,
        params: CreateAgentTaskParams,
    ) -> Result<crate::nexus::scheduler::CreateTaskResult, NexusError> {
        let CreateAgentTaskParams {
            dag_id,
            entry_group,
            input_data,
            metadata,
            execution_priority_fee_per_gas_unit,
            initial_schedule: initial_schedule_request,
            generator,
            agent_id,
            skill_id,
            payment,
        } = params;

        if initial_schedule_request.is_some()
            && generator != crate::nexus::scheduler::GeneratorKind::Queue
        {
            return Err(NexusError::Configuration(
                "Initial queue schedule can only be used with the queue generator".into(),
            ));
        }

        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;

        let mut tx = sui::tx::TransactionBuilder::new();
        let metadata_arg = scheduler_tx::new_metadata(&mut tx, objects, metadata.iter().cloned())
            .map_err(NexusError::TransactionBuilding)?;
        let constraints_arg =
            scheduler_tx::new_constraints_policy(&mut tx, objects, generator.into())
                .map_err(NexusError::TransactionBuilding)?;
        let execution_arg = scheduler_tx::new_agent_execution_policy(
            &mut tx,
            objects,
            dag_id,
            execution_priority_fee_per_gas_unit,
            entry_group.as_str(),
            &input_data,
            agent_id,
            skill_id,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let registry = tx.object(sui::tx::ObjectInput::shared(
            *objects.agent_registry.object_id(),
            objects.agent_registry.version(),
            true,
        ));
        let agent_ref = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        let (task, prepay_amount, occurrence_budget) = match payment {
            AgentTaskPayment::AddressFunded {
                prepay_amount,
                refund_recipient,
                occurrence_budget,
                selected_dag,
                authorization_templates,
            } => {
                let agent = agent_argument_from_metadata(&mut tx, &agent_ref, false)
                    .map_err(NexusError::TransactionBuilding)?;
                let prepay_amount_arg = tx.pure(&prepay_amount);
                let gas = tx.gas();
                let prepayment_coin = tx
                    .split_coins(gas, vec![prepay_amount_arg])
                    .into_iter()
                    .next()
                    .ok_or_else(|| {
                        NexusError::TransactionBuilding(anyhow::anyhow!(
                            "failed to split scheduled prepayment coin"
                        ))
                    })?;
                let task = tap_tx::new_invoker_funded_agent_task(
                    &mut tx,
                    objects,
                    metadata_arg,
                    constraints_arg,
                    execution_arg,
                    registry,
                    agent,
                    agent_id,
                    dag_id,
                    execution_priority_fee_per_gas_unit,
                    entry_group.as_str(),
                    &input_data,
                    skill_id,
                    selected_dag,
                    prepayment_coin,
                    refund_recipient.unwrap_or(address),
                    occurrence_budget,
                    authorization_templates,
                )
                .map_err(NexusError::TransactionBuilding)?;
                (task, prepay_amount, occurrence_budget)
            }
            AgentTaskPayment::AgentVault {
                prepay_amount,
                occurrence_budget,
                selected_dag,
                authorization_templates,
            } => {
                let agent = agent_argument_from_metadata(&mut tx, &agent_ref, true)
                    .map_err(NexusError::TransactionBuilding)?;
                let task = tap_tx::new_agent_funded_task(
                    &mut tx,
                    objects,
                    metadata_arg,
                    constraints_arg,
                    execution_arg,
                    registry,
                    agent,
                    agent_id,
                    dag_id,
                    execution_priority_fee_per_gas_unit,
                    entry_group.as_str(),
                    &input_data,
                    skill_id,
                    selected_dag,
                    prepay_amount,
                    occurrence_budget,
                    authorization_templates,
                )
                .map_err(NexusError::TransactionBuilding)?;
                (task, prepay_amount, occurrence_budget)
            }
        };

        let task_type = crate::idents::scheduler::into_type_tag(
            objects.scheduler_pkg_id,
            crate::idents::scheduler::Scheduler::TASK,
        );
        tx.move_call(
            sui::tx::Function::new(
                sui_framework::PACKAGE_ID,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module,
                sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name,
            )
            .with_type_args(vec![task_type]),
            vec![task],
        );

        let response = self.submit_tap_transaction(tx, address).await?;
        let task_id = crate::nexus::scheduler::extract_task_id(&response)?;

        let mut initial_schedule_result = None;
        if let Some(schedule) = initial_schedule_request {
            let scheduler = self.client.scheduler();
            let task_object = scheduler.fetch_task(task_id).await?;
            initial_schedule_result = Some(
                scheduler
                    .enqueue_occurrence(&task_object, schedule, address)
                    .await?,
            );
        }

        Ok(crate::nexus::scheduler::CreateTaskResult {
            tx_digest: response.digest,
            task_id,
            initial_schedule: initial_schedule_result,
            tap_payment: Some(crate::nexus::scheduler::CreateTaskTapPaymentResult {
                agent_id,
                skill_id,
                prepay_amount,
                occurrence_budget,
            }),
        })
    }

    /// Set the scheduler state for an explicit-agent scheduled task.
    pub async fn set_agent_task_state(
        &self,
        task_id: sui::types::Address,
        agent_id: AgentId,
        action: AgentTaskStateAction,
    ) -> Result<SetAgentTaskStateResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let objects = &self.client.nexus_objects;
        let crawler = self.client.crawler();
        let task_ref = crawler
            .get_object_metadata(task_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let agent_ref = crawler
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;
        let agent =
            agent_input_from_metadata(&agent_ref).map_err(NexusError::TransactionBuilding)?;

        let mut tx = sui::tx::TransactionBuilder::new();
        match action {
            AgentTaskStateAction::Pause => scheduler_tx::pause_time_constraint_for_agent_task(
                &mut tx, objects, &task_ref, agent,
            ),
            AgentTaskStateAction::Resume => scheduler_tx::resume_time_constraint_for_agent_task(
                &mut tx, objects, &task_ref, agent,
            ),
            AgentTaskStateAction::Cancel => scheduler_tx::cancel_time_constraint_for_agent_task(
                &mut tx, objects, &task_ref, agent,
            ),
        }
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.submit_tap_transaction(tx, address).await?;
        Ok(SetAgentTaskStateResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            task_id,
            agent_id,
            state: action,
        })
    }

    /// Create a standard Talus agent and register its first skill atomically.
    pub async fn bind_agent_skill(
        &self,
        params: BindAgentSkillParams,
    ) -> Result<BindAgentSkillResult, NexusError> {
        let BindAgentSkillParams { artifact } = params;

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        let mut tx = sui::tx::TransactionBuilder::new();
        let registry = tap_tx::agent_registry_arg(&mut tx, nexus_objects, true)
            .map_err(NexusError::TransactionBuilding)?;
        let agent = tap_tx::create_agent(&mut tx, nexus_objects, registry)
            .map_err(NexusError::TransactionBuilding)?;

        tap_tx::register_skill_with_fixed_tools(
            &mut tx,
            nexus_objects,
            registry,
            agent,
            artifact.dag_id,
            artifact.skill_name.as_bytes().to_vec(),
            artifact.requirements.input_schema_commitment.clone(),
            artifact.requirements.payment_policy.clone(),
            artifact.requirements.schedule_policy.clone(),
            artifact.requirements.fixed_tools.clone(),
        )
        .map_err(NexusError::TransactionBuilding)?;

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
                        && *tag.module() == crate::idents::tap::STANDARD_AGENT_MODULE
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
        })
    }

    /// Poll a [`ExecutionPayment`] until it reaches a terminal state
    /// (accomplished, refunded, or a non-pending [`ExecutionPaymentFinalState`])
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
            let payment = fetch_execution_payment(crawler, payment_id)
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
        tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .try_build()
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
    environment: Option<String>,
) -> anyhow::Result<CompiledPackage> {
    let mut build_config = crate::sui::build::BuildConfig::new_for_testing_replace_addresses(
        named_address_overrides
            .iter()
            .map(|(name, address)| (name.clone(), address.to_string().parse().unwrap()))
            .collect::<Vec<_>>(),
    );
    build_config.config.environment = environment;
    build_config.print_diags_to_stderr = false;
    build_config.build(package_path)
}

/// Fetch the shared standard TAP registry object from chain storage.
pub async fn fetch_agent_registry(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Response<AgentRegistry>> {
    let mut registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    registry.data.default_executor = fetch_default_dag_executor(crawler, registry.data.id).await?;

    Ok(registry)
}

async fn fetch_agent_registry_tables(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Response<AgentRegistry>> {
    let raw = crawler
        .get_object_contents_bcs::<AgentRegistryObject>(registry_id)
        .await?;
    let agent_records = crawler
        .get_dynamic_fields_bcs::<sui::types::Address, crate::types::AgentRecord>(
            raw.data.agents.id,
            raw.data.agents.size(),
        )
        .await?;

    let mut agents = Vec::with_capacity(agent_records.len());
    let mut skills = Vec::new();
    for (agent_id, agent) in agent_records {
        let skill_records = crawler
            .get_dynamic_fields_bcs::<SkillId, SkillRecord>(agent.skills.id, agent.skills.size())
            .await?;

        for (skill_id, mut skill) in skill_records {
            skill.agent_id = Some(agent_id);
            skill.skill_id = Some(skill_id);
            skills.push(skill);
        }
        agents.push(agent);
    }
    Ok(Response {
        object_id: raw.object_id,
        owner: raw.owner,
        version: raw.version,
        digest: raw.digest,
        balance: raw.balance,
        data: AgentRegistry {
            id: raw.data.id,
            agents,
            skills,
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
                    "AgentRegistry {} has multiple default DAG executor dynamic fields",
                    registry_id
                );
            }
            values.into_iter().next().map(|value| value.target())
        }
    };

    Ok(default_executor)
}

/// Fetch a pinned TAP skill revision from the real `AgentRegistry` vector layout.
pub async fn fetch_skill_revision(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<Response<SkillRevisionRecord>> {
    let registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    let record = registry.data.skill_revision_record(SkillRevisionKey {
        agent_id,
        skill_id,
        interface_revision,
    })?;

    Ok(registry_response_with_data(registry, record))
}

/// Resolve a fresh execution skill revision through the active revision stored on the skill.
pub async fn fetch_active_tap_skill_revision(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<SkillRevisionRecord>> {
    let registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    let record = registry
        .data
        .active_skill_revision_record(agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, record))
}

/// Fetch the shared TAP registry named by `NexusObjects`.
pub async fn fetch_configured_agent_registry(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<AgentRegistry>> {
    fetch_agent_registry(crawler, *objects.agent_registry.object_id()).await
}

/// Resolve a fresh execution skill revision through the configured TAP registry.
pub async fn fetch_configured_active_tap_skill_revision(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<SkillRevisionRecord>> {
    fetch_active_tap_skill_revision(
        crawler,
        *objects.agent_registry.object_id(),
        agent_id,
        skill_id,
    )
    .await
}

/// Resolve the active skill registration plus skill revision from the configured TAP registry.
pub async fn fetch_configured_active_tap_skill_execution_target(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<ActiveSkillExecutionTarget>> {
    let registry = fetch_configured_agent_registry(crawler, objects).await?;
    let target = resolve_active_tap_skill_execution_target(&registry.data, agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, target))
}

/// Resolve the configured default agent from the configured registry.
pub async fn fetch_configured_default_tap_dag_executor(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<DefaultDagExecutorRecord>> {
    let registry = fetch_configured_agent_registry(crawler, objects).await?;
    let target = resolve_default_tap_dag_executor(&registry.data)?;

    Ok(registry_response_with_data(registry, target))
}

/// Fetch a shared standard TAP execution payment object by object ID.
pub async fn fetch_execution_payment(
    crawler: &Crawler,
    payment_id: sui::types::Address,
) -> anyhow::Result<Response<ExecutionPayment>> {
    let payment = crawler
        .get_object_contents_bcs::<ExecutionPayment>(payment_id)
        .await
        .map_err(|error| {
            anyhow::anyhow!("payment '{payment_id}' did not decode as ExecutionPayment: {error}")
        })?;

    Ok(Response {
        object_id: payment.object_id,
        owner: payment.owner,
        version: payment.version,
        data: payment.data,
        digest: payment.digest,
        balance: payment.balance,
    })
}

/// Fetch the standard execution payment stored under a DAG execution object.
pub async fn fetch_execution_payment_for_execution(
    crawler: &Crawler,
    execution_id: sui::types::Address,
) -> anyhow::Result<Response<ExecutionPayment>> {
    let mut candidates = Vec::new();
    let mut decode_errors = Vec::new();
    for field_id in crawler
        .get_dynamic_object_field_child_ids(execution_id)
        .await?
    {
        match crawler
            .get_object_contents_bcs::<ExecutionPayment>(field_id)
            .await
        {
            Ok(payment) => {
                if payment.data.execution_id == execution_id {
                    candidates.push(Response {
                        object_id: payment.object_id,
                        owner: payment.owner,
                        version: payment.version,
                        data: payment.data,
                        digest: payment.digest,
                        balance: payment.balance,
                    });
                }
            }
            Err(e) => decode_errors.push(format!(
                "child '{}' did not decode as ExecutionPayment: {e}",
                field_id
            )),
        }
    }

    match candidates.as_slice() {
        [payment] => Ok(payment.clone()),
        [] => {
            if decode_errors.is_empty() {
                anyhow::bail!(
                    "execution payment dynamic field not found for execution '{execution_id}'"
                );
            }
            anyhow::bail!(
                "execution payment dynamic field not found for execution '{}'; candidate decode errors: {}",
                execution_id,
                decode_errors.join("; ")
            );
        }
        _ => anyhow::bail!(
            "multiple execution payment dynamic fields found for execution '{execution_id}'"
        ),
    }
}

/// Fetch wallet-owned standard TAP execution payment receipts.
pub async fn fetch_wallet_execution_payment_receipts(
    crawler: &Crawler,
    objects: &NexusObjects,
    owner: sui::types::Address,
) -> anyhow::Result<Vec<Response<ExecutionPaymentReceipt>>> {
    crawler
        .get_owned_objects(
            owner,
            sui::types::StructTag::new(
                objects.interface_pkg_id,
                crate::idents::tap::STANDARD_PAYMENT_MODULE,
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
) -> anyhow::Result<Response<ExecutionPaymentReceipt>> {
    crawler
        .get_dynamic_object_field::<ExecutionPaymentReceiptFieldKey, ExecutionPaymentReceipt>(
            agent_id,
            ExecutionPaymentReceiptFieldKey { execution_id },
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
        .get_dynamic_object_fields::<ExecutionPaymentHistoryFieldKey, ExecutionPaymentHistoryList>(
            agent_id,
        )
        .await?;
    let mut unresolved = Vec::new();
    let mut resolved = Vec::new();

    if let Some(list) = fields.remove(&ExecutionPaymentHistoryFieldKey { resolved: false }) {
        unresolved = list.data.execution_ids;
    }
    if let Some(list) = fields.remove(&ExecutionPaymentHistoryFieldKey { resolved: true }) {
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
pub async fn fetch_agent_payment_vault(
    crawler: &Crawler,
    vault_id: sui::types::Address,
) -> anyhow::Result<Response<AgentPaymentVault>> {
    crawler.get_object::<AgentPaymentVault>(vault_id).await
}

/// Fetch the standard Talus agent payment vault stored as a child of the agent object.
pub async fn fetch_agent_payment_vault_for_agent(
    crawler: &Crawler,
    agent_id: AgentId,
) -> anyhow::Result<Response<AgentPaymentVault>> {
    crawler
        .get_dynamic_object_field::<AgentVaultFieldKey, AgentPaymentVault>(
            agent_id,
            AgentVaultFieldKey {},
        )
        .await
}

/// Resolve a fresh execution skill revision from already fetched records.
pub fn resolve_active_skill_revision_record<'a>(
    records: &'a [SkillRevisionRecord],
    skills: &[SkillRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a SkillRevisionRecord, SkillRevisionResolutionError> {
    resolve_active_tap_skill_revision(records, skills, agent_id, skill_id)
}

fn registry_response_with_data<T>(registry: Response<AgentRegistry>, data: T) -> Response<T> {
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
            idents::{primitives, registry::AgentRegistry as AgentRegistryIdent, sui_framework},
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                AgentRecord,
                AgentRegistryObject,
                DefaultDagExecutor,
                ExecutionPaymentFinalState,
                ExecutionPaymentSourceKind,
                InterfaceRevision,
                MoveTable,
                NexusObjects,
                SkillConfig,
                SkillDagBinding,
                SkillPaymentPolicy,
                SkillRecord,
                SkillRequirements,
                SkillRevisionKey,
                SkillSchedulePolicy,
            },
        },
    };

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct Wrapper<T> {
        event: T,
    }

    fn submitted_programmable_transaction(
        request: &sui::grpc::ExecuteTransactionRequest,
    ) -> sui::types::ProgrammableTransaction {
        let transaction = request.transaction.as_ref().expect("submitted transaction");
        let transaction =
            sui::types::Transaction::try_from(transaction).expect("submitted transaction decodes");
        let sui::types::TransactionKind::ProgrammableTransaction(programmable) = transaction.kind
        else {
            panic!("expected programmable transaction");
        };
        programmable
    }

    fn assert_no_public_share_object(request: &sui::grpc::ExecuteTransactionRequest) {
        let transaction = submitted_programmable_transaction(request);
        assert!(!transaction.commands.iter().any(|command| matches!(
            command,
            sui::types::Command::MoveCall(call)
                if call.package == sui_framework::PACKAGE_ID
                    && call.module == sui_framework::Transfer::PUBLIC_SHARE_OBJECT.module
                    && call.function == sui_framework::Transfer::PUBLIC_SHARE_OBJECT.name
        )));
    }

    fn skill_revision(revision: u64) -> SkillRevisionRecord {
        SkillRevisionRecord {
            key: SkillRevisionKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceRevision(revision),
            },
            requirements: SkillRequirements {
                input_schema_commitment: vec![2],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
        }
    }

    fn registry() -> AgentRegistry {
        let agent = sui::types::Address::from_static("0xa");
        let skill_id = 11;
        let requirements = SkillRequirements {
            input_schema_commitment: vec![2],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };

        AgentRegistry {
            id: sui::types::Address::from_static("0xf"),
            agents: vec![AgentRecord {
                active: true,
                skills: MoveTable::new(sui::types::Address::from_static("0x90"), 1),
            }],
            skills: vec![SkillRecord {
                agent_id: Some(agent),
                skill_id: Some(skill_id),
                description: vec![4],
                active: true,
                dag_binding: SkillDagBinding::pinned(sui::types::Address::from_static("0x3")),
                requirements: requirements.clone(),
                current_interface_revision: InterfaceRevision(2),
                scheduled_task_count: 0,
            }],
            default_executor: Some(DefaultDagExecutor {
                agent_id: agent,
                skill_id,
            }),
        }
    }

    #[derive(Clone)]
    struct RegistryObjectMock {
        registry_object: AgentRegistryObject,
        agent_field_ref: sui::types::ObjectReference,
        skill_field_ref: sui::types::ObjectReference,
        default_executor_field_ref: Option<sui::types::ObjectReference>,
        default_executor_value: Option<DefaultDagExecutorValue>,
        agent_record: AgentRecord,
        skill_record: SkillRecord,
        skill_revision_record: SkillRevisionRecord,
    }

    fn registry_object_mock(registry: &AgentRegistry) -> RegistryObjectMock {
        assert_eq!(registry.agents.len(), 1, "test registry has one agent");
        assert_eq!(registry.skills.len(), 1, "test registry has one skill");
        let agent = registry.agents[0].clone();
        let skill_record = registry.skills[0].clone();
        let skill_revision_record = registry
            .active_skill_revision_record(
                skill_record.agent_id.expect("skill has agent id"),
                skill_record.skill_id.expect("skill has skill id"),
            )
            .expect("active skill revision derives from skill");
        let agent_field_ref = sui_mocks::mock_sui_object_ref();
        let skill_field_ref = sui_mocks::mock_sui_object_ref();
        let default_executor_field_ref = registry
            .default_executor
            .map(|_| sui_mocks::mock_sui_object_ref());
        let default_executor_value =
            registry
                .default_executor
                .map(|default_executor| DefaultDagExecutorValue {
                    agent: crate::types::Agent {
                        id: default_executor.agent_id,
                        next_skill_id: 1,
                        registry_id: Some(registry.id).into(),
                    },
                    skill_id: default_executor.skill_id,
                });

        RegistryObjectMock {
            registry_object: AgentRegistryObject {
                id: registry.id,
                agents: MoveTable::new(sui::types::Address::from_static("0x9000"), 1),
            },
            agent_field_ref,
            skill_field_ref,
            default_executor_field_ref,
            default_executor_value,
            agent_record: agent,
            skill_record,
            skill_revision_record,
        }
    }

    fn mock_fetch_registry_table_data(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &AgentRegistry,
    ) -> RegistryObjectMock {
        let mock = registry_object_mock(registry);
        sui_mocks::grpc::mock_get_object_bcs_for(
            ledger_service_mock,
            registry_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&mock.registry_object).expect("raw registry bcs"),
            sui::types::StructTag::new(
                nexus_objects.registry_pkg_id,
                crate::idents::registry::AGENT_REGISTRY_MODULE,
                sui::types::Identifier::from_static("AgentRegistry"),
                vec![],
            ),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                mock.skill_revision_record.key.agent_id,
                mock.agent_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.agent_field_ref.clone(),
                sui::types::Owner::Shared(1),
                mock.skill_revision_record.key.agent_id,
                mock.agent_record.clone(),
            )],
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            state_service_mock,
            vec![(
                mock.skill_revision_record.key.skill_id,
                mock.skill_field_ref.object_id().to_owned(),
            )],
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger_service_mock,
            vec![(
                mock.skill_field_ref.clone(),
                sui::types::Owner::Shared(1),
                mock.skill_revision_record.key.skill_id,
                mock.skill_record.clone(),
            )],
        );
        mock
    }

    fn mock_fetch_registry_from_tables(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &AgentRegistry,
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
        registry: &AgentRegistry,
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
    fn resolve_active_skill_revision_record_reuses_sdk_fail_closed_rule() {
        let records = vec![skill_revision(1), skill_revision(2)];
        let skills = vec![registry().skills[0].clone()];
        let resolved = resolve_active_skill_revision_record(
            &records,
            &skills,
            sui::types::Address::from_static("0xa"),
            11,
        )
        .expect("one active skill revision");

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(2));
    }

    #[test]
    fn registry_active_resolution_uses_skill_active_revision() {
        let registry = registry();
        let records = registry
            .skill_revision_records()
            .expect("skill revision records");

        assert_eq!(records.len(), 1);

        let resolved = registry
            .active_skill_revision_record(sui::types::Address::from_static("0xa"), 11)
            .expect("active skill revision");

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

        assert_eq!(
            target.skill.dag_binding,
            SkillDagBinding::pinned(sui::types::Address::from_static("0x3"))
        );
        assert_eq!(
            target.skill_revision.key.interface_revision,
            InterfaceRevision(2)
        );
    }

    #[test]
    fn configured_default_executor_reads_nexus_objects_metadata() {
        let objects = NexusObjects {
            default_dag_executor: DefaultDagExecutor {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            },
            ..crate::test_utils::sui_mocks::mock_nexus_objects()
        };

        assert_eq!(
            objects.default_dag_executor,
            DefaultDagExecutor {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            }
        );
    }

    #[tokio::test]
    async fn fetch_skill_revision_does_not_decode_default_executor() {
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

        let response = fetch_skill_revision(
            &crawler,
            registry.id,
            sui::types::Address::from_static("0xa"),
            11,
            InterfaceRevision(2),
        )
        .await
        .expect("skill revision recovery should not require default executor decoding");

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
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: std::path::PathBuf::from("dag.json"),
            requirements: SkillRequirements {
                input_schema_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceRevision(1),
        };

        TapPublishArtifact::from_config(&config, sui::types::Address::from_static("0xd"))
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
                        crate::idents::tap::STANDARD_AGENT_MODULE,
                        sui::types::Identifier::from_static("Agent"),
                        vec![],
                    ),
                    true,
                    agent_ref.version(),
                    agent_ref.object_id().as_bytes().to_vec(),
                )
                .expect("agent object contents include id"),
            ),
            sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
            digest,
            0,
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint_matching(
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
                    },
                })
                .unwrap(),
            )],
            assert_no_public_share_object,
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
            .create_agent()
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
    async fn tap_actions_bind_agent_skill_keeps_created_agent_owned() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let artifact = artifact();
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
                        crate::idents::tap::STANDARD_AGENT_MODULE,
                        sui::types::Identifier::from_static("Agent"),
                        vec![],
                    ),
                    true,
                    agent_ref.version(),
                    agent_ref.object_id().as_bytes().to_vec(),
                )
                .expect("agent object contents include id"),
            ),
            sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
            digest,
            0,
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint_matching(
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
                            agent_id: *agent_ref.object_id(),
                            vault_id: sui::types::Address::from_static("0xb"),
                        },
                    })
                    .unwrap(),
                ),
                wrapped_event(
                    &nexus_objects,
                    nexus_objects.registry_pkg_id,
                    "agent_registry",
                    "SkillRegisteredEvent",
                    bcs::to_bytes(&Wrapper {
                        event: events::SkillRegisteredEvent {
                            agent_id: *agent_ref.object_id(),
                            skill_id: 11,
                            dag_id: artifact.dag_id,
                            dag_binding: SkillDagBinding::pinned(artifact.dag_id),
                        },
                    })
                    .unwrap(),
                ),
            ],
            assert_no_public_share_object,
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
            .bind_agent_skill(BindAgentSkillParams { artifact })
            .await
            .expect("bind succeeds");

        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.agent_object.object_id(), agent_ref.object_id());
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
            sui::types::Owner::Address(sui::types::Address::from_static("0xc")),
            None,
        );
        let expected_registry_pkg_id = nexus_objects.registry_pkg_id;
        let expected_agent_id = *agent_ref.object_id();
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
                nexus_objects.registry_pkg_id,
                "agent_registry",
                "SkillRegisteredEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::SkillRegisteredEvent {
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        dag_id: artifact.dag_id,
                        dag_binding: SkillDagBinding::pinned(artifact.dag_id),
                    },
                })
                .unwrap(),
            )],
            move |request| {
                let transaction = submitted_programmable_transaction(request);
                let commands = &transaction.commands;
                assert!(commands.iter().any(|command| matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call.package == expected_registry_pkg_id
                            && call.function
                                == AgentRegistryIdent::REGISTER_SKILL_WITH_FIXED_TOOLS.name
                )));
                assert!(transaction.inputs.iter().any(|input| matches!(
                    input,
                    sui::types::Input::ImmutableOrOwned(object)
                        if object.object_id() == &expected_agent_id
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

    #[tokio::test]
    async fn tap_actions_update_skill_from_artifact_extracts_event() {
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
                nexus_objects.registry_pkg_id,
                "agent_registry",
                "SkillContractRevisionedEvent",
                bcs::to_bytes(&Wrapper {
                    event: events::SkillContractRevisionedEvent {
                        agent_id: *agent_ref.object_id(),
                        skill_id: 11,
                        current_interface_revision: InterfaceRevision(2),
                        dag_binding: SkillDagBinding::pinned(artifact.dag_id),
                        requirements: artifact.requirements.clone(),
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

                let move_calls = commands
                    .iter()
                    .filter_map(|command| match command {
                        sui::types::Command::MoveCall(call) => Some(call),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

                assert!(move_calls.iter().any(|call| {
                    call.package == expected_registry_pkg_id
                        && call.function == AgentRegistryIdent::UPDATE_DAG.name
                }));
                assert!(move_calls.iter().any(|call| {
                    call.package == expected_registry_pkg_id
                        && call.function == AgentRegistryIdent::UPDATE_SKILL_POLICIES.name
                }));
                assert!(!move_calls
                    .iter()
                    .any(|call| call.function.as_str() == "update_skill"));
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
            .update_skill_from_artifact(*agent_ref.object_id(), 11, &artifact)
            .await
            .expect("update skill succeeds");

        assert_eq!(result.agent_id, *agent_ref.object_id());
        assert_eq!(result.skill_id, 11);
        assert_eq!(result.current_interface_revision, InterfaceRevision(2));
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
                                == crate::idents::interface::Agent::DEPOSIT_AGENT_PAYMENT_VAULT.name
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

    /// `accomplish_execution_payment` wraps the on-chain
    /// `nexus_workflow::execution_settlement::accomplish_tap_execution_payment` PTB. The
    /// happy path fetches the shared `DAGExecution` object's metadata,
    /// builds a single move-call PTB targeting the workflow package, and
    /// returns the supplied `execution_id` verbatim in the result so
    /// callers can correlate the digest with the resolved payment.
    #[tokio::test]
    async fn tap_actions_accomplish_execution_payment_calls_workflow_dag_entrypoint() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xee"),
            9,
            sui::types::Digest::generate(&mut rng),
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        let expected_workflow_pkg_id = nexus_objects.workflow_pkg_id;
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
                // Exactly one move call: dag::accomplish_tap_execution_payment
                // on the workflow package.
                let move_calls: Vec<_> = commands
                    .iter()
                    .filter_map(|command| match command {
                        sui::types::Command::MoveCall(call) => Some(call),
                        _ => None,
                    })
                    .collect();
                assert_eq!(move_calls.len(), 1, "expected exactly one move call");
                let call = move_calls[0];
                assert_eq!(call.package, expected_workflow_pkg_id);
                assert_eq!(
                    call.function,
                    crate::idents::workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT
                        .name
                );
                assert_eq!(
                    call.module,
                    crate::idents::workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT
                        .module
                );
                // The only argument is the shared DAGExecution input.
                assert_eq!(call.arguments.len(), 1);
                assert!(matches!(call.arguments[0], sui::types::Argument::Input(_)));
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
            .accomplish_execution_payment(AccomplishExecutionPaymentParams {
                execution_id: *execution_ref.object_id(),
                agent_id: None,
            })
            .await
            .expect("accomplish execution payment succeeds");

        assert_eq!(result.execution_id, *execution_ref.object_id());
        assert!(result.agent_id.is_none());
        assert_eq!(result.tx_digest, digest);
    }

    /// When `params.agent_id` is supplied, the SDK fetches the agent
    /// object's metadata *in addition to* the execution, and the PTB calls
    /// the `_from_agent_vault` entrypoint with both shared inputs in order
    /// (agent first, execution second — matches the Move signature).
    #[tokio::test]
    async fn tap_actions_accomplish_execution_payment_routes_through_vault_when_agent_supplied() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xee"),
            9,
            sui::types::Digest::generate(&mut rng),
        );
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            5,
            sui::types::Digest::generate(&mut rng),
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        // Execution metadata fetched first (top of `accomplish_execution_payment`).
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            execution_ref.clone(),
            sui::types::Owner::Shared(execution_ref.version()),
            None,
        );
        // Agent metadata fetched only on the from-vault branch.
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref.clone(),
            sui::types::Owner::Shared(agent_ref.version()),
            None,
        );

        let expected_workflow_pkg_id = nexus_objects.workflow_pkg_id;
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
                let move_calls: Vec<_> = commands
                    .iter()
                    .filter_map(|command| match command {
                        sui::types::Command::MoveCall(call) => Some(call),
                        _ => None,
                    })
                    .collect();
                assert_eq!(move_calls.len(), 1, "expected exactly one move call");
                let call = move_calls[0];
                assert_eq!(call.package, expected_workflow_pkg_id);
                assert_eq!(
                    call.function,
                    crate::idents::workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT
                        .name
                );
                assert_eq!(
                    call.module,
                    crate::idents::workflow::ExecutionSettlement::ACCOMPLISH_TAP_EXECUTION_PAYMENT_FROM_AGENT_VAULT
                        .module
                );
                // (agent, execution) in that order — matches the Move
                // signature `(&mut Agent, &mut DAGExecution)`. Both are
                // shared-object inputs.
                assert_eq!(call.arguments.len(), 2);
                assert!(matches!(call.arguments[0], sui::types::Argument::Input(_)));
                assert!(matches!(call.arguments[1], sui::types::Argument::Input(_)));
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
            .accomplish_execution_payment(AccomplishExecutionPaymentParams {
                execution_id: *execution_ref.object_id(),
                agent_id: Some(*agent_ref.object_id()),
            })
            .await
            .expect("accomplish from-vault execution payment succeeds");

        assert_eq!(result.execution_id, *execution_ref.object_id());
        assert_eq!(result.agent_id, Some(*agent_ref.object_id()));
        assert_eq!(result.tx_digest, digest);
    }

    #[tokio::test]
    async fn tap_actions_get_skill_requirements_resolves_active_skill_revision() {
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
            result.active_skill_revision_key.interface_revision,
            InterfaceRevision(2)
        );
        assert_eq!(result.requirements.input_schema_commitment, vec![2]);
    }

    fn baseline_payment(
        accomplished: bool,
        refunded: bool,
        final_state: ExecutionPaymentFinalState,
    ) -> ExecutionPayment {
        ExecutionPayment {
            id: sui::types::Address::from_static("0x1"),
            execution_id: sui::types::Address::from_static("0x2"),
            agent_id: sui::types::Address::from_static("0xa"),
            skill_id: 11,
            interface_revision: InterfaceRevision(1),
            payment_policy: crate::types::SkillPaymentPolicy::UserFunded,
            source_kind: ExecutionPaymentSourceKind::UserFunded {
                user: sui::types::Address::from_static("0x1"),
            },
            max_budget: 1_000_000,
            locked_budget: 0,
            funds: crate::types::SuiBalance { value: 1_000_000 },
            consumed: 0,
            tool_cost_snapshot: crate::types::PaymentVecMap { contents: vec![] },
            accomplished,
            refunded,
            final_state,
            locked_vertices: vec![],
        }
    }

    #[test]
    fn canonical_execution_payment_keeps_policy_and_source() {
        let agent_id = sui::types::Address::from_static("0xa");
        let payment = ExecutionPayment {
            id: sui::types::Address::from_static("0x1"),
            execution_id: sui::types::Address::from_static("0x2"),
            agent_id,
            skill_id: 11,
            interface_revision: InterfaceRevision(1),
            payment_policy: crate::types::SkillPaymentPolicy::AgentFunded { max_budget: 100 },
            source_kind: ExecutionPaymentSourceKind::AgentFunded { agent_id },
            max_budget: 100,
            locked_budget: 0,
            funds: crate::types::SuiBalance { value: 100 },
            consumed: 0,
            accomplished: false,
            refunded: false,
            final_state: ExecutionPaymentFinalState::Pending,
            tool_cost_snapshot: crate::types::PaymentVecMap { contents: vec![] },
            locked_vertices: vec![],
        };

        assert_eq!(
            payment.payment_policy,
            crate::types::SkillPaymentPolicy::AgentFunded { max_budget: 100 }
        );
        assert_eq!(
            payment.source_kind,
            ExecutionPaymentSourceKind::AgentFunded { agent_id }
        );
        assert_eq!(payment.final_state, ExecutionPaymentFinalState::Pending);
    }

    #[test]
    fn payment_is_terminal_recognizes_each_settled_form() {
        assert!(!payment_is_terminal(&baseline_payment(
            false,
            false,
            ExecutionPaymentFinalState::Pending
        )));
        assert!(payment_is_terminal(&baseline_payment(
            true,
            false,
            ExecutionPaymentFinalState::Pending
        )));
        assert!(payment_is_terminal(&baseline_payment(
            false,
            true,
            ExecutionPaymentFinalState::Pending
        )));
        assert!(payment_is_terminal(&baseline_payment(
            false,
            false,
            ExecutionPaymentFinalState::Accomplished
        )));
        assert!(payment_is_terminal(&baseline_payment(
            false,
            false,
            ExecutionPaymentFinalState::Refunded
        )));
        assert!(!payment_is_terminal(&baseline_payment(
            false,
            false,
            ExecutionPaymentFinalState::Pending
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
