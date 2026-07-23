//! Read-only helpers and high-level actions for standard TAP.

#[cfg(feature = "move_publish")]
use crate::move_boundary::publish_dependency_ids_or_framework_defaults;
#[cfg(feature = "move_publish")]
use crate::types::{DagSpec, SkillConfig};
use {
    crate::{
        events::NexusEventKind,
        move_bindings::{
            interface::{
                agent::{Agent, AgentPaymentVault, AgentVaultFieldKey, SkillRequirement},
                payment::{ExecutionPayment, ExecutionPaymentFinalState},
                version::InterfaceVersion,
            },
            registry::agent_registry::{
                AgentRecord,
                AgentRegistry,
                DefaultDagExecutor,
                DefaultDagExecutorFieldKey,
                SkillRecord,
            },
        },
        nexus::{
            client::NexusClient,
            crawler::{Crawler, Response},
            error::NexusError,
            signer::ExecutedTransaction,
        },
        sui,
        transactions::{agent_input::AgentInput, dag as dag_tx, tap as tap_tx},
        types::{
            resolve_active_tap_skill_execution_target,
            resolve_active_tap_skill_revision,
            resolve_default_tap_dag_executor,
            ActiveSkillExecutionTarget,
            AgentId,
            AgentRegistrySnapshot,
            DefaultDagExecutorRecord,
            NexusObjects,
            SkillId,
            SkillRecordContext,
            SkillRevisionContext,
            SkillRevisionLookupError,
            SkillRevisionLookupKey,
            TapPublishArtifact,
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
    pub current_interface_revision: InterfaceVersion,
    pub dag_binding: crate::move_bindings::interface::agent::SkillDagBinding,
    pub requirements: SkillRequirement,
}

/// Result returned after resolving live skill requirements.
#[derive(Clone, Debug)]
pub struct GetSkillRequirementResult {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub active_skill_revision_key: SkillRevisionLookupKey,
    pub requirements: SkillRequirement,
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
        ExecutionPaymentFinalState::Accomplished | ExecutionPaymentFinalState::Refunded
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
        let modules = package.package.get_package_bytes(false);
        let dependencies = publish_dependency_ids_or_framework_defaults(
            package
                .get_dependency_storage_package_ids()
                .iter()
                .map(|id| {
                    id.to_string()
                        .parse::<sui::types::Address>()
                        .expect("compiled package dependency id must parse as Sui address")
                }),
        );
        let tx =
            tap_tx::publish_package_ptb(&self.client.nexus_objects, modules, dependencies, address)
                .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;
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
        dag: DagSpec,
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
        let tx = tap_tx::create_agent_for_self_ptb(nexus_objects, address)
            .map_err(NexusError::TransactionBuilding)?;
        let response = self.client.submit_transaction(tx, address).await?;
        let event = find_event(&response, |kind| match kind {
            NexusEventKind::AgentCreated(event) => Some(event),
            _ => None,
        })
        .ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "AgentCreatedEvent not found in TAP create-agent response"
            ))
        })?;

        let agent_tag = crate::move_bindings::struct_tag::<Agent>(nexus_objects);

        Ok(CreateAgentResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: event.agent_id.into(),
            agent_object: response
                .objects
                .iter()
                .find_map(|object| match object.object_type() {
                    sui::types::ObjectType::Struct(tag) if tag == agent_tag => {
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
        let dag = self
            .client
            .crawler()
            .get_object_metadata(artifact.dag_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();
        let agent =
            agent_input_from_metadata(&agent_object).map_err(NexusError::TransactionBuilding)?;
        let tx = tap_tx::register_skill_ptb(nexus_objects, agent, &dag, artifact)
            .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;
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
            agent_id: event.agent_id.into(),
            skill_id: event.skill_id,
        })
    }

    /// Fetch live skill requirements from the configured TAP registry.
    pub async fn get_skill_requirements(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
    ) -> Result<GetSkillRequirementResult, NexusError> {
        let target = fetch_configured_active_tap_skill_execution_target(
            self.client.crawler(),
            &self.client.nexus_objects,
            agent_id,
            skill_id,
        )
        .await
        .map_err(NexusError::Rpc)?
        .data;

        Ok(GetSkillRequirementResult {
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
        let dag = self
            .client
            .crawler()
            .get_object_metadata(artifact.dag_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let agent =
            agent_input_from_metadata(&agent_object).map_err(NexusError::TransactionBuilding)?;
        let tx =
            tap_tx::update_skill_from_artifact_ptb(nexus_objects, agent, &dag, skill_id, artifact)
                .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;
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
            agent_id: event.agent_id.into(),
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
        let agent =
            agent_input_from_metadata(&agent_object).map_err(NexusError::TransactionBuilding)?;
        let tx =
            tap_tx::deposit_agent_payment_vault_for_self_ptb(nexus_objects, agent, params.amount)
                .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;
        Ok(DepositAgentVaultResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            agent_id: params.agent_id,
            amount: params.amount,
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

        let tx = dag_tx::refill_tap_execution_payment_for_self_ptb(
            &self.client.nexus_objects,
            &execution_ref,
            params.amount,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;
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

        let agent =
            agent_input_from_metadata(&agent_ref).map_err(NexusError::TransactionBuilding)?;
        let tx = dag_tx::refill_tap_execution_payment_from_agent_vault_for_self_ptb(
            &self.client.nexus_objects,
            agent,
            &execution_ref,
            params.amount,
        )
        .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;
        Ok(RefillExecutionPaymentResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            execution_id: params.execution_id,
            agent_id: Some(params.agent_id),
            amount: params.amount,
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
        let dag = self
            .client
            .crawler()
            .get_object_metadata(artifact.dag_id)
            .await
            .map_err(NexusError::Rpc)?
            .object_ref();

        let tx = tap_tx::bind_agent_skill_ptb(nexus_objects, &dag, &artifact)
            .map_err(NexusError::TransactionBuilding)?;

        let response = self.client.submit_transaction(tx, address).await?;

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

        let agent_tag = crate::move_bindings::struct_tag::<Agent>(nexus_objects);
        let agent_object = response
            .objects
            .iter()
            .find_map(|object| match object.object_type() {
                sui::types::ObjectType::Struct(tag) if tag == agent_tag => {
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
            agent_id: agent_event.agent_id.into(),
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
) -> anyhow::Result<Response<AgentRegistrySnapshot>> {
    let mut registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    registry.data.default_executor = fetch_default_dag_executor(crawler, registry.data.id).await?;

    Ok(registry)
}

async fn fetch_agent_registry_tables(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Response<AgentRegistrySnapshot>> {
    let raw = crawler.get_object::<AgentRegistry>(registry_id).await?;
    let agent_records = crawler
        .get_dynamic_fields::<sui::types::Address, AgentRecord>(
            raw.data.agents.id(),
            raw.data.agents.size(),
        )
        .await?;

    let mut agents = Vec::with_capacity(agent_records.len());
    let mut skills = Vec::new();
    for (agent_id, agent) in agent_records {
        let skill_records = crawler
            .get_dynamic_fields::<SkillId, SkillRecord>(agent.skills.id(), agent.skills.size())
            .await?;

        for (skill_id, skill) in skill_records {
            skills.push(SkillRecordContext {
                agent_id,
                skill_id,
                record: skill,
            });
        }
        agents.push(agent);
    }
    Ok(Response {
        object_id: raw.object_id,
        owner: raw.owner,
        version: raw.version,
        digest: raw.digest,
        balance: raw.balance,
        data: AgentRegistrySnapshot {
            id: raw.data.id.into(),
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
        .get_dynamic_fields::<DefaultDagExecutorFieldKey, DefaultDagExecutor>(registry_id, 0)
        .await
    {
        Ok(mut fields) => fields.remove(&DefaultDagExecutorFieldKey::default()),
        Err(key_error) => {
            let values = match crawler
                .get_dynamic_field_values::<DefaultDagExecutor>(registry_id)
                .await
            {
                Ok(values) => values
                    .into_iter()
                    .map(|(_value_type, value)| value)
                    .collect::<Vec<_>>(),
                Err(value_error) => crawler
                    .get_dynamic_field_object_values::<
                        DefaultDagExecutorFieldKey,
                        DefaultDagExecutor,
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
            values.into_iter().next()
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
    interface_revision: InterfaceVersion,
) -> anyhow::Result<Response<SkillRevisionContext>> {
    let registry = fetch_agent_registry_tables(crawler, registry_id).await?;
    let record = registry
        .data
        .skill_revision_record(SkillRevisionLookupKey {
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
) -> anyhow::Result<Response<SkillRevisionContext>> {
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
) -> anyhow::Result<Response<AgentRegistrySnapshot>> {
    fetch_agent_registry(crawler, *objects.agent_registry.object_id()).await
}

/// Resolve a fresh execution skill revision through the configured TAP registry.
pub async fn fetch_configured_active_tap_skill_revision(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<SkillRevisionContext>> {
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
        .get_object::<ExecutionPayment>(payment_id)
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
        match crawler.get_object::<ExecutionPayment>(field_id).await {
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
            AgentVaultFieldKey::default(),
        )
        .await
}

/// Resolve a fresh execution skill revision from already fetched records.
pub fn resolve_active_skill_revision_context<'a>(
    records: &'a [SkillRevisionContext],
    skills: &[SkillRecordContext],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&'a SkillRevisionContext, SkillRevisionLookupError> {
    resolve_active_tap_skill_revision(records, skills, agent_id, skill_id)
}

fn registry_response_with_data<T>(
    registry: Response<AgentRegistrySnapshot>,
    data: T,
) -> Response<T> {
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
            move_bindings::{
                interface::{
                    agent::{
                        self as agent_binding,
                        Agent,
                        SkillDagBinding,
                        SkillRequirement,
                        SkillSchedulePolicy,
                    },
                    payment::{ExecutionPaymentFinalState, PaymentSourceKind, SkillPaymentPolicy},
                    version::InterfaceVersion,
                },
                primitives::{data::NexusData, event as event_binding},
                registry::agent_registry::{
                    self as agent_registry_binding,
                    AgentRecord,
                    AgentRegistry,
                    DefaultDagExecutor,
                    DefaultDagExecutorFieldKey,
                    SkillRecord,
                },
                sui_framework::{table::Table as MoveTable, transfer as transfer_binding},
            },
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                AgentRegistrySnapshot,
                DefaultDagExecutorTarget,
                NexusObjects,
                SkillConfig,
                SkillRecordContext,
                SkillRevisionLookupKey,
            },
        },
    };

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
    struct Wrapper<T> {
        event: T,
    }

    fn submitted_ptb(
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
        let transaction = submitted_ptb(request);
        let objects = sui_mocks::mock_nexus_objects();
        let target = generated_target(
            &objects,
            transfer_binding::public_share_object_target::<Agent>,
        );
        assert!(!transaction.commands.iter().any(|command| matches!(
            command,
            sui::types::Command::MoveCall(call)
                if call.package == target.package
                    && call.module == target.module
                    && call.function == target.function
        )));
    }

    fn generated_target(
        objects: &NexusObjects,
        target: impl FnOnce() -> Result<sui_move_call::CallTarget, sui_move_call::CallSpecError>,
    ) -> sui_move_call::CallTarget {
        let tx = crate::move_boundary::ptb(objects, |tx| {
            tx.call_target(target, vec![])?;
            Ok(())
        })
        .expect("generated target");
        let Some(sui::types::Command::MoveCall(call)) = tx.commands.first() else {
            panic!("expected generated target move call");
        };
        sui_move_call::CallTarget {
            package: call.package,
            module: call.module.clone(),
            function: call.function.clone(),
            type_arguments: call.type_arguments.clone(),
        }
    }

    fn call_matches_generated(
        call: &sui::types::MoveCall,
        target: &sui_move_call::CallTarget,
    ) -> bool {
        call.package == target.package
            && call.module == target.module
            && call.function == target.function
            && call.type_arguments == target.type_arguments
    }

    fn event_wrapper_tag(
        objects: &NexusObjects,
        inner: sui::types::StructTag,
    ) -> sui::types::StructTag {
        let wrapper =
            crate::move_bindings::struct_tag::<event_binding::EventWrapper<NexusData>>(objects);
        sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        )
    }

    fn object_id(bytes: sui::types::Address) -> crate::move_bindings::sui_framework::object::ID {
        crate::move_bindings::sui_framework::object::ID::new(bytes)
    }

    fn skill_revision(revision: u64) -> SkillRevisionContext {
        SkillRevisionContext {
            key: SkillRevisionLookupKey {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
                interface_revision: InterfaceVersion::new(revision),
            },
            requirements: SkillRequirement {
                input_commitment: vec![2],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
        }
    }

    fn registry() -> AgentRegistrySnapshot {
        let agent = sui::types::Address::from_static("0xa");
        let skill_id = 11;
        let requirements = SkillRequirement {
            input_commitment: vec![2],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: Vec::new(),
        };

        AgentRegistrySnapshot {
            id: sui::types::Address::from_static("0xf"),
            agents: vec![AgentRecord {
                active: true,
                skills: MoveTable::new(sui::types::Address::from_static("0x90"), 1),
            }],
            skills: vec![SkillRecordContext {
                agent_id: agent,
                skill_id,
                record: SkillRecord {
                    description: vec![4],
                    active: true,
                    dag_binding: SkillDagBinding::pinned(sui::types::Address::from_static("0x3")),
                    requirements: requirements.clone(),
                    current_interface_revision: InterfaceVersion::new(2),
                    scheduled_task_count: 0,
                },
            }],
            default_executor: Some(DefaultDagExecutor {
                agent: Agent::from_ids(agent, 1, Some(sui::types::Address::from_static("0xf"))),
                skill_id,
            }),
        }
    }

    #[derive(Clone)]
    struct RegistryObjectMock {
        registry_object: AgentRegistry,
        agent_field_ref: sui::types::ObjectReference,
        skill_field_ref: sui::types::ObjectReference,
        default_executor_field_ref: Option<sui::types::ObjectReference>,
        default_executor_value: Option<DefaultDagExecutor>,
        agent_record: AgentRecord,
        skill_record: SkillRecord,
        skill_revision_record: SkillRevisionContext,
    }

    fn registry_object_mock(registry: &AgentRegistrySnapshot) -> RegistryObjectMock {
        assert_eq!(registry.agents.len(), 1, "test registry has one agent");
        assert_eq!(registry.skills.len(), 1, "test registry has one skill");
        let agent = registry.agents[0].clone();
        let skill_context = registry.skills[0].clone();
        let skill_record = skill_context.record.clone();
        let skill_revision_record = registry
            .active_skill_revision_record(skill_context.agent_id, skill_context.skill_id)
            .expect("active skill revision derives from skill");
        let agent_field_ref = sui_mocks::mock_sui_object_ref();
        let skill_field_ref = sui_mocks::mock_sui_object_ref();
        let default_executor_field_ref = registry
            .default_executor
            .as_ref()
            .map(|_| sui_mocks::mock_sui_object_ref());
        let default_executor_value = registry.default_executor.clone();

        RegistryObjectMock {
            registry_object: AgentRegistry {
                id: crate::move_bindings::sui_framework::object::UID::new(registry.id),
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
        registry: &AgentRegistrySnapshot,
    ) -> RegistryObjectMock {
        let mock = registry_object_mock(registry);
        sui_mocks::grpc::mock_get_object_bcs_for(
            ledger_service_mock,
            registry_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&mock.registry_object).expect("raw registry bcs"),
            crate::move_bindings::struct_tag::<AgentRegistry>(nexus_objects),
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
        registry: &AgentRegistrySnapshot,
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
                vec![(
                    DefaultDagExecutorFieldKey::default(),
                    *field_ref.object_id(),
                )],
            );
            sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
                ledger_service_mock,
                vec![(
                    field_ref,
                    sui::types::Owner::Shared(1),
                    DefaultDagExecutorFieldKey::default(),
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
                DefaultDagExecutor,
            >(ledger_service_mock, vec![]);
        }
    }

    fn mock_fetch_registry_tables_only(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &NexusObjects,
        registry_ref: sui::types::ObjectReference,
        registry: &AgentRegistrySnapshot,
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
    fn resolve_active_skill_revision_context_reuses_sdk_fail_closed_rule() {
        let records = vec![skill_revision(1), skill_revision(2)];
        let skills = vec![registry().skills[0].clone()];
        let resolved = resolve_active_skill_revision_context(
            &records,
            &skills,
            sui::types::Address::from_static("0xa"),
            11,
        )
        .expect("one active skill revision");

        assert_eq!(resolved.key.interface_revision, InterfaceVersion::new(2));
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

        assert_eq!(resolved.key.interface_revision, InterfaceVersion::new(2));
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
            *target.skill.dag_binding(),
            SkillDagBinding::pinned(sui::types::Address::from_static("0x3"))
        );
        assert_eq!(
            target.skill_revision.key.interface_revision,
            InterfaceVersion::new(2)
        );
    }

    #[test]
    fn configured_default_executor_reads_nexus_objects_metadata() {
        let objects = NexusObjects {
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: sui::types::Address::from_static("0xa"),
                skill_id: 11,
            },
            ..crate::test_utils::sui_mocks::mock_nexus_objects()
        };

        assert_eq!(
            objects.default_dag_executor,
            DefaultDagExecutorTarget {
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
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(std::sync::Arc::new(tokio::sync::Mutex::new(client)));

        let response = fetch_skill_revision(
            &crawler,
            registry.id,
            sui::types::Address::from_static("0xa"),
            11,
            InterfaceVersion::new(2),
        )
        .await
        .expect("skill revision recovery should not require default executor decoding");

        assert_eq!(
            response.data.key.interface_revision,
            InterfaceVersion::new(2)
        );
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
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(std::sync::Arc::new(tokio::sync::Mutex::new(client)));

        let response = fetch_agent_registry(&crawler, registry.id)
            .await
            .expect("full registry recovery decodes the default executor");

        assert_eq!(
            response
                .data
                .default_executor
                .as_ref()
                .map(DefaultDagExecutor::target),
            Some(DefaultDagExecutorTarget {
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
            event_wrapper_tag(objects, inner),
            bytes,
        )
    }

    fn artifact() -> TapPublishArtifact {
        let config = SkillConfig {
            name: "weather skill".to_string(),
            dag_path: std::path::PathBuf::from("dag.json"),
            requirements: SkillRequirement {
                input_commitment: vec![1],
                payment_policy: SkillPaymentPolicy::default(),
                schedule_policy: SkillSchedulePolicy::default(),
                fixed_tools: Vec::new(),
            },
            interface_revision: InterfaceVersion::new(1),
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
                    crate::move_bindings::struct_tag::<Agent>(&nexus_objects),
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
                    event: crate::move_bindings::interface::agent::AgentCreatedEvent {
                        agent_id: object_id(sui::types::Address::from_static("0xa")),
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
        let dag_ref = sui::types::ObjectReference::new(
            artifact.dag_id,
            1,
            sui::types::Digest::generate(&mut rng),
        );
        let agent_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xa"),
            1,
            sui::types::Digest::generate(&mut rng),
        );
        let agent_object = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    crate::move_bindings::struct_tag::<Agent>(&nexus_objects),
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
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
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
                        event: crate::move_bindings::interface::agent::AgentCreatedEvent {
                            agent_id: object_id(*agent_ref.object_id()),
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
                        event:
                            crate::move_bindings::registry::agent_registry::SkillRegisteredEvent {
                                agent_id: object_id(*agent_ref.object_id()),
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
        let dag_ref = sui::types::ObjectReference::new(
            artifact.dag_id,
            1,
            sui::types::Digest::generate(&mut rng),
        );
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
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
        let expected_register_target = generated_target(
            &nexus_objects,
            agent_registry_binding::register_skill_target,
        );
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
                    event: crate::move_bindings::registry::agent_registry::SkillRegisteredEvent {
                        agent_id: object_id(*agent_ref.object_id()),
                        skill_id: 11,
                        dag_id: artifact.dag_id,
                        dag_binding: SkillDagBinding::pinned(artifact.dag_id),
                    },
                })
                .unwrap(),
            )],
            move |request| {
                let transaction = submitted_ptb(request);
                let commands = &transaction.commands;
                assert!(commands.iter().any(|command| matches!(
                    command,
                    sui::types::Command::MoveCall(call)
                        if call_matches_generated(call, &expected_register_target)
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
        let dag_ref = sui::types::ObjectReference::new(
            artifact.dag_id,
            1,
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
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(dag_ref.version()),
            None,
        );
        let expected_update_dag_target =
            generated_target(&nexus_objects, agent_registry_binding::update_dag_target);
        let expected_update_policies_target = generated_target(
            &nexus_objects,
            agent_registry_binding::update_skill_policies_target,
        );
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
                    event: crate::move_bindings::registry::agent_registry::SkillContractRevisionedEvent {
                        agent_id: object_id(*agent_ref.object_id()),
                        skill_id: 11,
                        current_interface_revision: InterfaceVersion::new(2),
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

                assert!(move_calls
                    .iter()
                    .any(|call| call_matches_generated(call, &expected_update_dag_target)));
                assert!(move_calls
                    .iter()
                    .any(|call| call_matches_generated(call, &expected_update_policies_target)));
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
        assert_eq!(result.current_interface_revision, InterfaceVersion::new(2));
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
        let expected_deposit_target = generated_target(
            &nexus_objects,
            agent_binding::deposit_agent_payment_vault_target,
        );
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
                        if call_matches_generated(call, &expected_deposit_target)
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
            InterfaceVersion::new(2)
        );
        assert_eq!(result.requirements.input_commitment, vec![2]);
    }

    fn baseline_payment(
        accomplished: bool,
        refunded: bool,
        final_state: ExecutionPaymentFinalState,
    ) -> ExecutionPayment {
        ExecutionPayment {
            id: crate::move_bindings::sui_framework::object::UID::new(
                sui::types::Address::from_static("0x1"),
            ),
            execution_id: sui::types::Address::from_static("0x2"),
            agent_id: crate::move_bindings::sui_framework::object::ID::new(
                sui::types::Address::from_static("0xa"),
            ),
            skill_id: 11,
            interface_revision: InterfaceVersion::new(1),
            payment_policy:
                crate::move_bindings::interface::payment::SkillPaymentPolicy::UserFunded,
            source_kind: PaymentSourceKind::user_funded(sui::types::Address::from_static("0x1")),
            max_budget_mist: 1_000_000,
            gas_budget_mist: 833_334,
            priority_fee_reserve_mist: 166_666,
            locked_budget_mist: 0,
            funds: crate::move_bindings::sui_framework::balance::Balance {
                value: 1_000_000,
                phantom_t0: std::marker::PhantomData,
            },
            consumed: 0,
            tool_fee_charged: 0,
            priority_fee_charged: 0,
            priority_fee_percentage: 20,
            tool_cost_snapshot: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
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
            id: crate::move_bindings::sui_framework::object::UID::new(
                sui::types::Address::from_static("0x1"),
            ),
            execution_id: sui::types::Address::from_static("0x2"),
            agent_id: crate::move_bindings::sui_framework::object::ID::new(agent_id),
            skill_id: 11,
            interface_revision: InterfaceVersion::new(1),
            payment_policy:
                crate::move_bindings::interface::payment::SkillPaymentPolicy::AgentFunded {
                    max_budget_mist: 100,
                },
            source_kind: PaymentSourceKind::agent_funded(agent_id),
            max_budget_mist: 100,
            gas_budget_mist: 84,
            priority_fee_reserve_mist: 16,
            locked_budget_mist: 0,
            funds: crate::move_bindings::sui_framework::balance::Balance {
                value: 100,
                phantom_t0: std::marker::PhantomData,
            },
            consumed: 0,
            tool_fee_charged: 0,
            priority_fee_charged: 0,
            priority_fee_percentage: 20,
            accomplished: false,
            refunded: false,
            final_state: ExecutionPaymentFinalState::Pending,
            tool_cost_snapshot: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
            locked_vertices: vec![],
        };

        assert_eq!(
            payment.payment_policy,
            crate::move_bindings::interface::payment::SkillPaymentPolicy::AgentFunded {
                max_budget_mist: 100
            }
        );
        assert_eq!(
            payment.source_kind,
            PaymentSourceKind::agent_funded(agent_id)
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
