use crate::{sui, types::NexusObjects};

/// Create a new [`sui::types::ObjectReference`] with random values.
pub fn mock_sui_object_ref() -> sui::types::ObjectReference {
    let mut rng = rand::thread_rng();

    sui::types::ObjectReference::new(
        sui::types::Address::generate(&mut rng),
        1,
        sui::types::Digest::generate(&mut rng),
    )
}

/// Create a new [`sui::EventID`] with random values.
pub fn mock_sui_event_id() -> (sui::types::Digest, u64) {
    let mut rng = rand::thread_rng();

    (sui::types::Digest::generate(&mut rng), 0)
}

/// Create a new [`sui::EventID`] with random values.
pub fn mock_sui_address() -> sui::types::Address {
    let mut rng = rand::thread_rng();

    sui::types::Address::generate(&mut rng)
}

/// Create a new [`sui::EventID`] with random values.
pub fn mock_nexus_objects() -> NexusObjects {
    let mut rng = rand::thread_rng();

    NexusObjects {
        workflow_pkg_id: sui::types::Address::generate(&mut rng),
        primitives_pkg_id: sui::types::Address::generate(&mut rng),
        interface_pkg_id: sui::types::Address::generate(&mut rng),
        network_id: sui::types::Address::generate(&mut rng),
        tool_registry: mock_sui_object_ref(),
        default_tap: mock_sui_object_ref(),
        gas_service: mock_sui_object_ref(),
        pre_key_vault: mock_sui_object_ref(),
    }
}

/// Generate a mock [`sui::types::Event`]
pub fn mock_sui_event(
    package_id: sui::types::Address,
    type_: sui::types::StructTag,
    contents: Vec<u8>,
) -> sui::types::Event {
    let mut rng = rand::thread_rng();

    sui::types::Event {
        package_id,
        type_,
        contents,
        sender: sui::types::Address::generate(&mut rng),
        module: sui::types::Identifier::new("test_module").unwrap(),
    }
}

/// Finish the given [`sui::tx::TransactionBuilder`] with mock data.
pub fn mock_finish_transaction(mut tx: sui::tx::TransactionBuilder) -> sui::types::Transaction {
    let mut rng = rand::thread_rng();
    let gas = mock_sui_object_ref();

    tx.set_sender(sui::types::Address::generate(&mut rng));
    tx.set_gas_budget(1000);
    tx.set_gas_price(1000);
    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas.object_id(),
        gas.version(),
        *gas.digest(),
    )]);

    tx.finish().expect("Transaction should build")
}

#[cfg(test)]
pub mod grpc {
    use {
        super::*,
        mockall::mock,
        std::time::SystemTime,
        sui_rpc::proto::sui::rpc::v2::{
            ledger_service_server::{LedgerService, LedgerServiceServer},
            subscription_service_server::{SubscriptionService, SubscriptionServiceServer},
            transaction_execution_service_server::{
                TransactionExecutionService,
                TransactionExecutionServiceServer,
            },
            *,
        },
        tonic::{Request, Response, Status},
    };

    // Mocking LedgerService RPC endpoints for deeper testing.
    mock! {
        pub LedgerService {}

        #[tonic::async_trait]
        impl LedgerService for LedgerService {
            async fn get_service_info(
                &self,
                request: Request<GetServiceInfoRequest>,
            ) -> Result<Response<GetServiceInfoResponse>, Status>;

            async fn get_object(
                &self,
                request: Request<GetObjectRequest>,
            ) -> Result<Response<GetObjectResponse>, Status>;

            async fn batch_get_objects(
                &self,
                request: Request<BatchGetObjectsRequest>,
            ) -> Result<Response<BatchGetObjectsResponse>, Status>;

            async fn get_transaction(
                &self,
                request: Request<GetTransactionRequest>,
            ) -> Result<Response<GetTransactionResponse>, Status>;

            async fn batch_get_transactions(
                &self,
                request: Request<BatchGetTransactionsRequest>,
            ) -> Result<Response<BatchGetTransactionsResponse>, Status>;

            async fn get_checkpoint(
                &self,
                request: Request<GetCheckpointRequest>,
            ) -> Result<Response<GetCheckpointResponse>, Status>;

            async fn get_epoch(
                &self,
                request: Request<GetEpochRequest>,
            ) -> Result<Response<GetEpochResponse>, Status>;
        }
    }

    // Mocking TransactionExecutionService RPC endpoints for deeper testing.
    mock! {
        pub TransactionExecutionService {}

        #[tonic::async_trait]
        impl TransactionExecutionService for TransactionExecutionService {
            async fn execute_transaction(
                &self,
                request: tonic::Request<ExecuteTransactionRequest>,
            ) -> Result<tonic::Response<ExecuteTransactionResponse>, tonic::Status>;

            async fn simulate_transaction(
                &self,
                request: tonic::Request<SimulateTransactionRequest>,
            ) -> Result<tonic::Response<SimulateTransactionResponse>, tonic::Status>;
        }
    }

    // Mocking SubscriptionService RPC endpoints for deeper testing.

    pub type BoxCheckpointStream = std::pin::Pin<
        Box<
            dyn futures::Stream<Item = Result<SubscribeCheckpointsResponse, Status>>
                + Send
                + 'static,
        >,
    >;

    #[tonic::async_trait]

    pub trait SubscriptionServiceWrapper: Send + Sync + 'static {
        async fn subscribe_checkpoints(
            &self,

            request: Request<SubscribeCheckpointsRequest>,
        ) -> Result<Response<BoxCheckpointStream>, Status>;
    }

    pub struct SubscriptionServiceAdapter<W: SubscriptionServiceWrapper> {
        pub inner: std::sync::Arc<W>,
    }

    impl<W: SubscriptionServiceWrapper> SubscriptionServiceAdapter<W> {
        pub fn new(inner: std::sync::Arc<W>) -> Self {
            Self { inner }
        }
    }

    #[tonic::async_trait]
    impl<W: SubscriptionServiceWrapper> SubscriptionService for SubscriptionServiceAdapter<W> {
        type SubscribeCheckpointsStream = BoxCheckpointStream;

        async fn subscribe_checkpoints(
            &self,
            request: Request<SubscribeCheckpointsRequest>,
        ) -> Result<Response<Self::SubscribeCheckpointsStream>, Status> {
            self.inner.subscribe_checkpoints(request).await
        }
    }

    mock! {
        pub SubscriptionService {}

        #[tonic::async_trait]
        impl SubscriptionServiceWrapper for SubscriptionService {
            async fn subscribe_checkpoints(
                &self,
                request: tonic::Request<SubscribeCheckpointsRequest>,
            ) -> Result<tonic::Response<BoxCheckpointStream>, tonic::Status>;
        }
    }

    #[derive(Default)]
    pub struct ServerMocks {
        pub ledger_service_mock: Option<MockLedgerService>,
        pub execution_service_mock: Option<MockTransactionExecutionService>,
        pub subscription_service_mock: Option<MockSubscriptionService>,
    }

    pub fn mock_server(mocks: ServerMocks) -> String {
        let port = portpicker::pick_unused_port().expect("No ports free");
        let addr = format!("127.0.0.1:{}", port);
        let thread_addr = addr.clone();

        let ledger_service = mocks.ledger_service_mock.map(LedgerServiceServer::new);
        let execution_service = mocks
            .execution_service_mock
            .map(TransactionExecutionServiceServer::new);
        let subscription_service = mocks.subscription_service_mock.map(|m| {
            SubscriptionServiceServer::new(SubscriptionServiceAdapter::new(std::sync::Arc::new(m)))
        });

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_optional_service(ledger_service)
                .add_optional_service(execution_service)
                .add_optional_service(subscription_service)
                .serve(thread_addr.parse().unwrap())
                .await
                .unwrap();
        });

        format!("http://{}", addr)
    }

    pub fn mock_execute_transaction_and_wait_for_checkpoint(
        tx_service: &mut MockTransactionExecutionService,
        sub_service: &mut MockSubscriptionService,
        ledger_service: &mut MockLedgerService,
        digest: sui::types::Digest,
        gas_coin_ref: sui::types::ObjectReference,
        objects: Vec<sui::types::Object>,
        changed_objects: Vec<sui::types::ChangedObject>,
        events: Vec<sui::types::Event>,
    ) {
        let mut changed_objects_with_coin = vec![sui::types::ChangedObject {
            object_id: sui::types::Address::from_static("0x1"),
            input_state: sui::types::ObjectIn::NotExist,
            output_state: sui::types::ObjectOut::ObjectWrite {
                digest: *gas_coin_ref.digest(),
                owner: sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
            },
            id_operation: sui::types::IdOperation::None,
        }];

        changed_objects_with_coin.extend(changed_objects.clone());

        sub_service
            .expect_subscribe_checkpoints()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::SubscribeCheckpointsResponse::default();
                let mut checkpoint = sui::grpc::Checkpoint::default();
                let mut tx = sui::grpc::ExecutedTransaction::default();

                tx.set_digest(digest);
                checkpoint.set_transactions(vec![tx]);
                checkpoint.set_sequence_number(1);
                response.set_checkpoint(checkpoint);

                let output = vec![Ok(response)];
                let stream = futures::stream::iter(output);

                Ok(tonic::Response::new(Box::pin(stream) as BoxCheckpointStream))
            });

        tx_service
            .expect_execute_transaction()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ExecuteTransactionResponse::default();
                let mut tx = sui::grpc::ExecutedTransaction::default();

                let mut tx_objects = sui::grpc::ObjectSet::default();
                tx_objects.set_objects(objects.clone().into_iter().map(Into::into).collect());
                tx.set_objects(tx_objects);

                let mut effects = sui::grpc::TransactionEffects::default();
                let effect = sui::types::TransactionEffectsV2 {
                    status: sui::types::ExecutionStatus::Success,
                    epoch: 1,
                    gas_used: sui::types::GasCostSummary {
                        computation_cost: 0,
                        storage_cost: 0,
                        storage_rebate: 0,
                        non_refundable_storage_fee: 0,
                    },
                    transaction_digest: digest,
                    gas_object_index: Some(0),
                    events_digest: None,
                    dependencies: vec![],
                    lamport_version: 1,
                    changed_objects: changed_objects_with_coin.clone(),
                    unchanged_consensus_objects: vec![],
                    auxiliary_data_digest: None,
                };
                effects.set_bcs(
                    bcs::to_bytes(&sui::types::TransactionEffects::V2(Box::new(effect))).unwrap(),
                );
                tx.set_effects(effects);

                let mut tx_events = sui::grpc::TransactionEvents::default();
                tx_events.set_events(events.clone().into_iter().map(Into::into).collect());
                tx.set_events(tx_events);
                tx.set_digest(digest);
                tx.set_checkpoint(1);

                response.set_transaction(tx);

                Ok(tonic::Response::new(response))
            });

        mock_get_object_metadata(
            ledger_service,
            gas_coin_ref,
            sui::types::Owner::Immutable,
            Some(1000),
        );
    }

    pub fn mock_reference_gas_price(
        ledger_service: &mut MockLedgerService,
        reference_gas_price: u64,
    ) {
        ledger_service
            .expect_get_epoch()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetEpochResponse::default();
                let mut epoch = sui::grpc::Epoch::default();
                epoch.set_reference_gas_price(reference_gas_price);
                response.set_epoch(epoch);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_get_object_metadata(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        balance: Option<u64>,
    ) {
        ledger_service
            .expect_get_object()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut grpc_object = sui::grpc::Object::default();
                grpc_object.set_owner(sui::grpc::Owner::from(owner));
                grpc_object.set_digest(*object_ref.digest());
                grpc_object.set_version(object_ref.version());
                grpc_object.set_balance(balance.unwrap_or(0));
                response.set_object(grpc_object);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_get_objects_metadata(
        ledger_service: &mut MockLedgerService,
        objects: Vec<(sui::types::ObjectReference, sui::types::Owner, Option<u64>)>,
    ) {
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                let mut objs = Vec::with_capacity(objects.len());
                for (object_ref, owner, balance) in objects.clone() {
                    let mut parent_object = sui::grpc::GetObjectResult::default();
                    let mut grpc_object = sui::grpc::Object::default();
                    grpc_object.set_owner(sui::grpc::Owner::from(owner));
                    grpc_object.set_digest(*object_ref.digest());
                    grpc_object.set_object_id(*object_ref.object_id());
                    grpc_object.set_version(object_ref.version());
                    grpc_object.set_balance(balance.unwrap_or(0));
                    parent_object.set_object(grpc_object);
                    objs.push(parent_object);
                }
                response.set_objects(objs);
                Ok(tonic::Response::new(response))
            });
    }

    /// Expect a `get_object` call and return an object populated with metadata
    /// and a JSON payload (converted into `prost_types::Value`).
    pub fn mock_get_object_json(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        json_value: serde_json::Value,
    ) {
        ledger_service
            .expect_get_object()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut grpc_object = sui::grpc::Object::default();
                grpc_object.set_owner(sui::grpc::Owner::from(owner.clone()));
                grpc_object.set_digest(*object_ref.digest());
                grpc_object.set_version(object_ref.version());
                grpc_object.set_object_id(object_ref.object_id().to_string());
                grpc_object.json = Some(Box::new(json_to_prost_value(&json_value)));
                response.set_object(grpc_object);
                Ok(tonic::Response::new(response))
            });
    }

    /// Expect a `get_epoch` call and return the end timestamp.
    pub fn mock_get_epoch(ledger_service: &mut MockLedgerService, epoch_end: SystemTime) {
        ledger_service
            .expect_get_epoch()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetEpochResponse::default();
                let mut epoch = sui::grpc::Epoch::default();
                epoch.set_end(epoch_end);
                response.set_epoch(epoch);
                Ok(tonic::Response::new(response))
            });
    }

    fn json_to_prost_value(value: &serde_json::Value) -> prost_types::Value {
        use prost_types::value::Kind;

        let kind = match value {
            serde_json::Value::Null => Kind::NullValue(0),
            serde_json::Value::Bool(b) => Kind::BoolValue(*b),
            serde_json::Value::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or_default()),
            serde_json::Value::String(s) => Kind::StringValue(s.clone()),
            serde_json::Value::Array(arr) => Kind::ListValue(prost_types::ListValue {
                values: arr.iter().map(json_to_prost_value).collect(),
            }),
            serde_json::Value::Object(map) => Kind::StructValue(prost_types::Struct {
                fields: map
                    .iter()
                    .map(|(k, v)| (k.clone(), json_to_prost_value(v)))
                    .collect(),
            }),
        };

        prost_types::Value { kind: Some(kind) }
    }
}

/// Mocking GQL endpoints for deeper testing.
pub mod gql {
    use {
        crate::{events::NexusEventKind, sui},
        mockito::{Mock, ServerGuard},
        serde_json::json,
    };

    pub fn mock_event_query(
        server: &mut ServerGuard,
        primitives_pkg_id: sui::types::Address,
        events: Vec<NexusEventKind>,
        digest: Option<sui::types::Digest>,
        end_cursor: Option<&str>,
    ) -> Mock {
        let mut rng = rand::thread_rng();

        let mock_events = events
            .iter()
            .enumerate()
            .map(|(id, event)| {
                json!({
                    "sequenceNumber": id,
                    "transaction": {
                        "digest": digest.unwrap_or_else(|| sui::types::Digest::generate(&mut rng)),
                    },
                    "transactionModule": {
                        "package": {
                            "address": primitives_pkg_id,
                        }
                    },
                    "contents": {
                        "json": serde_json::to_value(event).unwrap(),
                        "type": {
                            "repr": sui::types::StructTag::new(
                                primitives_pkg_id,
                                sui::types::Identifier::from_static("event"),
                                sui::types::Identifier::from_static("EventWrapper"),
                                vec![
                                    sui::types::TypeTag::Struct(
                                        Box::new(sui::types::StructTag::new(
                                            primitives_pkg_id,
                                            sui::types::Identifier::from_static("test"),
                                            event.name().parse().unwrap(),
                                            vec![],
                                        )),
                                    ),
                                ],
                            )
                        }
                    }
                })
            })
            .collect::<Vec<serde_json::Value>>();

        server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_body(
                json!({
                    "data": {
                        "events": {
                            "nodes": mock_events,
                            "pageInfo": {
                                "endCursor": end_cursor.unwrap_or("12345"),
                            }
                        },
                    },
                })
                .to_string(),
            )
            .expect(1)
            .create()
    }
}
