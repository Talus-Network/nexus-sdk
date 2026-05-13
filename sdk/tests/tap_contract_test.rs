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
            InterfaceRevision,
            MoveTable,
            NexusObjects,
            RuntimeVertex,
            TapAgentRecord,
            TapConfigDigestInput,
            TapDagBinding,
            TapDefaultExecutionTarget,
            TapEndpointActivation,
            TapEndpointKey,
            TapEndpointRecord,
            TapEndpointResolutionError,
            TapEndpointRevision,
            TapPaymentMode,
            TapPaymentPolicy,
            TapPaymentSource,
            TapPaymentSourceKind,
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
    serde_json::json,
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
        tap_registry: Some(object_ref("0xc", 1, 12)),
        default_tap_target: Some(TapDefaultExecutionTarget {
            agent_id: addr("0xa1"),
            skill_id: 177,
        }),
        gas_service: object_ref("0xd", 1, 13),
        leader_registry: object_ref("0xe", 1, 14),
        workflow_original_pkg_id: None,
        registry_original_pkg_id: None,
    }
}

fn requirements() -> TapSkillRequirements {
    TapSkillRequirements {
        input_schema_commitment: vec![1],
        workflow_commitment: vec![2],
        metadata_commitment: vec![3],
        payment_policy: TapPaymentPolicy {
            mode: TapPaymentMode::UserFunded,
            max_budget: 100,
            token_type_commitment: Vec::new(),
            refund_mode: 0,
        },
        schedule_policy: TapSchedulePolicy::default(),
        vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
    }
}

fn demo_tap_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move/demo_tap")
}

fn endpoint(revision: u64, active: bool) -> TapEndpointRecord {
    TapEndpointRecord {
        key: TapEndpointKey {
            agent_id: addr("0xa1"),
            skill_id: 177,
            interface_revision: InterfaceRevision(revision),
        },
        package_id: addr("0xc1"),
        endpoint_object: object_ref("0xd1", revision, revision as u8),
        shared_objects: vec![TapSharedObjectRef::immutable(addr("0xe1"))],
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
    let agent_id = addr("0xa1");
    let skill_id = 177;
    let requirements = requirements();

    TapRegistry {
        id: addr("0x91"),
        agents: vec![TapAgentRecord {
            agent_id,
            owner: addr("0x92"),
            operator: addr("0x93"),
            active: true,
            next_skill_index: 1,
            skills: MoveTable::new(addr("0x95"), 1),
            endpoints: MoveTable::new(addr("0x96"), 1),
            active_endpoints: vec![TapEndpointActivation {
                agent_id,
                skill_id,
                interface_revision: InterfaceRevision(active_revision),
            }],
        }],
        skills: vec![TapSkillRecord {
            agent_id,
            skill_id,
            dag_id: addr("0x94"),
            dag_binding: TapDagBinding::pinned(addr("0x94")),
            tap_package_id: addr("0xc1"),
            workflow_commitment: requirements.workflow_commitment.clone(),
            requirements_commitment: requirements.input_schema_commitment.clone(),
            metadata_commitment: requirements.metadata_commitment.clone(),
            payment_policy: requirements.payment_policy.clone(),
            schedule_policy: requirements.schedule_policy.clone(),
            capability_schema_commitment: vec![5],
            active: true,
        }],
        endpoints: vec![endpoint_revision(1, true), endpoint_revision(2, false)],
        active_endpoints: vec![TapEndpointActivation {
            agent_id,
            skill_id,
            interface_revision: InterfaceRevision(active_revision),
        }],
        default_target: Some(TapDefaultExecutionTarget { agent_id, skill_id }),
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

fn move_calls(tx: &sui::types::Transaction) -> Vec<&sui::types::MoveCall> {
    let sui::types::TransactionKind::ProgrammableTransaction(sui::types::ProgrammableTransaction {
        commands,
        ..
    }) = &tx.kind
    else {
        panic!("expected programmable transaction");
    };

    commands
        .iter()
        .filter_map(|command| match command {
            sui::types::Command::MoveCall(call) => Some(call),
            _ => None,
        })
        .collect()
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
        resolve_active_tap_endpoint(&records, addr("0xa1"), 177).expect("one active endpoint");

    assert_eq!(resolved.key.interface_revision, InterfaceRevision(1));

    let duplicate = vec![endpoint(1, true), endpoint(2, true)];
    assert!(matches!(
        resolve_active_tap_endpoint(&duplicate, addr("0xa1"), 177),
        Err(TapEndpointResolutionError::DuplicateActiveRevision { count: 2, .. })
    ));
}

#[test]
fn registry_recovery_uses_tap_registry_activation_layout() {
    let registry = registry_with_active_revision(2);
    let bytes = bcs::to_bytes(&registry).expect("registry BCS");
    let registry: TapRegistry = bcs::from_bytes(&bytes).expect("registry layout decodes");
    let active = registry
        .active_endpoint_record(addr("0xa1"), 177)
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
            agent_id: addr("0xa1"),
            skill_id: 177,
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
    assert_eq!(
        objects.default_tap_target(),
        Some(TapDefaultExecutionTarget {
            agent_id: addr("0xa1"),
            skill_id: 177,
        })
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

#[test]
fn configured_default_target_requires_metadata() {
    let mut objects = nexus_objects();
    objects.default_tap_target = None;

    let error = nexus_tap::configured_default_tap_target(&objects)
        .expect_err("configured default target should require metadata");

    assert!(error
        .to_string()
        .contains("NexusObjects missing default_tap_target metadata"));
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
        tap_selected_dag_id: None,
        tap_authorization_plan_commitment: None,
        tap_authorization_plan: Vec::new(),
        tap_scheduled_task_id: None,
        tap_scheduled_occurrence_index: None,
    }
}

#[test]
fn request_walk_standard_tap_context_is_all_or_none() {
    let legacy = request_walk_event();
    assert!(legacy.standard_tap_context().unwrap().is_none());

    let complete = RequestWalkExecutionEvent {
        tap_agent_id: Some(addr("0xa1")),
        tap_skill_id: Some(177),
        tap_interface_revision: Some(InterfaceRevision(7)),
        tap_endpoint_object_id: Some(addr("0xc1")),
        tap_payment_id: Some(addr("0xd1")),
        tap_selected_dag_id: Some(addr("0x51")),
        tap_authorization_plan_commitment: Some(vec![1, 2, 3]),
        ..legacy.clone()
    };
    let context = complete
        .standard_tap_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, addr("0xa1"));
    assert_eq!(context.skill_id, 177);
    assert_eq!(context.interface_revision, InterfaceRevision(7));
    assert_eq!(context.endpoint_object_id, addr("0xc1"));
    assert_eq!(context.payment_id, addr("0xd1"));
    assert_eq!(context.selected_dag_id, addr("0x51"));
    assert_eq!(context.authorization_plan_commitment, Some(vec![1, 2, 3]));
    assert!(context.authorization_plan.is_empty());

    let partial = RequestWalkExecutionEvent {
        tap_agent_id: Some(addr("0xa1")),
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
        "tap_skill_id": { "fields": { "vec": [{ "fields": { "value": "177" } }] } },
        "tap_interface_revision": { "fields": { "vec": [{ "fields": { "value": "7" } }] } },
        "tap_endpoint_object_id": { "vec": ["0xc1"] },
        "tap_payment_id": { "vec": ["0xd1"] },
        "tap_selected_dag_id": { "vec": ["0x51"] },
        "tap_authorization_plan_commitment": { "vec": [[1, 2, 3]] },
    }))
    .expect("event should deserialize");

    let context = event
        .standard_tap_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, addr("0xa1"));
    assert_eq!(context.skill_id, 177);
    assert_eq!(context.interface_revision, InterfaceRevision(7));
    assert_eq!(context.endpoint_object_id, addr("0xc1"));
    assert_eq!(context.payment_id, addr("0xd1"));
    assert_eq!(context.selected_dag_id, addr("0x51"));
    assert_eq!(context.authorization_plan_commitment, Some(vec![1, 2, 3]));
    assert!(context.authorization_plan.is_empty());
}

#[test]
fn request_walk_standard_tap_context_deserializes_authorization_plan() {
    let event: RequestWalkExecutionEvent = serde_json::from_value(serde_json::json!({
        "dag": "0x51",
        "execution": "0x52",
        "invoker": "0x53",
        "walk_index": 0,
        "next_vertex": { "Plain": { "vertex": { "name": "entry" } } },
        "evaluations": "0x54",
        "worksheet_from_type": { "name": "0x2::legacy::Witness" },
        "worksheet_from_uid": "0x55",
        "tap_agent_id": { "vec": ["0xa1"] },
        "tap_skill_id": { "vec": ["177"] },
        "tap_interface_revision": { "vec": [{ "value": "7" }] },
        "tap_endpoint_object_id": { "vec": ["0xc1"] },
        "tap_payment_id": { "vec": ["0xd1"] },
        "tap_selected_dag_id": { "vec": ["0x51"] },
        "tap_authorization_plan": [{
            "vertex": { "Plain": { "vertex": { "name": "entry" } } },
            "grant_id": "0xe1",
            "tool_package": "0xf1",
            "tool_module": [116, 111, 111, 108],
            "tool_function": [114, 117, 110],
            "operation_commitment": [1],
            "constraints_commitment": [2],
            "endpoint_revision": { "vec": [{ "value": "7" }] },
            "payment_id": { "vec": ["0xd1"] }
        }]
    }))
    .expect("event should deserialize");

    let context = event
        .standard_tap_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.authorization_plan.0.len(), 1);
    assert_eq!(context.authorization_plan.0[0].grant_id, addr("0xe1"));
    assert_eq!(context.authorization_plan.0[0].tool_module, "tool");
}

#[test]
fn config_digest_and_publish_artifact_are_deterministic() {
    let config = TapSkillConfig {
        name: "weather".to_string(),
        tap_package_name: "weather_tap".to_string(),
        dag_path: PathBuf::from("dag.json"),
        tap_package_path: PathBuf::from("tap"),
        requirements: requirements(),
        shared_objects: vec![TapSharedObjectRef::mutable(addr("0x21"))],
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
    assert_eq!(
        artifact
            .endpoint_config_digest_hex(addr("0x23"))
            .unwrap()
            .len(),
        64
    );
    assert_ne!(
        artifact.endpoint_config_digest(addr("0x23")).unwrap(),
        artifact.config_digest
    );
}

#[test]
fn registry_default_target_requires_runtime_selected_binding() {
    let mut registry = registry_with_active_revision(1);

    let error = nexus_sdk::types::resolve_default_tap_execution_target(&registry)
        .expect_err("pinned dag binding should not resolve as default target");
    assert!(error.to_string().contains("is not runtime-DAG selected"));

    registry.skills[0].dag_binding = TapDagBinding::RuntimeSelected;
    let resolved = nexus_sdk::types::resolve_default_tap_execution_target(&registry)
        .expect("runtime-selected binding resolves default target");

    assert_eq!(resolved.target.agent_id, addr("0xa1"));
    assert_eq!(resolved.target.skill_id, 177);
    assert_eq!(resolved.skill.dag_binding, TapDagBinding::RuntimeSelected);
}

#[test]
fn tap_execution_payment_model_matches_live_object_shape() {
    let payment: nexus_sdk::types::TapExecutionPayment = serde_json::from_value(json!({
        "id": "0xaa",
        "execution_id": "0xbb",
        "agent_id": "0xcc",
        "skill_id": "221",
        "interface_revision": { "value": "7" },
        "endpoint_object_id": "0xee",
        "payer": "0xff",
        "payment_mode": "user_funded",
        "source_kind": "agent_vault",
        "source_identity": "0xcc",
        "max_budget": "42",
        "locked_budget": "40",
        "consumed": "5",
        "refund_mode": 2,
        "payment_source_hash": [1, 2],
        "accomplished": true,
        "refunded": false
    }))
    .expect("payment object should deserialize");

    assert_eq!(payment.payment_id(), addr("0xaa"));
    assert_eq!(payment.execution_id, addr("0xbb"));
    assert_eq!(payment.endpoint_key().agent_id, addr("0xcc"));
    assert_eq!(payment.endpoint_key().skill_id, 221);
    assert_eq!(
        payment.endpoint_key().interface_revision,
        InterfaceRevision(7)
    );
    assert_eq!(payment.endpoint_object_id, addr("0xee"));
    assert_eq!(payment.payment_mode, TapPaymentMode::UserFunded);
    assert_eq!(payment.source_kind, Some(TapPaymentSourceKind::AgentVault));
    assert_eq!(payment.source_identity, Some(addr("0xcc")));
    assert_eq!(payment.max_budget, 42);
    assert_eq!(payment.locked_budget, 40);
    assert_eq!(payment.consumed, 5);
    assert_eq!(payment.payment_source_hash, vec![1, 2]);
    assert!(payment.accomplished);
    assert!(!payment.refunded);
}

#[test]
fn tap_payment_sources_validate_invoker_and_agent_vault_modes() {
    let agent_id = addr("0xa");
    let payer = addr("0x1");
    let invoker_source =
        nexus_sdk::types::tap_payment_source_for_invoker(payer).expect("invoker source");
    let agent_vault_source =
        nexus_sdk::types::tap_payment_source_for_agent_vault(agent_id).expect("agent vault source");

    let decoded_invoker =
        TapPaymentSource::from_bcs_bytes(&invoker_source).expect("typed invoker source decodes");
    let decoded_vault =
        TapPaymentSource::from_bcs_bytes(&agent_vault_source).expect("typed vault source decodes");
    assert_eq!(decoded_invoker.kind, TapPaymentSourceKind::Invoker);
    assert_eq!(decoded_invoker.identity, payer);
    assert_eq!(decoded_vault.kind, TapPaymentSourceKind::AgentVault);
    assert_eq!(decoded_vault.identity, agent_id);

    assert!(
        nexus_sdk::types::validate_standard_tap_payment_options(
            agent_id,
            &TapPaymentPolicy::default(),
            &invoker_source,
            0,
            0,
            payer,
        )
        .is_err(),
        "typed invoker sources are not accepted by the direct Move user-funded policy"
    );

    let payer_address_source =
        nexus_sdk::types::tap_payment_source_for_address(payer).expect("payer address source");
    nexus_sdk::types::validate_standard_tap_payment_options(
        agent_id,
        &TapPaymentPolicy::default(),
        &payer_address_source,
        0,
        0,
        payer,
    )
    .expect("user-funded payer address source validates");

    let agent_funded = TapPaymentPolicy {
        mode: TapPaymentMode::AgentFunded,
        ..TapPaymentPolicy::default()
    };
    assert!(
        nexus_sdk::types::validate_standard_tap_payment_options(
            agent_id,
            &agent_funded,
            &agent_vault_source,
            0,
            0,
            payer,
        )
        .is_err(),
        "typed agent-vault sources are not accepted by the direct Move agent-funded policy"
    );

    let agent_address_source =
        nexus_sdk::types::tap_payment_source_for_address(agent_id).expect("agent address source");
    nexus_sdk::types::validate_standard_tap_payment_options(
        agent_id,
        &agent_funded,
        &agent_address_source,
        0,
        0,
        payer,
    )
    .expect("agent-funded address source validates");
    assert!(nexus_sdk::types::validate_standard_tap_payment_options(
        agent_id,
        &agent_funded,
        &invoker_source,
        0,
        0,
        payer,
    )
    .is_err());
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

    tap_tx::bootstrap_default_runtime_dag_skill(
        &mut tx,
        &objects,
        registry,
        addr("0xa2"),
        addr("0xe2"),
        vec![9],
        vec![8],
        requirements().payment_policy.clone(),
        requirements().schedule_policy.clone(),
        vec![6],
        addr("0xf2"),
        1,
        vec![5],
        vec![TapSharedObjectRef::immutable(addr("0x32"))],
        vec![4],
        true,
    )
    .expect("bootstrap default builder");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    let endpoint = tap_tx::create_standard_endpoint(&mut tx, &objects, objects.interface_pkg_id)
        .expect("standard endpoint builder");
    tap_tx::share_standard_endpoint(&mut tx, &objects, endpoint);
    tap_tx::bootstrap_default_runtime_dag_skill_for_deployment(
        &mut tx,
        &objects,
        registry,
        addr("0xa3"),
        objects.interface_pkg_id,
        addr("0xf3"),
        1,
        vec![4; 32],
        vec![4],
    )
    .expect("deployment bootstrap builder");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, true));
    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        agent,
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
        vec![TapSharedObjectRef::immutable(addr("0x31"))],
        vec![6],
        true,
    )
    .expect("register skill builder");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, false));
    tap_tx::worksheet(&mut tx, &objects, registry, agent, 177, addr("0x41"))
        .expect("worksheet builder");

    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, true));
    let payment_id_arg = tx.input(pure_arg(&addr("0xc1")).unwrap());
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    tap_tx::execute_agent_skill(
        &mut tx,
        &objects,
        registry,
        agent,
        177,
        vec![9],
        payment_id_arg,
        addr("0x41"),
        None,
    )
    .expect("execute builder");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);

    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL.name));
    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::CREATE_STANDARD_ENDPOINT.name));
    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::SHARE_STANDARD_ENDPOINT.name));
    assert!(calls.iter().any(|call| {
        call.function
            == TapStandard::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT_WITH_PACKAGE.name
    }));
    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::REGISTER_SKILL.name));
    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::WORKSHEET.name));
    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::EXECUTE_AGENT_SKILL.name));
}

#[test]
fn demo_tap_publish_bind_and_execute_lifecycle_ptb() {
    let objects = nexus_objects();
    let agent_id = addr("0xa5");
    let skill_id = 181;
    let dag_id = addr("0xd5");
    let tap_package_id = addr("0xe5");
    let endpoint_object_id = addr("0xf5");
    let endpoint_object = object_ref("0xf5", 7, 8);
    let config = TapSkillConfig {
        name: "demo tap".to_string(),
        tap_package_name: "demo_tap".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        tap_package_path: demo_tap_package_path(),
        requirements: requirements(),
        shared_objects: vec![TapSharedObjectRef::immutable(addr("0x31"))],
        interface_revision: InterfaceRevision(1),
        active_for_new_executions: true,
    };
    assert!(config.tap_package_path.join("Move.toml").exists());
    let artifact =
        nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id, tap_package_id)
            .expect("publish artifact")
            .with_endpoint_object(endpoint_object.clone())
            .expect("endpoint-bound artifact");
    let config_digest = artifact
        .endpoint_config_digest(endpoint_object_id)
        .expect("endpoint-bound digest");

    let mut tx = sui::tx::TransactionBuilder::new();

    let endpoint = tap_tx::create_standard_endpoint(&mut tx, &objects, artifact.tap_package_id)
        .expect("create endpoint");
    tap_tx::share_standard_endpoint(&mut tx, &objects, endpoint);

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("registry");
    tap_tx::create_agent(&mut tx, &objects, registry, addr("0x91")).expect("create agent");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("registry");
    let agent_object = tx.input(sui::tx::Input::shared(agent_id, 1, true));
    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        agent_object,
        artifact.dag_id,
        artifact.tap_package_id,
        artifact.requirements.workflow_commitment.clone(),
        artifact.requirements.input_schema_commitment.clone(),
        artifact.requirements.metadata_commitment.clone(),
        artifact.requirements.payment_policy.clone(),
        artifact.requirements.schedule_policy.clone(),
        artifact
            .requirements
            .vertex_authorization_schema
            .schema_commitment
            .clone(),
        endpoint_object_id,
        artifact.endpoint_object_version.expect("endpoint version"),
        artifact
            .endpoint_object_digest
            .clone()
            .expect("endpoint digest"),
        artifact.shared_objects.clone(),
        config_digest,
        true,
    )
    .expect("register skill");

    let agent_object = tx.input(sui::tx::Input::shared(agent_id, 1, true));
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("registry");
    let payment_input =
        tap_tx::AgentSkillPaymentInput::agent_vault_source(agent_id, skill_id, 50, 0)
            .expect("agent-funded direct payment source");
    assert_eq!(
        payment_input.source,
        nexus_sdk::types::tap_payment_source_for_address(agent_id).expect("agent address source")
    );

    let payment_amount = tx.input(pure_arg(&50_u64).unwrap());
    let payment_coin = tx
        .split_coins(tx.gas(), vec![payment_amount])
        .nested(0)
        .expect("payment coin split result");
    let payment_id = tap_tx::create_agent_skill_payment(
        &mut tx,
        &objects,
        registry,
        agent_object,
        payment_coin,
        addr("0x99"),
        payment_input,
    )
    .expect("payment");

    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("registry");
    tap_tx::execute_agent_skill(
        &mut tx,
        &objects,
        registry,
        agent_object,
        skill_id,
        b"demo-input-commitment".to_vec(),
        payment_id,
        addr("0x99"),
        Some(b"demo-auth-plan".to_vec()),
    )
    .expect("execute skill");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);
    let find_call = |function: &sui::types::Identifier| {
        calls
            .iter()
            .position(|call| &call.function == function)
            .expect("expected lifecycle call")
    };

    let create_endpoint = find_call(&TapStandard::CREATE_STANDARD_ENDPOINT.name);
    let share_endpoint = find_call(&TapStandard::SHARE_STANDARD_ENDPOINT.name);
    let create_agent = find_call(&TapStandard::CREATE_AGENT.name);
    let register_skill = find_call(&TapStandard::REGISTER_SKILL.name);
    let payment = find_call(&TapStandard::CREATE_AGENT_SKILL_PAYMENT.name);
    let execute = find_call(&TapStandard::EXECUTE_AGENT_SKILL.name);

    assert!(create_endpoint < share_endpoint);
    assert!(share_endpoint < create_agent);
    assert!(create_agent < register_skill);
    assert!(register_skill < payment);
    assert!(payment < execute);
    assert!(calls
        .iter()
        .any(|call| call.function == TapStandard::SKILL_ID_FROM_U64.name));
}

#[test]
fn agent_payment_vault_builders_target_standard_tap_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("registry");
    let agent = tx.input(pure_arg(&1_u64).unwrap());
    let coin = tx.input(pure_arg(&2_u64).unwrap());

    tap_tx::deposit_agent_payment_vault(&mut tx, &objects, agent, coin);
    tap_tx::withdraw_agent_payment_vault(&mut tx, &objects, registry, agent, 25)
        .expect("withdraw vault");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);
    let deposit = calls
        .iter()
        .find(|call| call.function == TapStandard::DEPOSIT_AGENT_PAYMENT_VAULT.name)
        .expect("deposit vault call");
    let withdraw = calls
        .iter()
        .find(|call| call.function == TapStandard::WITHDRAW_AGENT_PAYMENT_VAULT.name)
        .expect("withdraw vault call");

    assert_eq!(deposit.package, objects.interface_pkg_id);
    assert_eq!(withdraw.package, objects.registry_pkg_id());
    assert_eq!(deposit.arguments, vec![agent, coin]);
    assert_eq!(withdraw.arguments.len(), 3);
}

#[test]
fn demo_tap_publish_artifact_resolves_registered_execution_target() {
    let agent_id = addr("0xa5");
    let skill_id = 181;
    let dag_id = addr("0xd5");
    let tap_package_id = addr("0xe5");
    let endpoint_object = object_ref("0xf5", 7, 8);
    let config = TapSkillConfig {
        name: "demo tap".to_string(),
        tap_package_name: "demo_tap".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        tap_package_path: demo_tap_package_path(),
        requirements: requirements(),
        shared_objects: vec![TapSharedObjectRef::immutable(addr("0x31"))],
        interface_revision: InterfaceRevision(1),
        active_for_new_executions: true,
    };
    assert!(config.tap_package_path.join("Move.toml").exists());
    let artifact =
        nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id, tap_package_id)
            .expect("publish artifact")
            .with_endpoint_object(endpoint_object.clone())
            .expect("endpoint-bound artifact");
    let endpoint_config_digest = artifact
        .endpoint_config_digest(*endpoint_object.object_id())
        .expect("endpoint digest");

    let registry = TapRegistry {
        id: addr("0x91"),
        agents: vec![TapAgentRecord {
            agent_id,
            owner: addr("0x92"),
            operator: addr("0x93"),
            active: true,
            next_skill_index: 1,
            skills: MoveTable::new(addr("0x95"), 1),
            endpoints: MoveTable::new(addr("0x96"), 1),
            active_endpoints: vec![TapEndpointActivation {
                agent_id,
                skill_id,
                interface_revision: artifact.interface_revision,
            }],
        }],
        skills: vec![TapSkillRecord {
            agent_id,
            skill_id,
            dag_id,
            dag_binding: TapDagBinding::pinned(dag_id),
            tap_package_id,
            workflow_commitment: artifact.requirements.workflow_commitment.clone(),
            requirements_commitment: artifact.requirements.input_schema_commitment.clone(),
            metadata_commitment: artifact.requirements.metadata_commitment.clone(),
            payment_policy: artifact.requirements.payment_policy.clone(),
            schedule_policy: artifact.requirements.schedule_policy.clone(),
            capability_schema_commitment: artifact
                .requirements
                .vertex_authorization_schema
                .schema_commitment
                .clone(),
            active: true,
        }],
        endpoints: vec![TapEndpointRevision {
            agent_id,
            skill_id,
            interface_revision: artifact.interface_revision,
            package_id: artifact.tap_package_id,
            endpoint_object_id: *endpoint_object.object_id(),
            endpoint_object_version: endpoint_object.version(),
            endpoint_object_digest: endpoint_object.digest().inner().to_vec(),
            shared_objects: artifact.shared_objects.clone(),
            requirements: artifact.requirements.clone(),
            config_digest: endpoint_config_digest.clone(),
            active_for_new_executions: true,
        }],
        active_endpoints: vec![TapEndpointActivation {
            agent_id,
            skill_id,
            interface_revision: artifact.interface_revision,
        }],
        default_target: None,
    };

    let target =
        nexus_sdk::types::resolve_active_tap_skill_execution_target(&registry, agent_id, skill_id)
            .expect("registered demo skill resolves");

    assert_eq!(target.skill.dag_binding, TapDagBinding::pinned(dag_id));
    assert_eq!(target.skill.tap_package_id, artifact.tap_package_id);
    assert_eq!(target.endpoint.package_id, artifact.tap_package_id);
    assert_eq!(target.endpoint.endpoint_object, endpoint_object);
    assert_eq!(target.endpoint.config_digest, endpoint_config_digest);
    assert_eq!(
        target.endpoint.requirements.workflow_commitment,
        artifact.requirements.workflow_commitment
    );
}

#[test]
fn transaction_builders_select_standard_runtime_worksheet_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry = tap_tx::tap_registry_arg(&mut tx, &objects).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, false));

    let worksheet = tap_tx::workflow_worksheet(&mut tx, &objects, registry, agent, 177)
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
        &mut tx,
        &objects,
        execution,
        worksheet,
        leader_cap,
        &RuntimeVertex::plain("entry"),
    )
    .expect("pre-stamp standard tap execution");

    let tx = finish_transaction(tx);
    assert_eq!(
        move_call(&tx, 0).function,
        nexus_sdk::idents::workflow::Dag::LEADER_STAMP_STANDARD_TAP_WORKSHEET.name
    );
    assert_eq!(
        move_call(&tx, 1).function,
        nexus_sdk::idents::move_std::Ascii::STRING.name
    );
    assert_eq!(
        move_call(&tx, 2).function,
        nexus_sdk::idents::workflow::Dag::RUNTIME_VERTEX_PLAIN_FROM_STRING.name
    );
    assert_eq!(
        move_call(&tx, 3).function,
        nexus_sdk::idents::workflow::Dag::PRE_STAMP_STANDARD_TAP_EXECUTION.name
    );
}
