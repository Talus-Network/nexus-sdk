use {
    crate::{
        move_bindings::{
            interface::{
                agent::{Agent, AgentInnerV1, SkillDagBinding},
                meta_schema::{MetaSchema, PortSchema, ValueKind},
                witness::V1 as InterfaceWitnessV1,
            },
            scheduler::{
                task::{Task, TaskController, TaskInnerV1},
                witness::V1 as SchedulerWitnessV1,
            },
            sui_framework::clock::Clock,
            workflow::{
                execution::{DAGExecution, DAGExecutionInnerV1},
                witness::V1 as WorkflowWitnessV1,
            },
        },
        move_boundary,
        nexus::{
            client::NexusClient,
            crawler::Response,
            tap,
            workflow::{self, DagSnapshot},
        },
        scheduler::{
            Deadline,
            Occurrence,
            Recurrence,
            Schedule,
            ScheduleError,
            SchedulerError,
            StartTime,
            TaskFunding,
            TaskOperation,
            TaskSpec,
        },
        transactions::scheduler::{
            PreparedFunding,
            PreparedOccurrence,
            PreparedRecurrence,
            PreparedSchedule,
            PreparedTask,
            ResolvedAuthority,
        },
        types::{NexusContext, SharedRoot},
    },
    anyhow::Context as _,
    std::{collections::BTreeMap, sync::Arc},
};

pub(super) struct FetchedTask {
    pub(super) context: Arc<NexusContext>,
    pub(super) object: Response<TaskInnerV1>,
}

pub(super) struct ResolvedTask {
    pub(super) context: Arc<NexusContext>,
    pub(super) object: Response<TaskInnerV1>,
    pub(super) authority: ResolvedAuthority,
}

pub(crate) async fn prepare_task(
    client: &NexusClient,
    context: &NexusContext,
    task: &TaskSpec,
) -> Result<PreparedTask, SchedulerError> {
    let dag_id = preflight_task_inputs(client, context, task).await?;
    let dag = client
        .object_reference(dag_id)
        .await
        .map_err(SchedulerError::from)?;
    let sender = client.owner().map_err(SchedulerError::from)?;
    let agent = match task.operation().agent_id() {
        Some(agent_id) => Some(agent_input(client, context, agent_id).await?),
        None => None,
    };
    let funding = match task.funding() {
        TaskFunding::Address {
            prepay_amount_mist,
            refund_recipient,
        } => PreparedFunding::Address {
            prepay_amount_mist,
            refund_recipient: refund_recipient.unwrap_or(sender),
        },
        TaskFunding::Agent { prepay_amount_mist } if agent.is_some() => {
            PreparedFunding::Agent { prepay_amount_mist }
        }
        TaskFunding::Agent { .. } => {
            return Err(ScheduleError::IncompatibleFunding {
                message: "a default DAG Task cannot use Agent vault funding",
            }
            .into());
        }
    };

    Ok(PreparedTask {
        operation: task.operation().clone(),
        dag,
        agent,
        entry_group: task.entry_group().to_owned(),
        inputs: task.inputs().clone(),
        funding,
        occurrence_budget_mist: task.occurrence_budget_mist(),
        failure_policy: task.failure_policy(),
    })
}

pub(super) async fn preflight_task_inputs(
    client: &NexusClient,
    context: &NexusContext,
    task: &TaskSpec,
) -> Result<crate::sui::types::Address, SchedulerError> {
    task.validate()?;
    let dag_id = effective_task_dag_id(client, context, task.operation()).await?;
    let dag = workflow::fetch_dag_snapshot(client, context, dag_id)
        .await
        .with_context(|| format!("could not inspect Task DAG '{dag_id}'"))
        .map_err(SchedulerError::transport)?;
    validate_task_inputs(&dag, task.entry_group(), task.inputs())?;
    Ok(dag_id)
}

async fn effective_task_dag_id(
    client: &NexusClient,
    context: &NexusContext,
    operation: &TaskOperation,
) -> Result<crate::sui::types::Address, SchedulerError> {
    match operation {
        TaskOperation::DefaultDag { dag_id } => Ok(*dag_id),
        TaskOperation::AgentSkill {
            agent_id,
            skill_id,
            selected_dag,
            ..
        } => {
            let target = tap::fetch_configured_active_tap_skill_execution_target(
                client, context, *agent_id, *skill_id,
            )
            .await
            .with_context(|| {
                format!("could not resolve active Agent '{agent_id}' skill {skill_id}")
            })
            .map_err(SchedulerError::transport)?;
            resolve_agent_skill_dag(
                *agent_id,
                *skill_id,
                target.data.skill.dag_binding(),
                *selected_dag,
            )
        }
    }
}

fn resolve_agent_skill_dag(
    agent_id: crate::sui::types::Address,
    skill_id: u64,
    binding: &SkillDagBinding,
    selected_dag: Option<crate::sui::types::Address>,
) -> Result<crate::sui::types::Address, SchedulerError> {
    match (binding, selected_dag) {
        (SkillDagBinding::Pinned { dag_id }, None) => Ok(*dag_id),
        (SkillDagBinding::Pinned { dag_id }, Some(selected_dag)) => {
            Err(SchedulerError::PinnedSkillDagSelectionConflict {
                agent_id,
                skill_id,
                pinned_dag: *dag_id,
                selected_dag,
            })
        }
        (SkillDagBinding::RuntimeSelected, Some(selected_dag)) => Ok(selected_dag),
        (SkillDagBinding::RuntimeSelected, None) => {
            Err(SchedulerError::RuntimeSelectedSkillDagRequired { agent_id, skill_id })
        }
    }
}

fn validate_task_inputs(
    dag: &DagSnapshot,
    entry_group: &str,
    inputs: &crate::scheduler::TaskInputs,
) -> Result<(), SchedulerError> {
    let Some(expected) = dag.entry_groups.get(entry_group) else {
        return Err(SchedulerError::TaskEntryGroupNotFound {
            dag_id: dag.dag_id,
            entry_group: entry_group.to_owned(),
            available: dag.entry_groups.keys().cloned().collect(),
        });
    };
    let received = inputs
        .iter()
        .map(|(vertex, ports)| (vertex.clone(), ports.keys().cloned().collect::<Vec<_>>()))
        .collect::<BTreeMap<_, _>>();
    if expected != &received {
        return Err(SchedulerError::TaskInputsMismatch {
            dag_id: dag.dag_id,
            entry_group: entry_group.to_owned(),
            expected: input_shape(expected, "<value>"),
            received: input_shape(&received, "<provided>"),
        });
    }
    for (vertex, ports) in inputs {
        let schema = dag.vertex_meta_schemas.get(vertex).ok_or_else(|| {
            SchedulerError::InconsistentChainState {
                message: format!(
                    "DAG '{}' entry vertex '{}' has no fetched MetaSchema",
                    dag.dag_id, vertex
                ),
            }
        })?;
        for (port, value) in ports {
            let port_schema = schema
                .input_ports
                .iter()
                .find(|schema| schema.port_name == port.as_bytes())
                .ok_or_else(|| SchedulerError::InconsistentChainState {
                    message: format!(
                        "DAG '{}' entry input '{}.{}' is absent from its vertex MetaSchema",
                        dag.dag_id, vertex, port
                    ),
                })?;
            if !MetaSchema::conforms_port(port_schema, value) {
                return Err(SchedulerError::TaskInputSchemaMismatch {
                    dag_id: dag.dag_id,
                    vertex: vertex.clone(),
                    port: port.clone(),
                    expected: port_schema_shape(port_schema).into_boxed_str(),
                    received: nexus_data_shape(value).into_boxed_str(),
                });
            }
        }
    }
    Ok(())
}

fn port_schema_shape(schema: &PortSchema) -> String {
    format!(
        "{}/{}",
        if schema.is_many { "many" } else { "one" },
        match schema.value_kind {
            ValueKind::Object => "object",
            ValueKind::Data => "data",
        }
    )
}

fn nexus_data_shape(value: &crate::types::NexusData) -> String {
    let cardinality = if value.is_many() { "many" } else { "one" };
    let kind = if value.values().is_err() {
        "empty"
    } else if value.is_object() {
        "object"
    } else {
        "data"
    };
    format!("{cardinality}/{kind}")
}

fn input_shape(shape: &BTreeMap<String, Vec<String>>, placeholder: &str) -> String {
    let shape = shape
        .iter()
        .map(|(vertex, ports)| {
            let ports = ports
                .iter()
                .map(|port| (port.clone(), placeholder))
                .collect::<BTreeMap<_, _>>();
            (vertex, ports)
        })
        .collect::<BTreeMap<_, _>>();
    serde_json::to_string(&shape).expect("a string input shape always serializes")
}

pub(crate) async fn prepare_schedule(
    client: &NexusClient,
    schedule: &Schedule,
) -> Result<PreparedSchedule, SchedulerError> {
    schedule.validate()?;
    let clock_ms = if schedule_requires_clock(schedule) {
        Some(clock_timestamp_ms(client).await?)
    } else {
        None
    };
    let occurrences = schedule
        .occurrences()
        .iter()
        .map(|occurrence| resolve_occurrence(occurrence, clock_ms))
        .collect::<Result<Vec<_>, _>>()?;
    let recurrence = schedule
        .recurrence()
        .map(|recurrence| resolve_recurrence(recurrence, clock_ms))
        .transpose()?;

    Ok(PreparedSchedule::new(occurrences, recurrence))
}

pub(super) async fn prepare_occurrence(
    client: &NexusClient,
    occurrence: &Occurrence,
) -> Result<PreparedOccurrence, SchedulerError> {
    occurrence.validate()?;
    let clock_ms = if occurrence_requires_clock(occurrence) {
        Some(clock_timestamp_ms(client).await?)
    } else {
        None
    };
    resolve_occurrence(occurrence, clock_ms)
}

pub(super) async fn prepare_recurrence(
    client: &NexusClient,
    recurrence: &Recurrence,
) -> Result<PreparedRecurrence, SchedulerError> {
    recurrence.validate()?;
    let clock_ms = if occurrence_requires_clock(recurrence.first()) {
        Some(clock_timestamp_ms(client).await?)
    } else {
        None
    };
    resolve_recurrence(recurrence, clock_ms)
}

pub(super) async fn fetch_task(
    client: &NexusClient,
    task_id: crate::sui::types::Address,
) -> Result<FetchedTask, SchedulerError> {
    fetch_task_with_roots(client, task_id, &[]).await
}

pub(super) async fn fetch_task_with_roots(
    client: &NexusClient,
    task_id: crate::sui::types::Address,
    required_roots: &[SharedRoot],
) -> Result<FetchedTask, SchedulerError> {
    client
        .crawler()
        .get_optional_object::<Task>(task_id)
        .await
        .map_err(SchedulerError::transport)?
        .ok_or(SchedulerError::TaskNotFound { task_id })?;

    let context = if required_roots.is_empty() {
        client.context_for_object(task_id).await
    } else {
        client
            .context_for_object_with_roots(task_id, required_roots)
            .await
    }
    .map_err(SchedulerError::from)?;
    let object = client
        .state_resolver()
        .load_inner::<Task, SchedulerWitnessV1, TaskInnerV1>(task_id, &context)
        .await
        .map_err(SchedulerError::from)?;

    Ok(FetchedTask { context, object })
}

pub(super) async fn resolve_task(
    client: &NexusClient,
    task_id: crate::sui::types::Address,
    required_roots: &[SharedRoot],
) -> Result<ResolvedTask, SchedulerError> {
    let task = fetch_task_with_roots(client, task_id, required_roots).await?;
    let authority = resolve_authority(client, &task.context, &task.object).await?;
    Ok(ResolvedTask {
        context: task.context,
        object: task.object,
        authority,
    })
}

async fn resolve_authority(
    client: &NexusClient,
    context: &NexusContext,
    task: &Response<TaskInnerV1>,
) -> Result<ResolvedAuthority, SchedulerError> {
    match &task.data.controller {
        TaskController::Address { pos0 } => {
            let sender = client.owner().map_err(SchedulerError::from)?;
            if *pos0 != sender {
                return Err(SchedulerError::AuthorityUnavailable {
                    task_id: task.object_id,
                    message: format!(
                        "controller address '{pos0}' differs from active address '{sender}'"
                    ),
                });
            }
            Ok(ResolvedAuthority::Address)
        }
        TaskController::Agent { pos0 } => Ok(ResolvedAuthority::Agent(
            agent_input(client, context, pos0.bytes).await?,
        )),
    }
}

async fn agent_input(
    client: &NexusClient,
    context: &NexusContext,
    agent_id: crate::types::AgentId,
) -> Result<crate::transactions::agent_input::AgentInput, SchedulerError> {
    client
        .state_resolver()
        .validate_state_pair::<Agent, InterfaceWitnessV1, AgentInnerV1>(agent_id, context)
        .await
        .map_err(SchedulerError::from)?;
    let metadata = client
        .crawler()
        .get_object_metadata(agent_id)
        .await
        .map_err(SchedulerError::transport)?;
    tap::agent_input_from_metadata(&metadata).map_err(|error| SchedulerError::InvalidObject {
        object_id: agent_id,
        message: error.to_string(),
    })
}

pub(super) async fn fetch_execution(
    client: &NexusClient,
    context: &NexusContext,
    execution_id: crate::sui::types::Address,
) -> Result<Option<Response<DAGExecutionInnerV1>>, SchedulerError> {
    let Some(_) = client
        .crawler()
        .get_optional_object::<DAGExecution>(execution_id)
        .await
        .map_err(SchedulerError::transport)?
    else {
        return Ok(None);
    };
    let execution = client
        .state_resolver()
        .load_inner::<DAGExecution, WorkflowWitnessV1, DAGExecutionInnerV1>(execution_id, context)
        .await
        .map_err(SchedulerError::from)?;
    Ok(Some(execution))
}

async fn clock_timestamp_ms(client: &NexusClient) -> Result<u64, SchedulerError> {
    client
        .crawler()
        .get_object::<Clock>(move_boundary::CLOCK_OBJECT_ID)
        .await
        .context("could not read the Sui Clock")
        .map(|clock| clock.data.timestamp_ms)
        .map_err(SchedulerError::transport)
}

fn schedule_requires_clock(schedule: &Schedule) -> bool {
    schedule.occurrences().iter().any(occurrence_requires_clock)
        || schedule
            .recurrence()
            .is_some_and(|recurrence| occurrence_requires_clock(recurrence.first()))
}

fn occurrence_requires_clock(occurrence: &Occurrence) -> bool {
    !matches!(occurrence.start(), StartTime::At { .. })
        || matches!(occurrence.deadline(), Some(Deadline::AfterStart { .. }))
}

fn resolve_recurrence(
    recurrence: &Recurrence,
    clock_ms: Option<u64>,
) -> Result<PreparedRecurrence, SchedulerError> {
    Ok(PreparedRecurrence::new(
        resolve_occurrence(recurrence.first(), clock_ms)?,
        recurrence.interval_ms(),
        recurrence.occurrences(),
    ))
}

fn resolve_occurrence(
    occurrence: &Occurrence,
    clock_ms: Option<u64>,
) -> Result<PreparedOccurrence, SchedulerError> {
    let start_time_ms = match occurrence.start() {
        StartTime::At { timestamp_ms } => timestamp_ms,
        StartTime::Now => required_clock(clock_ms)?,
        StartTime::After { offset_ms } => {
            checked_time("occurrence start", required_clock(clock_ms)?, offset_ms)?
        }
    };
    let deadline_ms = match occurrence.deadline() {
        None => None,
        Some(Deadline::At { timestamp_ms }) => {
            if timestamp_ms < start_time_ms {
                return Err(ScheduleError::DeadlineBeforeStart {
                    start_time_ms,
                    deadline_ms: timestamp_ms,
                }
                .into());
            }
            Some(timestamp_ms)
        }
        Some(Deadline::AfterStart { offset_ms }) => Some(checked_time(
            "occurrence deadline",
            start_time_ms,
            offset_ms,
        )?),
    };

    Ok(PreparedOccurrence::new(
        start_time_ms,
        deadline_ms,
        occurrence.priority_fee_percentage(),
    ))
}

fn required_clock(clock_ms: Option<u64>) -> Result<u64, SchedulerError> {
    clock_ms.ok_or_else(|| SchedulerError::InconsistentChainState {
        message: "relative schedule preparation omitted its Clock snapshot".to_owned(),
    })
}

fn checked_time(field: &'static str, base_ms: u64, offset_ms: u64) -> Result<u64, SchedulerError> {
    base_ms.checked_add(offset_ms).ok_or_else(|| {
        ScheduleError::TimeOverflow {
            field,
            base_ms,
            offset_ms,
        }
        .into()
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                interface::{
                    dag::{DAGInnerV1, DAG},
                    graph,
                },
                move_std::option::Option as MoveOption,
                sui_framework::{
                    linked_table::LinkedTable,
                    object::UID,
                    table::Table,
                    vec_map::{Entry as VecMapEntry, VecMap},
                },
            },
            nexus::{client::AddressBalanceGas, workflow::DagSnapshot},
            scheduler::Occurrence,
            test_utils::sui_mocks,
            transactions::scheduler::{PreparedOccurrence, PreparedRecurrence, PreparedSchedule},
            types::DEFAULT_PRIORITY_FEE_PERCENTAGE,
        },
        std::collections::BTreeMap,
    };

    fn dag_snapshot() -> DagSnapshot {
        DagSnapshot {
            dag_id: crate::sui::types::Address::from_static("0xd"),
            vertex_count: 1,
            entry_groups: BTreeMap::from([(
                "_default_group".to_owned(),
                BTreeMap::from([(
                    "sum".to_owned(),
                    vec!["0".to_owned(), "1".to_owned(), "2".to_owned()],
                )]),
            )]),
            vertex_meta_schemas: BTreeMap::from([(
                "sum".to_owned(),
                MetaSchema::new(
                    ["0", "1", "2"]
                        .into_iter()
                        .map(|port| {
                            PortSchema::new(port.as_bytes().to_vec(), false, ValueKind::Data)
                        })
                        .collect(),
                    vec![
                        crate::move_bindings::interface::meta_schema::OutputVariantSchema::new(
                            b"ok".to_vec(),
                            vec![],
                        ),
                    ],
                ),
            )]),
        }
    }

    #[test]
    fn task_inputs_must_match_the_selected_dag_entry_group() {
        let dag = dag_snapshot();
        let empty = BTreeMap::new();

        let error = validate_task_inputs(&dag, "_default_group", &empty)
            .expect_err("missing entry inputs must fail before submission");

        assert!(matches!(
            error,
            SchedulerError::TaskInputsMismatch {
                dag_id,
                ref entry_group,
                ref expected,
                ref received,
            } if dag_id == dag.dag_id
                && entry_group == "_default_group"
                && expected == r#"{"sum":{"0":"<value>","1":"<value>","2":"<value>"}}"#
                && received == "{}"
        ));
    }

    #[test]
    fn task_inputs_reject_an_unknown_entry_group() {
        let dag = dag_snapshot();

        let error = validate_task_inputs(&dag, "missing", &BTreeMap::new())
            .expect_err("an unknown entry group must fail before submission");

        assert!(matches!(
            error,
            SchedulerError::TaskEntryGroupNotFound {
                dag_id,
                ref entry_group,
                ref available,
            } if dag_id == dag.dag_id
                && entry_group == "missing"
                && available == &["_default_group".to_owned()]
        ));
    }

    #[test]
    fn task_inputs_accept_the_exact_entry_shape() {
        let dag = dag_snapshot();
        let inputs = BTreeMap::from([(
            "sum".to_owned(),
            BTreeMap::from([
                (
                    "0".to_owned(),
                    crate::types::NexusData::inline_data(b"state").expect("fixture is bounded"),
                ),
                (
                    "1".to_owned(),
                    crate::types::NexusData::inline_data(b"20").expect("fixture is bounded"),
                ),
                (
                    "2".to_owned(),
                    crate::types::NexusData::inline_data(b"22").expect("fixture is bounded"),
                ),
            ]),
        )]);

        validate_task_inputs(&dag, "_default_group", &inputs)
            .expect("the exact entry shape is valid");
    }

    #[test]
    fn task_inputs_reject_wrong_meta_schema_kind_and_empty_many() {
        let mut dag = dag_snapshot();
        let schema = dag.vertex_meta_schemas.get_mut("sum").unwrap();
        schema.input_ports[0].value_kind = ValueKind::Object;
        let mut inputs = BTreeMap::from([(
            "sum".to_owned(),
            BTreeMap::from([
                (
                    "0".to_owned(),
                    crate::types::NexusData::inline_data(b"state").unwrap(),
                ),
                (
                    "1".to_owned(),
                    crate::types::NexusData::inline_data(b"20").unwrap(),
                ),
                (
                    "2".to_owned(),
                    crate::types::NexusData::inline_data(b"22").unwrap(),
                ),
            ]),
        )]);

        assert!(matches!(
            validate_task_inputs(&dag, "_default_group", &inputs),
            Err(SchedulerError::TaskInputSchemaMismatch {
                ref vertex,
                ref port,
                ref expected,
                ref received,
                ..
            }) if vertex == "sum"
                && port == "0"
                && expected.as_ref() == "one/object"
                && received.as_ref() == "one/data"
        ));

        let schema = dag.vertex_meta_schemas.get_mut("sum").unwrap();
        schema.input_ports[0].is_many = true;
        inputs.get_mut("sum").unwrap().insert(
            "0".to_owned(),
            crate::types::NexusData::Many { values: Vec::new() },
        );
        assert!(matches!(
            validate_task_inputs(&dag, "_default_group", &inputs),
            Err(SchedulerError::TaskInputSchemaMismatch {
                ref vertex,
                ref port,
                ref expected,
                ref received,
                ..
            }) if vertex == "sum"
                && port == "0"
                && expected.as_ref() == "many/object"
                && received.as_ref() == "many/empty"
        ));
    }

    #[test]
    fn effective_agent_skill_dag_enforces_pinned_and_runtime_selection() {
        let agent_id = crate::sui::types::Address::from_static("0xa");
        let pinned = crate::sui::types::Address::from_static("0xd");
        let selected = crate::sui::types::Address::from_static("0xe");

        assert_eq!(
            resolve_agent_skill_dag(agent_id, 7, &SkillDagBinding::pinned(pinned), None,).unwrap(),
            pinned
        );
        assert!(matches!(
            resolve_agent_skill_dag(
                agent_id,
                7,
                &SkillDagBinding::pinned(pinned),
                Some(selected),
            ),
            Err(SchedulerError::PinnedSkillDagSelectionConflict { .. })
        ));
        assert_eq!(
            resolve_agent_skill_dag(
                agent_id,
                7,
                &SkillDagBinding::RuntimeSelected,
                Some(selected),
            )
            .unwrap(),
            selected
        );
        assert!(matches!(
            resolve_agent_skill_dag(agent_id, 7, &SkillDagBinding::RuntimeSelected, None,),
            Err(SchedulerError::RuntimeSelectedSkillDagRequired { .. })
        ));
    }

    async fn test_client(
        mocks: sui_mocks::grpc::ServerMocks,
        objects: &crate::types::NexusObjects,
    ) -> NexusClient {
        let rpc_url = sui_mocks::grpc::mock_server(mocks);
        let private_key = crate::sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());

        NexusClient::builder()
            .with_private_key(private_key)
            .with_rpc_url(&rpc_url)
            .with_address_balance_gas_config(AddressBalanceGas::new(1_000))
            .with_nexus_objects(objects.clone())
            .build()
            .await
            .expect("client builds")
    }

    fn mock_dag(
        ledger: &mut sui_mocks::grpc::MockLedgerService,
        state_service: &mut sui_mocks::grpc::MockStateService,
        context: &NexusContext,
        dag_id: crate::sui::types::Address,
        entry_group: &str,
    ) {
        let dag_ref = sui_mocks::object_ref_for_id(dag_id);
        let dag = DAG::new(UID::new(dag_id));
        let state = DAGInnerV1::new(
            LinkedTable::new(dag_id, 0),
            VecMap {
                contents: vec![VecMapEntry {
                    key: graph::EntryGroup::new(entry_group),
                    value: VecMap { contents: vec![] },
                }],
            },
            Table::new(dag_id, 0),
            Table::new(dag_id, 0),
            Table::new(dag_id, 0),
            MoveOption::from_option(None::<graph::PostFailureAction>),
        );
        sui_mocks::grpc::mock_object_state::<DAG, InterfaceWitnessV1, DAGInnerV1>(
            ledger,
            state_service,
            context,
            dag_ref,
            crate::sui::types::Owner::Shared(1),
            dag,
            state,
        );
    }

    #[tokio::test]
    async fn mixed_schedule_uses_one_clock_snapshot() {
        let context = sui_mocks::mock_nexus_context();
        let clock_ms = 1_000;
        let clock_ref = sui_mocks::object_ref_for_id(move_boundary::CLOCK_OBJECT_ID);
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            clock_ref,
            crate::sui::types::Owner::Shared(1),
            bcs::to_bytes(&Clock::new(move_boundary::CLOCK_OBJECT_ID, clock_ms))
                .expect("Clock serializes"),
        );
        let client = test_client(
            sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service_mock),
                ..Default::default()
            },
            context.objects(),
        )
        .await;
        let recurrence = Recurrence::new(Occurrence::after_ms(30), 100)
            .expect("recurrence is valid")
            .finite(2)
            .expect("recurrence count is valid");
        let schedule = Schedule::new()
            .with_occurrence(Occurrence::now())
            .with_occurrence(Occurrence::after_ms(20).deadline_after_ms(5))
            .with_occurrence(Occurrence::at_ms(2_000))
            .with_recurrence(recurrence);

        let prepared = prepare_schedule(&client, &schedule)
            .await
            .expect("Schedule resolves");

        assert_eq!(
            prepared,
            PreparedSchedule::new(
                vec![
                    PreparedOccurrence::new(clock_ms, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                    PreparedOccurrence::new(
                        clock_ms + 20,
                        Some(clock_ms + 25),
                        DEFAULT_PRIORITY_FEE_PERCENTAGE,
                    ),
                    PreparedOccurrence::new(2_000, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                ],
                Some(PreparedRecurrence::new(
                    PreparedOccurrence::new(clock_ms + 30, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                    100,
                    Some(2),
                )),
            )
        );
    }

    #[tokio::test]
    async fn absolute_schedule_preparation_does_not_read_the_clock() {
        let context = sui_mocks::mock_nexus_context();
        let client = test_client(sui_mocks::grpc::ServerMocks::default(), context.objects()).await;
        let occurrence = Occurrence::at_ms(2_000)
            .deadline_at_ms(2_500)
            .expect("deadline follows start");
        let recurrence =
            Recurrence::new(Occurrence::at_ms(3_000), 200).expect("recurrence is valid");
        let schedule = Schedule::new()
            .with_occurrence(occurrence)
            .with_recurrence(recurrence.clone());

        assert_eq!(
            prepare_schedule(&client, &schedule)
                .await
                .expect("absolute schedule resolves"),
            PreparedSchedule::new(
                vec![PreparedOccurrence::new(
                    2_000,
                    Some(2_500),
                    DEFAULT_PRIORITY_FEE_PERCENTAGE,
                )],
                Some(PreparedRecurrence::new(
                    PreparedOccurrence::new(3_000, None, DEFAULT_PRIORITY_FEE_PERCENTAGE,),
                    200,
                    None,
                )),
            )
        );
        assert_eq!(
            prepare_occurrence(&client, &occurrence)
                .await
                .expect("absolute occurrence resolves"),
            PreparedOccurrence::new(2_000, Some(2_500), DEFAULT_PRIORITY_FEE_PERCENTAGE,)
        );
        assert_eq!(
            prepare_recurrence(&client, &recurrence)
                .await
                .expect("absolute recurrence resolves"),
            PreparedRecurrence::new(
                PreparedOccurrence::new(3_000, None, DEFAULT_PRIORITY_FEE_PERCENTAGE),
                200,
                None,
            )
        );
    }

    #[tokio::test]
    async fn address_funded_task_preparation_preserves_authored_values() {
        let dag_id = crate::sui::types::Address::from_static("0x31");
        let context = sui_mocks::mock_nexus_context();
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service_mock = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        mock_dag(
            &mut ledger_service_mock,
            &mut state_service_mock,
            &context,
            dag_id,
            "main",
        );
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service_mock,
            &mut package_service_mock,
            context.packages(),
        );
        let client = test_client(
            sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service_mock),
                package_service_mock: Some(package_service_mock),
                state_service_mock: Some(state_service_mock),
                ..Default::default()
            },
            context.objects(),
        )
        .await;
        let refund_recipient = crate::sui::types::Address::from_static("0x32");
        let task = TaskSpec::new(
            crate::scheduler::TaskOperation::default_dag(dag_id),
            "main",
            TaskFunding::address_with_refund(90, refund_recipient),
            30,
        )
        .expect("Task is valid")
        .with_failure_policy(crate::scheduler::FailurePolicy::Pause);

        let prepared = prepare_task(&client, &context, &task)
            .await
            .expect("Task preparation succeeds");

        assert!(matches!(
            prepared.operation,
            crate::scheduler::TaskOperation::DefaultDag {
                dag_id: stored_dag,
            } if stored_dag == dag_id
        ));
        assert!(prepared.agent.is_none());
        assert_eq!(prepared.entry_group, "main");
        assert!(prepared.inputs.is_empty());
        assert_eq!(
            prepared.funding,
            PreparedFunding::Address {
                prepay_amount_mist: 90,
                refund_recipient,
            }
        );
        assert_eq!(prepared.occurrence_budget_mist, 30);
        assert_eq!(
            prepared.failure_policy,
            crate::scheduler::FailurePolicy::Pause
        );
    }

    #[test]
    fn relative_time_resolution_reports_missing_clock_and_overflow() {
        assert!(matches!(
            resolve_occurrence(&Occurrence::now(), None),
            Err(SchedulerError::InconsistentChainState { .. })
        ));
        assert!(matches!(
            resolve_occurrence(&Occurrence::after_ms(1), Some(u64::MAX)),
            Err(SchedulerError::Schedule(ScheduleError::TimeOverflow {
                field: "occurrence start",
                ..
            }))
        ));
        assert!(matches!(
            resolve_occurrence(&Occurrence::at_ms(u64::MAX).deadline_after_ms(1), None,),
            Err(SchedulerError::Schedule(ScheduleError::TimeOverflow {
                field: "occurrence deadline",
                ..
            }))
        ));
        assert!(matches!(
            resolve_occurrence(
                &Occurrence::after_ms(10)
                    .deadline_at_ms(5)
                    .expect("relative start is resolved later"),
                Some(100),
            ),
            Err(SchedulerError::Schedule(
                ScheduleError::DeadlineBeforeStart {
                    start_time_ms: 110,
                    deadline_ms: 5,
                }
            ))
        ));
    }
}
