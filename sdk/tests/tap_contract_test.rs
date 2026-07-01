#![cfg(feature = "full")]

use {
    nexus_sdk::{
        idents::{
            interface::Agent,
            primitives,
            registry::{AgentRegistry as AgentRegistryIdent, AGENT_REGISTRY_MODULE},
        },
        sui,
        transactions::tap as tap_tx,
        types::{
            interface::{
                agent::{
                    FixedTool,
                    SkillDagBinding,
                    SkillRecurrenceKind,
                    SkillRequirement,
                    SkillSchedulePolicy,
                },
                payment::{
                    ExecutionPayment,
                    ExecutionPaymentFinalState,
                    PaymentSourceKind,
                    SkillPaymentPolicy,
                },
                version::InterfaceVersion,
            },
            registry::agent_registry::{AgentRecord, DefaultDagExecutor, SkillRecord},
            resolve_active_tap_skill_revision,
            workflow::execution_events::RequestWalkExecutionEvent,
            AgentRegistrySnapshot,
            DefaultDagExecutorTarget,
            MoveTable,
            NexusObjects,
            RuntimeVertex,
            SkillConfig,
            SkillRecordContext,
            SkillRevisionContext,
            SkillRevisionLookupError,
            SkillRevisionLookupKey,
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

fn object_id(address: sui::types::Address) -> nexus_sdk::types::sui_framework::object::ID {
    nexus_sdk::types::sui_address_to_id(address)
}

fn interface_version(inner: u64) -> nexus_sdk::types::interface::version::InterfaceVersion {
    nexus_sdk::types::interface::version::InterfaceVersion { inner }
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
        default_dag_executor: DefaultDagExecutorTarget {
            agent_id: addr("0xa1"),
            skill_id: 177,
        },
        gas_service: object_ref("0xd", 1, 13),
        leader_registry: object_ref("0xe", 1, 14),
        workflow_original_pkg_id: None,
        scheduler_original_pkg_id: None,
    }
}

fn requirements() -> SkillRequirement {
    SkillRequirement {
        input_commitment: vec![1],
        payment_policy: SkillPaymentPolicy::UserFunded,
        schedule_policy: SkillSchedulePolicy::default(),
        fixed_tools: vec![FixedTool::new(addr("0x6"), "demo::tool::run")],
    }
}

fn skill_revision(revision: u64) -> SkillRevisionContext {
    SkillRevisionContext {
        key: SkillRevisionLookupKey {
            agent_id: addr("0xa1"),
            skill_id: 177,
            interface_revision: InterfaceVersion::new(revision),
        },
        requirements: requirements(),
    }
}

fn skill(agent_id: sui::types::Address, skill_id: u64, active_revision: u64) -> SkillRecordContext {
    SkillRecordContext {
        agent_id,
        skill_id,
        record: SkillRecord {
            description: b"demo skill".to_vec(),
            active: true,
            dag_binding: SkillDagBinding::pinned(addr("0x94")),
            requirements: requirements(),
            current_interface_revision: InterfaceVersion::new(active_revision),
            scheduled_task_count: 0,
        },
    }
}

fn registry_with_active_revision(active_revision: u64) -> AgentRegistrySnapshot {
    let agent_id = addr("0xa1");
    let skill_id = 177;

    AgentRegistrySnapshot {
        id: addr("0x91"),
        agents: vec![AgentRecord {
            active: true,
            skills: MoveTable::new(addr("0x95"), 1),
        }],
        skills: vec![skill(agent_id, skill_id, active_revision)],
        default_executor: Some(DefaultDagExecutor {
            agent: nexus_sdk::types::interface::agent::Agent::from_ids(
                agent_id,
                1,
                Some(addr("0x91")),
            ),
            skill_id,
        }),
    }
}

fn finish_transaction(mut tx: sui::tx::TransactionBuilder) -> sui::types::Transaction {
    let gas = object_ref("0xf1", 1, 9);

    tx.set_sender(addr("0xf2"));
    tx.set_gas_budget(1000);
    tx.set_gas_price(1000);
    tx.add_gas_objects(vec![sui::tx::ObjectInput::owned(
        *gas.object_id(),
        gas.version(),
        *gas.digest(),
    )]);

    tx.try_build().expect("transaction builds")
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

    assert_eq!(resolved.key.interface_revision, InterfaceVersion::new(1));

    let duplicate = vec![skill_revision(1), skill_revision(1)];
    assert!(matches!(
        resolve_active_tap_skill_revision(&duplicate, &skills, addr("0xa1"), 177),
        Err(SkillRevisionLookupError::DuplicateActiveRevision { count: 2, .. })
    ));
}

#[test]
fn registry_recovery_projects_current_skill_revision_as_endpoint() {
    let mut registry = registry_with_active_revision(2);
    registry.skills.push(skill(addr("0xb2"), 177, 5));

    let active = registry
        .active_skill_revision_record(addr("0xa1"), 177)
        .expect("active registry skill_revision projection");
    assert_eq!(active.key.interface_revision, InterfaceVersion::new(2));
    assert_eq!(active.requirements, requirements());

    let records = registry
        .skill_revision_records()
        .expect("registry skill_revision projections");
    assert_eq!(records.len(), 2);

    let pinned = registry
        .skill_revision_record(SkillRevisionLookupKey {
            agent_id: addr("0xa1"),
            skill_id: 177,
            interface_revision: InterfaceVersion::new(2),
        })
        .expect("current projected skill_revision");
    assert_eq!(pinned.key.interface_revision, InterfaceVersion::new(2));

    let skill_bytes = bcs::to_bytes(&registry.skills[0].record).expect("stored skill BCS");
    let stored_skill: SkillRecord = bcs::from_bytes(&skill_bytes).expect("stored skill decodes");
    assert_eq!(registry.skills[0].agent_id, addr("0xa1"));
    assert_eq!(registry.skills[0].skill_id, 177);
    assert_eq!(
        stored_skill.current_interface_revision,
        InterfaceVersion::new(2)
    );
}

#[test]
fn nexus_objects_carries_agent_registry_metadata() {
    let objects = nexus_objects();
    assert_eq!(*objects.agent_registry.object_id(), addr("0xc"));
    assert_eq!(
        objects.default_dag_executor,
        DefaultDagExecutorTarget {
            agent_id: addr("0xa1"),
            skill_id: 177,
        }
    );
}

fn request_walk_event() -> RequestWalkExecutionEvent {
    RequestWalkExecutionEvent {
        dag: object_id(addr("0x51")),
        execution: object_id(addr("0x52")),
        invoker: addr("0x53"),
        walk_index: 0,
        next_vertex: RuntimeVertex::plain("entry"),
        evaluations: object_id(addr("0x54")),
        agent_id: object_id(addr("0xa1")),
        skill_id: 177,
        interface_version: interface_version(7),
        scheduled_task_id: nexus_sdk::types::MoveOption(None),
        scheduled_occurrence_index: nexus_sdk::types::MoveOption(None),
    }
}

#[test]
fn request_walk_context_uses_required_agent_fields() {
    let event = request_walk_event();
    let context = event
        .to_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, addr("0xa1"));
    assert_eq!(context.skill_id, 177);
    assert_eq!(context.interface_revision, InterfaceVersion::new(7));
    assert_eq!(context.scheduled_task_id, None);
    assert_eq!(context.scheduled_occurrence_index, None);
}

#[test]
fn request_walk_context_deserializes_move_option_fields() {
    let event: RequestWalkExecutionEvent = serde_json::from_value(serde_json::json!({
        "dag": "0x51",
        "execution": "0x52",
        "invoker": "0x53",
        "walk_index": 0,
        "next_vertex": { "Plain": { "vertex": { "name": "entry" } } },
        "evaluations": "0x54",
        "worksheet_from_type": { "name": "0x2::legacy::Witness" },
        "worksheet_from_uid": "0x55",
        "agent_id": "0xa1",
        "skill_id": "177",
        "interface_version": 7,
        "scheduled_task_id": { "vec": ["0x66"] },
        "scheduled_occurrence_index": { "vec": ["3"] }
    }))
    .expect("event should deserialize");

    let context = event
        .to_context()
        .expect("complete context should parse")
        .expect("standard context should be present");

    assert_eq!(context.agent_id, addr("0xa1"));
    assert_eq!(context.skill_id, 177);
    assert_eq!(context.interface_revision, InterfaceVersion::new(7));
    assert_eq!(context.scheduled_task_id, Some(addr("0x66")));
    assert_eq!(context.scheduled_occurrence_index, Some(3));
}

#[test]
fn publish_artifact_preserves_skill_contract_without_endpoint_digest() {
    let config = SkillConfig {
        name: "weather".to_string(),
        dag_path: PathBuf::from("dag.json"),
        requirements: requirements(),
        interface_revision: InterfaceVersion::new(3),
    };

    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, addr("0x24"))
        .expect("valid artifact");
    assert_eq!(artifact.skill_name, "weather");
    assert_eq!(artifact.dag_id, addr("0x24"));
    assert_eq!(artifact.interface_revision, InterfaceVersion::new(3));
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

    registry.skills[0].record.dag_binding = SkillDagBinding::RuntimeSelected;
    let resolved = nexus_sdk::types::resolve_default_tap_dag_executor(&registry)
        .expect("runtime-selected binding resolves default DAG executor");

    assert_eq!(resolved.target.agent_id, addr("0xa1"));
    assert_eq!(resolved.target.skill_id, 177);
    assert_eq!(
        resolved.skill.dag_binding(),
        &SkillDagBinding::RuntimeSelected
    );
}

#[test]
fn tap_execution_payment_model_matches_live_object_shape() {
    let payment: ExecutionPayment = serde_json::from_value(json!({
        "id": "0xaa",
        "execution_id": "0xbb",
        "agent_id": "0xcc",
        "skill_id": "221",
        "interface_revision": { "value": "7" },
        "payment_policy": "UserFunded",
        "source_kind": {
            "AgentFunded": {
                "agent_id": "0xcc"
            }
        },
        "max_budget": "42",
        "locked_budget": "40",
        "consumed": "5",
        "funds": { "value": "100" },
        "final_state": "Accomplished",
        "tool_cost_snapshot": { "contents": [] },
        "locked_vertices": [],
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
        InterfaceVersion::new(7)
    );
    assert_eq!(
        payment.payment_policy,
        nexus_sdk::types::interface::payment::SkillPaymentPolicy::UserFunded
    );
    assert_eq!(
        payment.source_kind,
        PaymentSourceKind::agent_funded(addr("0xcc"))
    );
    assert_eq!(
        payment.final_state,
        ExecutionPaymentFinalState::Accomplished
    );
    assert_eq!(payment.max_budget, 42);
    assert_eq!(payment.locked_budget, 40);
    assert_eq!(payment.consumed, 5);
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
        PaymentSourceKind::from_bcs_bytes(&invoker_source).expect("typed invoker source decodes");
    let decoded_vault =
        PaymentSourceKind::from_bcs_bytes(&agent_vault_source).expect("typed vault source decodes");
    assert_eq!(decoded_invoker, PaymentSourceKind::user_funded(payer));
    assert_eq!(decoded_invoker.identity(), payer);
    assert_eq!(decoded_vault, PaymentSourceKind::agent_funded(agent_id));
    assert_eq!(decoded_vault.identity(), agent_id);

    nexus_sdk::types::validate_execution_payment_options(
        agent_id,
        &SkillPaymentPolicy::UserFunded,
        &invoker_source,
        0,
        payer,
    )
    .expect("generated invoker source validates for user-funded policy");

    let payer_address_source =
        nexus_sdk::types::payment_source_from_address(payer).expect("payer address source");
    nexus_sdk::types::validate_execution_payment_options(
        agent_id,
        &SkillPaymentPolicy::UserFunded,
        &payer_address_source,
        0,
        payer,
    )
    .expect("user-funded payer address source validates");

    let agent_funded = SkillPaymentPolicy::AgentFunded { max_budget: 100 };
    assert!(
        nexus_sdk::types::validate_execution_payment_options(
            agent_id,
            &agent_funded,
            &agent_vault_source,
            100,
            payer,
        )
        .is_ok(),
        "generated agent-vault sources validate for the direct Move agent-funded policy"
    );

    let agent_address_source =
        nexus_sdk::types::payment_source_from_address(agent_id).expect("agent address source");
    assert!(
        nexus_sdk::types::validate_execution_payment_options(
            agent_id,
            &agent_funded,
            &agent_address_source,
            100,
            payer,
        )
        .is_err(),
        "user-funded source for the agent id is not an agent-vault source"
    );
    assert!(nexus_sdk::types::validate_execution_payment_options(
        agent_id,
        &agent_funded,
        &agent_vault_source,
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
    let agent = tx.object(sui::tx::ObjectInput::shared(addr("0xa1"), 1, true));
    let requirements = requirements();
    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        agent,
        addr("0xd1"),
        b"demo".to_vec(),
        requirements.input_commitment,
        requirements.payment_policy,
        requirements.schedule_policy,
    )
    .expect("register skill builder");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);

    assert!(calls.iter().any(|call| {
        call.function == AgentRegistryIdent::BOOTSTRAP_DEFAULT_RUNTIME_DAG_SKILL_FOR_DEPLOYMENT.name
    }));
    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistryIdent::REGISTER_SKILL.name));
    assert!(!calls
        .iter()
        .any(|call| call.function == sui::types::Identifier::from_static("worksheet")));
}

#[test]
fn update_skill_compatibility_builds_dag_and_policy_calls() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("configured registry");
    let agent = tx.object(sui::tx::ObjectInput::shared(addr("0xa1"), 1, true));
    tap_tx::update_dag(&mut tx, &objects, registry, agent, 177, addr("0xd2"))
        .expect("update dag builder");

    let registry =
        tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("configured registry");
    let agent = tx.object(sui::tx::ObjectInput::shared(addr("0xa1"), 1, true));
    tap_tx::update_skill_policies(
        &mut tx,
        &objects,
        registry,
        agent,
        177,
        SkillPaymentPolicy::AgentFunded { max_budget: 100 },
        SkillSchedulePolicy {
            recurrence: SkillRecurrenceKind::Recursive {
                min_interval_ms: 5000,
                max_occurrences: nexus_sdk::types::MoveOption(Some(3)),
            },
            allow_recursive: true,
        },
    )
    .expect("update skill policies builder");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);

    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistryIdent::UPDATE_DAG.name));
    assert!(calls
        .iter()
        .any(|call| call.function == AgentRegistryIdent::UPDATE_SKILL_POLICIES.name));
    assert!(!calls
        .iter()
        .any(|call| call.function.as_str() == "update_skill"));
}

#[test]
fn demo_tap_publish_and_bind_lifecycle_ptb() {
    let objects = nexus_objects();
    let dag_id = addr("0xd5");
    let config = SkillConfig {
        name: "demo_agent".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        requirements: requirements(),
        interface_revision: InterfaceVersion::new(1),
    };
    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id)
        .expect("publish artifact");
    let mut tx = sui::tx::TransactionBuilder::new();

    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("registry");
    let agent_object = tap_tx::create_agent(&mut tx, &objects, registry).expect("create agent");

    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, true).expect("registry");
    tap_tx::register_skill(
        &mut tx,
        &objects,
        registry,
        agent_object,
        artifact.dag_id,
        artifact.skill_name.as_bytes().to_vec(),
        artifact.requirements.input_commitment.clone(),
        artifact.requirements.payment_policy.clone(),
        artifact.requirements.schedule_policy.clone(),
    )
    .expect("register skill");

    let dag = tx.object(sui::tx::ObjectInput::shared(dag_id, 1, false));
    let execution = tx.object(sui::tx::ObjectInput::shared(addr("0xe1"), 1, true));
    let leader_cap = tx.pure(&1_u64);
    nexus_sdk::transactions::dag::prepare_tool_result_submission_worksheet(
        &mut tx, &objects, dag, execution, leader_cap, 0,
    )
    .expect("workflow worksheet");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);
    let find_call = |function: &sui::types::Identifier| {
        calls
            .iter()
            .position(|call| &call.function == function)
            .expect("expected lifecycle call")
    };

    let create_agent = find_call(&AgentRegistryIdent::CREATE_AGENT.name);
    let register_skill = find_call(&AgentRegistryIdent::REGISTER_SKILL.name);
    let worksheet = find_call(
        &nexus_sdk::idents::workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET
            .name,
    );

    assert!(create_agent < register_skill);
    assert!(register_skill < worksheet);
    assert_eq!(
        move_call(&tx, worksheet).function,
        nexus_sdk::idents::workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET
            .name
    );
}

#[test]
fn agent_payment_vault_builders_target_tap_functions() {
    let objects = nexus_objects();
    let mut tx = sui::tx::TransactionBuilder::new();
    let registry = tap_tx::agent_registry_arg(&mut tx, &objects, false).expect("registry");
    let agent = tx.pure(&1_u64);
    let coin = tx.pure(&2_u64);

    tap_tx::deposit_agent_payment_vault(&mut tx, &objects, agent, coin);
    tap_tx::withdraw_agent_payment_vault(&mut tx, &objects, registry, agent, 25)
        .expect("withdraw vault");

    let tx = finish_transaction(tx);
    let calls = move_calls(&tx);
    let deposit = calls
        .iter()
        .find(|call| call.function == Agent::DEPOSIT_AGENT_PAYMENT_VAULT.name)
        .expect("deposit vault call");
    let withdraw = calls
        .iter()
        .find(|call| call.function == Agent::WITHDRAW_AGENT_PAYMENT_VAULT.name)
        .expect("withdraw vault call");

    assert_eq!(deposit.package, objects.interface_pkg_id);
    assert_eq!(withdraw.package, objects.registry_pkg_id);
    assert_eq!(
        deposit.arguments,
        vec![
            sui::types::Argument::Input(1),
            sui::types::Argument::Input(2)
        ]
    );
    assert_eq!(withdraw.arguments.len(), 3);
}

#[test]
fn demo_tap_publish_artifact_resolves_registered_execution_target() {
    let agent_id = addr("0xa5");
    let skill_id = 181;
    let dag_id = addr("0xd5");
    let config = SkillConfig {
        name: "demo_agent".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        requirements: requirements(),
        interface_revision: InterfaceVersion::new(1),
    };
    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id)
        .expect("publish artifact");

    let registry = AgentRegistrySnapshot {
        id: addr("0x91"),
        agents: vec![AgentRecord {
            active: true,
            skills: MoveTable::new(addr("0x95"), 1),
        }],
        skills: vec![SkillRecordContext {
            agent_id,
            skill_id,
            record: SkillRecord {
                description: artifact.skill_name.as_bytes().to_vec(),
                active: true,
                dag_binding: SkillDagBinding::pinned(dag_id),
                requirements: artifact.requirements.clone(),
                current_interface_revision: artifact.interface_revision,
                scheduled_task_count: 0,
            },
        }],
        default_executor: None,
    };

    let target =
        nexus_sdk::types::resolve_active_tap_skill_execution_target(&registry, agent_id, skill_id)
            .expect("registered demo skill resolves");

    assert_eq!(*target.skill.dag_binding(), SkillDagBinding::pinned(dag_id));
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
    let dag = tx.object(sui::tx::ObjectInput::shared(addr("0xd1"), 1, false));
    let execution = tx.object(sui::tx::ObjectInput::shared(addr("0xe1"), 1, true));
    let leader_cap = tx.pure(&1_u64);

    nexus_sdk::transactions::dag::prepare_tool_result_submission_worksheet(
        &mut tx, &objects, dag, execution, leader_cap, 7,
    )
    .expect("workflow worksheet builder");

    let tx = finish_transaction(tx);
    assert_eq!(
        move_call(&tx, 0).function,
        nexus_sdk::idents::workflow::ExecutionSubmission::PREPARE_TOOL_RESULT_SUBMISSION_WORKSHEET
            .name
    );
}
