//! Values used to author Tasks and Schedules.

use {
    crate::{
        scheduler::ScheduleError,
        sui,
        types::{
            AgentId,
            NexusData,
            SkillId,
            DEFAULT_PRIORITY_FEE_PERCENTAGE,
            MAX_PRIORITY_FEE_PERCENTAGE,
            MIN_PRIORITY_FEE_PERCENTAGE,
        },
    },
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// Input values keyed first by vertex name and then by input port name.
pub type TaskInputs = BTreeMap<String, BTreeMap<String, NexusData>>;

/// Work, funding, and failure behavior shared by every Task occurrence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskSpec {
    operation: TaskOperation,
    entry_group: String,
    inputs: TaskInputs,
    funding: TaskFunding,
    occurrence_budget_mist: u64,
    failure_policy: FailurePolicy,
}

impl TaskSpec {
    /// Creates a Task definition with no inputs and [`FailurePolicy::Continue`].
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::EmptyEntryGroup`] when `entry_group` is blank,
    /// or [`ScheduleError::ZeroOccurrenceBudget`] when
    /// `occurrence_budget_mist` is zero.
    pub fn new(
        operation: TaskOperation,
        entry_group: impl Into<String>,
        funding: TaskFunding,
        occurrence_budget_mist: u64,
    ) -> Result<Self, ScheduleError> {
        let task = Self {
            operation,
            entry_group: entry_group.into(),
            inputs: BTreeMap::new(),
            funding,
            occurrence_budget_mist,
            failure_policy: FailurePolicy::Continue,
        };
        task.validate()?;
        Ok(task)
    }

    /// Replaces the Task's vertex input values.
    #[must_use]
    pub fn with_inputs(mut self, inputs: TaskInputs) -> Self {
        self.inputs = inputs;
        self
    }

    /// Replaces the behavior applied after a failed occurrence.
    #[must_use]
    pub const fn with_failure_policy(mut self, policy: FailurePolicy) -> Self {
        self.failure_policy = policy;
        self
    }

    /// Returns the operation performed by every occurrence.
    pub const fn operation(&self) -> &TaskOperation {
        &self.operation
    }

    /// Returns the selected DAG entry group name.
    pub fn entry_group(&self) -> &str {
        &self.entry_group
    }

    /// Returns the input values keyed by vertex and port.
    pub const fn inputs(&self) -> &TaskInputs {
        &self.inputs
    }

    /// Returns the Task's funding and controller choice.
    pub const fn funding(&self) -> TaskFunding {
        self.funding
    }

    /// Returns the maximum amount one occurrence may consume.
    pub const fn occurrence_budget_mist(&self) -> u64 {
        self.occurrence_budget_mist
    }

    /// Returns the behavior applied after a failed occurrence.
    pub const fn failure_policy(&self) -> FailurePolicy {
        self.failure_policy
    }

    pub(crate) fn validate(&self) -> Result<(), ScheduleError> {
        if self.entry_group.trim().is_empty() {
            return Err(ScheduleError::EmptyEntryGroup);
        }
        if self.occurrence_budget_mist == 0 {
            return Err(ScheduleError::ZeroOccurrenceBudget);
        }
        if matches!(self.operation, TaskOperation::DefaultDag { .. })
            && matches!(self.funding, TaskFunding::Agent { .. })
        {
            return Err(ScheduleError::IncompatibleFunding {
                message: "a default DAG Task cannot use Agent vault funding",
            });
        }
        Ok(())
    }
}

/// Operation performed by every occurrence of a Task.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskOperation {
    /// Runs a published DAG through the network's default Agent.
    DefaultDag {
        /// Published DAG object identifier.
        dag_id: sui::types::Address,
    },
    /// Runs one registered Agent skill.
    AgentSkill {
        /// Agent object identifier.
        agent_id: AgentId,
        /// Agent local skill identifier.
        skill_id: SkillId,
        /// Optional DAG selected for the skill.
        selected_dag: Option<sui::types::Address>,
        /// Authorization templates materialized for each occurrence.
        authorization_templates: Vec<AuthorizationTemplate>,
    },
}

impl TaskOperation {
    /// Selects a published DAG for the network's default Agent.
    pub const fn default_dag(dag_id: sui::types::Address) -> Self {
        Self::DefaultDag { dag_id }
    }

    /// Selects one registered Agent skill.
    pub fn agent_skill(
        agent_id: AgentId,
        skill_id: SkillId,
        selected_dag: Option<sui::types::Address>,
        authorization_templates: Vec<AuthorizationTemplate>,
    ) -> Self {
        Self::AgentSkill {
            agent_id,
            skill_id,
            selected_dag,
            authorization_templates,
        }
    }

    /// Returns the selected Agent when this is an Agent skill operation.
    pub const fn agent_id(&self) -> Option<AgentId> {
        match self {
            Self::DefaultDag { .. } => None,
            Self::AgentSkill { agent_id, .. } => Some(*agent_id),
        }
    }

    /// Returns the DAG selected directly by this operation.
    ///
    /// An Agent skill with no selection resolves its DAG from the active skill
    /// registration during dispatch.
    pub const fn selected_dag_id(&self) -> Option<sui::types::Address> {
        match self {
            Self::DefaultDag { dag_id } => Some(*dag_id),
            Self::AgentSkill { selected_dag, .. } => *selected_dag,
        }
    }
}

/// Authorization material attached to each execution of an Agent skill.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationTemplate {
    skill_id: SkillId,
    vertex: String,
    recipient_id: sui::types::Address,
}

impl AuthorizationTemplate {
    /// Creates an authorization template for one DAG vertex.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::EmptyAuthorizationVertex`] when `vertex` is
    /// blank.
    pub fn new(
        skill_id: SkillId,
        vertex: impl Into<String>,
        recipient_id: sui::types::Address,
    ) -> Result<Self, ScheduleError> {
        let vertex = vertex.into();
        if vertex.trim().is_empty() {
            return Err(ScheduleError::EmptyAuthorizationVertex);
        }
        Ok(Self {
            skill_id,
            vertex,
            recipient_id,
        })
    }

    /// Returns the skill authorized by the template.
    pub const fn skill_id(&self) -> SkillId {
        self.skill_id
    }

    /// Returns the authorized DAG vertex name.
    pub fn vertex(&self) -> &str {
        &self.vertex
    }

    /// Returns the authorization recipient object.
    pub const fn recipient_id(&self) -> sui::types::Address {
        self.recipient_id
    }
}

/// Funding source and immutable controller for a Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskFunding {
    /// Reserves sender owned SUI and assigns address control.
    Address {
        /// Amount reserved when the Task is created.
        prepay_amount_mist: u64,
        /// Optional recipient of unused reserved funds.
        refund_recipient: Option<sui::types::Address>,
    },
    /// Reserves funds from the selected Agent and assigns Agent control.
    Agent {
        /// Amount reserved from the Agent's payment vault.
        prepay_amount_mist: u64,
    },
}

impl TaskFunding {
    /// Uses sender funds and refunds unused funds to the sender.
    pub const fn address(prepay_amount_mist: u64) -> Self {
        Self::Address {
            prepay_amount_mist,
            refund_recipient: None,
        }
    }

    /// Uses sender funds with an explicit refund recipient.
    pub const fn address_with_refund(
        prepay_amount_mist: u64,
        refund_recipient: sui::types::Address,
    ) -> Self {
        Self::Address {
            prepay_amount_mist,
            refund_recipient: Some(refund_recipient),
        }
    }

    /// Uses the selected Agent's payment vault.
    pub const fn agent(prepay_amount_mist: u64) -> Self {
        Self::Agent { prepay_amount_mist }
    }

    /// Returns the amount reserved when the Task is created.
    pub const fn prepay_amount_mist(self) -> u64 {
        match self {
            Self::Address {
                prepay_amount_mist, ..
            }
            | Self::Agent { prepay_amount_mist } => prepay_amount_mist,
        }
    }
}

/// Behavior applied after an occurrence settles unsuccessfully.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicy {
    /// Continue advertising eligible work.
    #[default]
    Continue,
    /// Pause the Task until its controller resumes it.
    Pause,
}

/// A composable collection of standalone occurrences and one recurrence.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    occurrences: Vec<Occurrence>,
    recurrence: Option<Recurrence>,
}

impl Schedule {
    /// Creates an empty, valid Schedule.
    pub const fn new() -> Self {
        Self {
            occurrences: Vec::new(),
            recurrence: None,
        }
    }

    /// Appends one standalone occurrence in deterministic insertion order.
    #[must_use]
    pub fn with_occurrence(mut self, occurrence: Occurrence) -> Self {
        self.occurrences.push(occurrence);
        self
    }

    /// Replaces the Schedule's optional recurrence.
    #[must_use]
    pub fn with_recurrence(mut self, recurrence: Recurrence) -> Self {
        self.recurrence = Some(recurrence);
        self
    }

    /// Returns whether the Schedule contains no work.
    pub fn is_empty(&self) -> bool {
        self.occurrences.is_empty() && self.recurrence.is_none()
    }

    /// Returns standalone occurrences in insertion order.
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }

    /// Returns the optional recurrence.
    pub const fn recurrence(&self) -> Option<&Recurrence> {
        self.recurrence.as_ref()
    }

    /// Verifies that the atomic create and schedule shortcut has work.
    ///
    /// Empty Schedules remain valid composition values; only the shortcut
    /// requires at least one standalone occurrence or recurrence.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::EmptySchedule`] when this Schedule is empty.
    pub fn validate_for_task_creation(&self) -> Result<(), ScheduleError> {
        self.validate()?;
        if self.is_empty() {
            Err(ScheduleError::EmptySchedule)
        } else {
            Ok(())
        }
    }

    pub(crate) fn validate(&self) -> Result<(), ScheduleError> {
        for occurrence in &self.occurrences {
            occurrence.validate()?;
        }
        if let Some(recurrence) = &self.recurrence {
            recurrence.validate()?;
        }
        Ok(())
    }
}

/// One lazily expanded recurring source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recurrence {
    first: Occurrence,
    interval_ms: u64,
    occurrences: Option<u64>,
}

impl Recurrence {
    /// Creates an unbounded recurrence.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::ZeroRecurrenceInterval`] when `interval_ms`
    /// is zero.
    pub fn new(first: Occurrence, interval_ms: u64) -> Result<Self, ScheduleError> {
        let recurrence = Self {
            first,
            interval_ms,
            occurrences: None,
        };
        recurrence.validate()?;
        Ok(recurrence)
    }

    /// Removes a finite occurrence limit.
    #[must_use]
    pub const fn unbounded(mut self) -> Self {
        self.occurrences = None;
        self
    }

    /// Limits the recurrence to `occurrences` total materializations.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::ZeroRecurrenceCount`] when `occurrences` is
    /// zero.
    pub fn finite(mut self, occurrences: u64) -> Result<Self, ScheduleError> {
        if occurrences == 0 {
            return Err(ScheduleError::ZeroRecurrenceCount);
        }
        self.occurrences = Some(occurrences);
        Ok(self)
    }

    /// Returns the first occurrence template.
    pub const fn first(&self) -> &Occurrence {
        &self.first
    }

    /// Returns the interval between materialized occurrences.
    pub const fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    /// Returns the optional total number of materializations.
    pub const fn occurrences(&self) -> Option<u64> {
        self.occurrences
    }

    pub(crate) fn validate(&self) -> Result<(), ScheduleError> {
        self.first.validate()?;
        if self.interval_ms == 0 {
            return Err(ScheduleError::ZeroRecurrenceInterval);
        }
        if self.occurrences == Some(0) {
            return Err(ScheduleError::ZeroRecurrenceCount);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum StartTime {
    Now,
    At { timestamp_ms: u64 },
    After { offset_ms: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum Deadline {
    At { timestamp_ms: u64 },
    AfterStart { offset_ms: u64 },
}

/// Relative or absolute intent for one scheduled occurrence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    start: StartTime,
    deadline: Option<Deadline>,
    priority_fee_percentage: u64,
}

impl Occurrence {
    /// Starts at the single chain time snapshot used to prepare its Schedule.
    pub const fn now() -> Self {
        Self {
            start: StartTime::Now,
            deadline: None,
            priority_fee_percentage: DEFAULT_PRIORITY_FEE_PERCENTAGE,
        }
    }

    /// Starts at an absolute millisecond timestamp.
    pub const fn at_ms(timestamp_ms: u64) -> Self {
        Self {
            start: StartTime::At { timestamp_ms },
            deadline: None,
            priority_fee_percentage: DEFAULT_PRIORITY_FEE_PERCENTAGE,
        }
    }

    /// Starts at an offset from the Schedule's single chain time snapshot.
    pub const fn after_ms(offset_ms: u64) -> Self {
        Self {
            start: StartTime::After { offset_ms },
            deadline: None,
            priority_fee_percentage: DEFAULT_PRIORITY_FEE_PERCENTAGE,
        }
    }

    /// Sets an absolute dispatch deadline.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::DeadlineBeforeStart`] when both timestamps are
    /// absolute and `timestamp_ms` precedes the start. Starts relative to chain
    /// time are checked during request preparation.
    pub fn deadline_at_ms(mut self, timestamp_ms: u64) -> Result<Self, ScheduleError> {
        if let StartTime::At {
            timestamp_ms: start_time_ms,
        } = self.start
        {
            if timestamp_ms < start_time_ms {
                return Err(ScheduleError::DeadlineBeforeStart {
                    start_time_ms,
                    deadline_ms: timestamp_ms,
                });
            }
        }
        self.deadline = Some(Deadline::At { timestamp_ms });
        Ok(self)
    }

    /// Sets the dispatch deadline to an offset from the resolved start.
    #[must_use]
    pub const fn deadline_after_ms(mut self, offset_ms: u64) -> Self {
        self.deadline = Some(Deadline::AfterStart { offset_ms });
        self
    }

    /// Sets the protocol priority fee percentage.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::PriorityFeeOutOfRange`] when `percentage` is
    /// outside the inclusive protocol range.
    pub fn with_priority_fee_percentage(mut self, percentage: u64) -> Result<Self, ScheduleError> {
        if !(MIN_PRIORITY_FEE_PERCENTAGE..=MAX_PRIORITY_FEE_PERCENTAGE).contains(&percentage) {
            return Err(ScheduleError::PriorityFeeOutOfRange {
                percentage,
                minimum: MIN_PRIORITY_FEE_PERCENTAGE,
                maximum: MAX_PRIORITY_FEE_PERCENTAGE,
            });
        }
        self.priority_fee_percentage = percentage;
        Ok(self)
    }

    /// Returns the configured priority fee percentage.
    pub const fn priority_fee_percentage(&self) -> u64 {
        self.priority_fee_percentage
    }

    pub(crate) const fn start(&self) -> StartTime {
        self.start
    }

    pub(crate) const fn deadline(&self) -> Option<Deadline> {
        self.deadline
    }

    pub(crate) fn validate(&self) -> Result<(), ScheduleError> {
        if !(MIN_PRIORITY_FEE_PERCENTAGE..=MAX_PRIORITY_FEE_PERCENTAGE)
            .contains(&self.priority_fee_percentage)
        {
            return Err(ScheduleError::PriorityFeeOutOfRange {
                percentage: self.priority_fee_percentage,
                minimum: MIN_PRIORITY_FEE_PERCENTAGE,
                maximum: MAX_PRIORITY_FEE_PERCENTAGE,
            });
        }
        if let (
            StartTime::At {
                timestamp_ms: start_time_ms,
            },
            Some(Deadline::At {
                timestamp_ms: deadline_ms,
            }),
        ) = (self.start, self.deadline)
        {
            if deadline_ms < start_time_ms {
                return Err(ScheduleError::DeadlineBeforeStart {
                    start_time_ms,
                    deadline_ms,
                });
            }
        }
        Ok(())
    }
}

/// Stable identity of one occurrence allocated by a Task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OccurrenceRef {
    task_id: sui::types::Address,
    occurrence_id: u64,
}

impl OccurrenceRef {
    /// Creates an occurrence reference.
    pub const fn new(task_id: sui::types::Address, occurrence_id: u64) -> Self {
        Self {
            task_id,
            occurrence_id,
        }
    }

    /// Returns the owning Task identifier.
    pub const fn task_id(self) -> sui::types::Address {
        self.task_id
    }

    /// Returns the Task local occurrence identifier.
    pub const fn occurrence_id(self) -> u64 {
        self.occurrence_id
    }
}

/// One occurrence offered to the leader network for dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchOffer {
    occurrence: OccurrenceRef,
    effective_start_time_ms: u64,
    deadline_ms: Option<u64>,
    priority_fee_percentage: u64,
}

impl DispatchOffer {
    /// Creates a validated dispatch offer at an ingestion boundary.
    ///
    /// # Errors
    ///
    /// Returns [`ScheduleError::DeadlineBeforeStart`] when the deadline
    /// precedes the effective start, or
    /// [`ScheduleError::PriorityFeeOutOfRange`] for an invalid fee.
    pub fn new(
        occurrence: OccurrenceRef,
        effective_start_time_ms: u64,
        deadline_ms: Option<u64>,
        priority_fee_percentage: u64,
    ) -> Result<Self, ScheduleError> {
        if let Some(deadline_ms) = deadline_ms {
            if deadline_ms < effective_start_time_ms {
                return Err(ScheduleError::DeadlineBeforeStart {
                    start_time_ms: effective_start_time_ms,
                    deadline_ms,
                });
            }
        }
        if !(MIN_PRIORITY_FEE_PERCENTAGE..=MAX_PRIORITY_FEE_PERCENTAGE)
            .contains(&priority_fee_percentage)
        {
            return Err(ScheduleError::PriorityFeeOutOfRange {
                percentage: priority_fee_percentage,
                minimum: MIN_PRIORITY_FEE_PERCENTAGE,
                maximum: MAX_PRIORITY_FEE_PERCENTAGE,
            });
        }
        Ok(Self {
            occurrence,
            effective_start_time_ms,
            deadline_ms,
            priority_fee_percentage,
        })
    }

    /// Returns the occurrence being offered.
    pub const fn occurrence(&self) -> OccurrenceRef {
        self.occurrence
    }

    /// Returns the earliest protocol adjusted dispatch timestamp.
    pub const fn effective_start_time_ms(&self) -> u64 {
        self.effective_start_time_ms
    }

    /// Returns the optional absolute dispatch deadline.
    pub const fn deadline_ms(&self) -> Option<u64> {
        self.deadline_ms
    }

    /// Returns the dispatch priority fee percentage.
    pub const fn priority_fee_percentage(&self) -> u64 {
        self.priority_fee_percentage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    #[test]
    fn empty_schedule_is_composable_but_not_a_creation_shortcut() {
        let schedule = Schedule::new();

        assert!(schedule.is_empty());
        assert_eq!(
            schedule.validate_for_task_creation(),
            Err(ScheduleError::EmptySchedule)
        );
    }

    #[test]
    fn mixed_schedule_preserves_insertion_order_and_recurrence() {
        let recurrence = Recurrence::new(Occurrence::now(), 1_000)
            .expect("nonzero interval")
            .finite(3)
            .expect("nonzero count");
        let mixed = Schedule::new()
            .with_occurrence(Occurrence::now())
            .with_occurrence(Occurrence::at_ms(2_000))
            .with_recurrence(recurrence);

        assert_eq!(mixed.occurrences().len(), 2);
        assert!(mixed.recurrence().is_some());
    }

    #[test]
    fn invalid_authoring_values_are_rejected() {
        let address = sui::types::Address::from_static("0x1");
        assert_eq!(
            TaskSpec::new(
                TaskOperation::default_dag(address),
                "main",
                TaskFunding::address(1),
                0,
            )
            .expect_err("zero budget"),
            ScheduleError::ZeroOccurrenceBudget
        );
        assert!(matches!(
            Occurrence::at_ms(20).deadline_at_ms(10),
            Err(ScheduleError::DeadlineBeforeStart { .. })
        ));
        assert!(matches!(
            Occurrence::now().with_priority_fee_percentage(MAX_PRIORITY_FEE_PERCENTAGE + 1),
            Err(ScheduleError::PriorityFeeOutOfRange { .. })
        ));
        assert_eq!(
            Recurrence::new(Occurrence::now(), 0).expect_err("zero interval"),
            ScheduleError::ZeroRecurrenceInterval
        );
        assert_eq!(
            Recurrence::new(Occurrence::now(), 1)
                .expect("valid recurrence")
                .finite(0)
                .expect_err("zero count"),
            ScheduleError::ZeroRecurrenceCount
        );
    }

    #[test]
    fn task_spec_preserves_authored_operation_funding_and_inputs() {
        let agent_id = address("0xa");
        let selected_dag = address("0xb");
        let recipient_id = address("0xc");
        let authorization = AuthorizationTemplate::new(7, "summarize", recipient_id)
            .expect("authorization vertex is present");
        assert_eq!(authorization.skill_id(), 7);
        assert_eq!(authorization.vertex(), "summarize");
        assert_eq!(authorization.recipient_id(), recipient_id);

        let operation =
            TaskOperation::agent_skill(agent_id, 11, Some(selected_dag), vec![authorization]);
        assert_eq!(operation.agent_id(), Some(agent_id));
        assert_eq!(TaskOperation::default_dag(selected_dag).agent_id(), None);

        let mut inputs = TaskInputs::new();
        inputs.insert("summarize".to_owned(), BTreeMap::new());
        let task = TaskSpec::new(operation, "main", TaskFunding::agent(90), 30)
            .expect("Agent funding matches an Agent skill")
            .with_inputs(inputs)
            .with_failure_policy(FailurePolicy::Pause);

        assert!(matches!(
            task.operation(),
            TaskOperation::AgentSkill {
                agent_id: stored_agent,
                skill_id: 11,
                selected_dag: Some(stored_dag),
                authorization_templates,
            } if *stored_agent == agent_id
                && *stored_dag == selected_dag
                && authorization_templates.len() == 1
        ));
        assert_eq!(task.entry_group(), "main");
        assert!(task.inputs().contains_key("summarize"));
        assert_eq!(task.funding(), TaskFunding::agent(90));
        assert_eq!(task.funding().prepay_amount_mist(), 90);
        assert_eq!(task.occurrence_budget_mist(), 30);
        assert_eq!(task.failure_policy(), FailurePolicy::Pause);
    }

    #[test]
    fn task_authoring_rejects_blank_names_and_incompatible_funding() {
        let dag_id = address("0xd");
        assert_eq!(
            AuthorizationTemplate::new(1, "  ", dag_id).expect_err("blank authorization vertex"),
            ScheduleError::EmptyAuthorizationVertex
        );
        assert_eq!(
            TaskSpec::new(
                TaskOperation::default_dag(dag_id),
                "  ",
                TaskFunding::address(1),
                1,
            )
            .expect_err("blank entry group"),
            ScheduleError::EmptyEntryGroup
        );
        assert!(matches!(
            TaskSpec::new(
                TaskOperation::default_dag(dag_id),
                "main",
                TaskFunding::agent(1),
                1,
            ),
            Err(ScheduleError::IncompatibleFunding { .. })
        ));

        let refund_recipient = address("0xe");
        let funding = TaskFunding::address_with_refund(40, refund_recipient);
        assert_eq!(funding.prepay_amount_mist(), 40);
        assert!(matches!(
            funding,
            TaskFunding::Address {
                prepay_amount_mist: 40,
                refund_recipient: Some(stored_recipient),
            } if stored_recipient == refund_recipient
        ));
        assert_eq!(TaskFunding::address(20).prepay_amount_mist(), 20);
    }

    #[test]
    fn schedule_composition_preserves_every_time_form() {
        let absolute = Occurrence::at_ms(100)
            .deadline_at_ms(120)
            .expect("deadline follows start")
            .with_priority_fee_percentage(MIN_PRIORITY_FEE_PERCENTAGE)
            .expect("minimum fee is valid");
        assert_eq!(absolute.start(), StartTime::At { timestamp_ms: 100 });
        assert_eq!(
            absolute.deadline(),
            Some(Deadline::At { timestamp_ms: 120 })
        );
        assert_eq!(
            absolute.priority_fee_percentage(),
            MIN_PRIORITY_FEE_PERCENTAGE
        );

        let relative = Occurrence::after_ms(10).deadline_after_ms(5);
        assert_eq!(relative.start(), StartTime::After { offset_ms: 10 });
        assert_eq!(
            relative.deadline(),
            Some(Deadline::AfterStart { offset_ms: 5 })
        );
        assert_eq!(Occurrence::now().start(), StartTime::Now);

        let recurrence = Recurrence::new(relative, 25)
            .expect("interval advances time")
            .finite(3)
            .expect("finite recurrence is nonempty");
        assert_eq!(recurrence.first(), &relative);
        assert_eq!(recurrence.interval_ms(), 25);
        assert_eq!(recurrence.occurrences(), Some(3));
        assert_eq!(recurrence.clone().unbounded().occurrences(), None);

        let schedule = Schedule::new()
            .with_occurrence(absolute)
            .with_recurrence(recurrence);
        assert!(!schedule.is_empty());
        assert_eq!(schedule.occurrences(), &[absolute]);
        assert!(schedule.recurrence().is_some());
        assert_eq!(schedule.validate_for_task_creation(), Ok(()));
    }

    #[test]
    fn nested_schedule_validation_rejects_invalid_stored_values() {
        let invalid_priority = Occurrence {
            start: StartTime::Now,
            deadline: None,
            priority_fee_percentage: MAX_PRIORITY_FEE_PERCENTAGE + 1,
        };
        assert!(matches!(
            Schedule::new().with_occurrence(invalid_priority).validate(),
            Err(ScheduleError::PriorityFeeOutOfRange { .. })
        ));

        let invalid_deadline = Occurrence {
            start: StartTime::At { timestamp_ms: 20 },
            deadline: Some(Deadline::At { timestamp_ms: 10 }),
            priority_fee_percentage: DEFAULT_PRIORITY_FEE_PERCENTAGE,
        };
        assert!(matches!(
            invalid_deadline.validate(),
            Err(ScheduleError::DeadlineBeforeStart { .. })
        ));

        let invalid_recurrence = Recurrence {
            first: Occurrence::now(),
            interval_ms: 1,
            occurrences: Some(0),
        };
        assert_eq!(
            invalid_recurrence.validate(),
            Err(ScheduleError::ZeroRecurrenceCount)
        );
        assert_eq!(
            Schedule::new()
                .with_recurrence(invalid_recurrence)
                .validate(),
            Err(ScheduleError::ZeroRecurrenceCount)
        );
    }

    #[test]
    fn dispatch_offer_validates_and_exposes_protocol_values() {
        let reference = OccurrenceRef::new(address("0xf"), 9);
        assert_eq!(reference.task_id(), address("0xf"));
        assert_eq!(reference.occurrence_id(), 9);

        let offer = DispatchOffer::new(reference, 100, Some(120), MAX_PRIORITY_FEE_PERCENTAGE)
            .expect("offer values are valid");
        assert_eq!(offer.occurrence(), reference);
        assert_eq!(offer.effective_start_time_ms(), 100);
        assert_eq!(offer.deadline_ms(), Some(120));
        assert_eq!(offer.priority_fee_percentage(), MAX_PRIORITY_FEE_PERCENTAGE);

        assert!(matches!(
            DispatchOffer::new(reference, 100, Some(99), DEFAULT_PRIORITY_FEE_PERCENTAGE),
            Err(ScheduleError::DeadlineBeforeStart { .. })
        ));
        assert!(matches!(
            DispatchOffer::new(reference, 100, None, MIN_PRIORITY_FEE_PERCENTAGE - 1),
            Err(ScheduleError::PriorityFeeOutOfRange { .. })
        ));
    }
}
