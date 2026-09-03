use {
    crate::{prelude::*, workflow},
    nexus_sdk::{
        nexus::client::NexusClient,
        scheduler::{
            FailurePolicy,
            Occurrence,
            Recurrence,
            Schedule,
            TaskFunding,
            TaskOperation,
            TaskSpec,
        },
        types::{DEFAULT_ENTRY_GROUP, DEFAULT_PRIORITY_FEE_PERCENTAGE},
        walrus::StorageConf,
    },
};

/// Arguments that define the work and funding for one Task.
#[derive(Args, Clone, Debug)]
pub(crate) struct TaskArgs {
    #[command(flatten)]
    operation: OperationArgs,

    #[command(flatten)]
    funding: FundingArgs,

    #[arg(
        long,
        short = 'e',
        default_value = DEFAULT_ENTRY_GROUP,
        value_name = "NAME",
        help_heading = "Operation",
        help = "DAG entry group used by every occurrence"
    )]
    entry_group: String,

    #[arg(
        long,
        short = 'i',
        value_parser = ValueParser::from(parse_json_string),
        value_name = "JSON",
        help_heading = "Inputs",
        help = "Input JSON used by every occurrence"
    )]
    input_json: Option<serde_json::Value>,

    #[arg(
        long,
        short = 'r',
        value_delimiter = ',',
        value_name = "VERTEX.PORT",
        help_heading = "Inputs",
        help = "Input fields committed to configured remote storage"
    )]
    remote: Vec<String>,

    #[arg(
        long,
        value_name = "MIST",
        help_heading = "Funding",
        help = "MIST placed in the Task reserve; address funding uses the signer Sui address balance"
    )]
    prepay_amount_mist: u64,

    #[arg(
        long,
        value_name = "MIST",
        help_heading = "Funding",
        help = "Maximum MIST available to each occurrence"
    )]
    occurrence_budget_mist: u64,

    #[arg(
        long,
        help_heading = "Policy",
        help = "Pause the Task after an unsuccessful settlement"
    )]
    pause_on_failure: bool,
}

impl TaskArgs {
    pub(crate) async fn into_preparation(self) -> Result<TaskPreparation, NexusCliError> {
        let operation = self.operation.into_operation()?;
        let funding = self.funding.into_funding(self.prepay_amount_mist);
        let task = TaskSpec::new(
            operation,
            self.entry_group,
            funding,
            self.occurrence_budget_mist,
        )
        .map_err(NexusCliError::Schedule)?
        .with_failure_policy(if self.pause_on_failure {
            FailurePolicy::Pause
        } else {
            FailurePolicy::Continue
        });
        let (input_plan, storage_conf) = prepare_input_plan(self.input_json, self.remote).await?;
        Ok(TaskPreparation {
            task,
            input_plan,
            storage_conf,
        })
    }
}

/// Locally validated Task authoring state awaiting authoritative DAG preflight and upload.
#[derive(Debug)]
pub(crate) struct TaskPreparation {
    task: TaskSpec,
    input_plan: workflow::EntryPortPlan,
    storage_conf: StorageConf,
}

impl TaskPreparation {
    pub(crate) async fn materialize(
        self,
        client: &NexusClient,
        scheduler_package: sui::types::Address,
    ) -> Result<TaskSpec, NexusCliError> {
        let preflight = self.task.clone().with_inputs(self.input_plan.task_inputs());
        client
            .scheduler()
            .preflight_task_inputs(scheduler_package, &preflight)
            .await?;
        let inputs = self.input_plan.materialize(&self.storage_conf).await?;
        Ok(self.task.with_inputs(inputs))
    }
}

/// Arguments selecting the operation performed by every occurrence.
#[derive(Args, Clone, Debug)]
#[command(group = clap::ArgGroup::new("task_operation")
    .required(true)
    .multiple(true)
    .args(["dag_id", "agent_id"]))]
struct OperationArgs {
    #[arg(
        long,
        short = 'd',
        value_name = "OBJECT_ID",
        help_heading = "Operation",
        help = "Published DAG for default operation or optional Agent selection"
    )]
    dag_id: Option<sui::types::Address>,

    #[arg(
        long,
        value_name = "OBJECT_ID",
        requires = "skill_id",
        help_heading = "Operation",
        help = "Agent object whose registered skill performs the work; requires --skill-id"
    )]
    agent_id: Option<sui::types::Address>,

    #[arg(
        long,
        value_name = "U64",
        requires = "agent_id",
        help_heading = "Operation",
        help = "Registered Agent skill identifier; requires --agent-id"
    )]
    skill_id: Option<u64>,

    #[arg(
        long = "authorization-binding",
        value_name = "VERTEX=OBJECT_ID",
        requires = "agent_id",
        help_heading = "Operation",
        help = "Bind a DAG vertex that requires a grant to its recipient object; repeat for each vertex"
    )]
    authorization_bindings: Vec<String>,
}

impl OperationArgs {
    fn into_operation(self) -> Result<TaskOperation, NexusCliError> {
        match (self.agent_id, self.skill_id) {
            (None, None) => {
                if !self.authorization_bindings.is_empty() {
                    return Err(NexusCliError::Any(anyhow!(
                        "--authorization-binding requires --agent-id and --skill-id"
                    )));
                }
                self.dag_id.map(TaskOperation::default_dag).ok_or_else(|| {
                    NexusCliError::Any(anyhow!("--dag-id is required for the default operation"))
                })
            }
            (Some(agent_id), Some(skill_id)) => Ok(TaskOperation::agent_skill(
                agent_id,
                skill_id,
                self.dag_id,
                parse_authorization_bindings(self.authorization_bindings)?,
            )),
            _ => Err(NexusCliError::Any(anyhow!(
                "--agent-id and --skill-id must be supplied together"
            ))),
        }
    }
}

fn parse_authorization_bindings(
    values: Vec<String>,
) -> Result<nexus_sdk::scheduler::AuthorizationBindings, NexusCliError> {
    let mut bindings = nexus_sdk::scheduler::AuthorizationBindings::new();
    for value in values {
        let (vertex, recipient) = value.split_once('=').ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "invalid authorization binding '{value}': expected '<VERTEX>=<OBJECT_ID>'"
            ))
        })?;
        if vertex.trim().is_empty() {
            return Err(NexusCliError::Any(anyhow!(
                "invalid authorization binding '{value}': vertex must not be empty"
            )));
        }
        let recipient = recipient.parse().map_err(|error| {
            NexusCliError::Any(anyhow!(
                "invalid authorization binding object ID '{recipient}': {error}"
            ))
        })?;
        if bindings.insert(vertex.to_owned(), recipient).is_some() {
            return Err(NexusCliError::Any(anyhow!(
                "authorization binding for vertex '{vertex}' was provided more than once"
            )));
        }
    }
    Ok(bindings)
}

/// Arguments selecting the Task reserve owner and controller.
#[derive(Args, Clone, Debug)]
struct FundingArgs {
    #[arg(
        long,
        requires = "agent_id",
        conflicts_with = "refund_recipient",
        help_heading = "Funding",
        help = "Use the selected Agent vault and Agent control"
    )]
    agent_funded: bool,

    #[arg(
        long,
        value_name = "ADDRESS",
        conflicts_with = "agent_funded",
        help_heading = "Funding",
        help = "Recipient of unused address funded reserve"
    )]
    refund_recipient: Option<sui::types::Address>,
}

impl FundingArgs {
    fn into_funding(self, prepay_amount_mist: u64) -> TaskFunding {
        if self.agent_funded {
            TaskFunding::agent(prepay_amount_mist)
        } else {
            self.refund_recipient.map_or_else(
                || TaskFunding::address(prepay_amount_mist),
                |recipient| TaskFunding::address_with_refund(prepay_amount_mist, recipient),
            )
        }
    }
}

/// Arguments that form a complete nonempty Schedule.
#[derive(Args, Clone, Debug)]
#[group(
    id = "schedule_source",
    required = true,
    multiple = true,
    args = [
        "schedule_file",
        "now",
        "at_ms",
        "after_ms",
        "recurrence_interval_ms"
    ]
)]
#[command(group = clap::ArgGroup::new("standalone_source")
    .args(["now", "at_ms", "after_ms"])
    .multiple(true))]
pub(crate) struct ScheduleArgs {
    #[arg(
        long,
        value_name = "PATH",
        value_parser = ValueParser::from(expand_tilde),
        conflicts_with_all = [
            "now",
            "at_ms",
            "after_ms",
            "deadline_at_ms",
            "deadline_after_ms",
            "priority_fee_percentage",
            "recurrence_interval_ms",
            "recurrence_at_ms",
            "recurrence_after_ms",
            "recurrence_occurrences",
            "recurrence_deadline_at_ms",
            "recurrence_deadline_after_ms",
            "recurrence_priority_fee_percentage"
        ],
        help_heading = "Schedule",
        help = "Read the complete Schedule from a JSON file"
    )]
    schedule_file: Option<PathBuf>,

    #[arg(
        long,
        help_heading = "Timing",
        help = "Add one occurrence at the current Sui Clock time"
    )]
    now: bool,

    #[arg(
        long,
        value_name = "MILLIS",
        help_heading = "Timing",
        help = "Add an occurrence at an absolute millisecond timestamp"
    )]
    at_ms: Vec<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        help_heading = "Timing",
        help = "Add an occurrence after an offset from one Sui Clock read"
    )]
    after_ms: Vec<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        requires = "standalone_source",
        conflicts_with = "deadline_after_ms",
        help_heading = "Timing",
        help = "Use one absolute deadline for inline standalone occurrences"
    )]
    deadline_at_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        requires = "standalone_source",
        conflicts_with = "deadline_at_ms",
        help_heading = "Timing",
        help = "Set each standalone deadline from its resolved start"
    )]
    deadline_after_ms: Option<u64>,

    #[arg(
        long,
        value_name = "PERCENTAGE",
        requires = "standalone_source",
        help_heading = "Timing",
        help = "Set the priority fee for inline standalone occurrences"
    )]
    priority_fee_percentage: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        help_heading = "Recurrence",
        help = "Add a recurrence with this millisecond interval"
    )]
    recurrence_interval_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        requires = "recurrence_interval_ms",
        conflicts_with = "recurrence_after_ms",
        help_heading = "Recurrence",
        help = "Start the recurrence at an absolute timestamp"
    )]
    recurrence_at_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        requires = "recurrence_interval_ms",
        conflicts_with = "recurrence_at_ms",
        help_heading = "Recurrence",
        help = "Start the recurrence after an offset from the Sui Clock"
    )]
    recurrence_after_ms: Option<u64>,

    #[arg(
        long,
        value_name = "COUNT",
        requires = "recurrence_interval_ms",
        help_heading = "Recurrence",
        help = "Limit the total recurring occurrences"
    )]
    recurrence_occurrences: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        requires = "recurrence_interval_ms",
        conflicts_with = "recurrence_deadline_after_ms",
        help_heading = "Recurrence",
        help = "Use one absolute deadline for the first recurring occurrence"
    )]
    recurrence_deadline_at_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        requires = "recurrence_interval_ms",
        conflicts_with = "recurrence_deadline_at_ms",
        help_heading = "Recurrence",
        help = "Set recurring deadlines from each resolved start"
    )]
    recurrence_deadline_after_ms: Option<u64>,

    #[arg(
        long,
        value_name = "PERCENTAGE",
        requires = "recurrence_interval_ms",
        help_heading = "Recurrence",
        help = "Set the priority fee for recurring occurrences"
    )]
    recurrence_priority_fee_percentage: Option<u64>,
}

impl ScheduleArgs {
    pub(crate) async fn into_schedule(self) -> Result<Schedule, NexusCliError> {
        if let Some(path) = self.schedule_file {
            let contents = tokio::fs::read_to_string(&path)
                .await
                .map_err(NexusCliError::Io)?;
            let schedule: Schedule = serde_json::from_str(&contents).map_err(|error| {
                NexusCliError::Any(anyhow!(
                    "could not parse Schedule file '{}': {error}",
                    path.display()
                ))
            })?;
            schedule
                .validate_for_task_creation()
                .map_err(NexusCliError::Schedule)?;
            return Ok(schedule);
        }

        let mut schedule = Schedule::new();
        if self.now {
            schedule = schedule.with_occurrence(self.apply_standalone(Occurrence::now())?);
        }
        for timestamp_ms in &self.at_ms {
            schedule =
                schedule.with_occurrence(self.apply_standalone(Occurrence::at_ms(*timestamp_ms))?);
        }
        for offset_ms in &self.after_ms {
            schedule =
                schedule.with_occurrence(self.apply_standalone(Occurrence::after_ms(*offset_ms))?);
        }
        if let Some(interval_ms) = self.recurrence_interval_ms {
            let first = match (self.recurrence_at_ms, self.recurrence_after_ms) {
                (Some(timestamp_ms), None) => Occurrence::at_ms(timestamp_ms),
                (None, Some(offset_ms)) => Occurrence::after_ms(offset_ms),
                (None, None) => Occurrence::now(),
                (Some(_), Some(_)) => {
                    return Err(NexusCliError::Any(anyhow!(
                        "recurrence start options are mutually exclusive"
                    )));
                }
            };
            let first = apply_occurrence_options(
                first,
                self.recurrence_deadline_at_ms,
                self.recurrence_deadline_after_ms,
                self.recurrence_priority_fee_percentage,
            )?;
            let recurrence =
                Recurrence::new(first, interval_ms).map_err(NexusCliError::Schedule)?;
            let recurrence = match self.recurrence_occurrences {
                Some(occurrences) => recurrence
                    .finite(occurrences)
                    .map_err(NexusCliError::Schedule)?,
                None => recurrence,
            };
            schedule = schedule.with_recurrence(recurrence);
        }
        schedule
            .validate_for_task_creation()
            .map_err(NexusCliError::Schedule)?;
        Ok(schedule)
    }

    fn apply_standalone(&self, occurrence: Occurrence) -> Result<Occurrence, NexusCliError> {
        apply_occurrence_options(
            occurrence,
            self.deadline_at_ms,
            self.deadline_after_ms,
            self.priority_fee_percentage,
        )
    }
}

/// Arguments defining one occurrence.
#[derive(Args, Clone, Debug)]
#[group(
    id = "occurrence_start",
    required = false,
    multiple = false,
    args = ["now", "at_ms", "after_ms"]
)]
pub(crate) struct OccurrenceArgs {
    #[arg(
        long,
        help_heading = "Timing",
        help = "Use the current Sui Clock time; this is the default"
    )]
    now: bool,

    #[arg(
        long,
        value_name = "MILLIS",
        help_heading = "Timing",
        help = "Use an absolute millisecond start timestamp"
    )]
    at_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        help_heading = "Timing",
        help = "Use an offset from the current Sui Clock"
    )]
    after_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        conflicts_with = "deadline_after_ms",
        help_heading = "Timing",
        help = "Use an absolute dispatch deadline"
    )]
    deadline_at_ms: Option<u64>,

    #[arg(
        long,
        value_name = "MILLIS",
        conflicts_with = "deadline_at_ms",
        help_heading = "Timing",
        help = "Set the deadline from the resolved start"
    )]
    deadline_after_ms: Option<u64>,

    #[arg(
        long,
        value_name = "PERCENTAGE",
        default_value_t = DEFAULT_PRIORITY_FEE_PERCENTAGE,
        help_heading = "Timing",
        help = "Set the dispatch priority fee percentage"
    )]
    priority_fee_percentage: u64,
}

impl Default for OccurrenceArgs {
    fn default() -> Self {
        Self {
            now: false,
            at_ms: None,
            after_ms: None,
            deadline_at_ms: None,
            deadline_after_ms: None,
            priority_fee_percentage: DEFAULT_PRIORITY_FEE_PERCENTAGE,
        }
    }
}

impl OccurrenceArgs {
    pub(crate) fn into_occurrence(self) -> Result<Occurrence, NexusCliError> {
        let occurrence = match (self.now, self.at_ms, self.after_ms) {
            (true, None, None) | (false, None, None) => Occurrence::now(),
            (false, Some(timestamp_ms), None) => Occurrence::at_ms(timestamp_ms),
            (false, None, Some(offset_ms)) => Occurrence::after_ms(offset_ms),
            _ => {
                return Err(NexusCliError::Any(anyhow!(
                    "occurrence start options are mutually exclusive"
                )));
            }
        };
        apply_occurrence_options(
            occurrence,
            self.deadline_at_ms,
            self.deadline_after_ms,
            Some(self.priority_fee_percentage),
        )
    }
}

/// Arguments defining one recurrence.
#[derive(Args, Clone, Debug)]
pub(crate) struct RecurrenceArgs {
    #[command(flatten)]
    first: OccurrenceArgs,

    #[arg(
        long,
        value_name = "MILLIS",
        help_heading = "Recurrence",
        help = "Interval between recurring occurrences"
    )]
    interval_ms: u64,

    #[arg(
        long,
        value_name = "COUNT",
        help_heading = "Recurrence",
        help = "Total recurring occurrences, omitted for no limit"
    )]
    occurrences: Option<u64>,
}

impl RecurrenceArgs {
    pub(crate) fn into_recurrence(self) -> Result<Recurrence, NexusCliError> {
        let recurrence = Recurrence::new(self.first.into_occurrence()?, self.interval_ms)
            .map_err(NexusCliError::Schedule)?;
        match self.occurrences {
            Some(occurrences) => recurrence
                .finite(occurrences)
                .map_err(NexusCliError::Schedule),
            None => Ok(recurrence),
        }
    }
}

fn apply_occurrence_options(
    mut occurrence: Occurrence,
    deadline_at_ms: Option<u64>,
    deadline_after_ms: Option<u64>,
    priority_fee_percentage: Option<u64>,
) -> Result<Occurrence, NexusCliError> {
    if let Some(timestamp_ms) = deadline_at_ms {
        occurrence = occurrence
            .deadline_at_ms(timestamp_ms)
            .map_err(NexusCliError::Schedule)?;
    }
    if let Some(offset_ms) = deadline_after_ms {
        occurrence = occurrence.deadline_after_ms(offset_ms);
    }
    if let Some(percentage) = priority_fee_percentage {
        occurrence = occurrence
            .with_priority_fee_percentage(percentage)
            .map_err(NexusCliError::Schedule)?;
    }
    Ok(occurrence)
}

async fn prepare_input_plan(
    input_json: Option<serde_json::Value>,
    remote: Vec<String>,
) -> Result<(workflow::EntryPortPlan, StorageConf), NexusCliError> {
    let conf = CliConf::load().await.unwrap_or_default();
    let input_json = input_json.unwrap_or_else(|| serde_json::json!({}));
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf: StorageConf = conf.data_storage.clone().into();
    let input_plan = workflow::EntryPortPlan::new(
        &input_json,
        preferred_remote_storage,
        &remote,
        &storage_conf,
    )?;
    conf.save().await.map_err(NexusCliError::Any)?;
    Ok((input_plan, storage_conf))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        mockito::Server,
        nexus_sdk::{
            move_bindings::{
                interface::{
                    dag::{DAGInnerV1, DAG},
                    era::V1 as InterfaceWitnessV1,
                    graph::{self, VertexInfo, VertexKind},
                    meta_schema::{MetaSchema, PortSchema, ValueKind},
                    verifier::ToolVerifierMode,
                },
                move_std::option::Option as MoveOption,
                registry::{
                    agent_registry::{AgentRegistry, AgentRegistryInnerV1},
                    era::V1 as RegistryWitnessV1,
                    leader::{LeaderRegistry, LeaderRegistryInnerV1},
                },
                sui_framework::{
                    linked_table::{LinkedTable, Node},
                    object::{ID, UID},
                    table::Table,
                    vec_map::{Entry as VecMapEntry, VecMap},
                    vec_set::VecSet,
                },
                tool::{
                    era::V1 as ToolWitnessV1,
                    tool_registry::{ToolRegistry, ToolRegistryInnerV1},
                },
            },
            scheduler::{ScheduleError, SchedulerError},
            test_utils::{nexus_mocks, sui_mocks},
            types::{NexusContext, PackageRole},
            walrus::{BlobObject, BlobStorage, NewlyCreated, StorageInfo},
        },
        std::collections::BTreeMap,
    };

    const BLOB_ID_A: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    fn mock_dag(
        ledger: &mut sui_mocks::grpc::MockLedgerService,
        state_service: &mut sui_mocks::grpc::MockStateService,
        context: &NexusContext,
        dag_id: sui::types::Address,
        entry_group: &str,
        ports: &[(&str, &str, ValueKind)],
    ) {
        let vertices_id = sui::types::Address::from_static("0xf0");
        let dag = DAG::new(UID::new(dag_id));
        let ports_by_vertex = ports.iter().fold(
            BTreeMap::<String, Vec<(String, ValueKind)>>::new(),
            |mut vertices, (vertex, port, kind)| {
                vertices
                    .entry((*vertex).to_owned())
                    .or_default()
                    .push(((*port).to_owned(), *kind));
                vertices
            },
        );
        let entry_ports = ports
            .iter()
            .map(|(vertex, port, _)| VecMapEntry {
                key: graph::Vertex::new(*vertex),
                value: VecSet {
                    contents: vec![graph::InputPort::new(*port)],
                },
            })
            .collect();
        let state = DAGInnerV1::new(
            true,
            LinkedTable::new(vertices_id, ports_by_vertex.len() as u64),
            VecMap {
                contents: vec![VecMapEntry {
                    key: graph::EntryGroup::new(entry_group),
                    value: VecMap {
                        contents: entry_ports,
                    },
                }],
            },
            Table::new(dag_id, 0),
            Table::new(dag_id, 0),
            Table::new(dag_id, 0),
            MoveOption::from_option(None::<graph::PostFailureAction>),
        );
        let vertex_fields = ports_by_vertex
            .into_iter()
            .enumerate()
            .map(|(index, (vertex, ports))| {
                let vertex = graph::Vertex::new(vertex);
                let field_id = sui::types::Address::new([(index + 1) as u8; 32]);
                let input_ports = ports
                    .iter()
                    .map(|(port, _)| graph::InputPort::new(port.as_str()))
                    .collect::<Vec<_>>();
                let meta_schema = MetaSchema::new(
                    ports
                        .iter()
                        .map(|(port, kind)| PortSchema::new(port.as_bytes().to_vec(), false, *kind))
                        .collect(),
                    vec![],
                );
                let node = Node {
                    prev: MoveOption::from_option(None::<graph::Vertex>),
                    next: MoveOption::from_option(None::<graph::Vertex>),
                    value: VertexInfo {
                        kind: VertexKind::OffChain {
                            tool_fqn: "test::fixture".to_owned().into(),
                        },
                        input_ports: VecSet {
                            contents: input_ports,
                        },
                        post_failure_action: MoveOption::from_option(
                            None::<graph::PostFailureAction>,
                        ),
                        tool_id: ID::new(sui::types::Address::from_static("0xf1")),
                        meta_schema: MoveOption::from_option(Some(meta_schema)),
                        verifier_mode: ToolVerifierMode::None,
                    },
                };
                (vertex, field_id, node)
            })
            .collect::<Vec<_>>();
        sui_mocks::grpc::mock_list_dynamic_fields_for(
            state_service,
            vertices_id,
            vertex_fields
                .iter()
                .map(|(vertex, field_id, _)| (vertex.clone(), *field_id))
                .collect(),
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            ledger,
            vertex_fields
                .into_iter()
                .map(|(vertex, field_id, node)| {
                    (
                        sui_mocks::object_ref_for_id(field_id),
                        sui::types::Owner::Object(vertices_id),
                        vertex,
                        node,
                    )
                })
                .collect(),
        );
        sui_mocks::grpc::mock_object_state::<DAG, InterfaceWitnessV1, DAGInnerV1>(
            ledger,
            state_service,
            context,
            sui_mocks::object_ref_for_id(dag_id),
            sui::types::Owner::Shared(1),
            dag,
            state,
        );
    }

    fn mock_creator_boundary(
        ledger: &mut sui_mocks::grpc::MockLedgerService,
        package_service: &mut sui_mocks::grpc::MockMovePackageService,
        state_service: &mut sui_mocks::grpc::MockStateService,
        context: &NexusContext,
    ) {
        sui_mocks::grpc::mock_nexus_package_graph(ledger, package_service, context.packages());

        let objects = context.objects();
        let agent_registry = objects.agent_registry.object_id();
        sui_mocks::grpc::mock_object_state_observation::<
            AgentRegistry,
            RegistryWitnessV1,
            AgentRegistryInnerV1,
        >(
            ledger,
            state_service,
            context,
            sui_mocks::object_ref_for_id(agent_registry),
            sui::types::Owner::Shared(objects.agent_registry.initial_shared_version),
            AgentRegistry::new(UID::new(agent_registry)),
        );

        let leader_registry = objects.leader_registry.object_id();
        sui_mocks::grpc::mock_object_state_observation::<
            LeaderRegistry,
            RegistryWitnessV1,
            LeaderRegistryInnerV1,
        >(
            ledger,
            state_service,
            context,
            sui_mocks::object_ref_for_id(leader_registry),
            sui::types::Owner::Shared(objects.leader_registry.initial_shared_version),
            LeaderRegistry::new(UID::new(leader_registry)),
        );

        let tool_registry = objects.tool_registry.object_id();
        sui_mocks::grpc::mock_object_state_observation::<
            ToolRegistry,
            ToolWitnessV1,
            ToolRegistryInnerV1,
        >(
            ledger,
            state_service,
            context,
            sui_mocks::object_ref_for_id(tool_registry),
            sui::types::Owner::Shared(objects.tool_registry.initial_shared_version),
            ToolRegistry::new(UID::new(tool_registry)),
        );
    }

    fn task_preparation(
        dag_id: sui::types::Address,
        input_plan: workflow::EntryPortPlan,
        storage_conf: StorageConf,
    ) -> TaskPreparation {
        TaskPreparation {
            task: TaskSpec::new(
                TaskOperation::default_dag(dag_id),
                "main",
                TaskFunding::address(1),
                1,
            )
            .expect("Task fixture is valid"),
            input_plan,
            storage_conf,
        }
    }

    #[test]
    fn occurrence_defaults_to_current_chain_time() {
        let occurrence = OccurrenceArgs::default()
            .into_occurrence()
            .expect("default occurrence is valid");
        let schedule = Schedule::new().with_occurrence(occurrence);
        assert_eq!(schedule.occurrences().len(), 1);
    }

    #[test]
    fn zero_recurrence_interval_is_rejected() {
        let error = Recurrence::new(Occurrence::now(), 0).expect_err("zero interval must fail");
        assert_eq!(error, ScheduleError::ZeroRecurrenceInterval);
    }

    #[tokio::test]
    async fn task_spec_validation_precedes_input_preparation() {
        let args = TaskArgs {
            operation: OperationArgs {
                dag_id: Some(sui::types::Address::from_static("0xd")),
                agent_id: None,
                skill_id: None,
                authorization_bindings: Vec::new(),
            },
            funding: FundingArgs {
                agent_funded: false,
                refund_recipient: None,
            },
            entry_group: DEFAULT_ENTRY_GROUP.to_owned(),
            input_json: Some(serde_json::json!("not an input object")),
            remote: Vec::new(),
            prepay_amount_mist: 0,
            occurrence_budget_mist: 0,
            pause_on_failure: false,
        };

        let error = args
            .into_preparation()
            .await
            .expect_err("zero budget must fail first");

        assert!(matches!(
            error,
            NexusCliError::Schedule(ScheduleError::ZeroOccurrenceBudget)
        ));
    }

    #[test]
    fn authorization_bindings_parse_vertex_recipients() {
        let recipient = sui::types::Address::from_static("0x42");
        let bindings = parse_authorization_bindings(vec![format!("check_message={recipient}")])
            .expect("binding is valid");

        assert_eq!(bindings.get("check_message"), Some(&recipient));
    }

    #[test]
    fn authorization_bindings_reject_duplicate_vertices() {
        let error = parse_authorization_bindings(vec![
            "check_message=0x42".to_owned(),
            "check_message=0x43".to_owned(),
        ])
        .expect_err("duplicate vertex must fail");

        assert!(error.to_string().contains("provided more than once"));
    }

    #[test]
    fn authorization_bindings_require_vertex_and_object_id() {
        for value in ["check_message", "=0x42", "check_message=invalid"] {
            let error = parse_authorization_bindings(vec![value.to_owned()])
                .expect_err("invalid binding must fail");
            assert!(error.to_string().contains("authorization binding"));
        }
    }

    #[tokio::test]
    async fn authoritative_dag_mismatch_precedes_walrus_requests() {
        let mut walrus = Server::new_async().await;
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(walrus.url()),
            walrus_aggregator_url: Some(walrus.url()),
            walrus_save_for_epochs: Some(2),
        };
        let put = walrus
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(0)
            .create_async()
            .await;
        let get = walrus
            .mock("GET", "/v1/blobs/json_blob_id")
            .expect(0)
            .create_async()
            .await;
        let plan = workflow::EntryPortPlan::new(
            &serde_json::json!({ "sum": { "right": "value" } }),
            None,
            &["sum.right".to_owned()],
            &storage_conf,
        )
        .expect("local preparation succeeds");
        let context = sui_mocks::mock_nexus_context();
        let dag_id = sui::types::Address::from_static("0xd");
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_creator_boundary(
            &mut ledger,
            &mut package_service,
            &mut state_service,
            &context,
        );
        mock_dag(
            &mut ledger,
            &mut state_service,
            &context,
            dag_id,
            "main",
            &[("sum", "left", ValueKind::Data)],
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;
        let scheduler_package = context.package_id(PackageRole::Scheduler).unwrap();

        let error = task_preparation(dag_id, plan, storage_conf)
            .materialize(&client, scheduler_package)
            .await
            .expect_err("authoritative DAG mismatch must fail");

        assert!(matches!(
            error,
            NexusCliError::Scheduler(SchedulerError::TaskInputsMismatch { .. })
        ));
        put.assert_async().await;
        get.assert_async().await;
    }

    #[tokio::test]
    async fn authoritative_schema_mismatch_precedes_walrus_requests() {
        let mut walrus = Server::new_async().await;
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(walrus.url()),
            walrus_aggregator_url: Some(walrus.url()),
            walrus_save_for_epochs: Some(2),
        };
        let put = walrus
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(0)
            .create_async()
            .await;
        let get = walrus
            .mock("GET", "/v1/blobs/json_blob_id")
            .expect(0)
            .create_async()
            .await;
        let plan = workflow::EntryPortPlan::new(
            &serde_json::json!({ "sum": { "right": "value" } }),
            None,
            &["sum.right".to_owned()],
            &storage_conf,
        )
        .expect("local preparation succeeds");
        let context = sui_mocks::mock_nexus_context();
        let dag_id = sui::types::Address::from_static("0xd");
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_creator_boundary(
            &mut ledger,
            &mut package_service,
            &mut state_service,
            &context,
        );
        mock_dag(
            &mut ledger,
            &mut state_service,
            &context,
            dag_id,
            "main",
            &[("sum", "right", ValueKind::Object)],
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;
        let scheduler_package = context.package_id(PackageRole::Scheduler).unwrap();

        let error = task_preparation(dag_id, plan, storage_conf)
            .materialize(&client, scheduler_package)
            .await
            .expect_err("authoritative schema mismatch must fail");

        assert!(matches!(
            error,
            NexusCliError::Scheduler(SchedulerError::TaskInputSchemaMismatch { .. })
        ));
        put.assert_async().await;
        get.assert_async().await;
    }

    #[tokio::test]
    async fn empty_many_input_precedes_walrus_requests() {
        let mut walrus = Server::new_async().await;
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(walrus.url()),
            walrus_aggregator_url: Some(walrus.url()),
            walrus_save_for_epochs: Some(2),
        };
        let put = walrus
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(0)
            .create_async()
            .await;
        let get = walrus
            .mock("GET", "/v1/blobs/empty")
            .expect(0)
            .create_async()
            .await;

        let error = workflow::EntryPortPlan::new(
            &serde_json::json!({ "sum": { "values": [] } }),
            None,
            &["sum.values".to_owned()],
            &storage_conf,
        )
        .expect_err("empty Many must fail before remote materialization");

        assert!(error.to_string().contains("requires at least one value"));
        put.assert_async().await;
        get.assert_async().await;
    }

    #[tokio::test]
    async fn pinned_skill_selection_conflict_precedes_walrus_requests() {
        let mut walrus = Server::new_async().await;
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(walrus.url()),
            walrus_aggregator_url: Some(walrus.url()),
            walrus_save_for_epochs: Some(2),
        };
        let put = walrus
            .mock("PUT", "/v1/blobs?epochs=2")
            .expect(0)
            .create_async()
            .await;
        let get = walrus
            .mock("GET", "/v1/blobs/json_blob_id")
            .expect(0)
            .create_async()
            .await;
        let plan = workflow::EntryPortPlan::new(
            &serde_json::json!({ "sum": { "right": "value" } }),
            None,
            &["sum.right".to_owned()],
            &storage_conf,
        )
        .expect("local preparation succeeds");
        let agent_id = sui::types::Address::from_static("0xa");
        let pinned_dag = sui::types::Address::from_static("0xd");
        let selected_dag = sui::types::Address::from_static("0xe");
        let client = nexus_mocks::mock_agent_skill_client_without_coins(
            agent_id,
            11,
            nexus_sdk::move_bindings::interface::agent::SkillDagBinding::pinned(pinned_dag),
        )
        .await;
        let task = TaskSpec::new(
            TaskOperation::agent_skill(agent_id, 11, Some(selected_dag), Default::default()),
            "main",
            TaskFunding::address(1),
            1,
        )
        .expect("Task fixture is valid");

        let error = TaskPreparation {
            task,
            input_plan: plan,
            storage_conf,
        }
        .materialize(&client, sui::types::Address::from_static("0xa5"))
        .await
        .expect_err("a caller-selected DAG must conflict with a pinned skill");

        assert!(
            matches!(
                error,
                NexusCliError::Scheduler(SchedulerError::PinnedSkillDagSelectionConflict { .. })
            ),
            "unexpected preflight error: {error:?}"
        );
        put.assert_async().await;
        get.assert_async().await;
    }

    #[tokio::test]
    async fn authoritative_dag_match_allows_walrus_materialization() {
        let mut walrus = Server::new_async().await;
        let storage_conf = StorageConf {
            walrus_publisher_url: Some(walrus.url()),
            walrus_aggregator_url: Some(walrus.url()),
            walrus_save_for_epochs: Some(2),
        };
        let upload = StorageInfo {
            newly_created: Some(NewlyCreated {
                blob_object: BlobObject {
                    blob_id: BLOB_ID_A.to_owned(),
                    id: "json_object_id".to_owned(),
                    storage: BlobStorage { end_epoch: 200 },
                },
            }),
            already_certified: None,
        };
        let put = walrus
            .mock("PUT", "/v1/blobs?epochs=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&upload).expect("upload response serializes"))
            .create_async()
            .await;
        let get = walrus
            .mock("GET", format!("/v1/blobs/{BLOB_ID_A}").as_str())
            .with_status(200)
            .with_body(br#""value""#)
            .create_async()
            .await;
        let plan = workflow::EntryPortPlan::new(
            &serde_json::json!({ "sum": { "right": "value" } }),
            None,
            &["sum.right".to_owned()],
            &storage_conf,
        )
        .expect("local preparation succeeds");
        let context = sui_mocks::mock_nexus_context();
        let dag_id = sui::types::Address::from_static("0xd");
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_creator_boundary(
            &mut ledger,
            &mut package_service,
            &mut state_service,
            &context,
        );
        mock_dag(
            &mut ledger,
            &mut state_service,
            &context,
            dag_id,
            "main",
            &[("sum", "right", ValueKind::Data)],
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client =
            nexus_mocks::mock_nexus_client_without_coins(context.objects(), &rpc_url).await;
        let scheduler_package = context.package_id(PackageRole::Scheduler).unwrap();

        let task = task_preparation(dag_id, plan, storage_conf)
            .materialize(&client, scheduler_package)
            .await
            .expect("matching authoritative DAG permits materialization");

        assert!(task.inputs()["sum"]["right"].has_walrus());
        put.assert_async().await;
        get.assert_async().await;
    }
}
