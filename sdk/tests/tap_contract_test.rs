#![cfg(feature = "full")]

use {
    nexus_sdk::{
        events::RequestWalkExecutionEvent,
        idents::{
            primitives,
            pure_arg,
            tap::{self, TapStandard},
        },
        nexus::tap as nexus_tap,
        sui,
        transactions::tap as tap_tx,
        types::{
            resolve_active_tap_endpoint,
            AgentId,
            InterfaceRevision,
            NexusObjects,
            RuntimeVertex,
            SkillId,
            TapAgentRecord,
            TapConfigDigestInput,
            TapEndpointActivation,
            TapEndpointKey,
            TapEndpointRecord,
            TapEndpointResolutionError,
            TapEndpointRevision,
            TapPaymentMode,
            TapPaymentPolicy,
            TapRegistry,
            TapSchedulePolicy,
            TapSharedObjectRef,
            TapSkillConfig,
            TapSkillRecord,
            TapSkillRequirements,
            TapVertexAuthorizationSchema,
            TypeName,
        },
    },
    std::{path::PathBuf, str::FromStr},
};

fn addr(value: &str) -> sui::types::Address {
    sui::types::Address::from_str(value).expect("valid address")
}

fn object_ref(id: &str, version: u64, digest_byte: u8) -> sui::types::ObjectReference {
    sui::types::ObjectReference::new(
        addr(id),
        version,
        sui::types::Digest::from([digest_byte; 32]),
    )
}

fn nexus_objects() -> NexusObjects {
    NexusObjects {
        workflow_pkg_id: addr("0x1"),
        primitives_pkg_id: addr("0x2"),
        interface_pkg_id: addr("0x3"),
        network_id: addr("0x4"),
        registry_pkg_id: addr("0x5"),
        tool_registry: object_ref("0x6", 1, 6),
        verifier_registry: object_ref("0x7", 1, 7),
        network_auth: object_ref("0x8", 1, 8),
        default_tap: object_ref("0x9", 1, 9),
        tap_registry: Some(object_ref("0xc", 1, 12)),
        gas_service: object_ref("0xd", 1, 13),
        leader_registry: object_ref("0xe", 1, 14),
        workflow_original_pkg_id: None,
        registry_original_pkg_id: None,
    }
}

fn requirements() -> TapSkillRequirements {
    TapSkillRequirements {
        input_schema_hash: vec![1],
        workflow_hash: vec![2],
        metadata_hash: vec![3],
        payment_policy: TapPaymentPolicy {
            mode: TapPaymentMode::UserFunded,
            max_budget: 100,
            token_type_hash: Vec::new(),
            auth_mode: 7,
            refund_mode: 0,
        },
        schedule_policy: TapSchedulePolicy::default(),
        vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
    }
}

fn endpoint(revision: u64, active: bool) -> TapEndpointRecord {
    TapEndpointRecord {
        key: TapEndpointKey {
            agent_id: AgentId(addr("0xa1")),
            skill_id: SkillId(addr("0xb1")),
            interface_revision: InterfaceRevision(revision),
        },
        package_id: addr("0xc1"),
        endpoint_object: object_ref("0xd1", revision, revision as u8),
        shared_objects: vec![TapSharedObjectRef::immutable(addr("0xe1"), 9)],
        config_digest: vec![8],
        requirements: requirements(),
        active_for_new_executions: active,
    }
}

fn endpoint_revision(revision: u64, active: bool) -> TapEndpointRevision {
    let record = endpoint(revision, active);

    TapEndpointRevision {
        agent_id: record.key.agent_id,
        skill_id: record.key.skill_id,
        interface_revision: record.key.interface_revision,
        package_id: record.package_id,
        endpoint_object_id: *record.endpoint_object.object_id(),
        endpoint_object_version: record.endpoint_object.version(),
        endpoint_object_digest: record.endpoint_object.digest().inner().to_vec(),
        shared_objects: record.shared_objects,
        requirements: record.requirements,
        config_digest: record.config_digest,
        active_for_new_executions: active,
    }
}

fn registry_with_active_revision(active_revision: u64) -> TapRegistry {
    let agent_id = AgentId(addr("0xa1"));
    let skill_id = SkillId(addr("0xb1"));
    let requirements = requirements();

    TapRegistry {
        id: addr("0x91"),
        agents: vec![TapAgentRecord {
            agent_id,
            owner: addr("0x92"),
            operator: addr("0x93"),
            metadata_hash: vec![1],
            auth_mode: 0,
            active: true,
        }],
        skills: vec![TapSkillRecord {
            agent_id,
            skill_id,
            dag_id: addr("0x94"),
            tap_package_id: addr("0xc1"),
            workflow_hash: requirements.workflow_hash.clone(),
            requirements_hash: requirements.input_schema_hash.clone(),
            metadata_hash: requirements.metadata_hash.clone(),
            payment_policy: requirements.payment_policy.clone(),
            schedule_policy: requirements.schedule_policy.clone(),
            capability_schema_hash: vec![5],
            active: true,
        }],
        endpoints: vec![endpoint_revision(1, true), endpoint_revision(2, false)],
        active_endpoints: vec![TapEndpointActivation {
            agent_id,
            skill_id,
            interface_revision: InterfaceRevision(active_revision),
        }],
    }
}

fn finish_transaction(mut tx: sui::tx::TransactionBuilder) -> sui::types::Transaction {
    let gas = object_ref("0xf1", 1, 9);

    tx.set_sender(addr("0xf2"));
    tx.set_gas_budget(1000);
    tx.set_gas_price(1000);
    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas.object_id(),
        gas.version(),
        *gas.digest(),
    )]);

    tx.finish().expect("transaction builds")
}

fn move_call(tx: &sui::types::Transaction, index: usize) -> &sui::types::MoveCall {
    let sui::types::TransactionKind::ProgrammableTransaction(sui::types::ProgrammableTransaction {
        commands,
        ..
    }) = &tx.kind
    else {
        panic!("expected programmable transaction");
    };

    match commands.get(index) {
        Some(sui::types::Command::MoveCall(call)) => call,
        other => panic!("expected move call at {index}, got {other:?}"),
    }
}

fn wrap_event(objects: &NexusObjects, inner: sui::types::StructTag) -> sui::types::Event {
    sui::types::Event {
        package_id: objects.primitives_pkg_id,
        module: primitives::Event::EVENT_WRAPPER.module,
        sender: addr("0xf3"),
        type_: sui::types::StructTag::new(
            objects.primitives_pkg_id,
            primitives::Event::EVENT_WRAPPER.module,
            primitives::Event::EVENT_WRAPPER.name,
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        ),
        contents: vec![],
    }
}

#[test]
fn active_endpoint_resolution_requires_exactly_one_active_revision() {
    let records = vec![endpoint(0, false), endpoint(1, true)];
    let resolved =
        resolve_active_tap_endpoint(&records, AgentId(addr("0xa1")), SkillId(addr("0xb1")))
            .expect("one active endpoint");

    assert_eq!(resolved.key.interface_revision, InterfaceRevision(1));

    let duplicate = vec![endpoint(1, true), endpoint(2, true)];
    assert!(matches!(
        resolve_active_tap_endpoint(&duplicate, AgentId(addr("0xa1")), SkillId(addr("0xb1"))),
        Err(TapEndpointResolutionError::DuplicateActiveRevision { count: 2, .. })
    ));
}

#[test]
fn registry_recovery_uses_tap_registry_activation_layout() {
    let registry = registry_with_active_revision(2);
    let bytes = bcs::to_bytes(&registry).expect("registry BCS");
    let registry: TapRegistry = bcs::from_bytes(&bytes).expect("registry layout decodes");
    let active = registry
        .active_endpoint_record(AgentId(addr("0xa1")), SkillId(addr("0xb1")))
        .expect("active registry endpoint");

    assert_eq!(active.key.interface_revision, InterfaceRevision(2));
    assert!(active.active_for_new_executions);

    let records = registry
        .endpoint_records()
        .expect("registry endpoint records");
    assert_eq!(records.len(), 2);
    assert!(!records[0].active_for_new_executions);
    assert!(records[1].active_for_new_executions);

    let pinned = registry
        .endpoint_record(TapEndpointKey {
            agent_id: AgentId(addr("0xa1")),
            skill_id: SkillId(addr("0xb1")),
            interface_revision: InterfaceRevision(1),
        })
        .expect("pinned endpoint");
    assert!(!pinned.active_for_new_executions);
}

#[test]
fn nexus_objects_carries_tap_registry_metadata() {
    let objects = nexus_objects();
    assert_eq!(
        objects.tap_registry().map(|registry| *registry.object_id()),
        Some(addr("0xc"))
    );
}

#[test]
fn configured_registry_recovery_requires_tap_registry_metadata() {
    let mut objects = nexus_objects();
    objects.tap_registry = None;

    let error = nexus_tap::configured_tap_registry_id(&objects)
        .expect_err("configured recovery should reject missing registry metadata");

    assert!(error
        .to_string()
        .contains("NexusObjects missing tap_registry object reference"));
}

fn request_walk_event() -> RequestWalkExecutionEvent {
    RequestWalkExecutionEvent {
        dag: addr("0x51"),
        execution: addr("0x52"),
        invoker: addr("0x53"),
        walk_index: 0,
        next_vertex: RuntimeVertex::plain("entry"),
        evaluations: addr("0x54"),
        worksheet_from_type: TypeName::new("0x2::legacy::Witness"),
        worksheet_from_uid: addr("0x55"),
        tap_agent_id: None,
        tap_skill_id: None,
        tap_interface_revision: None,
        tap_endpoint_object_id: None,
        tap_payment_id: None,
        tap_authorization_plan_hash: None,
    }
}

#[test]
fn request_walk_standard_tap_context_is_all_or_none() {
    let legacy = request_walk_event();
    assert!(legacy.standard_tap_context().unwrap().is_none());

    let complete = RequestWalkExecutionEvent {
        tap_agent_id: Some(AgentId(addr("0xa1"))),
        tap_skill_id: Some(SkillId(addr("0xb1"))),
        tap_interface_revision: Some(InterfaceRevision(7)),
        tap_endpoint_object_id: Some(addr("0xc1")),
        tap_payment_id: Some(addr("0xd1")),
        tap_authorization_plan_hash: Some(vec![1, 2, 3]),
        ..legacy.clone()
    };
    let context = complete
        .standard_tap_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, AgentId(addr("0xa1")));
    assert_eq!(context.skill_id, SkillId(addr("0xb1")));
    assert_eq!(context.interface_revision, InterfaceRevision(7));
    assert_eq!(context.endpoint_object_id, addr("0xc1"));
    assert_eq!(context.payment_id, addr("0xd1"));
    assert_eq!(context.authorization_plan_hash, Some(vec![1, 2, 3]));

    let partial = RequestWalkExecutionEvent {
        tap_agent_id: Some(AgentId(addr("0xa1"))),
        ..legacy
    };
    let error = partial
        .standard_tap_context()
        .expect_err("partial context should fail closed");
    assert!(error.to_string().contains("missing tap_skill_id"));
}

#[test]
fn request_walk_standard_tap_context_deserializes_move_option_fields() {
    let event: RequestWalkExecutionEvent = serde_json::from_value(serde_json::json!({
        "dag": "0x51",
        "execution": "0x52",
        "invoker": "0x53",
        "walk_index": 0,
        "next_vertex": { "Plain": { "vertex": { "name": "entry" } } },
        "evaluations": "0x54",
        "worksheet_from_type": { "name": "0x2::legacy::Witness" },
        "worksheet_from_uid": "0x55",
        "tap_agent_id": { "fields": { "vec": [{ "fields": { "value": "0xa1" } }] } },
        "tap_skill_id": { "fields": { "vec": [{ "fields": { "value": "0xb1" } }] } },
        "tap_interface_revision": { "fields": { "vec": [{ "fields": { "value": "7" } }] } },
        "tap_endpoint_object_id": { "vec": ["0xc1"] },
        "tap_payment_id": { "vec": ["0xd1"] },
        "tap_authorization_plan_hash": { "vec": [[1, 2, 3]] },
    }))
    .expect("event should deserialize");

    let context = event
        .standard_tap_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, AgentId(addr("0xa1")));
    assert_eq!(context.skill_id, SkillId(addr("0xb1")));
    assert_eq!(context.interface_revision, InterfaceRevision(7));
    assert_eq!(context.endpoint_object_id, addr("0xc1"));
    assert_eq!(context.payment_id, addr("0xd1"));
    assert_eq!(context.authorization_plan_hash, Some(vec![1, 2, 3]));
}

#[test]
fn config_digest_and_publish_artifact_are_deterministic() {
    let config = TapSkillConfig {
        name: "weather".to_string(),
        tap_package_name: "weather_tap".to_string(),
        dag_path: PathBuf::from("dag.json"),
        tap_package_path: PathBuf::from("tap"),
        requirements: requirements(),
        shared_objects: vec![TapSharedObjectRef::mutable(addr("0x21"), 5)],
        interface_revision: InterfaceRevision(3),
        active_for_new_executions: true,
    };

    let input = TapConfigDigestInput {
        package_id: addr("0x22"),
        endpoint_object_id: Some(addr("0x23")),
        interface_revision: config.interface_revision,
        shared_objects: config.shared_objects.clone(),
        requirements: config.requirements.clone(),
    };

    assert_eq!(input.digest().unwrap(), input.digest().unwrap());

    let artifact =
        nexus_sdk::types::TapPublishArtifact::from_config(&config, addr("0x24"), addr("0x25"))
            .expect("valid artifact");
    assert_eq!(artifact.config_digest_hex.len(), 64);
    assert_eq!(artifact.dag_id, addr("0x24"));
    assert_eq!(artifact.tap_package_id, addr("0x25"));
}

#[test]
fn standard_tap_events_are_nexus_events() {
    let objects = nexus_objects();
    let endpoint_event = wrap_event(
        &objects,
        sui::types::StructTag::new(
            objects.interface_pkg_id,
            tap::STANDARD_TAP_MODULE,
            TapStandard::ENDPOINT_REVISION_ANNOUNCED_EVENT.name,
            vec![],
        ),
    );

    assert!(objects.is_event_from_nexus(&endpoint_event));

    let unrelated_interface_event = wrap_event(
        &objects,
        sui::types::StructTag::new(
            objects.interface_pkg_id,
            sui::types::Identifier::from_static("unrelated"),
            TapStandard::ENDPOINT_REVISION_ANNOUNCED_EVENT.name,
            vec![],
        ),
    );

    assert!(!objects.is_event_from_nexus(&unrelated_interface_event));
}

#[test]
fn transaction_builders_select_standard_tap_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");

    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        AgentId(addr("0xa1")),
        addr("0xd1"),
        addr("0xe1"),
        vec![1],
        vec![2],
        vec![3],
        requirements().payment_policy,
        requirements().schedule_policy,
        vec![4],
        addr("0xf1"),
        1,
        vec![5],
        vec![TapSharedObjectRef::immutable(addr("0x31"), 9)],
        vec![6],
        true,
    )
    .expect("register skill builder");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    tap_tx::worksheet(
        &mut tx,
        &objects,
        registry,
        AgentId(addr("0xa1")),
        SkillId(addr("0xb1")),
        addr("0x41"),
    )
    .expect("worksheet builder");

    let payment = tx.input(pure_arg(&3_u64).unwrap());
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    tap_tx::execute_agent_skill(
        &mut tx,
        &objects,
        registry,
        AgentId(addr("0xa1")),
        SkillId(addr("0xb1")),
        vec![9],
        payment,
        None,
    )
    .expect("execute builder");

    let tx = finish_transaction(tx);
    assert_eq!(move_call(&tx, 0).function, TapStandard::REGISTER_SKILL.name);
    assert_eq!(move_call(&tx, 1).function, TapStandard::WORKSHEET.name);
    assert_eq!(
        move_call(&tx, 2).function,
        TapStandard::EXECUTE_AGENT_SKILL.name
    );
}

#[test]
fn transaction_builders_select_standard_runtime_worksheet_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");

    let worksheet = tap_tx::workflow_worksheet(
        &mut tx,
        &objects,
        registry,
        AgentId(addr("0xa1")),
        SkillId(addr("0xb1")),
    )
    .expect("workflow worksheet builder");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    tap_tx::confirm_tool_eval_for_walk(&mut tx, &objects, registry, worksheet);

    let tx = finish_transaction(tx);
    assert_eq!(
        move_call(&tx, 0).function,
        TapStandard::WORKFLOW_WORKSHEET.name
    );
    assert_eq!(
        move_call(&tx, 1).function,
        TapStandard::CONFIRM_TOOL_EVAL_FOR_WALK.name
    );
}

#[test]
fn dag_transaction_helpers_select_standard_runtime_stamp_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let leader_registry = tx.input(pure_arg(&1_u64).unwrap());
    let execution = tx.input(pure_arg(&2_u64).unwrap());
    let worksheet = tx.input(pure_arg(&3_u64).unwrap());
    let leader_cap = tx.input(pure_arg(&4_u64).unwrap());

    nexus_sdk::transactions::dag::leader_stamp_standard_tap_worksheet(
        &mut tx,
        &objects,
        leader_registry,
        execution,
        worksheet,
        leader_cap,
    );

    let execution = tx.input(pure_arg(&5_u64).unwrap());
    let worksheet = tx.input(pure_arg(&6_u64).unwrap());
    let leader_cap = tx.input(pure_arg(&7_u64).unwrap());
    nexus_sdk::transactions::dag::pre_stamp_standard_tap_execution(
        &mut tx, &objects, execution, worksheet, leader_cap,
    );

    let tx = finish_transaction(tx);
    assert_eq!(
        move_call(&tx, 0).function,
        nexus_sdk::idents::workflow::Dag::LEADER_STAMP_STANDARD_TAP_WORKSHEET.name
    );
    assert_eq!(
        move_call(&tx, 1).function,
        nexus_sdk::idents::workflow::Dag::PRE_STAMP_STANDARD_TAP_EXECUTION.name
    );
}
