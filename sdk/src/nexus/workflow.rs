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
            error::NexusError,
            models::{ClaimedGas, Dag, ExecutionGas},
        },
        sui,
        transactions::dag,
        types::{
            derive_execution_gas_id,
            Dag as JsonDag,
            DataStorage,
            PortsData,
            StorageConf,
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

pub struct PublishResult {
    pub tx_digest: sui::types::Digest,
    pub dag_object_id: sui::types::Address,
}

pub struct ExecuteResult {
    pub tx_digest: sui::types::Digest,
    pub execution_object_id: sui::types::Address,
    pub tx_checkpoint: u64,
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
    pub leader_claims: HashMap<sui::types::Digest, ClaimedGas>,
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
            dag_object_id,
        })
    }

    /// Execute a published DAG with the given object ID.
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

        let invoker_gas = self.client.fetch_invoker_gas().await?;
        let tools_gas = self.client.fetch_tool_gas_for_dag(&dag.data).await?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = dag::execute(
            &mut tx,
            nexus_objects,
            &dag.object_ref(),
            priority_fee_per_gas_unit,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &input_data,
            &invoker_gas,
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

        let signature = self.client.signer.sign_tx(&tx).await?;

        let response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

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

    /// Calculate the gas cost of a finished DAG execution based on the provided
    /// execution object ID.
    pub async fn execution_cost(
        &self,
        dag_execution_id: sui::types::Address,
    ) -> Result<ExecutionCostResult, NexusError> {
        // Derive the `ExecutionGas` object ID.
        let gas_service_object_id = *self.client.nexus_objects.gas_service.object_id();
        let execution_gas_id = derive_execution_gas_id(gas_service_object_id, dag_execution_id)
            .map_err(NexusError::Parsing)?;

        let crawler = self.client.crawler();
        let execution_gas = crawler
            .get_object::<ExecutionGas>(execution_gas_id)
            .await
            .map_err(NexusError::Rpc)?
            .data;

        let leader_claims = crawler
            .get_dynamic_fields(&execution_gas.claimed_leader_gas)
            .await
            .map_err(NexusError::Rpc)?
            .into_iter()
            .map(|(digest, claim)| {
                let digest = sui::types::Digest::from_bytes(digest.as_slice())
                    .unwrap_or(sui::types::Digest::ZERO);

                (digest, claim)
            })
            .collect();

        Ok(ExecutionCostResult { leader_claims })
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
                crawler::DynamicMap,
                models::{Dag, DagVertexInfo, DagVertexKind},
            },
            sui::traits::*,
            test_utils::{nexus_mocks, sui_mocks},
            types::{
                NexusData,
                PostFailureAction,
                RuntimeVertex,
                Storable,
                TypeName,
                WorkflowFailureClass,
            },
        },
        serde::Serialize,
        serde_json::json,
    };

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
    }

    #[tokio::test]
    async fn test_workflow_actions_execute() {
        let mut rng = rand::thread_rng();
        let tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let invoker_gas_ref = sui_mocks::mock_sui_object_ref();
        let dag_ref = sui_mocks::mock_sui_object_ref();
        let tool_gas_ref = sui_mocks::mock_sui_object_ref();

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
        };

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            dag_ref.clone(),
            sui::types::Owner::Shared(0),
            json!(dag),
        );

        // InvokerGas
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            invoker_gas_ref,
            sui::types::Owner::Shared(0),
            None,
        );

        // Dag.vertices
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(TypeName::new("InvokerGas"), *tool_gas_ref.object_id())],
        );

        // DAGVertexInfo
        let vertex_info = DagVertexInfo {
            kind: DagVertexKind::OffChain {
                tool_fqn: fqn!("xyz.taluslabs.test@1"),
            },
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
        let execution_object_id = sui::types::Address::generate(&mut rand::thread_rng());

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
        let execution_gas_ref = sui_mocks::mock_sui_object_ref();
        let execution_gas_claims_id = sui_mocks::mock_sui_address();
        let leader_claim_object_ref = sui_mocks::mock_sui_object_ref();
        let claim_digest = sui::types::Digest::generate(&mut rand::thread_rng());
        let execution_id = sui::types::Address::generate(&mut rand::thread_rng());

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        // Mock ExecutionGas object response
        let execution_gas = ExecutionGas {
            claimed_leader_gas: DynamicMap::new(execution_gas_claims_id, 1),
        };

        sui_mocks::grpc::mock_get_object_json(
            &mut ledger_service_mock,
            execution_gas_ref.clone(),
            sui::types::Owner::Shared(0),
            json!(execution_gas),
        );

        // ExecutionGas.vault
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(
                claim_digest.as_bytes().to_vec(),
                *leader_claim_object_ref.object_id(),
            )],
        );

        // ClaimedGas
        let claimed_gas = ClaimedGas {
            execution: 100_000,
            priority: 10_000,
        };

        sui_mocks::grpc::mock_get_objects_json(
            &mut ledger_service_mock,
            vec![(
                leader_claim_object_ref.clone(),
                sui::types::Owner::Shared(0),
                json!({ "value": claimed_gas }),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &rpc_url).await;

        let result = client
            .workflow()
            .execution_cost(execution_id)
            .await
            .expect("Failed to fetch execution cost");

        assert_eq!(result.leader_claims.len(), 1);
        let (digest, funds) = result.leader_claims.iter().next().unwrap();
        assert_eq!(funds.execution, 100_000);
        assert_eq!(funds.priority, 10_000);
        assert_eq!(digest, &claim_digest);
    }
}
