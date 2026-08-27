use crate::{
    move_bindings::{
        interface::{
            agent::{SkillDagBinding, SkillRequirement, SkillSchedulePolicy},
            payment::SkillPaymentPolicy,
            version::InterfaceVersion,
        },
        move_std::option::Option as MoveOption,
        registry::{
            agent_registry::{AgentRecord, AgentRegistry, AgentRegistryInnerV1, SkillRecord},
            era::V1 as RegistryWitnessV1,
            leader::{LeaderRegistry, LeaderRegistryInnerV1},
            network_auth::{
                IdentityKey,
                KeyBinding,
                KeyBindingInnerV1,
                KeyRecord,
                NetworkAuth,
                NetworkAuthInnerV1,
            },
        },
        sui_framework::{
            object::{ID, UID},
            table::Table as MoveTable,
            vec_set::VecSet,
        },
        tool::{
            era::V1 as ToolWitnessV1,
            tool_registry::{ToolRegistry, ToolRegistryInnerV1},
        },
    },
    nexus::client::NexusClient,
    sui,
    test_utils::sui_mocks,
    types::{NexusObjects, PackageRole},
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
    let registry_id = nexus_objects.agent_registry.object_id();
    let agents_table_id = sui::types::Address::from_static("0x9002");
    let skills_table_id = sui::types::Address::from_static("0x9003");
    let agent_key = ID::new(agent_id);
    let agent_field_id = agents_table_id.derive_dynamic_child_id(
        &<ID as talus_sui_move::MoveType>::type_tag_static(),
        &bcs::to_bytes(&agent_key).expect("Agent key serializes"),
    );
    let agent_field_ref = sui_mocks::object_ref_for_id(agent_field_id);
    let skill_field_id = skills_table_id.derive_dynamic_child_id(
        &sui::types::TypeTag::U64,
        &bcs::to_bytes(&skill_id).expect("Skill key serializes"),
    );
    let skill_field_ref = sui_mocks::object_ref_for_id(skill_field_id);
    let registry = AgentRegistry::new(UID::new(registry_id));
    let registry_state = AgentRegistryInnerV1::new(MoveTable::new(agents_table_id, 1));
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
    };
    let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
    let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
    let mut state_service = sui_mocks::grpc::MockStateService::new();
    let context = sui_mocks::mock_nexus_context_for(&nexus_objects);
    sui_mocks::grpc::mock_object_state::<AgentRegistry, RegistryWitnessV1, AgentRegistryInnerV1>(
        &mut ledger_service,
        &mut state_service,
        &context,
        sui_mocks::object_ref_for_id(registry_id),
        sui::types::Owner::Shared(nexus_objects.agent_registry.initial_shared_version),
        registry,
        registry_state,
    );
    sui_mocks::grpc::mock_get_dynamic_table_value_bcs(
        &mut ledger_service,
        agent_field_ref,
        sui::types::Owner::Object(agents_table_id),
        agent_key,
        agent_record,
    );
    sui_mocks::grpc::mock_get_dynamic_table_value_bcs(
        &mut ledger_service,
        skill_field_ref,
        sui::types::Owner::Object(skills_table_id),
        skill_id,
        skill_record,
    );

    let leader_registry_id = nexus_objects.leader_registry.object_id();
    sui_mocks::grpc::mock_object_state_observation::<
        LeaderRegistry,
        RegistryWitnessV1,
        LeaderRegistryInnerV1,
    >(
        &mut ledger_service,
        &mut state_service,
        &context,
        sui_mocks::object_ref_for_id(leader_registry_id),
        sui::types::Owner::Shared(nexus_objects.leader_registry.initial_shared_version),
        LeaderRegistry::new(UID::new(leader_registry_id)),
    );
    let tool_registry_id = nexus_objects.tool_registry.object_id();
    sui_mocks::grpc::mock_object_state_observation::<
        ToolRegistry,
        ToolWitnessV1,
        ToolRegistryInnerV1,
    >(
        &mut ledger_service,
        &mut state_service,
        &context,
        sui_mocks::object_ref_for_id(tool_registry_id),
        sui::types::Owner::Shared(nexus_objects.tool_registry.initial_shared_version),
        ToolRegistry::new(UID::new(tool_registry_id)),
    );
    sui_mocks::grpc::mock_nexus_package_graph(
        &mut ledger_service,
        &mut package_service,
        context.packages(),
    );
    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger_service),
        package_service_mock: Some(package_service),
        state_service_mock: Some(state_service),
        ..Default::default()
    });

    mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await
}

/// Create a read only Nexus client with one mocked allowed leader.
pub async fn mock_network_auth_client_without_wallet() -> NexusClient {
    let nexus_objects = sui_mocks::mock_nexus_objects();
    let network_auth_id = nexus_objects.network_auth.object_id();
    let leader_id = sui::types::Address::from_static("0x71");
    let identity = IdentityKey::leader(leader_id);
    let context = sui_mocks::mock_nexus_context_for(&nexus_objects);
    let binding_id = crate::move_bindings::derive_network_auth_binding_id(
        context
            .type_origin(PackageRole::Registry, "network_auth", "NetworkAuth")
            .expect("test context contains NetworkAuth"),
        network_auth_id,
        &identity,
    )
    .expect("leader binding id");
    let key_table_id = sui::types::Address::from_static("0x72");
    let active_kid = 0;
    let network_auth = NetworkAuth::new(UID::new(network_auth_id));
    let network_auth_state = NetworkAuthInnerV1::new(
        UID::new(sui::types::Address::from_static("0x75")),
        VecSet {
            contents: vec![identity.clone()],
        },
    );
    let binding = KeyBinding::new(UID::new(binding_id));
    let binding_state = KeyBindingInnerV1::new(
        identity,
        MoveOption::from_option(None::<Vec<u8>>),
        1,
        MoveOption::from_option(Some(active_kid)),
        MoveTable::new(key_table_id, 1),
    );
    let key_record = KeyRecord::new(0, vec![9; 32], 0, MoveOption::from_option(None::<u64>));
    let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
    let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
    let mut state_service = sui_mocks::grpc::MockStateService::new();
    sui_mocks::grpc::mock_object_state::<NetworkAuth, RegistryWitnessV1, NetworkAuthInnerV1>(
        &mut ledger_service,
        &mut state_service,
        &context,
        sui_mocks::object_ref_for_id(network_auth_id),
        sui::types::Owner::Shared(nexus_objects.network_auth.initial_shared_version),
        network_auth,
        network_auth_state,
    );
    sui_mocks::grpc::mock_object_state::<KeyBinding, RegistryWitnessV1, KeyBindingInnerV1>(
        &mut ledger_service,
        &mut state_service,
        &context,
        sui_mocks::object_ref_for_id(binding_id),
        sui::types::Owner::Shared(1),
        binding,
        binding_state,
    );
    sui_mocks::grpc::mock_get_dynamic_field_by_key(
        &mut ledger_service,
        key_table_id,
        &sui::types::TypeTag::U64,
        active_kid,
        key_record,
    );
    sui_mocks::grpc::mock_nexus_package_graph(
        &mut ledger_service,
        &mut package_service,
        context.packages(),
    );
    let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
        ledger_service_mock: Some(ledger_service),
        package_service_mock: Some(package_service),
        state_service_mock: Some(state_service),
        ..Default::default()
    });

    mock_nexus_client_without_coins(&nexus_objects, &rpc_url).await
}
