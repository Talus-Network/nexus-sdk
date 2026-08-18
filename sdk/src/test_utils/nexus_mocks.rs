use crate::{
    move_bindings::{
        interface::{
            agent::{SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
            payment::SkillPaymentPolicy,
            version::InterfaceVersion,
        },
        move_std::option::Option as MoveOption,
        registry::{
            agent_registry::{AgentRecord, AgentRegistry, AgentRegistryStateV1, SkillRecord},
            network_auth::{
                IdentityKey,
                KeyBinding,
                KeyBindingStateV1,
                KeyRecord,
                NetworkAuth,
                NetworkAuthStateV1,
            },
        },
        sui_framework::{
            object::{ID, UID},
            table::Table as MoveTable,
            vec_set::VecSet,
            versioned::Versioned,
        },
    },
    nexus::client::NexusClient,
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

/// Create a coin-free client whose configured Agent registry contains one active skill.
pub async fn mock_agent_skill_client_without_coins(
    agent_id: sui::types::Address,
    skill_id: u64,
    dag_binding: SkillDagBinding,
) -> NexusClient {
    let nexus_objects = sui_mocks::mock_nexus_objects();
    let registry_id = *nexus_objects.agent_registry.object_id();
    let registry_state_id = sui::types::Address::from_static("0x9001");
    let agents_table_id = sui::types::Address::from_static("0x9002");
    let skills_table_id = sui::types::Address::from_static("0x9003");
    let agent_key = ID::new(agent_id);
    let agent_field_id = agents_table_id.derive_dynamic_child_id(
        &<ID as sui_move::MoveType>::type_tag_static(),
        &bcs::to_bytes(&agent_key).expect("Agent key serializes"),
    );
    let agent_field_ref = sui_mocks::object_ref_for_id(agent_field_id);
    let skill_field_id = skills_table_id.derive_dynamic_child_id(
        &sui::types::TypeTag::U64,
        &bcs::to_bytes(&skill_id).expect("Skill key serializes"),
    );
    let skill_field_ref = sui_mocks::object_ref_for_id(skill_field_id);
    let registry = AgentRegistry::new(
        UID::new(registry_id),
        Versioned::new(UID::new(registry_state_id), 1),
    );
    let registry_state = AgentRegistryStateV1::new(
        ID::new(sui::types::Address::ZERO),
        1,
        MoveTable::new(agents_table_id, 1),
    );
    let agent_record = AgentRecord {
        active: true,
        skills: MoveTable::new(skills_table_id, 1),
    };
    let skill_record = SkillRecord {
        description: vec![],
        active: true,
        dag_binding,
        requirements: SkillRequirement {
            input_commitment: vec![2],
            payment_policy: SkillPaymentPolicy::default(),
            schedule_policy: SkillSchedulePolicy::default(),
            fixed_tools: vec![],
        },
        current_interface_revision: InterfaceVersion::new(1),
        scheduled_task_count: 0,
    };
    let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
    sui_mocks::grpc::mock_get_object_value_bcs_for(
        &mut ledger_service,
        nexus_objects.agent_registry.clone(),
        sui::types::Owner::Shared(1),
        &registry,
        crate::move_bindings::struct_tag::<AgentRegistry>(&nexus_objects),
    );
    sui_mocks::grpc::mock_versioned_payload(
        &mut ledger_service,
        registry_state_id,
        1,
        registry_state,
    );
    sui_mocks::grpc::mock_get_dynamic_table_value_bcs(
        &mut ledger_service,
        agent_field_ref,
        sui::types::Owner::Shared(1),
        agent_key,
        agent_record,
    );
    sui_mocks::grpc::mock_get_dynamic_table_value_bcs(
        &mut ledger_service,
        skill_field_ref,
        sui::types::Owner::Shared(1),
        skill_id,
        skill_record,
    );
    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger_service),
        ..Default::default()
    });

    mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await
}

/// Create a read only Nexus client with one mocked allowed leader.
pub async fn mock_network_auth_client_without_wallet() -> NexusClient {
    let nexus_objects = sui_mocks::mock_nexus_objects();
    let network_auth_id = *nexus_objects.network_auth.object_id();
    let leader_id = sui::types::Address::from_static("0x71");
    let identity = IdentityKey::leader(leader_id);
    let derivation_client = NexusClient::builder()
        .with_rpc_url("http://127.0.0.1:1")
        .with_nexus_objects(nexus_objects.clone())
        .build()
        .await
        .expect("derivation client");
    let binding_id = derivation_client
        .network_auth()
        .binding_object_id(&identity)
        .await
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
        UID::new(sui::types::Address::from_static("0x75")),
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
    let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
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
    sui_mocks::grpc::mock_get_dynamic_field_by_key(
        &mut ledger_service,
        key_table_id,
        &sui::types::TypeTag::U64,
        active_kid,
        key_record,
    );
    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger_service),
        ..Default::default()
    });

    NexusClient::builder()
        .with_rpc_url(&rpc_url)
        .with_nexus_objects(nexus_objects)
        .build()
        .await
        .expect("mock read only Nexus client")
}
