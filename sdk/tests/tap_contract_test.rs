#![cfg(feature = "full")]

use {
    nexus_sdk::{
        events::RequestWalkExecutionEvent,
        idents::{
            primitives,
            pure_arg,
            registry::{AgentRegistry, AGENT_REGISTRY_MODULE},
            sui_framework,
            tap::TapStandard,
        },
        sui,
        transactions::tap as tap_tx,
        types::{
            resolve_active_tap_skill_revision,
            DefaultDagExecutor,
            InterfaceRevision,
            MoveTable,
            NexusObjects,
            RuntimeVertex,
            TapAgentRecord,
            TapDagBinding,
            TapFixedTool,
            TapPaymentPolicy,
            TapPaymentSource,
            TapPaymentSourceKind,
            TapRecurrenceKind,
            TapRegistry,
            TapSchedulePolicy,
            TapSkillConfig,
            TapSkillRecord,
            TapSkillRequirements,
            TapSkillRevisionKey,
            TapSkillRevisionRecord,
            TapSkillRevisionResolutionError,
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
        scheduler_pkg_id: addr("0x11"),
        primitives_pkg_id: addr("0x2"),
        interface_pkg_id: addr("0x3"),
        network_id: addr("0x4"),
        registry_pkg_id: addr("0x5"),
        tool_registry: object_ref("0x6", 1, 6),
        verifier_registry: object_ref("0x7", 1, 7),
        network_auth: object_ref("0x8", 1, 8),
        agent_registry: object_ref("0xc", 1, 12),
        default_dag_executor: DefaultDagExecutor {
            agent_id: addr("0xa1"),
            skill_id: 177,
        },
        gas_service: object_ref("0xd", 1, 13),
        leader_registry: object_ref("0xe", 1, 14),
        workflow_original_pkg_id: None,
        scheduler_original_pkg_id: None,
    }
}

fn requirements() -> TapSkillRequirements {
    TapSkillRequirements {
        input_schema_commitment: vec![1],
        payment_policy: TapPaymentPolicy::UserFunded,
        schedule_policy: TapSchedulePolicy::default(),
        fixed_tools: vec![TapFixedTool {
            tool_registry_id: addr("0x6"),
            tool_fqn: "demo::tool::run".to_string(),
        }],
    }
}

fn demo_tap_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/move/demo_tap")
}

fn skill_revision(revision: u64) -> TapSkillRevisionRecord {
    TapSkillRevisionRecord {
        key: TapSkillRevisionKey {
            agent_id: addr("0xa1"),
            skill_id: 177,
            interface_revision: InterfaceRevision(revision),
        },
        requirements: requirements(),
    }
}

fn skill(agent_id: sui::types::Address, skill_id: u64, active_revision: u64) -> TapSkillRecord {
    TapSkillRecord {
        agent_id: Some(agent_id),
        skill_id: Some(skill_id),
        description: b"demo skill".to_vec(),
        active: true,
        dag_binding: TapDagBinding::pinned(addr("0x94")),
        requirements: requirements(),
        current_interface_revision: InterfaceRevision(active_revision),
        scheduled_task_count: 0,
    }
}

fn registry_with_active_revision(active_revision: u64) -> TapRegistry {
    let agent_id = addr("0xa1");
    let skill_id = 177;

    TapRegistry {
        id: addr("0x91"),
        agents: vec![TapAgentRecord {
            active: true,
            skills: MoveTable::new(addr("0x95"), 1),
        }],
        skills: vec![skill(agent_id, skill_id, active_revision)],
        default_executor: Some(DefaultDagExecutor { agent_id, skill_id }),
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
fn active_skill_revision_resolution_uses_skill_current_revision_pointer() {
    let records = vec![skill_revision(0), skill_revision(1)];
    let skills = registry_with_active_revision(1).skills;
    let resolved = resolve_active_tap_skill_revision(&records, &skills, addr("0xa1"), 177)
        .expect("one active skill_revision projection");

    assert_eq!(resolved.key.interface_revision, InterfaceRevision(1));

    let duplicate = vec![skill_revision(1), skill_revision(1)];
    assert!(matches!(
        resolve_active_tap_skill_revision(&duplicate, &skills, addr("0xa1"), 177),
        Err(TapSkillRevisionResolutionError::DuplicateActiveRevision { count: 2, .. })
    ));
}

#[test]
fn registry_recovery_projects_current_skill_revision_as_endpoint() {
    let mut registry = registry_with_active_revision(2);
    registry.skills.push(skill(addr("0xb2"), 177, 5));

    let active = registry
        .active_skill_revision_record(addr("0xa1"), 177)
        .expect("active registry skill_revision projection");
    assert_eq!(active.key.interface_revision, InterfaceRevision(2));
    assert_eq!(active.requirements, requirements());

    let records = registry
        .skill_revision_records()
        .expect("registry skill_revision projections");
    assert_eq!(records.len(), 2);

    let pinned = registry
        .skill_revision_record(TapSkillRevisionKey {
            agent_id: addr("0xa1"),
            skill_id: 177,
            interface_revision: InterfaceRevision(2),
        })
        .expect("current projected skill_revision");
    assert_eq!(pinned.key.interface_revision, InterfaceRevision(2));

    let skill_bytes = bcs::to_bytes(&registry.skills[0]).expect("stored skill BCS");
    let stored_skill: TapSkillRecord = bcs::from_bytes(&skill_bytes).expect("stored skill decodes");
    assert_eq!(stored_skill.agent_id, None);
    assert_eq!(stored_skill.skill_id, None);
    assert_eq!(
        stored_skill.current_interface_revision,
        InterfaceRevision(2)
    );
}

#[test]
fn nexus_objects_carries_agent_registry_metadata() {
    let objects = nexus_objects();
    assert_eq!(*objects.agent_registry.object_id(), addr("0xc"));
    assert_eq!(
        objects.default_dag_executor,
        DefaultDagExecutor {
            agent_id: addr("0xa1"),
            skill_id: 177,
        }
    );
}

fn request_walk_event() -> RequestWalkExecutionEvent {
    RequestWalkExecutionEvent {
        dag: addr("0x51"),
        execution: addr("0x52"),
        invoker: addr("0x53"),
        walk_index: 0,
        next_vertex: RuntimeVertex::plain("entry"),
        evaluations: addr("0x54"),
        tap_agent_id: None,
        tap_skill_id: None,
        tap_interface_revision: None,
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
        "tap_payment_id": { "vec": ["0xd1"] },
        "tap_selected_dag_id": { "vec": ["0x51"] },
        "tap_authorization_plan_commitment": { "vec": [[1, 2, 3]] }
    }))
    .expect("event should deserialize");

    let context = event
        .standard_tap_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, addr("0xa1"));
    assert_eq!(context.skill_id, 177);
    assert_eq!(context.interface_revision, InterfaceRevision(7));
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
            "interface_revision": { "vec": [{ "value": "7" }] },
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
fn publish_artifact_preserves_skill_contract_without_endpoint_digest() {
    let config = TapSkillConfig {
        name: "weather".to_string(),
        tap_package_name: "weather_tap".to_string(),
        dag_path: PathBuf::from("dag.json"),
        tap_package_path: PathBuf::from("tap"),
        requirements: requirements(),
        interface_revision: InterfaceRevision(3),
    };

    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, addr("0x24"))
        .expect("valid artifact");
    assert_eq!(artifact.skill_name, "weather");
    assert_eq!(artifact.dag_id, addr("0x24"));
    assert_eq!(artifact.interface_revision, InterfaceRevision(3));
    assert_eq!(artifact.requirements, config.requirements);

    let value = serde_json::to_value(&artifact).expect("artifact json");
    assert!(value.get("tap_package_id").is_none());
    assert!(value.get("config_digest").is_none());
    assert!(value.get("config_digest_hex").is_none());
    assert!(value.get("shared_objects").is_none());
}

#[test]
fn registry_default_executor_requires_runtime_selected_binding() {
    let mut registry = registry_with_active_revision(1);

    let error = nexus_sdk::types::resolve_default_tap_dag_executor(&registry)
        .expect_err("pinned dag binding should not resolve as default DAG executor");
    assert!(error.to_string().contains("is not runtime-DAG selected"));

    registry.skills[0].dag_binding = TapDagBinding::RuntimeSelected;
    let resolved = nexus_sdk::types::resolve_default_tap_dag_executor(&registry)
        .expect("runtime-selected binding resolves default DAG executor");

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
        "payer": "0xff",
        "payment_mode": "user_funded",
        "source_kind": "agent_vault",
        "source_identity": "0xcc",
        "max_budget": "42",
        "locked_budget": "40",
        "consumed": "5",
        "payment_source_hash": [1, 2],
        "accomplished": true,
        "refunded": false
    }))
    .expect("payment object should deserialize");

    assert_eq!(payment.payment_id(), addr("0xaa"));
    assert_eq!(payment.execution_id, addr("0xbb"));
    assert_eq!(payment.skill_revision_key().agent_id, addr("0xcc"));
    assert_eq!(payment.skill_revision_key().skill_id, 221);
    assert_eq!(
        payment.skill_revision_key().interface_revision,
        InterfaceRevision(7)
    );
    assert_eq!(
        payment.payment_mode,
        nexus_sdk::types::TapPaymentMode::UserFunded
    );
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
            &TapPaymentPolicy::UserFunded,
            &invoker_source,
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
        &TapPaymentPolicy::UserFunded,
        &payer_address_source,
        0,
        payer,
    )
    .expect("user-funded payer address source validates");

    let agent_funded = TapPaymentPolicy::AgentFunded { max_budget: 100 };
    assert!(
        nexus_sdk::types::validate_standard_tap_payment_options(
            agent_id,
            &agent_funded,
            &agent_vault_source,
            100,
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
        100,
        payer,
    )
    .expect("agent-funded address source validates at the policy cap");
    assert!(nexus_sdk::types::validate_standard_tap_payment_options(
        agent_id,
        &agent_funded,
        &agent_address_source,
        101,
        payer,
    )
    .is_err());
}

#[test]
fn standard_tap_events_are_nexus_events() {
    let objects = nexus_objects();
    let revisioned_event = wrap_event(
        &objects,
        sui::types::StructTag::new(
            objects.registry_pkg_id,
            AGENT_REGISTRY_MODULE,
            sui::types::Identifier::from_static("SkillContractRevisionedEvent"),
            vec![],
        ),
    );

    assert!(objects.is_event_from_nexus(&revisioned_event));

    let execution_requested_event = wrap_event(
        &objects,
        sui::types::StructTag::new(
            objects.workflow_pkg_id,
            sui::types::Identifier::from_static("dag"),
            sui::types::Identifier::from_static("AgentSkillExecutionRequestedEvent"),
            vec![],
        ),
    );

    assert!(objects.is_event_from_nexus(&execution_requested_event));

    let unrelated_interface_event = wrap_event(
        &objects,
        sui::types::StructTag::new(
            objects.interface_pkg_id,
            sui::types::Identifier::from_static("unrelated"),
            sui::types::Identifier::from_static("SkillContractRevisionedEvent"),
            vec![],
        ),
    );

    assert!(!objects.is_event_from_nexus(&unrelated_interface_event));
}

#[test]
fn transaction_builders_select_tap_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("configured registry");

    tap_tx::bootstrap_default_runtime_dag_skill_for_deployment(&mut tx, &objects, registry)
        .expect("deployment bootstrap builder");

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, true));
    let requirements = requirements();
    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        agent,
        addr("0xd1"),
        b"demo".to_vec(),
        requirements.input_schema_commitment,
        requirements.payment_policy,
        requirements.schedule_policy,
    )
    .expect("register skill builder");

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, false).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, false));
    tap_tx::worksheet(&mut tx, &objects, registry, agent, 177, addr("0x41"))
        .expect("worksheet builder");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);

    assert!(calls.iter().any(|call| {
        call.function == AgentRegistry::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT.name
    }));
    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistry::REGISTER_SKILL.name));
    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistry::WORKSHEET.name));
}

#[test]
fn update_skill_compatibility_builds_dag_and_policy_calls() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, true));
    tap_tx::update_dag(&mut tx, &objects, registry, agent, 177, addr("0xd2"))
        .expect("update dag builder");

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("configured registry");
    let agent = tx.input(sui::tx::Input::shared(addr("0xa1"), 1, true));
    tap_tx::update_skill_policies(
        &mut tx,
        &objects,
        registry,
        agent,
        177,
        TapPaymentPolicy::AgentFunded { max_budget: 100 },
        TapSchedulePolicy {
            recurrence: TapRecurrenceKind::Recursive {
                min_interval_ms: 5000,
                max_occurrences: Some(3),
            },
            allow_recursive: true,
        },
    )
    .expect("update skill policies builder");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);

    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistry::UPDATE_DAG.name));
    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistry::UPDATE_SKILL_POLICIES.name));
    assert!(!calls
        .iter()
        .any(|call| call.function.as_str() == "update_skill"));
}

#[test]
fn demo_tap_publish_and_bind_lifecycle_ptb() {
    let objects = nexus_objects();
    let agent_id = addr("0xa5");
    let dag_id = addr("0xd5");
    let config = TapSkillConfig {
        name: "demo tap".to_string(),
        tap_package_name: "demo_tap".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        tap_package_path: demo_tap_package_path(),
        requirements: requirements(),
        interface_revision: InterfaceRevision(1),
    };
    assert!(config.tap_package_path.join("Move.toml").exists());
    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id)
        .expect("publish artifact");
    let mut tx = sui::tx::TransactionBuilder::new();

    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("registry");
    tap_tx::create_agent(&mut tx, &objects, registry).expect("create agent");

    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("registry");
    let agent_object = tx.input(sui::tx::Input::shared(agent_id, 1, true));
    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        agent_object,
        artifact.dag_id,
        artifact.skill_name.as_bytes().to_vec(),
        artifact.requirements.input_schema_commitment.clone(),
        artifact.requirements.payment_policy.clone(),
        artifact.requirements.schedule_policy.clone(),
    )
    .expect("register skill");

    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, false).expect("registry");
    tap_tx::workflow_worksheet_for_ids(&mut tx, &objects, registry, agent_id, 181)
        .expect("workflow worksheet");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);
    let find_call = |function: &sui::types::Identifier| {
        calls
            .iter()
            .position(|call| &call.function == function)
            .expect("expected lifecycle call")
    };

    let create_agent = find_call(&AgentRegistry::CREATE_AGENT.name);
    let register_skill = find_call(&AgentRegistry::REGISTER_SKILL.name);
    let worksheet = find_call(&AgentRegistry::WORKFLOW_WORKSHEET_FOR_IDS.name);

    assert!(create_agent < register_skill);
    assert!(register_skill < worksheet);
    assert_eq!(
        move_call(&tx, worksheet).function,
        TapStandard::WORKFLOW_WORKSHEET_FOR_IDS.name
    );
}

#[test]
fn agent_payment_vault_builders_target_tap_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, false).expect("registry");
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
    assert_eq!(withdraw.package, objects.registry_pkg_id);
    assert_eq!(deposit.arguments, vec![agent, coin]);
    assert_eq!(withdraw.arguments.len(), 3);
}

#[test]
fn demo_tap_publish_artifact_resolves_registered_execution_target() {
    let agent_id = addr("0xa5");
    let skill_id = 181;
    let dag_id = addr("0xd5");
    let config = TapSkillConfig {
        name: "demo tap".to_string(),
        tap_package_name: "demo_tap".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        tap_package_path: demo_tap_package_path(),
        requirements: requirements(),
        interface_revision: InterfaceRevision(1),
    };
    assert!(config.tap_package_path.join("Move.toml").exists());
    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id)
        .expect("publish artifact");

    let registry = TapRegistry {
        id: addr("0x91"),
        agents: vec![TapAgentRecord {
            active: true,
            skills: MoveTable::new(addr("0x95"), 1),
        }],
        skills: vec![TapSkillRecord {
            agent_id: Some(agent_id),
            skill_id: Some(skill_id),
            description: artifact.skill_name.as_bytes().to_vec(),
            active: true,
            dag_binding: TapDagBinding::pinned(dag_id),
            requirements: artifact.requirements.clone(),
            current_interface_revision: artifact.interface_revision,
            scheduled_task_count: 0,
        }],
        default_executor: None,
    };

    let target =
        nexus_sdk::types::resolve_active_tap_skill_execution_target(&registry, agent_id, skill_id)
            .expect("registered demo skill resolves");

    assert_eq!(target.skill.dag_binding, TapDagBinding::pinned(dag_id));
    assert_eq!(target.skill_revision.requirements, artifact.requirements);
    assert_eq!(
        target.skill_revision.key.interface_revision,
        artifact.interface_revision
    );
}

#[test]
fn transaction_builders_select_standard_runtime_worksheet_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, false).expect("configured registry");

    let worksheet =
        tap_tx::workflow_worksheet_for_ids(&mut tx, &objects, registry, addr("0xa1"), 177)
            .expect("workflow worksheet builder");

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, false).expect("configured registry");
    tap_tx::confirm_tool_eval_for_walk(&mut tx, &objects, registry, worksheet);

    let tx = finish_transaction(tx);
    assert_eq!(
        move_call(&tx, 0).function,
        sui_framework::Object::ID_FROM_ADDRESS.name
    );
    assert_eq!(
        move_call(&tx, 1).function,
        AgentRegistry::WORKFLOW_WORKSHEET_FOR_IDS.name
    );
    assert_eq!(
        move_call(&tx, 2).function,
        AgentRegistry::CONFIRM_TOOL_EVAL_FOR_WALK.name
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

    nexus_sdk::transactions::dag::leader_stamp_tap_worksheet(
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
    nexus_sdk::transactions::dag::pre_stamp_tap_execution(
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
        nexus_sdk::idents::workflow::Dag::LEADER_STAMP_TAP_WORKSHEET.name
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
        nexus_sdk::idents::workflow::Dag::PRE_STAMP_TAP_EXECUTION.name
    );
}
