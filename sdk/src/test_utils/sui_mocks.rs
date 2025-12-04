use crate::{sui, types::NexusObjects};

/// Create a new [`sui::Coin`] with random values.
pub fn mock_sui_coin(balance: u64) -> sui::Coin {
    sui::Coin {
        coin_type: "Sui".to_string(),
        coin_object_id: sui::ObjectID::random(),
        version: sui::SequenceNumber::from_u64(1),
        digest: sui::ObjectDigest::random(),
        balance,
        previous_transaction: sui::TransactionDigest::random(),
    }
}

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
pub fn mock_sui_event_id() -> sui::EventID {
    sui::EventID {
        tx_digest: sui::TransactionDigest::random(),
        event_seq: 0,
    }
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

/// Generate a new Sui address and its corresponding mnemonic.
pub fn mock_sui_mnemonic() -> (sui::Address, String) {
    let derivation_path = None;
    let word_length = None;

    let (addr, _, _, secret_mnemonic) =
        sui::generate_new_key(sui::SignatureScheme::ED25519, derivation_path, word_length)
            .expect("Failed to generate key.");

    (addr, secret_mnemonic)
}

pub mod grpc {
    use {
        mockall::mock,
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

        let ledger_service = mocks
            .ledger_service_mock
            .map(|m| LedgerServiceServer::new(m));
        let execution_service = mocks
            .execution_service_mock
            .map(|m| TransactionExecutionServiceServer::new(m));
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
}

/// Mocking RPC endpoints for deeper testing.
pub mod rpc {
    use {
        crate::{events::NexusEventKind, sui, test_utils::sui_mocks},
        mockito::{Matcher, Mock, ServerGuard},
        serde_json::json,
    };

    #[derive(serde::Deserialize)]
    struct PartialRequest {
        jsonrpc: String,
        id: u64,
    }

    pub fn mock_event_api_query_events(
        server: &mut ServerGuard,
        events: Vec<(String, NexusEventKind)>,
    ) -> Mock {
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "suix_queryEvents"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": {
                        "data": events
                            .iter()
                            .map(|(event_name, event)|
                                sui::Event {
                                    id: sui_mocks::mock_sui_event_id(),
                                    package_id: sui::ObjectID::random(),
                                    parsed_json: serde_json::to_value(event).expect("Failed to serialize event"),
                                    transaction_module: sui::move_ident_str!("test").into(),
                                    sender: sui::ObjectID::random().into(),
                                    type_: sui::MoveStructTag {
                                        address: sui::ObjectID::random().into(),
                                        module: sui::move_ident_str!("event").into(),
                                        name: sui::move_ident_str!("EventWrapper").into(),
                                        type_params: vec![
                                            sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                                                address: sui::ObjectID::random().into(),
                                                module: sui::move_ident_str!("test").into(),
                                                name: sui::Identifier::new(event_name.clone()).expect("Failed to parse event name"),
                                                type_params: vec![],
                                            })),
                                        ],
                                    },
                                    bcs: sui::BcsEvent::Base64 { bcs: vec![] },
                                    timestamp_ms: None,
                                }
                            )
                            .collect::<Vec<sui::Event>>(),
                        "nextCursor": null,
                        "hasNextPage": false,
                    },
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .create()
    }
}
