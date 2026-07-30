use crate::{
    move_bindings::{
        move_std::option::Option as MoveOption,
        registry::network_auth::{
            IdentityKey,
            KeyBinding,
            KeyBindingStateV1,
            KeyRecord,
            NetworkAuth,
            NetworkAuthStateV1,
        },
        sui_framework::{
            object::{ID, UID},
            table::Table as MoveTable,
            vec_set::VecSet,
            versioned::Versioned,
        },
    },
    nexus::{client::NexusClient, network_auth::NetworkAuthReader},
    sui,
    test_utils::sui_mocks,
    types::NexusObjects,
};

/// Create a mock [`NexusClient`] that is connected to a mock RPC using [`mockito`].
pub async fn mock_nexus_client(nexus_objects: &NexusObjects, rpc_url: &str) -> NexusClient {
    let mut rng = rand::thread_rng();
    let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);

    let coin = sui_mocks::mock_sui_object_ref();

    NexusClient::builder()
        .with_private_key(pk)
        .with_rpc_url(rpc_url)
        .with_nexus_objects(nexus_objects.clone())
        .with_gas(vec![coin], 1000)
        .build()
        .await
        .expect("Failed to build NexusClient")
}

/// Create a mock [`NexusClient`] with no attached gas source.
///
/// Read-boundary tests use this fixture to prove that no owned gas coin or
/// reference-gas-price RPC is needed to construct the client.
pub async fn mock_nexus_client_without_coins(
    nexus_objects: &NexusObjects,
    rpc_url: &str,
) -> NexusClient {
    let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());

    NexusClient::builder()
        .with_private_key(pk)
        .with_rpc_url(rpc_url)
        .with_nexus_objects(nexus_objects.clone())
        .build()
        .await
        .expect("Failed to build coin-free NexusClient")
}

/// Create a direct network-auth reader with one mocked allowed leader.
pub fn mock_network_auth_reader_without_wallet() -> NetworkAuthReader {
    let nexus_objects = sui_mocks::mock_nexus_objects();
    let network_auth_id = *nexus_objects.network_auth.object_id();
    let leader_id = sui::types::Address::from_static("0x71");
    let identity = IdentityKey::leader(leader_id);
    let derivation_reader = NetworkAuthReader::from_rpc_url(
        "http://127.0.0.1:1",
        nexus_objects.registry_type_origin_pkg_id(),
        network_auth_id,
    )
    .expect("derivation-only network-auth reader");
    let binding_id = derivation_reader
        .binding_object_id(&identity)
        .expect("leader binding id");
    let key_table_id = sui::types::Address::from_static("0x72");
    let network_auth_state_id = sui::types::Address::from_static("0x73");
    let binding_state_id = sui::types::Address::from_static("0x74");
    let active_kid = 0;
    let network_auth = NetworkAuth::new(
        UID::new(network_auth_id),
        Versioned::new(UID::new(network_auth_state_id), 1),
    );
    let network_auth_state = NetworkAuthStateV1::new(
        ID::new(sui::types::Address::ZERO),
        1,
        VecSet {
            contents: vec![identity.clone()],
        },
    );
    let binding = KeyBinding::new(
        UID::new(binding_id),
        Versioned::new(UID::new(binding_state_id), 1),
    );
    let binding_state = KeyBindingStateV1::new(
        1,
        identity,
        MoveOption::from_option(None::<Vec<u8>>),
        1,
        MoveOption::from_option(Some(active_kid)),
        MoveTable::new(key_table_id, 1),
    );
    let key_record = KeyRecord::new(0, vec![9; 32], 0, MoveOption::from_option(None::<u64>));
    let field_ref = sui_mocks::mock_sui_object_ref();
    let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
    let mut state_service = sui_mocks::grpc::MockStateService::new();
    sui_mocks::grpc::mock_get_object_value_bcs_for(
        &mut ledger_service,
        nexus_objects.network_auth.clone(),
        sui::types::Owner::Shared(1),
        &network_auth,
        crate::move_bindings::struct_tag::<NetworkAuth>(&nexus_objects),
    );
    sui_mocks::grpc::mock_versioned_payload(
        &mut ledger_service,
        network_auth_state_id,
        1,
        network_auth_state,
    );
    sui_mocks::grpc::mock_get_object_value_bcs_for(
        &mut ledger_service,
        sui_mocks::object_ref_for_id(binding_id),
        sui::types::Owner::Shared(1),
        &binding,
        crate::move_bindings::struct_tag::<KeyBinding>(&nexus_objects),
    );
    sui_mocks::grpc::mock_versioned_payload(
        &mut ledger_service,
        binding_state_id,
        1,
        binding_state,
    );
    sui_mocks::grpc::mock_list_dynamic_fields(
        &mut state_service,
        vec![(active_kid, *field_ref.object_id())],
    );
    sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
        &mut ledger_service,
        vec![(
            field_ref,
            sui::types::Owner::Shared(1),
            active_kid,
            key_record,
        )],
    );
    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger_service),
        state_service_mock: Some(state_service),
        ..Default::default()
    });

    NetworkAuthReader::from_rpc_url(
        &rpc_url,
        nexus_objects.registry_type_origin_pkg_id(),
        network_auth_id,
    )
    .expect("mock network-auth reader")
}
