use {
    crate::{prelude::*, workflow},
    nexus_sdk::{
        scheduler::{
            FailurePolicy,
            Occurrence,
            Recurrence,
            Schedule,
            TaskFunding,
            TaskInputs,
            TaskOperation,
            TaskSpec,
        },
        types::{DEFAULT_ENTRY_GROUP, DEFAULT_PRIORITY_FEE_PERCENTAGE},
        walrus::StorageConf,
    },
    std::collections::BTreeMap,
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
    pub(crate) async fn into_spec(self) -> Result<TaskSpec, NexusCliError> {
        let operation = self.operation.into_operation()?;
        let funding = self.funding.into_funding(self.prepay_amount_mist);
        let inputs = prepare_inputs(self.input_json, self.remote).await?;
        TaskSpec::new(
            operation,
            self.entry_group,
            funding,
            self.occurrence_budget_mist,
        )
        .map(|task| {
            task.with_inputs(inputs)
                .with_failure_policy(if self.pause_on_failure {
                    FailurePolicy::Pause
                } else {
                    FailurePolicy::Continue
                })
        })
        .map_err(NexusCliError::Schedule)
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
}

impl OperationArgs {
    fn into_operation(self) -> Result<TaskOperation, NexusCliError> {
        match (self.agent_id, self.skill_id) {
            (None, None) => self.dag_id.map(TaskOperation::default_dag).ok_or_else(|| {
                NexusCliError::Any(anyhow!("--dag-id is required for the default operation"))
            }),
            (Some(agent_id), Some(skill_id)) => Ok(TaskOperation::agent_skill(
                agent_id,
                skill_id,
                self.dag_id,
                Vec::new(),
            )),
            _ => Err(NexusCliError::Any(anyhow!(
                "--agent-id and --skill-id must be supplied together"
            ))),
        }
    }
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

async fn prepare_inputs(
    input_json: Option<serde_json::Value>,
    remote: Vec<String>,
) -> Result<TaskInputs, NexusCliError> {
    let conf = CliConf::load().await.unwrap_or_default();
    let input_json = input_json.unwrap_or_else(|| serde_json::json!({}));
    let preferred_remote_storage = conf.data_storage.preferred_remote_storage;
    let storage_conf: StorageConf = conf.data_storage.clone().into();
    let ports_data =
        workflow::process_entry_ports(&input_json, preferred_remote_storage, &remote).await?;
    let mut inputs = BTreeMap::new();
    for (vertex, data) in ports_data {
        let committed = data.commit_all(&storage_conf).await.map_err(|error| {
            NexusCliError::Any(anyhow!(
                "failed to store input data: {error}. Configure remote storage before retrying"
            ))
        })?;
        inputs.insert(vertex, committed.into_map().into_iter().collect());
    }
    conf.save().await.map_err(NexusCliError::Any)?;
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::scheduler::ScheduleError};

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
}
