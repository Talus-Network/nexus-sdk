use {
    crate::{
        sui,
        types::{
            NexusContext,
            NexusObjects,
            NexusPackages,
            ObjectIdentity,
            PackageLink,
            PackageLinkage,
            PackageVersion,
            SharedRoot,
            TypeOrigins,
            UsTokenConfig,
        },
    },
    std::sync::Arc,
    sui_transaction_builder as tx,
};

/// Create a new [`sui::types::ObjectReference`] with random values.
pub fn mock_sui_object_ref() -> sui::types::ObjectReference {
    let mut rng = rand::thread_rng();

    sui::types::ObjectReference::new(
        sui::types::Address::generate(&mut rng),
        1,
        sui::types::Digest::generate(&mut rng),
    )
}

pub fn object_ref_for_id(object_id: sui::types::Address) -> sui::types::ObjectReference {
    sui::types::ObjectReference::new(object_id, 1, sui::types::Digest::from([1; 32]))
}

/// Creates a random event cursor.
pub fn mock_sui_event_id() -> (sui::types::Digest, u64) {
    let mut rng = rand::thread_rng();

    (sui::types::Digest::generate(&mut rng), 0)
}

/// Creates a random [`sui::types::Address`].
pub fn mock_sui_address() -> sui::types::Address {
    let mut rng = rand::thread_rng();

    sui::types::Address::generate(&mut rng)
}

/// Creates random [`NexusObjects`].
pub fn mock_nexus_objects() -> NexusObjects {
    let mut rng = rand::thread_rng();
    let shared_root = || {
        let object = mock_sui_object_ref();
        SharedRoot::new(*object.object_id(), object.version())
    };
    let identity = || ObjectIdentity::new(sui::types::Address::generate(rand::thread_rng()));

    NexusObjects {
        chain_id: sui::types::Digest::ZERO.to_string(),
        network_id: sui::types::Address::generate(&mut rng),
        tool_registry: shared_root(),
        network_auth: shared_root(),
        agent_registry: shared_root(),
        leader_registry: shared_root(),
        priority_fee_vault: shared_root(),
        runtime_authority: shared_root(),
        leader_admin_cap: identity(),
        tool_registry_admin_cap: identity(),
        slashing_cap: identity(),
        priority_fee_vault_owner_cap: identity(),
        initial_leader_cap: identity(),
        runtime_authority_cap: identity(),
        us_token: UsTokenConfig::new(
            sui::types::Address::generate(&mut rng),
            sui::types::Address::generate(&mut rng),
            sui::types::Address::generate(&mut rng),
        ),
    }
}

/// Creates the package graph used by SDK unit tests.
pub fn mock_nexus_packages() -> NexusPackages {
    fn package(
        address: &'static str,
        datatypes: &[(&str, &str)],
        dependencies: &[&'static str],
    ) -> PackageVersion {
        let address = sui::types::Address::from_static(address);
        let mut origins = TypeOrigins::new();
        for (module, datatype) in datatypes {
            origins
                .entry((*module).to_owned())
                .or_default()
                .insert((*datatype).to_owned(), address);
        }
        let linkage = dependencies
            .iter()
            .map(|dependency| {
                let dependency = sui::types::Address::from_static(dependency);
                (
                    dependency,
                    PackageLink {
                        storage_id: dependency,
                        version: 1,
                    },
                )
            })
            .collect::<PackageLinkage>();
        PackageVersion::new(address, address, 1, origins, linkage)
    }

    NexusPackages {
        primitives: Some(package(
            "0xa1",
            &[
                ("object_state", "Inner"),
                ("object_state", "Witness"),
                ("event", "EventWrapper"),
                ("proof_of_uid", "ProofOfUID"),
                ("tagged_output", "TaggedOutput"),
            ],
            &[],
        )),
        interface: Some(package(
            "0xa2",
            &[
                ("era", "V1"),
                ("agent", "Agent"),
                ("agent", "AgentInnerV1"),
                ("agent", "AgentPaymentVault"),
                ("agent", "AgentPaymentVaultInnerV1"),
                ("authorization", "AgentSkillAuthorization"),
                ("authorization", "AgentSkillAuthorizationInnerV1"),
                ("dag", "DAG"),
                ("dag", "DAGInnerV1"),
                ("graph", "Vertex"),
                ("graph", "VertexEvaluations"),
                ("graph", "VertexEvaluationsInnerV1"),
                ("onchain_tool_result", "OnchainToolResult"),
                ("onchain_tool_result", "OnchainToolResultInnerV1"),
                ("payment", "ExecutionPayment"),
                ("payment", "ExecutionPaymentInnerV1"),
                ("payment", "TaskPaymentReserve"),
                ("payment", "TaskPaymentReserveInnerV1"),
                ("verifier", "VerificationVerdict"),
            ],
            &["0xa1"],
        )),
        tool: Some(package(
            "0xa7",
            &[
                ("era", "V1"),
                ("tool_registry", "ToolRegistry"),
                ("tool_registry", "ToolRegistryInnerV1"),
                ("tool_registry", "Tool"),
                ("tool_registry", "ToolInnerV1"),
                ("tool_cashier", "ToolCashier"),
                ("tool_cashier", "ToolCashierInnerV1"),
                ("tool_cashier", "ToolCashierKey"),
                ("finite_credits", "Policy"),
                ("fixed_price", "Policy"),
                ("free_invocation", "Policy"),
                ("time_pass", "Policy"),
            ],
            &["0xa1", "0xa2"],
        )),
        registry: Some(package(
            "0xa3",
            &[
                ("era", "V1"),
                ("agent_registry", "AgentRegistry"),
                ("agent_registry", "AgentRegistryInnerV1"),
                ("leader", "LeaderRegistry"),
                ("leader", "LeaderRegistryInnerV1"),
                ("network_auth", "NetworkAuth"),
                ("network_auth", "NetworkAuthInnerV1"),
                ("network_auth", "IdentityKey"),
                ("network_auth", "KeyBinding"),
                ("network_auth", "KeyBindingInnerV1"),
                ("priority_fee_vault", "PriorityFeeVault"),
                ("priority_fee_vault", "PriorityFeeVaultInnerV1"),
            ],
            &["0xa1", "0xa2", "0xa7"],
        )),
        workflow: Some(package(
            "0xa4",
            &[
                ("era", "V1"),
                ("execution", "DAGExecution"),
                ("execution", "DAGExecutionInnerV1"),
            ],
            &["0xa1", "0xa2", "0xa7", "0xa3"],
        )),
        scheduler: Some(package(
            "0xa5",
            &[("era", "V1"), ("task", "Task"), ("task", "TaskInnerV1")],
            &["0xa1", "0xa2", "0xa7", "0xa3", "0xa4"],
        )),
    }
}

/// Creates an operation context for `objects` using the test package graph.
pub fn mock_nexus_context_for(objects: &NexusObjects) -> NexusContext {
    NexusContext::new(Arc::new(objects.clone()), mock_nexus_packages())
}

/// Creates stable environment identity and an operation package graph.
pub fn mock_nexus_context() -> NexusContext {
    let objects = mock_nexus_objects();
    mock_nexus_context_for(&objects)
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

/// Finish the given test transaction builder with mock gas data.
pub fn mock_finish_transaction(mut tx: tx::TransactionBuilder) -> sui::types::Transaction {
    let mut rng = rand::thread_rng();
    let gas = mock_sui_object_ref();

    tx.set_sender(sui::types::Address::generate(&mut rng));
    tx.set_gas_budget(1000);
    tx.set_gas_price(1000);
    tx.add_gas_objects(vec![tx::ObjectInput::owned(
        *gas.object_id(),
        gas.version(),
        *gas.digest(),
    )]);

    tx.try_build().expect("Transaction should build")
}

pub mod grpc {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            move_bindings::{
                move_std::{option::Option as MoveOption, type_name::TypeName},
                primitives::{data::NexusData, event as event_move},
                sui_framework::object::{ID, UID},
            },
            types::PackageRole,
        },
        mockall::mock,
        serde::Serialize,
        std::time::SystemTime,
        sui_rpc::proto::sui::rpc::v2::{
            ledger_service_server::{LedgerService, LedgerServiceServer},
            move_package_service_server::{MovePackageService, MovePackageServiceServer},
            state_service_server::{StateService, StateServiceServer},
            subscription_service_server::{SubscriptionService, SubscriptionServiceServer},
            transaction_execution_service_server::{
                TransactionExecutionService,
                TransactionExecutionServiceServer,
            },
            *,
        },
        sui_sdk_types::bcs::ToBcs,
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

            async fn list_events(
                &self,
                request: Request<ListEventsRequest>,
            ) -> Result<Response<BoxListEventsStream>, Status>;
        }
    }

    mock! {
        pub StateService {}

        #[tonic::async_trait]
        impl StateService for StateService {
            async fn list_dynamic_fields(
                &self,
                request: Request<ListDynamicFieldsRequest>,
            ) -> Result<Response<ListDynamicFieldsResponse>, Status>;

            async fn list_owned_objects(
                &self,
                request: Request<ListOwnedObjectsRequest>,
            ) -> Result<Response<ListOwnedObjectsResponse>, Status>;

            async fn get_coin_info(
                &self,
                request: Request<GetCoinInfoRequest>,
            ) -> Result<Response<GetCoinInfoResponse>, Status>;

            async fn get_balance(
                &self,
                request: Request<GetBalanceRequest>,
            ) -> Result<Response<GetBalanceResponse>, Status>;

            async fn list_balances(
                &self,
                request: Request<ListBalancesRequest>,
            ) -> Result<Response<ListBalancesResponse>, Status>;
        }
    }

    mock! {
        pub MovePackageService {}

        #[tonic::async_trait]
        impl MovePackageService for MovePackageService {
            async fn get_package(
                &self,
                request: Request<GetPackageRequest>,
            ) -> Result<Response<GetPackageResponse>, Status>;
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

    pub type BoxEventStream = std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<SubscribeEventsResponse, Status>> + Send + 'static>,
    >;

    pub type BoxListEventsStream = std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<ListEventsResponse, Status>> + Send + 'static>,
    >;

    /// The digest observed from a transaction submitted to a mock server.
    #[derive(Clone)]
    pub struct SubmittedTransaction {
        digest: tokio::sync::watch::Receiver<Option<sui::types::Digest>>,
    }

    impl SubmittedTransaction {
        /// Returns the submitted transaction digest after execution.
        pub fn digest(&self) -> sui::types::Digest {
            self.digest
                .borrow()
                .as_ref()
                .copied()
                .expect("the mock transaction should have been submitted")
        }
    }

    #[tonic::async_trait]

    pub trait SubscriptionServiceWrapper: Send + Sync + 'static {
        async fn subscribe_checkpoints(
            &self,

            request: Request<SubscribeCheckpointsRequest>,
        ) -> Result<Response<BoxCheckpointStream>, Status>;

        async fn subscribe_events(
            &self,
            request: Request<SubscribeEventsRequest>,
        ) -> Result<Response<BoxEventStream>, Status>;
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
        async fn subscribe_checkpoints(
            &self,
            request: Request<SubscribeCheckpointsRequest>,
        ) -> Result<Response<BoxCheckpointStream>, Status> {
            self.inner.subscribe_checkpoints(request).await
        }

        async fn subscribe_events(
            &self,
            request: Request<SubscribeEventsRequest>,
        ) -> Result<Response<BoxEventStream>, Status> {
            self.inner.subscribe_events(request).await
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

            async fn subscribe_events(
                &self,
                request: tonic::Request<SubscribeEventsRequest>,
            ) -> Result<tonic::Response<BoxEventStream>, tonic::Status>;
        }
    }

    #[derive(Default)]
    pub struct ServerMocks {
        /// Chain identity reported by the mock Sui service.
        pub chain_id: sui::types::Digest,
        pub ledger_service_mock: Option<MockLedgerService>,
        pub package_service_mock: Option<MockMovePackageService>,
        pub execution_service_mock: Option<MockTransactionExecutionService>,
        pub subscription_service_mock: Option<MockSubscriptionService>,
        pub state_service_mock: Option<MockStateService>,
    }

    pub fn mock_server(mut mocks: ServerMocks) -> String {
        // Bind a listener first so the returned URL is immediately connectable.
        //
        // This avoids flaky tests under parallel execution where `pick_unused_port` can race and
        // where the server may not yet be bound by the time the client connects.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("local addr");
        listener.set_nonblocking(true).expect("set nonblocking");
        let listener =
            tokio::net::TcpListener::from_std(listener).expect("tokio listener from std");
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

        let mut ledger_service = mocks.ledger_service_mock.take().unwrap_or_default();
        let chain_id = mocks.chain_id;
        ledger_service
            .expect_get_service_info()
            .times(0..)
            .returning(move |_request| {
                let mut response = sui::grpc::GetServiceInfoResponse::default();
                response.chain_id = Some(chain_id.to_string());
                Ok(tonic::Response::new(response))
            });
        let ledger_service = Some(LedgerServiceServer::new(ledger_service));
        let package_service = mocks
            .package_service_mock
            .map(MovePackageServiceServer::new);
        let execution_service = mocks
            .execution_service_mock
            .map(TransactionExecutionServiceServer::new);
        let subscription_service = mocks.subscription_service_mock.map(|m| {
            SubscriptionServiceServer::new(SubscriptionServiceAdapter::new(std::sync::Arc::new(m)))
        });
        let state_service = mocks.state_service_mock.map(StateServiceServer::new);

        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_optional_service(ledger_service)
                .add_optional_service(package_service)
                .add_optional_service(execution_service)
                .add_optional_service(subscription_service)
                .add_optional_service(state_service)
                .serve_with_incoming(incoming)
                .await
                .unwrap();
        });

        format!("http://{}", addr)
    }

    /// Serve immutable package metadata for one test operation graph.
    pub fn mock_nexus_package_graph(
        ledger_service: &mut MockLedgerService,
        package_service: &mut MockMovePackageService,
        packages: &NexusPackages,
    ) {
        mock_package_versions(ledger_service, package_service, packages.all().cloned());
    }

    #[derive(Serialize)]
    struct RuntimeAuthorityFixture {
        id: UID,
        scheduler_upgrade_cap: MoveOption<ID>,
        current_runtime: MoveOption<TypeName>,
        current_runtime_package: MoveOption<ID>,
        paused: bool,
    }

    /// Serve the fixed runtime root bound to the Scheduler package in `context`.
    pub fn mock_runtime_authority(
        ledger_service: &mut MockLedgerService,
        context: &NexusContext,
        paused: bool,
    ) {
        let root = context.runtime_authority;
        let runtime_package = context
            .require_package(PackageRole::Scheduler)
            .expect("mock context contains Scheduler")
            .storage_id;
        let state = RuntimeAuthorityFixture {
            id: UID::new(root.object_id()),
            scheduler_upgrade_cap: MoveOption::from_option(Some(ID::new(runtime_package))),
            current_runtime: MoveOption::from_option(Some(TypeName::new(&format!(
                "{runtime_package}::era::RuntimeV1"
            )))),
            current_runtime_package: MoveOption::from_option(Some(ID::new(runtime_package))),
            paused,
        };
        mock_get_object_bcs(
            ledger_service,
            object_ref_for_id(root.object_id()),
            sui::types::Owner::Shared(root.initial_shared_version),
            bcs::to_bytes(&state).expect("RuntimeAuthority fixture serializes as BCS"),
        );
    }

    /// Serve the fixed runtime root before its one time Scheduler binding.
    pub fn mock_unbound_runtime_authority(
        ledger_service: &mut MockLedgerService,
        context: &NexusContext,
    ) {
        let root = context.runtime_authority;
        let state = RuntimeAuthorityFixture {
            id: UID::new(root.object_id()),
            scheduler_upgrade_cap: MoveOption::from_option(None),
            current_runtime: MoveOption::from_option(None),
            current_runtime_package: MoveOption::from_option(None),
            paused: false,
        };
        mock_get_object_bcs(
            ledger_service,
            object_ref_for_id(root.object_id()),
            sui::types::Owner::Shared(root.initial_shared_version),
            bcs::to_bytes(&state).expect("RuntimeAuthority fixture serializes as BCS"),
        );
    }

    /// Serve immutable metadata for the supplied [`crate::types::PackageVersion`] values.
    pub fn mock_package_versions(
        ledger_service: &mut MockLedgerService,
        package_service: &mut MockMovePackageService,
        packages: impl IntoIterator<Item = crate::types::PackageVersion>,
    ) {
        let packages = packages
            .into_iter()
            .map(|package| {
                let mut grpc_package = sui::grpc::Package::default();
                grpc_package.set_storage_id(package.storage_id);
                grpc_package.set_original_id(package.initial_id);
                grpc_package.set_version(package.version);
                grpc_package.type_origins = package
                    .type_origins
                    .iter()
                    .flat_map(|(module, datatypes)| {
                        datatypes.iter().map(move |(datatype, package_id)| {
                            let mut origin = sui::grpc::TypeOrigin::default();
                            origin.set_module_name(module.clone());
                            origin.set_datatype_name(datatype.clone());
                            origin.set_package_id(*package_id);
                            origin
                        })
                    })
                    .collect();
                grpc_package.linkage = package
                    .linkage
                    .iter()
                    .map(|(lineage, link)| {
                        let mut linkage = sui::grpc::Linkage::default();
                        linkage.set_original_id(*lineage);
                        linkage.set_upgraded_id(link.storage_id);
                        linkage.set_upgraded_version(link.version);
                        linkage
                    })
                    .collect();
                (package.storage_id, grpc_package)
            })
            .collect::<std::collections::HashMap<_, _>>();

        let service_packages = packages.clone();
        package_service
            .expect_get_package()
            .times(0..)
            .returning(move |request| {
                let package_id = request
                    .get_ref()
                    .package_id
                    .as_deref()
                    .and_then(|value| value.parse::<sui::types::Address>().ok())
                    .ok_or_else(|| tonic::Status::invalid_argument("missing package ID"))?;
                let package = service_packages
                    .get(&package_id)
                    .cloned()
                    .ok_or_else(|| tonic::Status::not_found("package is absent"))?;
                let mut response = sui::grpc::GetPackageResponse::default();
                response.set_package(package);
                Ok(tonic::Response::new(response))
            });

        let package_ids = packages
            .keys()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request
                    .get_ref()
                    .object_id
                    .as_deref()
                    .and_then(|value| value.parse::<sui::types::Address>().ok())
                    .is_some_and(|package_id| package_ids.contains(&package_id))
            })
            .times(0..)
            .returning(move |request| {
                let package_id = request
                    .get_ref()
                    .object_id
                    .as_deref()
                    .and_then(|value| value.parse::<sui::types::Address>().ok())
                    .ok_or_else(|| tonic::Status::invalid_argument("missing package ID"))?;
                let package = packages
                    .get(&package_id)
                    .cloned()
                    .ok_or_else(|| tonic::Status::not_found("package is absent"))?;
                let mut object = sui::grpc::Object::default();
                object.set_object_id(package_id);
                object.set_version(package.version());
                object.set_object_type("package");
                object.set_package(package);
                let mut response = sui::grpc::GetObjectResponse::default();
                response.set_object(object);
                Ok(tonic::Response::new(response))
            });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mock_execute_transaction_and_wait_for_checkpoint(
        tx_service: &mut MockTransactionExecutionService,
        sub_service: &mut MockSubscriptionService,
        ledger_service: &mut MockLedgerService,
        gas_coin_ref: sui::types::ObjectReference,
        objects: Vec<sui::types::Object>,
        changed_objects: Vec<sui::types::ChangedObject>,
        events: Vec<sui::types::Event>,
    ) -> SubmittedTransaction {
        mock_execute_transaction_and_wait_for_checkpoint_matching(
            tx_service,
            sub_service,
            ledger_service,
            gas_coin_ref,
            objects,
            changed_objects,
            events,
            |_| {},
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mock_execute_transaction_and_wait_for_checkpoint_matching<F>(
        tx_service: &mut MockTransactionExecutionService,
        sub_service: &mut MockSubscriptionService,
        ledger_service: &mut MockLedgerService,
        gas_coin_ref: sui::types::ObjectReference,
        objects: Vec<sui::types::Object>,
        changed_objects: Vec<sui::types::ChangedObject>,
        events: Vec<sui::types::Event>,
        assert_request: F,
    ) -> SubmittedTransaction
    where
        F: Fn(&ExecuteTransactionRequest) + Send + Sync + 'static,
    {
        mock_execute_transaction_and_wait_for_checkpoint_inner(
            tx_service,
            sub_service,
            ledger_service,
            Some(gas_coin_ref),
            objects,
            changed_objects,
            events,
            assert_request,
        )
    }

    /// Configures execution and checkpoint mocks for a transaction without an
    /// owned gas object.
    #[allow(clippy::too_many_arguments)]
    pub fn mock_execute_transaction_without_gas_and_wait_for_checkpoint<F>(
        tx_service: &mut MockTransactionExecutionService,
        sub_service: &mut MockSubscriptionService,
        ledger_service: &mut MockLedgerService,
        objects: Vec<sui::types::Object>,
        changed_objects: Vec<sui::types::ChangedObject>,
        events: Vec<sui::types::Event>,
        assert_request: F,
    ) -> SubmittedTransaction
    where
        F: Fn(&ExecuteTransactionRequest) + Send + Sync + 'static,
    {
        mock_execute_transaction_and_wait_for_checkpoint_inner(
            tx_service,
            sub_service,
            ledger_service,
            None,
            objects,
            changed_objects,
            events,
            assert_request,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn mock_execute_transaction_and_wait_for_checkpoint_inner<F>(
        tx_service: &mut MockTransactionExecutionService,
        sub_service: &mut MockSubscriptionService,
        ledger_service: &mut MockLedgerService,
        gas_coin_ref: Option<sui::types::ObjectReference>,
        objects: Vec<sui::types::Object>,
        changed_objects: Vec<sui::types::ChangedObject>,
        events: Vec<sui::types::Event>,
        assert_request: F,
    ) -> SubmittedTransaction
    where
        F: Fn(&ExecuteTransactionRequest) + Send + Sync + 'static,
    {
        let (submitted_digest, observed_digest) = tokio::sync::watch::channel(None);
        let checkpoint_digest = observed_digest.clone();
        let mut changed_objects_with_coin = gas_coin_ref
            .as_ref()
            .map(|gas_coin_ref| sui::types::ChangedObject {
                object_id: sui::types::Address::from_static("0x1"),
                input_state: sui::types::ObjectIn::NotExist,
                output_state: sui::types::ObjectOut::ObjectWrite {
                    digest: *gas_coin_ref.digest(),
                    owner: sui::types::Owner::Address(sui::types::Address::from_static("0x1")),
                },
                id_operation: sui::types::IdOperation::None,
            })
            .into_iter()
            .collect::<Vec<_>>();
        let gas_object_index = gas_coin_ref.as_ref().map(|_| 0);

        changed_objects_with_coin.extend(changed_objects.clone());

        sub_service
            .expect_subscribe_checkpoints()
            .times(1)
            .returning(move |_request| {
                let mut checkpoint_digest = checkpoint_digest.clone();
                let stream = futures::stream::once(async move {
                    let digest = {
                        let observed = checkpoint_digest
                            .wait_for(Option::is_some)
                            .await
                            .map_err(|_| tonic::Status::aborted("transaction was not submitted"))?;
                        observed.expect("the observed digest is present")
                    };
                    let mut response = sui::grpc::SubscribeCheckpointsResponse::default();
                    let mut checkpoint = sui::grpc::Checkpoint::default();
                    let mut tx = sui::grpc::ExecutedTransaction::default();

                    tx.set_digest(digest);
                    checkpoint.set_transactions(vec![tx]);
                    checkpoint.set_sequence_number(1);
                    response.set_checkpoint(checkpoint);

                    Ok(response)
                });

                Ok(tonic::Response::new(Box::pin(stream) as BoxCheckpointStream))
            });

        tx_service
            .expect_execute_transaction()
            .times(1)
            .returning(move |request| {
                assert_request(request.get_ref());
                let transaction =
                    sui::types::Transaction::try_from(request.get_ref().transaction())
                        .expect("the submitted transaction should decode");
                let digest = transaction.digest();
                submitted_digest
                    .send(Some(digest))
                    .expect("the checkpoint observer should remain active");
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
                    gas_object_index,
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

        if let Some(gas_coin_ref) = gas_coin_ref {
            mock_get_object_metadata(
                ledger_service,
                gas_coin_ref,
                sui::types::Owner::Immutable,
                Some(1000),
            );
        }

        ledger_service
            .expect_get_transaction()
            .withf(|request| {
                request
                    .get_ref()
                    .read_mask
                    .as_ref()
                    .is_some_and(|mask| mask.paths.iter().any(|path| path == "timestamp"))
            })
            .times(2)
            .returning(|_| Err(tonic::Status::not_found("transaction is not indexed yet")));

        SubmittedTransaction {
            digest: observed_digest,
        }
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

    /// Configures the epoch used by address balance transaction construction.
    pub fn mock_submission_context(
        ledger_service: &mut MockLedgerService,
        reference_gas_price: u64,
        epoch_number: u64,
    ) {
        ledger_service
            .expect_get_epoch()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetEpochResponse::default();
                let mut epoch = sui::grpc::Epoch::default();
                epoch.set_epoch(epoch_number);
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
            .withf(|request| {
                request
                    .get_ref()
                    .read_mask
                    .as_ref()
                    .is_none_or(|mask| !mask.paths.iter().any(|path| path == "contents"))
            })
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

    pub fn mock_get_object_metadata_exact(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        balance: Option<u64>,
    ) {
        let expected_id = object_ref.object_id().to_string();
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                let request = request.get_ref();
                request.object_id.as_deref() == Some(expected_id.as_str())
                    && request
                        .read_mask
                        .as_ref()
                        .is_none_or(|mask| !mask.paths.iter().any(|path| path == "contents"))
            })
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut grpc_object = sui::grpc::Object::default();
                grpc_object.set_owner(sui::grpc::Owner::from(owner));
                grpc_object.set_digest(*object_ref.digest());
                grpc_object.set_object_id(*object_ref.object_id());
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
                grpc_object.set_owner(sui::grpc::Owner::from(owner));
                grpc_object.set_digest(*object_ref.digest());
                grpc_object.set_version(object_ref.version());
                grpc_object.set_object_id(object_ref.object_id().to_string());
                grpc_object.json = Some(Box::new(json_to_prost_value(&json_value)));
                response.set_object(grpc_object);
                Ok(tonic::Response::new(response))
            });
    }

    /// Expect a `get_object` call and return an object populated with metadata
    /// and BCS contents.
    pub fn mock_get_object_bcs(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        contents: Vec<u8>,
    ) {
        let expected_id = object_ref.object_id().to_string();
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request.get_ref().object_id.as_deref() == Some(expected_id.as_str())
            })
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut grpc_object = sui::grpc::Object::default();
                grpc_object.set_owner(sui::grpc::Owner::from(owner));
                grpc_object.set_digest(*object_ref.digest());
                grpc_object.set_version(object_ref.version());
                grpc_object.set_object_id(object_ref.object_id().to_string());
                let mut bcs = sui::grpc::Bcs::default();
                bcs.value = Some(contents.clone().into());
                grpc_object.contents = Some(bcs);
                response.set_object(grpc_object);
                Ok(tonic::Response::new(response))
            });
    }

    /// Expect one object request for `object_id` and report that it is absent.
    pub fn mock_get_object_not_found(
        ledger_service: &mut MockLedgerService,
        object_id: sui::types::Address,
    ) {
        let expected_id = object_id.to_string();
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request.get_ref().object_id.as_deref() == Some(expected_id.as_str())
            })
            .times(1)
            .returning(|_| Err(tonic::Status::not_found("object is absent")));
    }

    pub fn mock_get_object_bcs_for(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        contents: Vec<u8>,
        object_type: sui::types::StructTag,
    ) {
        let expected_id = object_ref.object_id().to_string();
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request.get_ref().object_id.as_deref() == Some(expected_id.as_str())
            })
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut grpc_object = sui::grpc::Object::default();
                grpc_object.set_owner(sui::grpc::Owner::from(owner));
                grpc_object.set_digest(*object_ref.digest());
                grpc_object.set_version(object_ref.version());
                grpc_object.set_object_id(object_ref.object_id().to_string());
                grpc_object.set_object_type(object_type.to_string());
                let mut bcs = sui::grpc::Bcs::default();
                bcs.set_name(object_type.to_string());
                bcs.set_value(contents.clone());
                grpc_object.contents = Some(bcs);
                response.set_object(grpc_object);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_get_object_value_bcs_for<T: Serialize>(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        value: &T,
        object_type: sui::types::StructTag,
    ) {
        mock_get_object_bcs_for(
            ledger_service,
            object_ref,
            owner,
            bcs::to_bytes(value).expect("mock object value serializes as BCS"),
            object_type,
        );
    }

    fn mock_object_state_metadata<A, W, V>(
        ledger_service: &mut MockLedgerService,
        state_service: &mut MockStateService,
        context: &NexusContext,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        anchor: A,
    ) -> (
        sui::types::Address,
        crate::move_bindings::primitives::object_state::Inner,
        sui::types::StructTag,
    )
    where
        A: Serialize + sui_move::MoveStruct + Clone + Send + Sync + 'static,
        W: sui_move::MoveStruct,
        V: sui_move::MoveStruct,
    {
        use crate::move_bindings::primitives::object_state::{Inner, Witness};

        let object_id = *object_ref.object_id();
        let anchor_type = crate::move_bindings::struct_tag::<A>(context);
        let witness_key = Witness::new(false);
        let inner_key = Inner::new(false);
        let witness_key_type = crate::move_bindings::type_tag::<Witness>(context);
        let inner_key_type = crate::move_bindings::type_tag::<Inner>(context);
        let witness_type = crate::move_bindings::type_tag::<W>(context);
        let inner_type = crate::move_bindings::type_tag::<V>(context);
        let witness_field_id = object_id.derive_dynamic_child_id(
            &witness_key_type,
            &bcs::to_bytes(&witness_key).expect("Witness key serializes"),
        );
        let inner_field_id = object_id.derive_dynamic_child_id(
            &inner_key_type,
            &bcs::to_bytes(&inner_key).expect("Inner key serializes"),
        );

        let dynamic_field_type = |key: sui::types::TypeTag, value: sui::types::TypeTag| {
            sui::types::StructTag::new(
                sui::types::Address::from_static("0x2"),
                sui::types::Identifier::new("dynamic_field").unwrap(),
                sui::types::Identifier::new("Field").unwrap(),
                vec![key, value],
            )
        };
        let witness_field_type = dynamic_field_type(witness_key_type, witness_type.clone());
        let inner_field_type = dynamic_field_type(inner_key_type, inner_type.clone());

        let expected_parent = object_id.to_string();
        let listed_inner_field_type = inner_field_type.clone();
        state_service
            .expect_list_dynamic_fields()
            .withf(move |request| request.get_ref().parent_opt() == Some(expected_parent.as_str()))
            .times(1..)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let mut witness_field = sui::grpc::DynamicField::default();
                witness_field.set_field_id(witness_field_id);
                witness_field.set_child_id(witness_field_id);
                witness_field.set_value_type(witness_type.to_string());
                let mut witness_object = sui::grpc::Object::default();
                witness_object.set_object_type(witness_field_type.to_string());
                witness_field.set_field_object(witness_object);

                let mut inner_field = sui::grpc::DynamicField::default();
                inner_field.set_field_id(inner_field_id);
                inner_field.set_child_id(inner_field_id);
                inner_field.set_value_type(inner_type.to_string());
                let mut inner_object = sui::grpc::Object::default();
                inner_object.set_object_type(listed_inner_field_type.to_string());
                inner_field.set_field_object(inner_object);
                response.set_dynamic_fields(vec![witness_field, inner_field]);
                Ok(tonic::Response::new(response))
            });

        let expected_anchor_id = object_id.to_string();
        let anchor_contents = bcs::to_bytes(&anchor).expect("Anchor serializes");
        let anchor_owner = owner;
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                let request = request.get_ref();
                request.object_id.as_deref() == Some(expected_anchor_id.as_str())
                    && request.version.is_none()
                    && request.read_mask.as_ref().is_none_or(|mask| {
                        !mask.paths.iter().any(|path| path == "previous_transaction")
                    })
            })
            .times(1..)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut object = sui::grpc::Object::default();
                object.set_object_id(object_id);
                object.set_owner(sui::grpc::Owner::from(anchor_owner));
                object.set_version(object_ref.version());
                object.set_digest(*object_ref.digest());
                object.set_object_type(anchor_type.to_string());
                let mut contents = sui::grpc::Bcs::default();
                contents.set_name(anchor_type.to_string());
                contents.set_value(anchor_contents.clone());
                object.set_contents(contents);
                response.set_object(object);
                Ok(tonic::Response::new(response))
            });

        (inner_field_id, inner_key, inner_field_type)
    }

    /// Mock an object anchor and its typed state field metadata.
    ///
    /// Use this for operations that validate the state pair without decoding
    /// the stored value. Reads may repeat without encoding RPC call counts in
    /// the test.
    #[allow(clippy::too_many_arguments)]
    pub fn mock_object_state_observation<A, W, V>(
        ledger_service: &mut MockLedgerService,
        state_service: &mut MockStateService,
        context: &NexusContext,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        anchor: A,
    ) where
        A: Serialize + sui_move::MoveStruct + Clone + Send + Sync + 'static,
        W: sui_move::MoveStruct,
        V: sui_move::MoveStruct,
    {
        let _ = mock_object_state_metadata::<A, W, V>(
            ledger_service,
            state_service,
            context,
            object_ref,
            owner,
            anchor,
        );
    }

    /// Mock one object anchor and its typed object state fields.
    ///
    /// The anchor and field metadata may be read repeatedly. The
    /// [`Witness`](crate::move_bindings::primitives::object_state::Witness)
    /// value is not fetched because package authority is selected from its
    /// exact field type.
    #[allow(clippy::too_many_arguments)]
    pub fn mock_object_state<A, W, V>(
        ledger_service: &mut MockLedgerService,
        state_service: &mut MockStateService,
        context: &NexusContext,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        anchor: A,
        inner: V,
    ) where
        A: Serialize + sui_move::MoveStruct + Clone + Send + Sync + 'static,
        W: sui_move::MoveStruct,
        V: Serialize + sui_move::MoveStruct + Clone + Send + Sync + 'static,
    {
        #[derive(Clone, Serialize)]
        struct DynamicFieldValue<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        let object_id = *object_ref.object_id();
        let (inner_field_id, inner_key, inner_field_type) = mock_object_state_metadata::<A, W, V>(
            ledger_service,
            state_service,
            context,
            object_ref,
            owner,
            anchor,
        );

        let inner_field = DynamicFieldValue {
            id: inner_field_id,
            name: inner_key,
            value: inner,
        };
        let inner_contents = bcs::to_bytes(&inner_field).expect("Inner field serializes");
        let expected_inner_id = inner_field_id.to_string();
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request.get_ref().object_id.as_deref() == Some(expected_inner_id.as_str())
            })
            .times(0..)
            .returning(move |_request| {
                let mut response = sui::grpc::GetObjectResponse::default();
                let mut object = sui::grpc::Object::default();
                object.set_object_id(inner_field_id);
                object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Object(object_id)));
                object.set_version(1);
                object.set_digest(sui::types::Digest::from([1; 32]));
                object.set_object_type(inner_field_type.to_string());
                let mut contents = sui::grpc::Bcs::default();
                contents.set_name(inner_field_type.to_string());
                contents.set_value(inner_contents.clone());
                object.set_contents(contents);
                response.set_object(object);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_get_objects_bcs(
        ledger_service: &mut MockLedgerService,
        objects: Vec<(
            sui::types::ObjectReference,
            sui::types::Owner,
            Vec<u8>,
            sui::types::StructTag,
        )>,
    ) {
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                let mut objs = Vec::with_capacity(objects.len());

                for (object_ref, owner, contents, object_type) in objects.clone() {
                    let mut result = sui::grpc::GetObjectResult::default();
                    let mut grpc_object = sui::grpc::Object::default();
                    grpc_object.set_owner(sui::grpc::Owner::from(owner));
                    grpc_object.set_digest(*object_ref.digest());
                    grpc_object.set_version(object_ref.version());
                    grpc_object.set_object_id(object_ref.object_id().to_string());
                    grpc_object.set_object_type(object_type.to_string());
                    let mut bcs = sui::grpc::Bcs::default();
                    bcs.set_name(object_type.to_string());
                    bcs.set_value(contents);
                    grpc_object.contents = Some(bcs);
                    result.set_object(grpc_object);
                    objs.push(result);
                }

                response.set_objects(objs);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_empty_batch_get_objects(ledger_service: &mut MockLedgerService, times: usize) {
        ledger_service
            .expect_batch_get_objects()
            .times(times)
            .returning(|_request| {
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::default(),
                ))
            });
    }

    /// Expect one exact dynamic field request derived from its parent and typed key.
    ///
    /// The returned address is the canonical field object ID used by the mock.
    ///
    /// # Panics
    ///
    /// Panics when the key or field fixture cannot be encoded as BCS.
    pub fn mock_get_dynamic_field_by_key<K, V>(
        ledger_service: &mut MockLedgerService,
        parent_id: sui::types::Address,
        key_type: &sui::types::TypeTag,
        key: K,
        value: V,
    ) -> sui::types::Address
    where
        K: Serialize,
        V: Serialize,
    {
        #[derive(Serialize)]
        struct DynamicFieldValueBcs<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        let field_id = parent_id.derive_dynamic_child_id(
            key_type,
            &bcs::to_bytes(&key).expect("dynamic field key serializes"),
        );
        let field = DynamicFieldValueBcs {
            id: field_id,
            name: key,
            value,
        };
        mock_get_object_bcs(
            ledger_service,
            object_ref_for_id(field_id),
            sui::types::Owner::Object(parent_id),
            bcs::to_bytes(&field).expect("dynamic field serializes"),
        );
        field_id
    }

    /// Expect an exact dynamic object wrapper request followed by its child request.
    ///
    /// The returned address is the canonical wrapper field ID used by the mock.
    ///
    /// # Panics
    ///
    /// Panics when the key, wrapper field, or child cannot be encoded as BCS.
    pub fn mock_get_dynamic_object_field_by_key<K, V>(
        ledger_service: &mut MockLedgerService,
        parent_id: sui::types::Address,
        key_type: &sui::types::TypeTag,
        key: K,
        child_ref: sui::types::ObjectReference,
        child_owner: sui::types::Owner,
        child: V,
    ) -> sui::types::Address
    where
        K: Serialize,
        V: Serialize,
    {
        #[derive(Serialize)]
        struct DynamicObjectFieldName<K> {
            name: K,
        }

        #[derive(Serialize)]
        struct ObjectId {
            bytes: sui::types::Address,
        }

        let wrapper_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_object_field"),
            sui::types::Identifier::from_static("Wrapper"),
            vec![key_type.clone()],
        )));
        let wrapper_id = mock_get_dynamic_field_by_key(
            ledger_service,
            parent_id,
            &wrapper_type,
            DynamicObjectFieldName { name: key },
            ObjectId {
                bytes: *child_ref.object_id(),
            },
        );
        mock_get_object_bcs(
            ledger_service,
            child_ref,
            child_owner,
            bcs::to_bytes(&child).expect("dynamic object child serializes"),
        );
        wrapper_id
    }

    pub fn mock_get_dynamic_field_values_bcs<T>(
        ledger_service: &mut MockLedgerService,
        objects: Vec<(sui::types::ObjectReference, sui::types::Owner, T)>,
    ) where
        T: Serialize + Clone + Send + 'static,
    {
        let objects = objects
            .into_iter()
            .map(|(object_ref, owner, value)| {
                (
                    object_ref,
                    owner,
                    bcs::to_bytes(&value).expect("dynamic field value serializes"),
                )
            })
            .collect::<Vec<_>>();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                let mut objs = Vec::with_capacity(objects.len());

                for (object_ref, owner, contents) in objects.clone() {
                    let mut result = sui::grpc::GetObjectResult::default();
                    let mut grpc_object = sui::grpc::Object::default();
                    grpc_object.set_owner(sui::grpc::Owner::from(owner));
                    grpc_object.set_digest(*object_ref.digest());
                    grpc_object.set_version(object_ref.version());
                    grpc_object.set_object_id(object_ref.object_id().to_string());
                    let mut bcs = sui::grpc::Bcs::default();
                    bcs.set_value(contents);
                    grpc_object.contents = Some(bcs);
                    result.set_object(grpc_object);
                    objs.push(result);
                }

                response.set_objects(objs);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_get_dynamic_table_values_bcs<K, V>(
        ledger_service: &mut MockLedgerService,
        objects: Vec<(sui::types::ObjectReference, sui::types::Owner, K, V)>,
    ) where
        K: Serialize + Clone + Send + 'static,
        V: Serialize + Clone + Send + 'static,
    {
        #[derive(Clone, Serialize)]
        struct DynamicFieldValueBcs<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        let objects = objects
            .into_iter()
            .map(|(object_ref, owner, name, value)| {
                let field = DynamicFieldValueBcs {
                    id: *object_ref.object_id(),
                    name,
                    value,
                };
                (
                    object_ref,
                    owner,
                    bcs::to_bytes(&field).expect("dynamic table field serializes"),
                )
            })
            .collect::<Vec<_>>();

        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                let mut objs = Vec::with_capacity(objects.len());

                for (object_ref, owner, contents) in objects.clone() {
                    let mut result = sui::grpc::GetObjectResult::default();
                    let mut grpc_object = sui::grpc::Object::default();
                    grpc_object.set_owner(sui::grpc::Owner::from(owner));
                    grpc_object.set_digest(*object_ref.digest());
                    grpc_object.set_version(object_ref.version());
                    grpc_object.set_object_id(object_ref.object_id().to_string());
                    let mut bcs = sui::grpc::Bcs::default();
                    bcs.set_value(contents);
                    grpc_object.contents = Some(bcs);
                    result.set_object(grpc_object);
                    objs.push(result);
                }

                response.set_objects(objs);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_get_dynamic_table_value_bcs<K, V>(
        ledger_service: &mut MockLedgerService,
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        name: K,
        value: V,
    ) where
        K: Serialize,
        V: Serialize,
    {
        #[derive(Serialize)]
        struct DynamicFieldValueBcs<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        let field = DynamicFieldValueBcs {
            id: *object_ref.object_id(),
            name,
            value,
        };
        mock_get_object_bcs(
            ledger_service,
            object_ref,
            owner,
            bcs::to_bytes(&field).expect("dynamic table field serializes"),
        );
    }

    /// Expect a `batch_get_objects` call and return an object populated with metadata
    /// and a JSON payload (converted into `prost_types::Value`).
    pub fn mock_get_objects_json(
        ledger_service: &mut MockLedgerService,
        objects: Vec<(
            sui::types::ObjectReference,
            sui::types::Owner,
            serde_json::Value,
        )>,
    ) {
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                let mut objs = Vec::with_capacity(objects.len());

                for (object_ref, owner, json_value) in objects.clone() {
                    let mut result = sui::grpc::GetObjectResult::default();
                    let mut grpc_object = sui::grpc::Object::default();
                    grpc_object.set_owner(sui::grpc::Owner::from(owner));
                    grpc_object.set_digest(*object_ref.digest());
                    grpc_object.set_version(object_ref.version());
                    grpc_object.set_object_id(object_ref.object_id().to_string());
                    grpc_object.json = Some(Box::new(json_to_prost_value(&json_value)));
                    result.set_object(grpc_object.clone());
                    objs.push(result);
                }

                response.set_objects(objs);
                Ok(tonic::Response::new(response))
            });
    }

    /// Expect a `get_epoch` call and return the end timestamp.
    pub fn mock_get_epoch_end(ledger_service: &mut MockLedgerService, epoch_end: SystemTime) {
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

    /// Expect a `list_dynamic_fields` call and return the given dynamic fields.
    pub fn mock_list_dynamic_fields<T: Serialize + Clone + Send + 'static>(
        state_service: &mut MockStateService,
        fields: Vec<(T, sui::types::Address)>,
    ) {
        state_service
            .expect_list_dynamic_fields()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let mut dynamic_fields = Vec::new();

                for (key, id) in fields.clone() {
                    let mut dynamic_field = sui::grpc::DynamicField::default();
                    dynamic_field.set_child_id(id);
                    dynamic_field.set_field_id(id);
                    dynamic_field.set_name(key.to_bcs().expect("Cannot serialize BCS key"));
                    // dynamic_field.set_value(key.to_bcs().expect("Cannot serialize BCS key"));
                    dynamic_fields.push(dynamic_field);
                }

                response.set_dynamic_fields(dynamic_fields);
                Ok(tonic::Response::new(response))
            });
    }

    /// Expect a dynamic field listing for one exact parent object.
    pub fn mock_list_dynamic_fields_for<T: Serialize + Clone + Send + 'static>(
        state_service: &mut MockStateService,
        parent_id: sui::types::Address,
        fields: Vec<(T, sui::types::Address)>,
    ) {
        let expected_parent = parent_id.to_string();
        state_service
            .expect_list_dynamic_fields()
            .withf(move |request| request.get_ref().parent_opt() == Some(expected_parent.as_str()))
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let dynamic_fields = fields
                    .clone()
                    .into_iter()
                    .map(|(key, id)| {
                        let mut field = sui::grpc::DynamicField::default();
                        field.set_child_id(id);
                        field.set_field_id(id);
                        field.set_name(key.to_bcs().expect("Cannot serialize BCS key"));
                        field
                    })
                    .collect();
                response.set_dynamic_fields(dynamic_fields);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_empty_dynamic_fields(state_service: &mut MockStateService, times: usize) {
        state_service
            .expect_list_dynamic_fields()
            .times(times)
            .returning(|_request| {
                Ok(tonic::Response::new(
                    sui::grpc::ListDynamicFieldsResponse::default(),
                ))
            });
    }

    pub fn mock_list_dynamic_object_fields<T: Serialize + Clone + Send + 'static>(
        state_service: &mut MockStateService,
        fields: Vec<(T, sui::types::Address, sui::types::Address)>,
    ) {
        state_service
            .expect_list_dynamic_fields()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                let mut dynamic_fields = Vec::new();

                for (key, field_id, child_id) in fields.clone() {
                    let mut dynamic_field = sui::grpc::DynamicField::default();
                    dynamic_field.set_child_id(child_id);
                    dynamic_field.set_field_id(field_id);
                    dynamic_field.set_name(key.to_bcs().expect("Cannot serialize BCS key"));
                    dynamic_fields.push(dynamic_field);
                }

                response.set_dynamic_fields(dynamic_fields);
                Ok(tonic::Response::new(response))
            });
    }

    pub fn mock_events_get_checkpoint(
        ledger_service: &mut MockLedgerService,
        objects: NexusObjects,
        nexus_events: Vec<NexusEventKind>,
        cp: u64,
    ) {
        let context = mock_nexus_context_for(&objects);
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
                    let interface = context
                        .require_package(crate::types::PackageRole::Interface)
                        .expect("test context contains Interface");
                    let workflow = context
                        .require_package(crate::types::PackageRole::Workflow)
                        .expect("test context contains Workflow");
                    let (event_pkg_id, event_type_origin, event_module, event_name) = match event {
                        NexusEventKind::DAGCreated(_) => (
                            interface.storage_id,
                            interface.initial_id,
                            "dag",
                            event.name(),
                        ),
                        NexusEventKind::WalkAdvanced(_)
                        | NexusEventKind::EndStateReached(_)
                        | NexusEventKind::ExecutionFinished(_)
                        | NexusEventKind::TerminalErrEvalRecorded(_) => (
                            workflow.storage_id,
                            workflow.initial_id,
                            "execution_events",
                            event.name(),
                        ),
                        _ => panic!("Unsupported event type for mock event serialization"),
                    };
                    let wrapper_tag = crate::move_bindings::struct_tag::<
                        event_move::EventWrapper<NexusData>,
                    >(&context);
                    let t = format!(
                        "{}::{}::{}<{}::{}::{}>",
                        wrapper_tag.address(),
                        wrapper_tag.module(),
                        wrapper_tag.name(),
                        event_type_origin,
                        event_module,
                        event_name
                    );

                    let mut grpc_event = sui::grpc::Event::default();
                    grpc_event.set_package_id(event_pkg_id);
                    grpc_event.set_module(wrapper_tag.module().to_string());
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
                        NexusEventKind::DAGCreated(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        NexusEventKind::TerminalErrEvalRecorded(e) => {
                            bcs::to_bytes(&Wrapper { event: e }).unwrap()
                        }
                        _ => unreachable!("unsupported event type rejected before serialization"),
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

    pub fn mock_events_stream(sub_service: &mut MockSubscriptionService, cp: u64) {
        sub_service
            .expect_subscribe_checkpoints()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::SubscribeCheckpointsResponse::default();
                let mut checkpoint = sui::grpc::Checkpoint::default();
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(sui::types::Digest::ZERO);
                checkpoint.set_transactions(vec![transaction]);
                checkpoint.set_sequence_number(cp);
                response.set_checkpoint(checkpoint);

                let output = Ok(response);
                let stream = futures::stream::repeat(output.clone());

                Ok(tonic::Response::new(Box::pin(stream) as BoxCheckpointStream))
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
