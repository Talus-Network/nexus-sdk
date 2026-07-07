#![cfg(all(feature = "full", feature = "test_utils"))]

use {
    nexus_sdk::{
        move_bindings::{
            interface::{
                agent::{
                    self as agent_binding,
                    FixedTool,
                    SkillDagBinding,
                    SkillRecurrenceKind,
                    SkillRequirement,
                    SkillSchedulePolicy,
                },
                graph::RuntimeVertex,
                payment::{self as payment_binding, PaymentSourceKind, SkillPaymentPolicy},
                version::InterfaceVersion,
            },
            primitives::event as event_binding,
            registry::agent_registry::{
                self as agent_registry_binding,
                AgentRecord,
                DefaultDagExecutor,
                SkillRecord,
            },
            struct_tag,
            sui_framework::table::Table as MoveTable,
            workflow::{
                execution_events::RequestWalkExecutionEvent,
                execution_submission as execution_submission_binding,
            },
        },
        sui,
        test_utils::ptb as move_boundary,
        types::{
            resolve_active_tap_skill_revision,
            AgentRegistrySnapshot,
            DefaultDagExecutorTarget,
            NexusObjects,
            SkillConfig,
            SkillRecordContext,
            SkillRevisionContext,
            SkillRevisionLookupError,
            SkillRevisionLookupKey,
        },
    },
    std::{path::PathBuf, str::FromStr},
};

fn generated_target(
    objects: &NexusObjects,
    target: impl FnOnce() -> Result<sui_move_call::CallTarget, sui_move_call::CallSpecError>,
) -> sui_move_call::CallTarget {
    let tx = move_boundary::ptb(objects, |tx| {
        tx.call_target(target, vec![])?;
        Ok(())
    })
    .expect("generated target is valid");
    let Some(sui::types::Command::MoveCall(call)) = tx.commands.first() else {
        panic!("expected generated target move call");
    };
    sui_move_call::CallTarget {
        package: call.package,
        module: call.module.clone(),
        function: call.function.clone(),
        type_arguments: call.type_arguments.clone(),
    }
}

fn generated_function(
    objects: &NexusObjects,
    target: impl FnOnce() -> Result<sui_move_call::CallTarget, sui_move_call::CallSpecError>,
) -> sui::types::Identifier {
    generated_target(objects, target).function
}

fn append_standard_runtime_worksheet(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    dag: sui::types::Argument,
    execution: sui::types::Argument,
    leader_cap: sui::types::Argument,
    walk_index: u64,
) -> anyhow::Result<sui::types::Argument> {
    let agent_registry_ref = tx.objects().agent_registry.clone();
    let leader_registry_ref = tx.objects().leader_registry.clone();
    let agent_registry = tx.shared_object(&agent_registry_ref, false)?;
    let leader_registry = tx.shared_object(&leader_registry_ref, false)?;
    let walk_index = tx.arg(&walk_index)?;
    let clock = tx.clock()?;

    tx.call_target(
        execution_submission_binding::prepare_tool_result_submission_worksheet_target,
        vec![
            dag,
            agent_registry,
            leader_registry,
            execution,
            leader_cap,
            walk_index,
            clock,
        ],
    )
}

fn tap_agent_registry_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    mutable: bool,
) -> anyhow::Result<sui::types::Argument> {
    let registry_ref = tx.objects().agent_registry.clone();
    Ok(tx.shared_object(&registry_ref, mutable)?)
}

fn tap_payment_policy_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    payment_policy: &SkillPaymentPolicy,
) -> anyhow::Result<sui::types::Argument> {
    match payment_policy {
        SkillPaymentPolicy::UserFunded => {
            tx.call_target(payment_binding::payment_policy_user_funded_target, vec![])
        }
        SkillPaymentPolicy::AgentFunded { max_budget } => {
            let max_budget = tx.arg(max_budget)?;
            tx.call_target(
                payment_binding::payment_policy_agent_funded_target,
                vec![max_budget],
            )
        }
    }
}

fn tap_schedule_policy_arg(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    schedule_policy: &SkillSchedulePolicy,
) -> anyhow::Result<sui::types::Argument> {
    let recurrence = match &schedule_policy.recurrence {
        SkillRecurrenceKind::Once => {
            tx.call_target(agent_binding::recurrence_once_target, vec![])?
        }
        SkillRecurrenceKind::Recursive {
            min_interval_ms,
            max_occurrences,
        } => {
            let min_interval_ms = tx.arg(min_interval_ms)?;
            let max_occurrences = match max_occurrences.as_option() {
                Some(value) => {
                    let value = tx.arg(value)?;
                    tx.option::<u64>(Some(value))?
                }
                None => tx.option::<u64>(None)?,
            };
            tx.call_target(
                agent_binding::recurrence_recursive_target,
                vec![min_interval_ms, max_occurrences],
            )?
        }
    };
    let allow_recursive = tx.arg(&schedule_policy.allow_recursive)?;

    tx.call_target(
        agent_binding::schedule_policy_target,
        vec![recurrence, allow_recursive],
    )
}

fn append_tap_register_skill(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    registry: sui::types::Argument,
    agent: sui::types::Argument,
    dag_id: sui::types::Address,
    description: Vec<u8>,
    input_commitment: Vec<u8>,
    payment_policy: &SkillPaymentPolicy,
    schedule_policy: &SkillSchedulePolicy,
) -> anyhow::Result<sui::types::Argument> {
    let payment_policy = tap_payment_policy_arg(tx, payment_policy)?;
    let schedule_policy = tap_schedule_policy_arg(tx, schedule_policy)?;
    let dag_id = tx.arg(&dag_id)?;
    let description = tx.arg(&description)?;
    let input_commitment = tx.arg(&input_commitment)?;

    tx.call_target(
        agent_registry_binding::register_skill_target,
        vec![
            registry,
            agent,
            dag_id,
            description,
            input_commitment,
            payment_policy,
            schedule_policy,
        ],
    )
}

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

fn object_id(address: sui::types::Address) -> nexus_sdk::move_bindings::sui_framework::object::ID {
    nexus_sdk::move_bindings::sui_framework::object::ID::new(address)
}

fn interface_version(inner: u64) -> nexus_sdk::move_bindings::interface::version::InterfaceVersion {
    nexus_sdk::move_bindings::interface::version::InterfaceVersion { inner }
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
            agent: nexus_sdk::move_bindings::interface::agent::Agent::from_ids(
                agent_id,
                1,
                Some(addr("0x91")),
            ),
            skill_id,
        }),
    }
}

fn expect_command_call(
    tx: &sui::types::ProgrammableTransaction,
    index: usize,
) -> &sui::types::MoveCall {
    match tx.commands.get(index) {
        Some(sui::types::Command::MoveCall(call)) => call,
        other => panic!("expected move call at {index}, got {other:?}"),
    }
}

fn move_calls(tx: &sui::types::ProgrammableTransaction) -> Vec<&sui::types::MoveCall> {
    tx.commands
        .iter()
        .filter_map(|command| match command {
            sui::types::Command::MoveCall(call) => Some(call),
            _ => None,
        })
        .collect()
}

fn wrap_event(objects: &NexusObjects, inner: sui::types::StructTag) -> sui::types::Event {
    let wrapper_template =
        struct_tag::<event_binding::EventWrapper<RequestWalkExecutionEvent>>(objects);
    let wrapper = sui::types::StructTag::new(
        *wrapper_template.address(),
        wrapper_template.module().clone(),
        wrapper_template.name().clone(),
        vec![sui::types::TypeTag::Struct(Box::new(inner))],
    );
    sui::types::Event {
        package_id: objects.primitives_pkg_id,
        module: wrapper.module().clone(),
        sender: addr("0xf3"),
        type_: wrapper,
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
        scheduled_task_id: nexus_sdk::move_bindings::move_std::option::Option::from_option(None),
        scheduled_occurrence_index: nexus_sdk::move_bindings::move_std::option::Option::from_option(
            None,
        ),
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
fn tap_payment_sources_validate_invoker_and_agent_vault_modes() {
    let agent_id = addr("0xa");
    let payer = addr("0x1");
    let invoker_source =
        nexus_sdk::types::payment_source_from_address(payer).expect("invoker source");
    let agent_vault_source =
        bcs::to_bytes(&PaymentSourceKind::agent_funded(agent_id)).expect("agent vault source");

    let decoded_invoker = bcs::from_bytes::<PaymentSourceKind>(&invoker_source)
        .expect("typed invoker source decodes");
    let decoded_vault = bcs::from_bytes::<PaymentSourceKind>(&agent_vault_source)
        .expect("typed vault source decodes");
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
            struct_tag::<AgentRecord>(&objects).module().clone(),
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
    let agent = object_ref("0xa1", 1, 0xa1);
    let tx = move_boundary::ptb(&objects, |tx| {
        let registry = tap_agent_registry_arg(tx, true).expect("configured registry");

        tx.call_target(
            agent_registry_binding::bootstrap_default_runtime_dag_skill_for_deployment_target,
            vec![registry],
        )
        .expect("deployment bootstrap builder");

        let registry = tap_agent_registry_arg(tx, true).expect("configured registry");
        let agent = tx.shared_object(&agent, true)?;
        let requirements = requirements();
        append_tap_register_skill(
            tx,
            registry,
            agent,
            addr("0xd1"),
            b"demo".to_vec(),
            requirements.input_commitment,
            &requirements.payment_policy,
            &requirements.schedule_policy,
        )
        .expect("register skill builder");

        Ok(())
    })
    .expect("transaction should build");
    let calls = move_calls(&tx);

    assert!(calls.iter().any(|call| {
        call.function
            == generated_function(
                &objects,
                agent_registry_binding::bootstrap_default_runtime_dag_skill_for_deployment_target,
            )
    }));
    assert!(calls.iter().any(|call| {
        call.function == generated_function(&objects, agent_registry_binding::register_skill_target)
    }));
    assert!(!calls
        .iter()
        .any(|call| call.function == sui::types::Identifier::from_static("worksheet")));
}

#[test]
fn update_skill_compatibility_builds_dag_and_policy_calls() {
    let objects = nexus_objects();
    let agent = object_ref("0xa1", 1, 0xa1);
    let tx = move_boundary::ptb(&objects, |tx| {
        let registry = tap_agent_registry_arg(tx, true).expect("configured registry");
        let agent_arg = tx.shared_object(&agent, true)?;
        let skill_id = tx.arg(&177_u64)?;
        let dag_id = tx.arg(&addr("0xd2"))?;
        tx.call_target(
            agent_registry_binding::update_dag_target,
            vec![registry, agent_arg, skill_id, dag_id],
        )
        .expect("update dag builder");

        let registry = tap_agent_registry_arg(tx, true).expect("configured registry");
        let agent_arg = tx.shared_object(&agent, true)?;
        let skill_id = tx.arg(&177_u64)?;
        let payment_policy =
            tap_payment_policy_arg(tx, &SkillPaymentPolicy::AgentFunded { max_budget: 100 })?;
        let schedule_policy = tap_schedule_policy_arg(
            tx,
            &SkillSchedulePolicy {
                recurrence: SkillRecurrenceKind::Recursive {
                    min_interval_ms: 5000,
                    max_occurrences:
                        nexus_sdk::move_bindings::move_std::option::Option::from_option(Some(3)),
                },
                allow_recursive: true,
            },
        )?;
        tx.call_target(
            agent_registry_binding::update_skill_policies_target,
            vec![
                registry,
                agent_arg,
                skill_id,
                payment_policy,
                schedule_policy,
            ],
        )
        .expect("update skill policies builder");

        Ok(())
    })
    .expect("transaction should build");
    let calls = move_calls(&tx);

    assert!(calls.iter().any(|call| {
        call.function == generated_function(&objects, agent_registry_binding::update_dag_target)
    }));
    assert!(calls.iter().any(|call| {
        call.function
            == generated_function(
                &objects,
                agent_registry_binding::update_skill_policies_target,
            )
    }));
    assert!(!calls
        .iter()
        .any(|call| call.function.as_str() == "update_skill"));
}

#[test]
fn demo_tap_publish_and_bind_lifecycle_ptb() {
    let objects = nexus_objects();
    let dag_id = addr("0xd5");
    let dag_ref = object_ref("0xd5", 1, 0xd5);
    let execution_ref = object_ref("0xe1", 1, 0xe1);
    let config = SkillConfig {
        name: "demo_agent".to_string(),
        dag_path: PathBuf::from("demo-dag.json"),
        requirements: requirements(),
        interface_revision: InterfaceVersion::new(1),
    };
    let artifact = nexus_sdk::types::TapPublishArtifact::from_config(&config, dag_id)
        .expect("publish artifact");
    let tx = move_boundary::ptb(&objects, |tx| {
        let registry = tap_agent_registry_arg(tx, true).expect("registry");
        let agent_object = tx
            .call_target(agent_registry_binding::create_agent_target, vec![registry])
            .expect("create agent");

        let registry = tap_agent_registry_arg(tx, true).expect("registry");
        append_tap_register_skill(
            tx,
            registry,
            agent_object,
            artifact.dag_id,
            artifact.skill_name.as_bytes().to_vec(),
            artifact.requirements.input_commitment.clone(),
            &artifact.requirements.payment_policy,
            &artifact.requirements.schedule_policy,
        )
        .expect("register skill");

        let dag = tx.shared_object(&dag_ref, false)?;
        let execution = tx.shared_object(&execution_ref, true)?;
        let leader_cap = tx.arg(&1_u64)?;
        append_standard_runtime_worksheet(tx, dag, execution, leader_cap, 0)
            .expect("workflow worksheet");

        Ok(())
    })
    .expect("transaction should build");
    let calls = move_calls(&tx);
    let find_call = |function: &sui::types::Identifier| {
        calls
            .iter()
            .position(|call| &call.function == function)
            .expect("expected lifecycle call")
    };

    let create_agent = find_call(&generated_function(
        &objects,
        agent_registry_binding::create_agent_target,
    ));
    let register_skill = find_call(&generated_function(
        &objects,
        agent_registry_binding::register_skill_target,
    ));
    let worksheet = find_call(&generated_function(
        &objects,
        execution_submission_binding::prepare_tool_result_submission_worksheet_target,
    ));

    assert!(create_agent < register_skill);
    assert!(register_skill < worksheet);
    assert_eq!(
        calls[worksheet].function,
        generated_function(
            &objects,
            execution_submission_binding::prepare_tool_result_submission_worksheet_target,
        )
    );
}

#[test]
fn agent_payment_vault_builders_target_tap_functions() {
    let objects = nexus_objects();
    let tx = move_boundary::ptb(&objects, |tx| {
        let registry = tap_agent_registry_arg(tx, false).expect("registry");
        let agent = tx.arg(&1_u64)?;
        let coin = tx.arg(&2_u64)?;

        tx.call_target(
            agent_binding::deposit_agent_payment_vault_target,
            vec![agent, coin],
        )
        .expect("deposit vault");
        let amount = tx.arg(&25_u64)?;
        tx.call_target(
            agent_registry_binding::withdraw_agent_payment_vault_target,
            vec![registry, agent, amount],
        )
        .expect("withdraw vault");

        Ok(())
    })
    .expect("transaction should build");
    let calls = move_calls(&tx);
    let deposit = calls
        .iter()
        .find(|call| {
            call.function
                == generated_function(&objects, agent_binding::deposit_agent_payment_vault_target)
        })
        .expect("deposit vault call");
    let withdraw = calls
        .iter()
        .find(|call| {
            call.function
                == generated_function(
                    &objects,
                    agent_registry_binding::withdraw_agent_payment_vault_target,
                )
        })
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
    let dag_ref = object_ref("0xd1", 1, 0xd1);
    let execution_ref = object_ref("0xe1", 1, 0xe1);
    let tx = move_boundary::ptb(&objects, |tx| {
        let dag = tx.shared_object(&dag_ref, false)?;
        let execution = tx.shared_object(&execution_ref, true)?;
        let leader_cap = tx.arg(&1_u64)?;

        append_standard_runtime_worksheet(tx, dag, execution, leader_cap, 7)
            .expect("workflow worksheet builder");

        Ok(())
    })
    .expect("transaction should build");
    assert_eq!(
        expect_command_call(&tx, 0).function,
        generated_function(
            &objects,
            execution_submission_binding::prepare_tool_result_submission_worksheet_target,
        )
    );
}
