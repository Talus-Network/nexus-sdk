//! Commands related to workflow management in Nexus.
//!
//! - [`WorkflowActions::publish`] to publish a [`Dag`] instance to Nexus.
//! - [`WorkflowActions::execute`] to execute a published DAG.
//! - [`WorkflowActions::inspect_execution`] to monitor the execution of a DAG.

use {
    crate::{
        crypto::session::Session,
        events::{EventPage, NexusEvent, NexusEventKind},
        idents::workflow,
        nexus::{client::NexusClient, error::NexusError},
        sui,
        transactions::dag,
        types::{Dag, PortsData, StorageConf, DEFAULT_ENTRY_GROUP},
    },
    anyhow::anyhow,
    std::{collections::HashMap, sync::Arc},
    tokio::{
        sync::{
            mpsc::{unbounded_channel, UnboundedReceiver},
            Mutex,
        },
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

pub struct WorkflowActions {
    pub(super) client: NexusClient,
}

impl WorkflowActions {
    /// Publish the provided JSON [`Dag`].
    pub async fn publish(&self, json_dag: Dag) -> Result<PublishResult, NexusError> {
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

                if *object_type.address() == nexus_objects.workflow_pkg_id
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
    /// The `entry_data` [`HashMap`] already holds information about encryption
    /// and storage kind for each port.
    ///
    /// `session` is NOT committed in this function.
    ///
    /// `storage_conf` can accept [`StorageConf::default`] if no remote storage
    /// is expected.
    ///
    /// `gas_price` is the per-transaction priority fee to pass down to the DAG
    /// execution.
    ///
    /// Use [`WorkflowActions::inspect_execution`] to monitor the execution.
    pub async fn execute(
        &self,
        dag_object_id: sui::types::Address,
        entry_data: HashMap<String, PortsData>,
        gas_price: u64,
        entry_group: Option<&str>,
        storage_conf: &StorageConf,
        session: Arc<Mutex<Session>>,
    ) -> Result<ExecuteResult, NexusError> {
        // == Commit data to their respective storage ==

        let mut input_data = HashMap::new();

        for (vertex, ports_data) in entry_data {
            let committed_data = ports_data
                .commit_all(storage_conf, Arc::clone(&session))
                .await
                .map_err(|e| {
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
            .get_object_metadata(dag_object_id)
            .await
            .map_err(|e| NexusError::Rpc(e))?;

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = dag::execute(
            &mut tx,
            nexus_objects,
            &dag.object_ref(),
            gas_price,
            entry_group.unwrap_or(DEFAULT_ENTRY_GROUP),
            &input_data,
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

                if *object_type.address() == nexus_objects.workflow_pkg_id
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
        let timeout = timeout.unwrap_or(Duration::from_secs(300));

        let poller = {
            let fetcher = self.client.event_fetcher().clone();

            tokio::spawn(async move {
                let (_poller, mut next_page) =
                    fetcher.poll_nexus_events(None, Some(execution_checkpoint));

                let timeout = tokio::time::sleep(timeout);

                tokio::pin!(timeout);

                loop {
                    tokio::select! {
                        maybe_page = next_page.recv() => {
                            let events = match maybe_page {
                                Some(EventPage { events, .. }) => events,
                                None => return Err(NexusError::Channel(anyhow!("Event stream closed unexpectedly while inspecting DAG execution '{dag_execution_id}'"))),
                            };

                            for event in events {
                                let execution_id = match &event.data {
                                    NexusEventKind::WalkAdvanced(e) => e.execution,
                                    NexusEventKind::WalkFailed(e) => e.execution,
                                    NexusEventKind::EndStateReached(e) => e.execution,
                                    NexusEventKind::ExecutionFinished(e) => e.execution,
                                    _ => continue,
                                };

                                // Only process events for the given execution ID.
                                if execution_id != dag_execution_id {
                                    continue;
                                }

                                if matches!(&event.data, NexusEventKind::ExecutionFinished(_)) {
                                    tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;

                                    return Ok(());
                                }

                                tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;
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
                WalkAdvancedEvent,
            },
            sui::traits::*,
            test_utils::{nexus_mocks, sui_mocks},
            types::{NexusData, RuntimeVertex, TypeName},
        },
        mockito::Server,
        serde_json::json,
    };

    #[tokio::test]
    async fn test_workflow_actions_publish() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_digest = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();

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
            &mut ledger_service_mock,
            digest,
            gas_coin_digest,
            vec![dag_created],
            vec![],
            vec![],
        );

        let grpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &grpc_url, None).await;

        let dag = Dag {
            vertices: vec![],
            edges: vec![],
            default_values: None,
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
        let gas_coin_digest = sui::types::Digest::generate(&mut rng);
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let dag_object_id = sui::types::Address::generate(&mut rng);
        let execution_object_id = sui::types::Address::generate(&mut rng);
        let (sender, _) = nexus_mocks::mock_session();

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();

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

        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            sui::types::ObjectReference::new(dag_object_id, 0, tx_digest),
            sui::types::Owner::Shared(0),
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut ledger_service_mock,
            tx_digest,
            gas_coin_digest,
            vec![execution_created],
            vec![],
            vec![],
        );

        let grpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(&nexus_objects, &grpc_url, None).await;

        let entry_data = HashMap::from([(
            "entry_vertex".to_string(),
            PortsData::from_map(HashMap::from([
                (
                    "entry_port".to_string(),
                    NexusData::new_inline(json!("data")),
                ),
                (
                    "entry_port_encrypted".to_string(),
                    NexusData::new_inline_encrypted(json!("data")),
                ),
            ])),
        )]);

        let price_priority_fee = 0_u64;

        let result = client
            .workflow()
            .execute(
                dag_object_id,
                entry_data,
                price_priority_fee,
                None,
                &StorageConf::default(),
                sender,
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

        let mut server = Server::new_async().await;
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let grpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: None,
        });

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
        });

        let first_call = sui_mocks::gql::mock_event_query(
            &mut server,
            nexus_objects.primitives_pkg_id,
            vec![walk_advanced_event],
            None,
            None,
        );

        let second_call = sui_mocks::gql::mock_event_query(
            &mut server,
            nexus_objects.primitives_pkg_id,
            vec![end_state_reached_event],
            None,
            None,
        );

        let third_call = sui_mocks::gql::mock_event_query(
            &mut server,
            nexus_objects.primitives_pkg_id,
            vec![execution_finished_event],
            None,
            None,
        );

        let client = nexus_mocks::mock_nexus_client(
            &nexus_objects,
            &grpc_url,
            Some(&format!("{}/graphql", server.url())),
        )
        .await;

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
            events.push(event);
        }

        first_call.assert_async().await;
        second_call.assert_async().await;
        third_call.assert_async().await;

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

        let mut server = Server::new_async().await;
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        let grpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: None,
        });

        let first_call = sui_mocks::gql::mock_event_query(
            &mut server,
            nexus_objects.primitives_pkg_id,
            vec![],
            None,
            None,
        );

        let client = nexus_mocks::mock_nexus_client(
            &nexus_objects,
            &grpc_url,
            Some(&format!("{}/graphql", server.url())),
        )
        .await;

        let mut result = client
            .workflow()
            .inspect_execution(
                execution_object_id,
                1,
                Some(std::time::Duration::from_millis(100)),
            )
            .await
            .expect("Failed to setup channel");

        let mut events = vec![];

        while let Some(event) = result.next_event.recv().await {
            events.push(event);
        }

        first_call.assert_async().await;

        assert_eq!(events.len(), 0);
        assert!(matches!(
            result.poller.await.unwrap(),
            Err(NexusError::Timeout(_))
        ));
    }
}
