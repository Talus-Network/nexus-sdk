//! Commands related to workflow management in Nexus.
//!
//! - [`WorkflowActions::publish`] to publish a [`Dag`] instance to Nexus.
//! - [`WorkflowActions::execute`] to execute a published DAG.
//! - [`WorkflowActions::inspect_execution`] to monitor the execution of a DAG.

use {
    crate::{
        crypto::session::Session,
        events::{NexusEvent, NexusEventKind},
        idents::{primitives, workflow},
        nexus::{client::NexusClient, error::NexusError},
        object_crawler::fetch_one,
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
        time::{Duration, Instant},
    },
};

pub struct PublishResult {
    pub tx_digest: sui::types::Digest,
    pub dag_object_id: sui::types::Address,
}

pub struct ExecuteResult {
    pub tx_digest: sui::types::Digest,
    pub execution_object_id: sui::types::Address,
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
        let sui_client = &self.client.sui_client;
        let nexus_objects = &self.client.nexus_objects;
        let dag = fetch_one::<serde_json::Value>(sui_client, dag_object_id)
            .await
            .map_err(NexusError::Rpc)?;

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
        execution_tx_digest: sui::types::Digest,
        timeout: Option<Duration>,
    ) -> Result<InspectExecutionResult, NexusError> {
        Err(NexusError::Configuration("Not yet implemented".to_string()))
        // TODO: fix when we can fetch events from GQL
        // Setup MSPC channel.
        // let (tx, rx) = unbounded_channel::<NexusEvent>();

        // let mut cursor = Some(sui::EventID {
        //     tx_digest: execution_tx_digest
        //         .to_string()
        //         .parse()
        //         .expect("TODO: remove old sdk"),
        //     event_seq: 0,
        // });

        // let sui_client = self.client.sui_client.clone();

        // // Create some initial timings and restrictions.
        // let timeout = timeout.unwrap_or(Duration::from_secs(300));
        // let mut poll_interval = Duration::from_millis(100);
        // let max_poll_interval = Duration::from_secs(2);
        // let started = Instant::now();

        // let primitives_pkg_id = self.client.nexus_objects.primitives_pkg_id;

        // let poller = tokio::spawn(async move {
        //     // Loop until we find an [`NexusEventKind::ExecutionFinished`] event.
        //     loop {
        //         if started.elapsed() > timeout {
        //             return Err(NexusError::Timeout(anyhow!("Timeout {timeout:?} reached while inspecting DAG execution '{dag_execution_id}'")));
        //         }

        //         let query = sui::EventFilter::MoveEventModule {
        //             package: sui::ObjectID::from_hex_literal(&primitives_pkg_id.to_string())
        //                 .expect("TODO: remove ObjectID"),
        //             module: sui::Identifier::new(primitives::Event::EVENT_WRAPPER.module.as_str())
        //                 .expect("TODO: old SDK"),
        //         };

        //         let limit = None;
        //         let descending_order = false;

        //         let page = sui_client
        //             .event_api()
        //             .query_events(query, cursor, limit, descending_order)
        //             .await
        //             .map_err(|e| NexusError::Rpc(e.into()))?;

        //         cursor = page.next_cursor;

        //         let mut found_event = false;

        //         for event in page.data {
        //             let Ok(event): anyhow::Result<NexusEvent> = event.try_into() else {
        //                 continue;
        //             };

        //             let execution_id = match &event.data {
        //                 NexusEventKind::WalkAdvanced(e) => e.execution,
        //                 NexusEventKind::WalkFailed(e) => e.execution,
        //                 NexusEventKind::EndStateReached(e) => e.execution,
        //                 NexusEventKind::ExecutionFinished(e) => e.execution,
        //                 _ => continue,
        //             };

        //             // Only process events for the given execution ID.
        //             if execution_id != dag_execution_id {
        //                 continue;
        //             }

        //             // We did find relevant events, do not increase the polling
        //             // interval.
        //             found_event = true;

        //             if matches!(&event.data, NexusEventKind::ExecutionFinished(_)) {
        //                 tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;

        //                 return Ok(());
        //             }

        //             tx.send(event).map_err(|e| NexusError::Channel(e.into()))?;
        //         }

        //         // If no new events were found, increase the polling interval.
        //         // Otherwise, reset it to the initial value.
        //         if found_event {
        //             poll_interval = Duration::from_millis(100);
        //         } else {
        //             poll_interval = (poll_interval * 2).min(max_poll_interval);
        //         }

        //         tokio::time::sleep(poll_interval).await;
        //     }
        // });

        // Ok(InspectExecutionResult {
        //     next_event: rx,
        //     poller,
        // })
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            events::{
                EndStateReachedEvent,
                ExecutionFinishedEvent,
                NexusEventKind,
                WalkAdvancedEvent,
            },
            idents::workflow,
            nexus::error::NexusError,
            sui,
            test_utils::{nexus_mocks, sui_mocks},
            types::{Dag, NexusData, PortsData, RuntimeVertex, StorageConf, TypeName},
        },
        serde_json::json,
        std::collections::{BTreeMap, HashMap},
    };

    // TODO: redo tests with tonic server mocks
    // #[tokio::test]
    // async fn test_workflow_actions_publish() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;

    //     let dag_object_id = sui::types::Address::random();
    //     let dag_created = sui::ObjectChange::Created {
    //         sender: sui::types::Address::random().into(),
    //         owner: sui::Owner::Shared {
    //             initial_shared_version: sui::SequenceNumber::from_u64(1),
    //         },
    //         object_type: sui::MoveStructTag {
    //             address: *nexus_client.nexus_objects.workflow_pkg_id,
    //             module: workflow::Dag::DAG.module,
    //             name: workflow::Dag::DAG.name,
    //             type_params: vec![],
    //         },
    //         object_id: dag_object_id,
    //         version: sui::SequenceNumber::from_u64(1),
    //         digest: sui::types::Digest::random(),
    //     };

    //     let tx_digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             tx_digest,
    //             None,
    //             None,
    //             None,
    //             Some(vec![dag_created]),
    //         );

    //     let dag = Dag {
    //         vertices: vec![],
    //         edges: vec![],
    //         default_values: None,
    //         entry_groups: None,
    //         outputs: None,
    //     };

    //     let result = nexus_client
    //         .workflow()
    //         .publish(dag)
    //         .await
    //         .expect("Failed to publish DAG");

    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.dag_object_id, dag_object_id);
    //     assert_eq!(result.tx_digest, tx_digest);
    // }

    // #[tokio::test]
    // async fn test_workflow_actions_execute() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
    //     let (sender, _) = nexus_mocks::mock_session();

    //     let dag_object_id = sui::types::Address::random();
    //     let execution_object_id = sui::types::Address::random();
    //     let execution_created = sui::ObjectChange::Created {
    //         sender: sui::types::Address::random().into(),
    //         owner: sui::Owner::Shared {
    //             initial_shared_version: sui::SequenceNumber::from_u64(1),
    //         },
    //         object_type: sui::MoveStructTag {
    //             address: *nexus_client.nexus_objects.workflow_pkg_id,
    //             module: workflow::Dag::DAG_EXECUTION.module,
    //             name: workflow::Dag::DAG_EXECUTION.name,
    //             type_params: vec![],
    //         },
    //         object_id: execution_object_id,
    //         version: sui::SequenceNumber::from_u64(1),
    //         digest: sui::types::Digest::random(),
    //     };

    //     let dag_object = sui::ParsedMoveObject {
    //         type_: sui::MoveStructTag {
    //             address: *nexus_client.nexus_objects.workflow_pkg_id,
    //             module: workflow::Dag::DAG.module,
    //             name: workflow::Dag::DAG.name,
    //             type_params: vec![],
    //         },
    //         has_public_transfer: false,
    //         fields: sui::MoveStruct::WithFields(BTreeMap::from([(
    //             "test".into(),
    //             sui::MoveValue::Number(1),
    //         )])),
    //     };

    //     let get_object_call =
    //         sui_mocks::rpc::mock_read_api_get_object(&mut server, dag_object_id, dag_object);

    //     let tx_digest = sui::TransactionDigest::random();
    //     let (execute_call, confirm_call) =
    //         sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
    //             &mut server,
    //             tx_digest,
    //             None,
    //             None,
    //             None,
    //             Some(vec![execution_created]),
    //         );

    //     let entry_data = HashMap::from([(
    //         "entry_vertex".to_string(),
    //         PortsData::from_map(HashMap::from([
    //             (
    //                 "entry_port".to_string(),
    //                 NexusData::new_inline(json!("data")),
    //             ),
    //             (
    //                 "entry_port_encrypted".to_string(),
    //                 NexusData::new_inline_encrypted(json!("data")),
    //             ),
    //         ])),
    //     )]);

    //     let price_priority_fee = 0_u64;

    //     let result = nexus_client
    //         .workflow()
    //         .execute(
    //             dag_object_id,
    //             entry_data,
    //             price_priority_fee,
    //             None,
    //             &StorageConf::default(),
    //             sender,
    //         )
    //         .await
    //         .expect("Failed to execute DAG");

    //     get_object_call.assert_async().await;

    //     execute_call.assert_async().await;
    //     confirm_call.assert_async().await;

    //     assert_eq!(result.execution_object_id, execution_object_id);
    //     assert_eq!(result.tx_digest, tx_digest);
    // }

    // #[tokio::test]
    // async fn test_workflow_actions_inspect_execution() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;

    //     let dag_object_id = sui::types::Address::random();
    //     let execution_object_id = sui::types::Address::random();
    //     let execution_tx_digest = sui::TransactionDigest::random();

    //     let walk_advanced_event = NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
    //         dag: dag_object_id,
    //         execution: execution_object_id,
    //         walk_index: 0,
    //         vertex: RuntimeVertex::Plain {
    //             vertex: TypeName::new("initial"),
    //         },
    //         variant: TypeName::new("ok"),
    //         variant_ports_to_data: PortsData::from_map(HashMap::new()),
    //     });

    //     let end_state_reached_event = NexusEventKind::EndStateReached(EndStateReachedEvent {
    //         dag: dag_object_id,
    //         execution: execution_object_id,
    //         walk_index: 0,
    //         vertex: RuntimeVertex::Plain {
    //             vertex: TypeName::new("initial"),
    //         },
    //         variant: TypeName::new("ok"),
    //         variant_ports_to_data: PortsData::from_map(HashMap::new()),
    //     });

    //     let execution_finished_event = NexusEventKind::ExecutionFinished(ExecutionFinishedEvent {
    //         dag: dag_object_id,
    //         execution: execution_object_id,
    //         has_any_walk_failed: false,
    //         has_any_walk_succeeded: true,
    //     });

    //     let first_call = sui_mocks::rpc::mock_event_api_query_events(
    //         &mut server,
    //         vec![("WalkAdvancedEvent".to_string(), walk_advanced_event.clone())],
    //     );

    //     let second_call = sui_mocks::rpc::mock_event_api_query_events(
    //         &mut server,
    //         vec![(
    //             "EndStateReachedEvent".to_string(),
    //             end_state_reached_event.clone(),
    //         )],
    //     );

    //     let third_call = sui_mocks::rpc::mock_event_api_query_events(
    //         &mut server,
    //         vec![(
    //             "ExecutionFinishedEvent".to_string(),
    //             execution_finished_event.clone(),
    //         )],
    //     );

    //     let mut result = nexus_client
    //         .workflow()
    //         .inspect_execution(
    //             execution_object_id,
    //             execution_tx_digest,
    //             Some(std::time::Duration::from_secs(5)),
    //         )
    //         .await
    //         .expect("Failed to setup channel");

    //     let mut events = vec![];

    //     while let Some(event) = result.next_event.recv().await {
    //         events.push(event);
    //     }

    //     first_call.assert_async().await;
    //     second_call.assert_async().await;
    //     third_call.assert_async().await;

    //     assert_eq!(events.len(), 3);
    //     assert!(matches!(events[0].data, NexusEventKind::WalkAdvanced(_)));
    //     assert!(matches!(events[1].data, NexusEventKind::EndStateReached(_)));
    //     assert!(matches!(
    //         events[2].data,
    //         NexusEventKind::ExecutionFinished(_)
    //     ));
    //     assert!(result.poller.await.unwrap().is_ok());
    // }

    // #[tokio::test]
    // async fn test_workflow_actions_inspect_execution_timeout() {
    //     let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;

    //     let execution_object_id = sui::types::Address::random();
    //     let execution_tx_digest = sui::TransactionDigest::random();

    //     let first_call = sui_mocks::rpc::mock_event_api_query_events(&mut server, vec![]);

    //     let mut result = nexus_client
    //         .workflow()
    //         .inspect_execution(
    //             execution_object_id,
    //             execution_tx_digest,
    //             Some(std::time::Duration::from_millis(100)),
    //         )
    //         .await
    //         .expect("Failed to setup channel");

    //     let mut events = vec![];

    //     while let Some(event) = result.next_event.recv().await {
    //         events.push(event);
    //     }

    //     first_call.assert_async().await;

    //     assert_eq!(events.len(), 0);
    //     assert!(matches!(
    //         result.poller.await.unwrap(),
    //         Err(NexusError::Timeout(_))
    //     ));
    // }

    // #[tokio::test]
    // async fn test_workflow_actions_inspect_execution_rpc_fail() {
    //     let (_, nexus_client) = nexus_mocks::mock_nexus_client().await;

    //     let execution_object_id = sui::types::Address::random();
    //     let execution_tx_digest = sui::TransactionDigest::random();

    //     let mut result = nexus_client
    //         .workflow()
    //         .inspect_execution(
    //             execution_object_id,
    //             execution_tx_digest,
    //             Some(std::time::Duration::from_millis(100)),
    //         )
    //         .await
    //         .expect("Failed to setup channel");

    //     let mut events = vec![];

    //     while let Some(event) = result.next_event.recv().await {
    //         events.push(event);
    //     }

    //     assert_eq!(events.len(), 0);
    //     assert!(matches!(
    //         result.poller.await.unwrap(),
    //         Err(NexusError::Rpc(_))
    //     ));
    // }
}
