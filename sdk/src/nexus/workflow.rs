//! Commands related to workflow management in Nexus.
//!
//! - [`WorkflowActions::publish`] to publish a [`Dag`] instance to Nexus.
//! - [`WorkflowActions::execute`] to execute a published DAG.
//! - [`WorkflowActions::inspect_execution`] to monitor the execution of a DAG.

use {
    crate::{
        events::{
            EndStateReachedEvent,
            EventPage,
            ExecutionFinishedEvent,
            NexusEvent,
            NexusEventKind,
            TerminalErrEvalRecordedEvent,
        },
        idents::workflow,
        nexus::{
            client::NexusClient,
            crawler::{Crawler, Response},
            error::NexusError,
            models::{Dag, DagExecution, DagVertexInfo},
            tap,
        },
        sui,
        transactions::dag,
        types::{
            resolve_active_tap_skill_execution_target,
            resolve_default_tap_dag_executor,
            tap_payment_source_for_address,
            validate_authorization_plan,
            validate_standard_tap_payment_options,
            AgentId,
            Dag as JsonDag,
            DataStorage,
            PortsData,
            RuntimeVertex,
            SkillId,
            StorageConf,
            TapDagBinding,
            DefaultDagExecutorRecord,
            TapEndpointKey,
            TapExecutionPayment,
            TapRegistry,
            TapVertexAuthorizationPlan,
            TapVertexAuthorizationPlanEntry,
            VerifierConfig,
            VerifierMode,
            WorkflowFailureClass,
            DEFAULT_ENTRY_GROUP,
        },
    },
    anyhow::anyhow,
    std::collections::HashMap,
    tokio::{
        sync::mpsc::{unbounded_channel, UnboundedReceiver},
        task::JoinHandle,
        time::Duration,
    },
};

#[derive(Clone, Debug)]
pub struct PublishResult {
    pub tx_digest: sui::types::Digest,
    pub tx_checkpoint: u64,
    pub dag_object_id: sui::types::Address,
}

pub struct ExecuteResult {
    pub tx_digest: sui::types::Digest,
    pub execution_object_id: sui::types::Address,
    pub tx_checkpoint: u64,
    pub standard_tap: Option<StandardTapSubmitMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandardTapSubmitMetadata {
    pub agent_id: AgentId,
    pub skill_id: SkillId,
    pub dag_id: sui::types::Address,
    pub endpoint_key: TapEndpointKey,
    pub endpoint_object: sui::types::ObjectReference,
    pub payment_max_budget: u64,
    pub payment_refund_mode: u8,
    pub authorization_plan_commitment: Option<Vec<u8>>,
    pub authorization_plan: TapVertexAuthorizationPlan,
}

#[derive(Clone, Debug, Default)]
pub struct StandardTapExecuteOptions {
    pub payment_source: Vec<u8>,
    pub payment_coin: Option<sui::types::ObjectReference>,
    pub payment_coin_balance: Option<u64>,
    pub payment_max_budget: u64,
    pub payment_refund_mode: u8,
    pub authorization_plan_commitment: Option<Vec<u8>>,
    pub authorization_plan: Vec<TapVertexAuthorizationPlanEntry>,
}

fn resolve_default_standard_tap_dag_executor(
    objects: &crate::types::NexusObjects,
    registry: &TapRegistry,
) -> anyhow::Result<DefaultDagExecutorRecord> {
    if let Some(configured) = objects.default_tap_target() {
        if let Ok(target) = resolve_active_tap_skill_execution_target(
            registry,
            configured.agent_id,
            configured.skill_id,
        ) {
            if target.skill.dag_binding == TapDagBinding::RuntimeSelected {
                return Ok(DefaultDagExecutorRecord {
                    target: configured,
                    skill: target.skill,
                    endpoint: target.endpoint,
                });
            }
        }
    }

    resolve_default_tap_dag_executor(registry)
}

pub struct InspectExecutionResult {
    pub next_event: UnboundedReceiver<NexusEvent>,
    pub poller: JoinHandle<Result<(), NexusError>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowExecutionTerminalState {
    Succeeded,
    Failed,
    Aborted,
    NoWalkOutcome,
}

#[derive(Clone, Debug)]
pub struct ResolvedEndState {
    pub event: EndStateReachedEvent,
    pub resolved_ports_to_data: HashMap<String, DataStorage>,
}

#[derive(Clone, Debug)]
pub struct InspectExecutionCompletionResult {
    pub terminal_state: WorkflowExecutionTerminalState,
    pub execution_finished: ExecutionFinishedEvent,
    pub end_states: Vec<ResolvedEndState>,
    pub terminal_err_eval_recordings: Vec<TerminalErrEvalRecordedEvent>,
    pub events: Vec<NexusEvent>,
}

pub struct ExecutionCostResult {
    pub payment_id: sui::types::Address,
    pub max_budget: u64,
    pub locked_budget: u64,
    pub consumed: u64,
    pub outstanding_locks: u64,
    pub accomplished: bool,
    pub refunded: bool,
}

pub struct WorkflowActions {
    pub(super) client: NexusClient,
}

fn event_execution_id(event: &NexusEventKind) -> Option<sui::types::Address> {
    match event {
        NexusEventKind::WalkAdvanced(e) => Some(e.execution),
        NexusEventKind::WalkFailed(e) => Some(e.execution),
        NexusEventKind::TerminalErrEvalRecorded(e) => Some(e.execution),
        NexusEventKind::WalkAborted(e) => Some(e.execution),
        NexusEventKind::WalkCancelled(e) => Some(e.execution),
        NexusEventKind::EndStateReached(e) => Some(e.execution),
        NexusEventKind::ExecutionFinished(e) => Some(e.execution),
        _ => None,
    }
}

fn terminal_state_from_execution_finished(
    execution_finished: &ExecutionFinishedEvent,
) -> WorkflowExecutionTerminalState {
    if execution_finished.was_aborted {
        WorkflowExecutionTerminalState::Aborted
    } else if execution_finished.has_any_walk_failed {
        WorkflowExecutionTerminalState::Failed
    } else if execution_finished.has_any_walk_succeeded {
        WorkflowExecutionTerminalState::Succeeded
    } else {
        WorkflowExecutionTerminalState::NoWalkOutcome
    }
}

async fn build_execution_completion_result(
    events: Vec<NexusEvent>,
    dag_execution_id: sui::types::Address,
    storage_conf: &StorageConf,
) -> Result<InspectExecutionCompletionResult, NexusError> {
    let mut end_states = Vec::new();
    let mut terminal_err_eval_recordings = Vec::new();
    let mut execution_finished = None;

    for event in &events {
        match &event.data {
            NexusEventKind::EndStateReached(end_state) => {
                let resolved_ports_to_data = end_state
                    .variant_ports_to_data
                    .clone()
                    .fetch_all(storage_conf)
                    .await
                    .map_err(|e| {
                        NexusError::Storage(anyhow!(
                            "Failed to fetch output data for execution '{dag_execution_id}': {e}"
                        ))
                    })?;

                end_states.push(ResolvedEndState {
                    event: end_state.clone(),
                    resolved_ports_to_data,
                });
            }
            NexusEventKind::TerminalErrEvalRecorded(recorded) => {
                terminal_err_eval_recordings.push(recorded.clone());
            }
            NexusEventKind::ExecutionFinished(finished) => {
                execution_finished = Some(finished.clone());
            }
            _ => {}
        }
    }

    let execution_finished = execution_finished.ok_or_else(|| {
        NexusError::Channel(anyhow!(
            "ExecutionFinished event not found while inspecting DAG execution '{dag_execution_id}'"
        ))
    })?;

    Ok(InspectExecutionCompletionResult {
        terminal_state: terminal_state_from_execution_finished(&execution_finished),
        execution_finished,
        end_states,
        terminal_err_eval_recordings,
        events,
    })
}

pub fn verifier_mode_requires_proof(mode: VerifierMode) -> bool {
    matches!(
        mode,
        VerifierMode::LeaderRegisteredKey
            | VerifierMode::LeaderNautilusEnclave
            | VerifierMode::ToolVerifierContract
    )
}

pub fn effective_verifier_config(
    dag_default: &VerifierConfig,
    vertex_override: Option<&VerifierConfig>,
) -> VerifierConfig {
    vertex_override
        .cloned()
        .unwrap_or_else(|| dag_default.clone())
}

pub fn dag_vertex_requires_verifier_proof(dag: &Dag, vertex: &DagVertexInfo) -> bool {
    verifier_mode_requires_proof(
        effective_verifier_config(&dag.leader_verifier, vertex.leader_verifier.as_ref()).mode,
    ) || verifier_mode_requires_proof(
        effective_verifier_config(&dag.tool_verifier, vertex.tool_verifier.as_ref()).mode,
    )
}

pub async fn offchain_success_requires_verifier_proof(
    crawler: &Crawler,
    dag_object_id: sui::types::Address,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    let dag = crawler.get_object::<Dag>(dag_object_id).await?;
    let mut vertices = crawler.get_dynamic_fields(&dag.data.vertices).await?;
    let vertex_name = next_vertex.name();
    let vertex = vertices
        .remove(&vertex_name)
        .ok_or_else(|| anyhow!("Vertex '{vertex_name}' not found in DAG verifier config"))?;

    Ok(dag_vertex_requires_verifier_proof(&dag.data, &vertex))
}

pub async fn fetch_vertex_input_port_names(
    crawler: &Crawler,
    dag: &Dag,
    vertex_name: &crate::types::TypeName,
) -> anyhow::Result<Vec<String>> {
    let mut vertices = crawler.get_dynamic_fields(&dag.vertices).await?;
    let vertex = vertices.remove(vertex_name).ok_or_else(|| {
        anyhow!("Vertex '{vertex_name}' not found in DAG vertices dynamic fields")
    })?;

    Ok(vertex.declared_input_port_names())
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct ExecutionErrEvalArbitrationState {
    #[serde(default)]
    terminal_records: serde_json::Value,
}

async fn fetch_execution_err_eval_state(
    crawler: &Crawler,
    object_id: sui::types::Address,
) -> anyhow::Result<Response<ExecutionErrEvalArbitrationState>> {
    crawler.get_object(object_id).await
}

pub fn execution_terminal_record_matches_retryable_vertex(
    value: &serde_json::Value,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(record_value) = object.get(&walk_index.to_string()) {
                if let Some(record) =
                    crate::types::parse_execution_terminal_record_value(record_value)?
                {
                    return Ok(&record.vertex == next_vertex
                        && record.failure_class == WorkflowFailureClass::Retryable);
                }
            }

            if let (Some(key), Some(record_value)) = (object.get("key"), object.get("value")) {
                if crate::types::parse_u64_value(key)? == Some(walk_index) {
                    if let Some(record) =
                        crate::types::parse_execution_terminal_record_value(record_value)?
                    {
                        return Ok(&record.vertex == next_vertex
                            && record.failure_class == WorkflowFailureClass::Retryable);
                    }
                }
            }

            for nested_key in ["contents", "entries", "fields", "inner", "vec", "value"] {
                if let Some(nested) = object.get(nested_key) {
                    if execution_terminal_record_matches_retryable_vertex(
                        nested,
                        walk_index,
                        next_vertex,
                    )? {
                        return Ok(true);
                    }
                }
            }

            for (name, nested) in object {
                if matches!(
                    name.as_str(),
                    "contents" | "entries" | "fields" | "inner" | "vec" | "value"
                ) {
                    continue;
                }

                if execution_terminal_record_matches_retryable_vertex(
                    nested,
                    walk_index,
                    next_vertex,
                )? {
                    return Ok(true);
                }
            }

            Ok(false)
        }
        serde_json::Value::Array(values) => {
            for nested in values {
                if execution_terminal_record_matches_retryable_vertex(
                    nested,
                    walk_index,
                    next_vertex,
                )? {
                    return Ok(true);
                }
            }

            Ok(false)
        }
        _ => Ok(false),
    }
}

pub async fn should_settle_tool_err_eval_gas(
    crawler: &Crawler,
    execution: sui::types::Address,
    walk_index: u64,
    next_vertex: &RuntimeVertex,
) -> anyhow::Result<bool> {
    let state = fetch_execution_err_eval_state(crawler, execution).await?;
    execution_terminal_record_matches_retryable_vertex(
        &state.data.terminal_records,
        walk_index,
        next_vertex,
    )
}

impl WorkflowActions {
    /// Publish the provided JSON [`Dag`].
    pub async fn publish(&self, json_dag: JsonDag) -> Result<PublishResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        // == Craft and submit the publish DAG transaction ==

        let mut tx = sui::tx::TransactionBuilder::new();

        let mut dag_arg = dag::empty(&mut tx, nexus_objects);

        dag_arg = match dag::create(&mut tx, nexus_objects, dag_arg, json_dag) {
            Ok(dag_arg) => dag_arg,
            Err(e) => {
                return Err(NexusError::TransactionBuilding(e));
            }
        };

        dag::publish(&mut tx, nexus_objects, dag_arg);

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

        // == Find the published DAG object ID ==

        let dag_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && *object_type.module() == workflow::Dag::DAG.module
                    && *object_type.name() == workflow::Dag::DAG.name
                {
                    Some(obj.object_id())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG object ID not found in TX response"))
            })?;

        Ok(PublishResult {
            tx_digest: response.digest,
            tx_checkpoint: response.checkpoint,
            dag_object_id,
        })
    }

    /// Execute a published DAG through the configured standard default TAP DAG executor.
    ///
    /// The `entry_data` [`HashMap`] already holds information about the storage
    /// kind for each port.
    ///
    /// `storage_conf` can accept [`StorageConf::default`] if no remote storage
    /// is expected.
    ///
    /// `priority_fee_per_gas_unit` is the per-transaction priority fee to pass
    /// down to the DAG execution.
    ///
    /// Use [`WorkflowActions::inspect_execution`] to monitor the execution.
    pub async fn execute(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, PortsData>,
        priority_fee_per_gas_unit: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
    ) -> Result<ExecuteResult, NexusError> {
        let address = self.client.signer.get_active_address();
        self.execute_standard_tap_default(
            dag_object_id,
            entry_data,
            priority_fee_per_gas_unit,
            entry_group,
            storage_conf,
            StandardTapExecuteOptions {
                payment_source: tap_payment_source_for_address(address)
                    .map_err(NexusError::TransactionBuilding)?,
                payment_coin: None,
                payment_coin_balance: None,
                payment_max_budget: self.client.gas.get_budget(),
                payment_refund_mode: 0,
                authorization_plan_commitment: None,
                authorization_plan: Vec::new(),
            },
        )
        .await
    }

    /// Execute a published DAG through the configured standard default TAP DAG executor
    /// with explicit standard payment options.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_standard_tap_default(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, PortsData>,
        priority_fee_per_gas_unit: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
        options: StandardTapExecuteOptions,
    ) -> Result<ExecuteResult, NexusError> {
        // == Commit data to their respective storage ==

        let mut input_data = HashMap::new();

        for (vertex, ports_data) in entry_data {
            let committed_data = ports_data.commit_all(storage_conf).await.map_err(|e| {
                NexusError::Storage(anyhow!("Failed to commit data for port '{vertex}': {e}"))
            })?;

            input_data.insert(vertex, committed_data);
        }

        // == Craft and submit the execute DAG transaction ==

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let dag = self
            .client
            .crawler()
            .get_object::<Dag>(dag_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;

        let registry = tap::fetch_configured_tap_registry(self.client.crawler(), nexus_objects)
            .await
            .map_err(NexusError::Rpc)?;
        let default_executor =
            resolve_default_standard_tap_dag_executor(nexus_objects, &registry.data)
                .map_err(NexusError::Parsing)?;
        let agent = self
            .client
            .crawler()
            .get_object_metadata(default_executor.target.agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();
        validate_standard_tap_payment_options(
            default_executor.target.agent_id,
            &default_executor.endpoint.requirements.payment_policy,
            &options.payment_source,
            options.payment_max_budget,
            options.payment_refund_mode,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        if let Some(balance) = options.payment_coin_balance {
            if balance < options.payment_max_budget {
                return Err(NexusError::TransactionBuilding(anyhow!(
                    "standard TAP payment coin balance {balance} is below requested budget {}",
                    options.payment_max_budget
                )));
            }
        }
        let authorization_plan = TapVertexAuthorizationPlan(options.authorization_plan.clone());
        if !authorization_plan.is_empty() || options.authorization_plan_commitment.is_some() {
            validate_authorization_plan(
                &default_executor.endpoint.requirements,
                &authorization_plan,
                options.authorization_plan_commitment.as_deref(),
            )
            .map_err(|error| NexusError::TransactionBuilding(error.into()))?;
        }
        let authorization_plan_commitment = if authorization_plan.is_empty() {
            options.authorization_plan_commitment.clone()
        } else {
            Some(
                authorization_plan
                    .hash()
                    .map_err(NexusError::TransactionBuilding)?,
            )
        };
        let standard = dag::StandardTapExecuteInput {
            agent_id: default_executor.target.agent_id,
            skill_id: default_executor.target.skill_id,
            payment_source: options.payment_source,
            payment_coin: options.payment_coin,
            payment_coin_balance: options.payment_coin_balance,
            payment_max_budget: options.payment_max_budget,
            payment_refund_mode: options.payment_refund_mode,
            authorization_plan_commitment: authorization_plan_commitment.clone(),
            authorization_plan: authorization_plan.0.clone(),
        };

        if let Err(e) = dag::execute_default_standard_tap(
            &mut tx,
            nexus_objects,
            &dag.object_ref(),
            &agent.object_ref(),
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &input_data,
            &standard,
            &tools_gas,
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
        let owned_payment_coin = standard
            .payment_coin
            .as_ref()
            .map(|payment_coin| *payment_coin.object_id());

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;
        if let Some(payment_coin_id) = owned_payment_coin {
            if let Some(updated_payment_coin) = response
                .objects
                .iter()
                .find(|object| object.object_id() == payment_coin_id)
            {
                let payment_gas_config = self.client.gas_config();
                payment_gas_config
                    .release_gas_coin(sui::types::ObjectReference::new(
                        updated_payment_coin.object_id(),
                        updated_payment_coin.version(),
                        updated_payment_coin.digest(),
                    ))
                    .await;
            }
        }

        // == Find the created DAG execution object ID ==

        let execution_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && *object_type.module() == workflow::Dag::DAG_EXECUTION.module
                    && *object_type.name() == workflow::Dag::DAG_EXECUTION.name
                {
                    Some(obj.object_id())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG execution object ID not found in TX response"))
            })?;

        Ok(ExecuteResult {
            tx_digest: response.digest,
            execution_object_id,
            tx_checkpoint: response.checkpoint,
            standard_tap: Some(StandardTapSubmitMetadata {
                agent_id: default_executor.target.agent_id,
                skill_id: default_executor.target.skill_id,
                dag_id: dag.object_id,
                endpoint_key: default_executor.endpoint.key,
                endpoint_object: default_executor.endpoint.endpoint_object,
                payment_max_budget: options.payment_max_budget,
                payment_refund_mode: options.payment_refund_mode,
                authorization_plan_commitment,
                authorization_plan,
            }),
        })
    }

    /// Execute the active standard TAP skill for `(agent_id, skill_id)`.
    ///
    /// This resolves the registered DAG from the configured TAP registry, then
    /// calls the standard workflow entry instead of the legacy default TAP entry.
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_standard_tap(
        &self,
        agent_id: AgentId,
        skill_id: SkillId,
        entry_data: HashMap<String, PortsData>,
        priority_fee_per_gas_unit: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
        options: StandardTapExecuteOptions,
    ) -> Result<ExecuteResult, NexusError> {
        let mut input_data = HashMap::new();

        for (vertex, ports_data) in entry_data {
            let committed_data = ports_data.commit_all(storage_conf).await.map_err(|e| {
                NexusError::Storage(anyhow!("Failed to commit data for port '{vertex}': {e}"))
            })?;

            input_data.insert(vertex, committed_data);
        }

        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;
        let target = tap::fetch_configured_active_tap_skill_execution_target(
            self.client.crawler(),
            nexus_objects,
            agent_id,
            skill_id,
        )
        .await
        .map_err(NexusError::Rpc)?
        .data;

        let dag_id = match target.skill.dag_binding {
            TapDagBinding::Pinned { dag_id } => dag_id,
            TapDagBinding::RuntimeSelected => {
                return Err(NexusError::Parsing(anyhow!(
                    "runtime-selected skill {agent_id}:{skill_id} requires an explicit DAG selection; use WorkflowActions::execute or `nexus dag execute`"
                )));
            }
        };

        let dag = self
            .client
            .crawler()
            .get_object::<Dag>(dag_id)
            .await
            .map_err(NexusError::Rpc)?;

        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;
        let agent_object = self
            .client
            .crawler()
            .get_object_metadata(agent_id)
            .await
            .map_err(NexusError::Rpc)?;

        let mut tx = sui::tx::TransactionBuilder::new();
        let authorization_plan = TapVertexAuthorizationPlan(options.authorization_plan.clone());
        validate_authorization_plan(
            &target.endpoint.requirements,
            &authorization_plan,
            options.authorization_plan_commitment.as_deref(),
        )
        .map_err(|error| NexusError::TransactionBuilding(error.into()))?;
        let authorization_plan_commitment = if authorization_plan.is_empty() {
            options.authorization_plan_commitment.clone()
        } else {
            Some(
                authorization_plan
                    .hash()
                    .map_err(NexusError::TransactionBuilding)?,
            )
        };
        validate_standard_tap_payment_options(
            agent_id,
            &target.endpoint.requirements.payment_policy,
            &options.payment_source,
            options.payment_max_budget,
            options.payment_refund_mode,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        if let Some(balance) = options.payment_coin_balance {
            if balance < options.payment_max_budget {
                return Err(NexusError::TransactionBuilding(anyhow!(
                    "standard TAP payment coin balance {balance} is below requested budget {}",
                    options.payment_max_budget
                )));
            }
        }
        let standard = dag::StandardTapExecuteInput {
            agent_id,
            skill_id,
            payment_source: options.payment_source,
            payment_coin: options.payment_coin,
            payment_coin_balance: options.payment_coin_balance,
            payment_max_budget: options.payment_max_budget,
            payment_refund_mode: options.payment_refund_mode,
            authorization_plan_commitment: authorization_plan_commitment.clone(),
            authorization_plan: authorization_plan.0.clone(),
        };

        if let Err(e) = dag::execute_standard_tap(
            &mut tx,
            nexus_objects,
            &dag.object_ref(),
            &agent_object.object_ref(),
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &input_data,
            &standard,
            &tools_gas,
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
        let owned_payment_coin = standard
            .payment_coin
            .as_ref()
            .map(|payment_coin| *payment_coin.object_id());

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;
        if let Some(payment_coin_id) = owned_payment_coin {
            if let Some(updated_payment_coin) = response
                .objects
                .iter()
                .find(|object| object.object_id() == payment_coin_id)
            {
                self.client
                    .gas
                    .release_gas_coin(sui::types::ObjectReference::new(
                        updated_payment_coin.object_id(),
                        updated_payment_coin.version(),
                        updated_payment_coin.digest(),
                    ))
                    .await;
            }
        }

        let execution_object_id = response
            .objects
            .into_iter()
            .find_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if nexus_objects.is_workflow_package(*object_type.address())
                    && *object_type.module() == workflow::Dag::DAG_EXECUTION.module
                    && *object_type.name() == workflow::Dag::DAG_EXECUTION.name
                {
                    Some(obj.object_id())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!("DAG execution object ID not found in TX response"))
            })?;

        Ok(ExecuteResult {
            tx_digest: response.digest,
            execution_object_id,
            tx_checkpoint: response.checkpoint,
            standard_tap: Some(StandardTapSubmitMetadata {
                agent_id,
                skill_id,
                dag_id,
                endpoint_key: target.endpoint.key,
                endpoint_object: target.endpoint.endpoint_object,
                payment_max_budget: options.payment_max_budget,
                payment_refund_mode: options.payment_refund_mode,
                authorization_plan_commitment,
                authorization_plan,
            }),
        })
    }

    /// Inspect a DAG execution based on the provided execution object ID and
    /// transaction digest.
    ///
    /// Channel sender will drop once we find an `ExecutionFinished` event or
    /// timeout occurs.
    ///
    /// The poller task is also returned so that the user can ensure its
    /// completion.
    pub async fn inspect_execution(
        &self,
        dag_execution_id: sui::types::Address,
        execution_checkpoint: u64,
        timeout: Option<Duration>,
    ) -> Result<InspectExecutionResult, NexusError> {
        // Setup MSPC channel.
        let (tx, rx) = unbounded_channel::<NexusEvent>();

        // Create some initial timings and restrictions.
        let timeout = timeout.unwrap_or(Duration::from_secs(3600));
        let poller = self.client.event_poller().clone();
        let mut next_page = poller
            .start_polling(Some(execution_checkpoint))
            .map_err(|e| NexusError::Configuration(format!("{e}")))?;

        let poller = {
            tokio::spawn(async move {
                let timeout = tokio::time::sleep(timeout);

                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        maybe_page = next_page.recv() => {
                            let events = match maybe_page {
                                Some(Ok(EventPage { events, .. })) => events,
                                Some(Err(e)) => return Err(NexusError::Channel(anyhow!("Error fetching events: {}", e))),
                                None => return Err(NexusError::Channel(anyhow!("Event stream closed unexpectedly while inspecting DAG execution '{dag_execution_id}'"))),
                            };

                            let mut execution_finished_seen = false;

                            for event in events {
                                let Some(execution_id) = event_execution_id(&event.data) else {
                                    continue;
                                };

                                // Only process events for the given execution ID.
                                if execution_id != dag_execution_id {
                                    continue;
                                }

                                if matches!(&event.data, NexusEventKind::ExecutionFinished(_)) {
                                    tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;
                                    execution_finished_seen = true;
                                    continue;
                                }

                                tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;
                            }

                            if execution_finished_seen {
                                return Ok(());
                            }
                        }

                        _ = &mut timeout => {
                            return Err(NexusError::Timeout(anyhow!("Timeout {timeout:?} reached while inspecting DAG execution '{dag_execution_id}'")));
                        }
                    }
                }
            })
        };

        Ok(InspectExecutionResult {
            next_event: rx,
            poller,
        })
    }

    /// Inspect a DAG execution until completion and return a structured summary
    /// with resolved end-state data.
    pub async fn inspect_execution_until_completion(
        &self,
        dag_execution_id: sui::types::Address,
        execution_checkpoint: u64,
        timeout: Option<Duration>,
        storage_conf: &StorageConf,
    ) -> Result<InspectExecutionCompletionResult, NexusError> {
        let mut inspection = self
            .inspect_execution(dag_execution_id, execution_checkpoint, timeout)
            .await?;

        let mut events = Vec::new();

        while let Some(event) = inspection.next_event.recv().await {
            events.push(event);
        }

        let poller_result = inspection.poller.await.map_err(|e| {
            NexusError::Channel(anyhow!(
                "Execution inspection task failed for DAG execution '{dag_execution_id}': {e}"
            ))
        })?;
        poller_result?;

        build_execution_completion_result(events, dag_execution_id, storage_conf).await
    }

    /// Fetch the standard TAP execution payment cost summary for a DAG
    /// execution.
    pub async fn execution_cost(
        &self,
        dag_execution_id: sui::types::Address,
    ) -> Result<ExecutionCostResult, NexusError> {
        let crawler = self.client.crawler();
        let execution = crawler
            .get_object::<DagExecution>(dag_execution_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;
        let context = execution
            .standard_tap_context()
            .map_err(NexusError::Parsing)?
            .ok_or_else(|| {
                NexusError::Parsing(anyhow!(
                    "DAG execution '{dag_execution_id}' has no standard TAP payment context"
                ))
            })?;
        let payment = tap::fetch_tap_execution_payment(crawler, context.payment_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;

        Ok(ExecutionCostResult::from_payment(payment))
    }
}

impl ExecutionCostResult {
    fn from_payment(payment: TapExecutionPayment) -> Self {
        Self {
            payment_id: payment.payment_id(),
            max_budget: payment.max_budget,
            locked_budget: payment.locked_budget,
            consumed: payment.consumed,
            outstanding_locks: payment.outstanding_locks(),
            accomplished: payment.accomplished,
            refunded: payment.refunded,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            events::{
                EndStateReachedEvent,
                ExecutionFinishedEvent,
                NexusEventKind,
                TerminalErrEvalRecordedEvent,
                WalkAdvancedEvent,
            },
            fqn,
            nexus::{
                crawler::{DynamicMap, Set},
                models::{Dag, DagVertexInfo, DagVertexKind},
            },
            sui::traits::*,
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                derive_tool_gas_id,
                InterfaceRevision,
                MoveTable,
                NexusData,
                PostFailureAction,
                RuntimeVertex,
                Storable,
                TapAgentRecord,
                TapDagBinding,
                DefaultDagExecutor,
                TapEndpointActivation,
                TapEndpointRevision,
                TapEndpointRevisionKey,
                TapPaymentPolicy,
                TapRegistry,
                TapRegistryObject,
                TapSchedulePolicy,
                TapSharedObjectRef,
                TapSkillRecord,
                TapSkillRequirements,
                TapVertexAuthorizationSchema,
                TypeName,
                VerifierConfig,
                VerifierMode,
                WorkflowFailureClass,
            },
        },
        serde::Serialize,
        serde_json::json,
    };

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

        RegistryObjectMock {
            registry_object: TapRegistryObject {
                id: registry.id,
                agents: MoveTable::new(sui::types::Address::from_static("0x9000"), 1),
                default_executor: registry.default_executor.into(),
            },
            agent_field_ref: sui_mocks::mock_sui_object_ref(),
            skill_field_ref: sui_mocks::mock_sui_object_ref(),
            endpoint_field_ref: sui_mocks::mock_sui_object_ref(),
            agent_record: agent,
            skill_record,
            endpoint_record,
        }
    }

    fn mock_fetch_registry_from_tables(
        ledger_service_mock: &mut sui_mocks::grpc::MockLedgerService,
        state_service_mock: &mut sui_mocks::grpc::MockStateService,
        nexus_objects: &crate::types::NexusObjects,
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

    fn mock_events_get_checkpoint_with_supported_events(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        objects: crate::types::NexusObjects,
        nexus_events: Vec<NexusEventKind>,
        cp: u64,
    ) {
        ledger_service
            .expect_get_checkpoint()
            .returning(move |_request| {
                let mut response = sui::grpc::GetCheckpointResponse::default();
                let mut checkpoint = sui::grpc::Checkpoint::default();
                let mut transactions = vec![];
                for _ in 0..10 {
                    let mut transaction = sui::grpc::ExecutedTransaction::default();
                    transaction.set_digest(sui::types::Digest::ZERO);
                    transactions.push(transaction);
                }
                checkpoint.set_transactions(transactions);
                checkpoint.set_sequence_number(cp);
                response.set_checkpoint(checkpoint);
                Ok(tonic::Response::new(response))
            });

        ledger_service
            .expect_batch_get_transactions()
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetTransactionsResponse::default();
                let mut result = sui::grpc::GetTransactionResult::default();
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(sui::types::Digest::ZERO);
                transaction.set_checkpoint(1);
                let mut events = vec![];

                #[derive(Serialize)]
                struct Wrapper<T> {
                    event: T,
                }

                for event in nexus_events.clone() {
                    let t = format!(
                        "{}::event::EventWrapper<{}::dag::{}>",
                        objects.primitives_pkg_id,
                        objects.workflow_pkg_id,
                        event.name()
                    );

                    let mut grpc_event = sui::grpc::Event::default();
                    grpc_event.set_package_id(objects.workflow_pkg_id);
                    grpc_event.set_module("dag".to_string());
                    grpc_event.set_sender(sui::types::Address::ZERO);
                    grpc_event.set_event_type(t);
                    grpc_event.set_contents(match event {
                        NexusEventKind::WalkAdvanced(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::EndStateReached(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::ExecutionFinished(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::TerminalErrEvalRecorded(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::DAGCreated(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        _ => panic!("Unsupported event type for BCS serialization"),
                    });
                    events.push(grpc_event);
                }
                let mut tx_events = sui::grpc::TransactionEvents::default();
                tx_events.set_events(events);
                transaction.set_events(tx_events);
                result.set_transaction(transaction);
                response.set_transactions(vec![result]);
                Ok(tonic::Response::new(response))
            });
    }

    #[tokio::test]
    async fn test_workflow_actions_publish() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let dag_created = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.workflow_pkg_id,
                        sui::types::Identifier::from_static("dag"),
                        sui::types::Identifier::from_static("DAG"),
                        vec![],
                    ),
                    true,
                    0,
                    dag_object_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Shared(0),
            digest,
            1000,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            digest,
            gas_coin_ref.clone(),
            vec![dag_created],
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

        let dag = JsonDag {
            vertices: vec![],
            edges: vec![],
            default_values: None,
            post_failure_action: None,
            leader_verifier: None,
            tool_verifier: None,
            entry_groups: None,
            outputs: None,
        };

        let result = client
            .workflow()
            .publish(dag)
            .await
            .expect("Failed to publish DAG");

        assert_eq!(result.dag_object_id, dag_object_id);
        assert_eq!(result.tx_digest, digest);
        assert_eq!(result.tx_checkpoint, 1);
    }

    #[tokio::test]
    async fn test_workflow_actions_execute() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let mut nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();
        let default_agent = sui::types::Address::generate(&mut rng);
        let default_skill_id = 11;
        let default_agent_ref = sui::types::ObjectReference::new(
            default_agent,
            1,
            sui::types::Digest::generate(&mut rng),
        );
        nexus_objects.default_tap_target = Some(DefaultDagExecutor {
            agent_id: default_agent,
            skill_id: default_skill_id,
        });

        let requirements = TapSkillRequirements {
            input_schema_commitment: vec![1],
            workflow_commitment: vec![2],
            metadata_commitment: vec![3],
            payment_policy: TapPaymentPolicy::default(),
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        };
        let tap_registry = TapRegistry {
            id: *nexus_objects
                .tap_registry()
                .expect("tap registry ref")
                .object_id(),
            agents: vec![TapAgentRecord {
                agent_id: default_agent,
                owner: sui::types::Address::generate(&mut rng),
                operator: sui::types::Address::generate(&mut rng),
                active: true,
                next_skill_index: 1,
                skills: MoveTable::new(sui::types::Address::generate(&mut rng), 1),
                endpoints: MoveTable::new(sui::types::Address::generate(&mut rng), 1),
                active_endpoints: vec![TapEndpointActivation {
                    agent_id: default_agent,
                    skill_id: default_skill_id,
                    interface_revision: InterfaceRevision(1),
                }],
            }],
            skills: vec![TapSkillRecord {
                agent_id: default_agent,
                skill_id: default_skill_id,
                dag_id: *dag_ref.object_id(),
                dag_binding: TapDagBinding::runtime_selected(),
                tap_package_id: sui::types::Address::generate(&mut rng),
                workflow_commitment: requirements.workflow_commitment.clone(),
                requirements_commitment: requirements.input_schema_commitment.clone(),
                metadata_commitment: requirements.metadata_commitment.clone(),
                payment_policy: requirements.payment_policy.clone(),
                schedule_policy: requirements.schedule_policy.clone(),
                capability_schema_commitment: vec![],
                active: true,
            }],
            endpoints: vec![TapEndpointRevision {
                agent_id: default_agent,
                skill_id: default_skill_id,
                interface_revision: InterfaceRevision(1),
                package_id: sui::types::Address::generate(&mut rng),
                endpoint_object_id: sui::types::Address::generate(&mut rng),
                endpoint_object_version: 1,
                endpoint_object_digest: sui::types::Digest::generate(&mut rng).inner().to_vec(),
                shared_objects: vec![TapSharedObjectRef::immutable(
                    sui::types::Address::generate(&mut rng),
                )],
                requirements: requirements.clone(),
                config_digest: vec![9],
                active_for_new_executions: true,
            }],
            active_endpoints: vec![TapEndpointActivation {
                agent_id: default_agent,
                skill_id: default_skill_id,
                interface_revision: InterfaceRevision(1),
            }],
            default_executor: Some(DefaultDagExecutor {
                agent_id: default_agent,
                skill_id: default_skill_id,
            }),
        };

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let execution_created = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.workflow_pkg_id,
                        sui::types::Identifier::from_static("dag"),
                        sui::types::Identifier::from_static("DAGExecution"),
                        vec![],
                    ),
                    true,
                    0,
                    execution_object_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Shared(0),
            tx_digest,
            1000,
        );

        // DAG
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 1),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        };

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            json!(dag),
        );

        // Dag.vertices
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(TypeName::new("ToolVertex"), *tool_gas_ref.object_id())],
        );

        // DAGVertexInfo
        let vertex_info = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: fqn!("xyz.taluslabs.test@1"),
            },
            leader_verifier: None,
            tool_verifier: None,
            input_ports: Set::default(),
        };

        sui_mocks::grpc::mock_get_objects_json(
            &mut ledger_service_mock,
            vec![(
                tool_gas_ref.clone(),
                sui::types::Owner::Shared(0),
                json!({ "value": vertex_info }),
            )],
        );

        // ToolGas
        sui_mocks::grpc::mock_get_objects_metadata(
            &mut ledger_service_mock,
            vec![(tool_gas_ref, sui::types::Owner::Shared(0), None)],
        );

        mock_fetch_registry_from_tables(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &nexus_objects,
            nexus_objects
                .tap_registry()
                .expect("tap registry ref")
                .clone(),
            &tap_registry,
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            default_agent_ref,
            sui::types::Owner::Shared(0),
            None,
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref.clone(),
            vec![execution_created],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            state_service_mock: Some(state_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let entry_data = HashMap::from([(
            "entry_vertex".to_string(),
            PortsData::from_map(HashMap::from([
                (
                    "entry_port".to_string(),
                    NexusData::new_inline(json!("data")),
                ),
                (
                    "another_entry_port".to_string(),
                    NexusData::new_inline(json!("data")),
                ),
            ])),
        )]);

        let price_priority_fee = 0_u64;

        let result = client
            .workflow()
            .execute(
                *dag_ref.object_id(),
                entry_data,
                price_priority_fee,
                None,
                &StorageConf::default(),
            )
            .await
            .expect("Failed to execute DAG");

        assert_eq!(result.execution_object_id, execution_object_id);
        assert_eq!(result.tx_digest, tx_digest);
        let standard_tap = result.standard_tap.expect("standard TAP metadata");
        assert_eq!(standard_tap.payment_max_budget, 1000);
    }

    #[tokio::test]
    async fn test_workflow_actions_execute_standard_tap_pinned_skill() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let tool_fqn = fqn!("xyz.taluslabs.standard_tap@1");
        let tool_gas_id = derive_tool_gas_id(*nexus_objects.gas_service.object_id(), &tool_fqn)
            .expect("derive tool gas id");
        let tool_gas_ref = sui::types::ObjectReference::new(
            tool_gas_id,
            1,
            sui::types::Digest::generate(&mut rng),
        );
        let agent_id = sui::types::Address::generate(&mut rng);
        let skill_id = 22;
        let agent_ref =
            sui::types::ObjectReference::new(agent_id, 2, sui::types::Digest::generate(&mut rng));
        let endpoint_digest = sui::types::Digest::generate(&mut rng);
        let requirements = TapSkillRequirements {
            input_schema_commitment: vec![1],
            workflow_commitment: vec![2],
            metadata_commitment: vec![3],
            payment_policy: TapPaymentPolicy::default(),
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        };
        let tap_registry = TapRegistry {
            id: *nexus_objects
                .tap_registry()
                .expect("tap registry ref")
                .object_id(),
            agents: vec![TapAgentRecord {
                agent_id,
                owner: sui::types::Address::generate(&mut rng),
                operator: sui::types::Address::generate(&mut rng),
                active: true,
                next_skill_index: 1,
                skills: MoveTable::new(sui::types::Address::generate(&mut rng), 1),
                endpoints: MoveTable::new(sui::types::Address::generate(&mut rng), 1),
                active_endpoints: vec![TapEndpointActivation {
                    agent_id,
                    skill_id,
                    interface_revision: InterfaceRevision(1),
                }],
            }],
            skills: vec![TapSkillRecord {
                agent_id,
                skill_id,
                dag_id: *dag_ref.object_id(),
                dag_binding: TapDagBinding::pinned(*dag_ref.object_id()),
                tap_package_id: sui::types::Address::generate(&mut rng),
                workflow_commitment: requirements.workflow_commitment.clone(),
                requirements_commitment: requirements.input_schema_commitment.clone(),
                metadata_commitment: requirements.metadata_commitment.clone(),
                payment_policy: requirements.payment_policy.clone(),
                schedule_policy: requirements.schedule_policy.clone(),
                capability_schema_commitment: vec![],
                active: true,
            }],
            endpoints: vec![TapEndpointRevision {
                agent_id,
                skill_id,
                interface_revision: InterfaceRevision(1),
                package_id: sui::types::Address::generate(&mut rng),
                endpoint_object_id: sui::types::Address::generate(&mut rng),
                endpoint_object_version: 7,
                endpoint_object_digest: endpoint_digest.inner().to_vec(),
                shared_objects: vec![TapSharedObjectRef::immutable(
                    sui::types::Address::generate(&mut rng),
                )],
                requirements: requirements.clone(),
                config_digest: vec![9],
                active_for_new_executions: true,
            }],
            active_endpoints: vec![TapEndpointActivation {
                agent_id,
                skill_id,
                interface_revision: InterfaceRevision(1),
            }],
            default_executor: None,
        };

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        let execution_created = sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.workflow_pkg_id,
                        sui::types::Identifier::from_static("dag"),
                        sui::types::Identifier::from_static("DAGExecution"),
                        vec![],
                    ),
                    true,
                    0,
                    execution_object_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Shared(0),
            tx_digest,
            1000,
        );
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 1),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig::default(),
            tool_verifier: VerifierConfig::default(),
        };
        mock_fetch_registry_from_tables(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &nexus_objects,
            nexus_objects
                .tap_registry()
                .expect("tap registry ref")
                .clone(),
            &tap_registry,
        );
        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            json!(dag),
        );
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(TypeName::new("ToolVertex"), *tool_gas_ref.object_id())],
        );
        let vertex_info = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: tool_fqn.clone(),
            },
            leader_verifier: None,
            tool_verifier: None,
            input_ports: Set::default(),
        };
        sui_mocks::grpc::mock_get_objects_json(
            &mut ledger_service_mock,
            vec![(
                tool_gas_ref.clone(),
                sui::types::Owner::Shared(0),
                json!({ "value": vertex_info }),
            )],
        );
        sui_mocks::grpc::mock_get_objects_metadata(
            &mut ledger_service_mock,
            vec![(tool_gas_ref, sui::types::Owner::Shared(0), None)],
        );
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            agent_ref,
            sui::types::Owner::Shared(2),
            None,
        );
        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_ref,
            vec![execution_created],
            vec![],
            vec![],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            state_service_mock: Some(state_service_mock),
        });
        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;
        let entry_data = HashMap::from([(
            "entry_vertex".to_string(),
            PortsData::from_map(HashMap::from([(
                "entry_port".to_string(),
                NexusData::new_inline(json!("data")),
            )])),
        )]);

        let result = client
            .workflow()
            .execute_standard_tap(
                agent_id,
                skill_id,
                entry_data,
                0,
                Some("custom"),
                &StorageConf::default(),
                StandardTapExecuteOptions {
                    payment_max_budget: 100,
                    ..Default::default()
                },
            )
            .await
            .expect("standard TAP execution succeeds");

        assert_eq!(result.execution_object_id, execution_object_id);
        assert_eq!(result.tx_digest, tx_digest);
        let metadata = result.standard_tap.expect("standard TAP metadata");
        assert_eq!(metadata.agent_id, agent_id);
        assert_eq!(metadata.skill_id, skill_id);
        assert_eq!(metadata.dag_id, *dag_ref.object_id());
        assert_eq!(metadata.payment_max_budget, 100);
        assert!(metadata.authorization_plan.is_empty());
    }

    #[tokio::test]
    async fn test_workflow_actions_inspect_execution() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let walk_advanced_event = NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
            dag: dag_object_id,
            execution: execution_object_id,
            walk_index: 0,
            vertex: RuntimeVertex::Plain {
                vertex: TypeName::new("initial"),
            },
            variant: TypeName::new("ok"),
            variant_ports_to_data: PortsData::from_map(HashMap::new()),
        });

        let end_state_reached_event = NexusEventKind::EndStateReached(EndStateReachedEvent {
            dag: dag_object_id,
            execution: execution_object_id,
            walk_index: 0,
            vertex: RuntimeVertex::Plain {
                vertex: TypeName::new("initial"),
            },
            variant: TypeName::new("ok"),
            variant_ports_to_data: PortsData::from_map(HashMap::new()),
        });
        let execution_finished_event = NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
            dag: dag_object_id,
            execution: execution_object_id,
            has_any_walk_failed: false,
            has_any_walk_succeeded: true,
            was_aborted: false,
        });

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);

        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![
                walk_advanced_event.clone(),
                end_state_reached_event.clone(),
                execution_finished_event.clone(),
            ],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let mut result = client
            .workflow()
            .inspect_execution(
                execution_object_id,
                1,
                Some(std::time::Duration::from_secs(5)),
            )
            .await
            .expect("Failed to setup channel");

        let mut events = vec![];

        while let Some(event) = result.next_event.recv().await {
            match &event.data {
                NexusEventKind::ExecutionFinished(_) => {
                    events.push(event);

                    break;
                }
                _ => events.push(event),
            }
        }

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0].data, NexusEventKind::WalkAdvanced(_)));
        assert!(matches!(events[1].data, NexusEventKind::EndStateReached(_)));
        assert!(matches!(
            events[2].data,
            NexusEventKind::ExecutionFinished(_)
        ));
        assert!(result.poller.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_workflow_actions_inspect_execution_timeout() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);

        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let mut result = client
            .workflow()
            .inspect_execution(
                execution_object_id,
                1,
                Some(std::time::Duration::from_secs(3)),
            )
            .await
            .expect("Failed to setup channel");

        let mut events = vec![];

        while let Some(event) = result.next_event.recv().await {
            events.push(event);
        }

        assert_eq!(events.len(), 0);
        assert!(matches!(
            result.poller.await.unwrap(),
            Err(NexusError::Timeout(_))
        ));
    }

    #[tokio::test]
    async fn test_workflow_actions_inspect_execution_until_completion() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);
        let execution_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let walk_advanced_event = NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
            dag: dag_object_id,
            execution: execution_object_id,
            walk_index: 0,
            vertex: RuntimeVertex::plain("initial"),
            variant: TypeName::new("ok"),
            variant_ports_to_data: PortsData::from_map(HashMap::new()),
        });
        let terminal_err_eval_event =
            NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
                dag: dag_object_id,
                execution: execution_object_id,
                walk_index: 1,
                vertex: RuntimeVertex::plain("failable"),
                leader: sui::types::Address::THREE,
                failure_class: WorkflowFailureClass::TerminalToolFailure,
                outcome: PostFailureAction::Terminate,
                reason: "tool failed".to_string(),
                err_eval_hash: vec![9, 8, 7],
                duplicate: false,
            });
        let end_state_reached_event = NexusEventKind::EndStateReached(EndStateReachedEvent {
            dag: dag_object_id,
            execution: execution_object_id,
            walk_index: 0,
            vertex: RuntimeVertex::plain("final"),
            variant: TypeName::new("ok"),
            variant_ports_to_data: PortsData::from_map(HashMap::from([(
                "answer".to_string(),
                NexusData::new_inline(json!(42)),
            )])),
        });
        let execution_finished_event = NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
            dag: dag_object_id,
            execution: execution_object_id,
            has_any_walk_failed: true,
            has_any_walk_succeeded: true,
            was_aborted: false,
        });

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);
        mock_events_get_checkpoint_with_supported_events(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![
                walk_advanced_event,
                terminal_err_eval_event,
                end_state_reached_event,
                execution_finished_event,
            ],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .inspect_execution_until_completion(
                execution_object_id,
                1,
                Some(std::time::Duration::from_secs(5)),
                &StorageConf::default(),
            )
            .await
            .expect("Failed to inspect execution until completion");

        assert_eq!(
            result.terminal_state,
            WorkflowExecutionTerminalState::Failed
        );
        assert!(result.execution_finished.has_any_walk_failed);
        assert!(result.execution_finished.has_any_walk_succeeded);
        assert!(matches!(
            result.events.last().map(|event| &event.data),
            Some(NexusEventKind::ExecutionFinished(_))
        ));
        assert_eq!(result.terminal_err_eval_recordings.len(), 1);
        assert_eq!(
            result.terminal_err_eval_recordings[0].failure_class,
            WorkflowFailureClass::TerminalToolFailure
        );
        assert!(result.events.len() >= 2);
    }

    #[tokio::test]
    async fn test_workflow_actions_inspect_execution_until_completion_timeout() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(rand::thread_rng());

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);
        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);
        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            nexus_objects.clone(),
            vec![],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .inspect_execution_until_completion(
                execution_object_id,
                1,
                Some(std::time::Duration::from_secs(3)),
                &StorageConf::default(),
            )
            .await;

        assert!(matches!(result, Err(NexusError::Timeout(_))));
    }

    #[test]
    fn test_event_execution_id_supports_terminal_err_eval_recorded() {
        let execution = sui::types::Address::TWO;
        let event = NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
            dag: sui::types::Address::ZERO,
            execution,
            walk_index: 2,
            vertex: RuntimeVertex::plain("failable"),
            leader: sui::types::Address::THREE,
            failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
            outcome: PostFailureAction::Terminate,
            reason: "timeout".to_string(),
            err_eval_hash: vec![4, 5, 6],
            duplicate: true,
        });

        assert_eq!(event_execution_id(&event), Some(execution));
    }

    #[test]
    fn test_terminal_state_from_execution_finished() {
        let success = ExecutionFinishedEvent {
            dag: sui::types::Address::ZERO,
            execution: sui::types::Address::TWO,
            has_any_walk_failed: false,
            has_any_walk_succeeded: true,
            was_aborted: false,
        };
        let failed = ExecutionFinishedEvent {
            has_any_walk_failed: true,
            has_any_walk_succeeded: false,
            ..success.clone()
        };
        let aborted = ExecutionFinishedEvent {
            has_any_walk_failed: true,
            has_any_walk_succeeded: true,
            was_aborted: true,
            ..success.clone()
        };
        let no_walk_outcome = ExecutionFinishedEvent {
            has_any_walk_failed: false,
            has_any_walk_succeeded: false,
            was_aborted: false,
            ..success
        };

        assert_eq!(
            terminal_state_from_execution_finished(&success),
            WorkflowExecutionTerminalState::Succeeded
        );
        assert_eq!(
            terminal_state_from_execution_finished(&failed),
            WorkflowExecutionTerminalState::Failed
        );
        assert_eq!(
            terminal_state_from_execution_finished(&aborted),
            WorkflowExecutionTerminalState::Aborted
        );
        assert_eq!(
            terminal_state_from_execution_finished(&no_walk_outcome),
            WorkflowExecutionTerminalState::NoWalkOutcome
        );
    }

    #[tokio::test]
    async fn test_build_execution_completion_result_resolves_end_states() {
        let execution = sui::types::Address::TWO;
        let events = vec![
            NexusEvent {
                id: (sui::types::Digest::ZERO, 0),
                generics: vec![],
                data: NexusEventKind::TerminalErrEvalRecorded(TerminalErrEvalRecordedEvent {
                    dag: sui::types::Address::ZERO,
                    execution,
                    walk_index: 1,
                    vertex: RuntimeVertex::plain("failable"),
                    leader: sui::types::Address::THREE,
                    failure_class: WorkflowFailureClass::TerminalToolFailure,
                    outcome: PostFailureAction::Terminate,
                    reason: "tool failed".to_string(),
                    err_eval_hash: vec![1, 2, 3],
                    duplicate: false,
                }),
                distribution: None,
            },
            NexusEvent {
                id: (sui::types::Digest::ZERO, 1),
                generics: vec![],
                data: NexusEventKind::EndStateReached(EndStateReachedEvent {
                    dag: sui::types::Address::ZERO,
                    execution,
                    walk_index: 0,
                    vertex: RuntimeVertex::plain("final"),
                    variant: TypeName::new("ok"),
                    variant_ports_to_data: PortsData::from_map(HashMap::from([(
                        "answer".to_string(),
                        NexusData::new_inline(json!(42)),
                    )])),
                }),
                distribution: None,
            },
            NexusEvent {
                id: (sui::types::Digest::ZERO, 2),
                generics: vec![],
                data: NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
                    dag: sui::types::Address::ZERO,
                    execution,
                    has_any_walk_failed: true,
                    has_any_walk_succeeded: true,
                    was_aborted: false,
                }),
                distribution: None,
            },
        ];

        let result = build_execution_completion_result(events, execution, &StorageConf::default())
            .await
            .expect("summary should build");

        assert_eq!(
            result.terminal_state,
            WorkflowExecutionTerminalState::Failed
        );
        assert_eq!(result.terminal_err_eval_recordings.len(), 1);
        assert_eq!(result.end_states.len(), 1);
        assert_eq!(
            result.end_states[0].event.vertex,
            RuntimeVertex::plain("final")
        );
        assert_eq!(
            result.end_states[0]
                .resolved_ports_to_data
                .get("answer")
                .expect("answer port")
                .as_json(),
            &json!(42)
        );
    }

    #[tokio::test]
    async fn test_workflow_actions_execution_cost() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_ref = sui_mocks::mock_sui_object_ref();
        let payment_ref = sui_mocks::mock_sui_object_ref();
        let execution_id = *execution_ref.object_id();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let payment_id = *payment_ref.object_id();

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_ref,
            sui::types::Owner::Shared(0),
            json!({
                "invoker": "0x1",
                "tap_agent_id": { "vec": ["0xa"] },
                "tap_skill_id": { "vec": ["11"] },
                "tap_interface_revision": { "vec": ["7"] },
                "tap_endpoint_object_id": { "vec": ["0xc"] },
                "tap_payment_id": { "vec": [payment_id.to_string()] },
                "tap_selected_dag_id": { "vec": ["0xe"] },
                "tap_authorization_plan_commitment": { "vec": [] },
                "tap_authorization_plan": [],
                "tap_scheduled_task_id": { "vec": [] },
                "tap_scheduled_occurrence_index": { "vec": [] }
            }),
        );

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            payment_ref.clone(),
            sui::types::Owner::Shared(0),
            json!({
                "id": payment_ref.object_id().to_string(),
                "execution_id": execution_id.to_string(),
                "agent_id": "0xa",
                "skill_id": "11",
                "interface_revision": "7",
                "endpoint_object_id": "0xc",
                "payer": "0x1",
                "payment_mode": "user_funded",
                "source_kind": "invoker",
                "source_identity": "0x1",
                "max_budget": "100000",
                "locked_budget": "100000",
                "consumed": "42000",
                "refund_mode": 0,
                "payment_source_hash": [],
                "accomplished": true,
                "refunded": false,
                "final_state": "accomplished",
                "locked_vertices": []
            }),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .execution_cost(execution_id)
            .await
            .expect("Failed to fetch execution cost");

        assert_eq!(result.payment_id, *payment_ref.object_id());
        assert_eq!(result.max_budget, 100_000);
        assert_eq!(result.locked_budget, 100_000);
        assert_eq!(result.consumed, 42_000);
        assert_eq!(result.outstanding_locks, 0);
        assert!(result.accomplished);
        assert!(!result.refunded);
    }

    #[test]
    fn dag_vertex_requires_verifier_proof_prefers_vertex_override() {
        let dag = Dag {
            vertices: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            defaults_to_input_ports: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            edges: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            outputs: DynamicMap::new(sui_mocks::mock_sui_address(), 0),
            leader_verifier: VerifierConfig {
                mode: VerifierMode::LeaderRegisteredKey,
                method: "signed_http_v1".to_string(),
            },
            tool_verifier: VerifierConfig::default(),
        };
        let vertex = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: fqn!("xyz.example.tool@1"),
            },
            leader_verifier: Some(VerifierConfig::default()),
            tool_verifier: None,
            input_ports: Set::default(),
        };

        assert!(!dag_vertex_requires_verifier_proof(&dag, &vertex));
    }

    #[test]
    fn execution_terminal_record_matches_retryable_vertex_handles_wrapped_and_plain_json() {
        let vertex = RuntimeVertex::with_iterator("v1", 2, 5);
        let wrapped = json!({
            "fields": {
                "contents": [{
                    "fields": {
                        "key": "9",
                        "value": {
                            "fields": {
                                "record": {
                                    "fields": {
                                        "vertex": {
                                            "fields": {
                                                "_variant_name": "WithIterator",
                                                "vertex": { "name": "v1" },
                                                "iteration": { "value": "2" },
                                                "out_of": { "u64": 5 }
                                            }
                                        },
                                        "failure_class": {
                                            "fields": {
                                                "@variant": "Retryable"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }]
            }
        });
        let plain = json!({
            "9": {
                "vertex": {
                    "Plain": {
                        "vertex": {
                            "name": "terminal_vertex"
                        }
                    }
                },
                "failure_class": "terminal_submission_failure"
            }
        });

        assert!(execution_terminal_record_matches_retryable_vertex(&wrapped, 9, &vertex).unwrap());
        assert!(!execution_terminal_record_matches_retryable_vertex(
            &plain,
            9,
            &RuntimeVertex::plain("terminal_vertex"),
        )
        .unwrap());
    }

    #[test]
    fn fetch_vertex_input_port_names_reads_declared_ports_from_typed_vertex() {
        let vertex = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: fqn!("xyz.example.tool@1"),
            },
            leader_verifier: None,
            tool_verifier: None,
            input_ports: [
                crate::nexus::models::DagInputPort {
                    name: "z_port".to_string(),
                },
                crate::nexus::models::DagInputPort {
                    name: "a_port".to_string(),
                },
            ]
            .into_iter()
            .collect(),
        };

        assert_eq!(
            vertex.declared_input_port_names(),
            vec!["a_port".to_string(), "z_port".to_string()]
        );
    }
}
