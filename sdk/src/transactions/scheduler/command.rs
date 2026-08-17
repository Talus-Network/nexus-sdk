use {
    super::{
        authority::ResolvedAuthority,
        encode::{execution_config_arg, failure_policy_arg, occurrence_args, recurrence_args},
    },
    crate::{
        move_bindings::{
            scheduler::scheduler as scheduler_binding,
            workflow::{
                execution_entries as execution_entries_binding,
                tool_cashier_adapter as tool_cashier_adapter_binding,
            },
        },
        move_boundary::NexusPtbBuilder,
        scheduler::{FailurePolicy, ScheduleError, SchedulerError, TaskInputs, TaskOperation},
        sui,
        transactions::agent_input::AgentInput,
        types::NexusObjects,
    },
    std::collections::HashSet,
    sui_sdk_types::{Argument, ProgrammableTransaction},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreparedFunding {
    Address {
        prepay_amount_mist: u64,
        refund_recipient: sui::types::Address,
    },
    Agent {
        prepay_amount_mist: u64,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedTask {
    pub(crate) operation: TaskOperation,
    pub(crate) agent: Option<AgentInput>,
    pub(crate) entry_group: String,
    pub(crate) inputs: TaskInputs,
    pub(crate) funding: PreparedFunding,
    pub(crate) occurrence_budget_mist: u64,
    pub(crate) failure_policy: FailurePolicy,
}

pub(super) struct CreatedTask {
    pub(super) task: Argument,
    pub(super) pointer: Argument,
    pub(super) authority: ResolvedAuthority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedOccurrence {
    pub(super) start_time_ms: u64,
    pub(super) deadline_ms: Option<u64>,
    pub(super) priority_fee_percentage: u64,
}

impl PreparedOccurrence {
    pub(crate) const fn new(
        start_time_ms: u64,
        deadline_ms: Option<u64>,
        priority_fee_percentage: u64,
    ) -> Self {
        Self {
            start_time_ms,
            deadline_ms,
            priority_fee_percentage,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparedRecurrence {
    pub(super) first: PreparedOccurrence,
    pub(super) interval_ms: u64,
    pub(super) occurrences: Option<u64>,
}

impl PreparedRecurrence {
    pub(crate) const fn new(
        first: PreparedOccurrence,
        interval_ms: u64,
        occurrences: Option<u64>,
    ) -> Self {
        Self {
            first,
            interval_ms,
            occurrences,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreparedSchedule {
    pub(super) occurrences: Vec<PreparedOccurrence>,
    pub(super) recurrence: Option<PreparedRecurrence>,
}

impl PreparedSchedule {
    pub(crate) fn new(
        occurrences: Vec<PreparedOccurrence>,
        recurrence: Option<PreparedRecurrence>,
    ) -> Self {
        Self {
            occurrences,
            recurrence,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.occurrences.is_empty() && self.recurrence.is_none()
    }
}

fn ptb(
    objects: &NexusObjects,
    build: impl FnOnce(&mut NexusPtbBuilder) -> Result<(), SchedulerError>,
) -> Result<ProgrammableTransaction, SchedulerError> {
    let mut transaction = NexusPtbBuilder::new(std::sync::Arc::new(objects.clone()));
    build(&mut transaction)?;
    Ok(transaction.finish())
}

fn incompatible_funding(message: &'static str) -> SchedulerError {
    ScheduleError::IncompatibleFunding { message }.into()
}

pub(super) fn create_unshared_task(
    transaction: &mut NexusPtbBuilder,
    task: &PreparedTask,
) -> Result<CreatedTask, SchedulerError> {
    if task.occurrence_budget_mist == 0 {
        return Err(ScheduleError::ZeroOccurrenceBudget.into());
    }

    let leader_registry_ref = transaction.objects().leader_registry.clone();
    let leader_registry = transaction
        .shared_object(&leader_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let registry_ref = transaction.objects().agent_registry.clone();
    let registry = transaction
        .shared_object(&registry_ref, true)
        .map_err(SchedulerError::transaction)?;
    let execution = execution_config_arg(transaction, task)?;
    let occurrence_budget_mist = transaction
        .arg(&task.occurrence_budget_mist)
        .map_err(SchedulerError::transaction)?;
    let failure_policy = failure_policy_arg(transaction, task.failure_policy)?;
    match (&task.operation, &task.funding, &task.agent) {
        (
            TaskOperation::DefaultDag { .. },
            PreparedFunding::Address {
                prepay_amount_mist,
                refund_recipient,
            },
            None,
        ) => {
            let prepayment = transaction
                .withdraw_sui_coin(*prepay_amount_mist)
                .map_err(SchedulerError::transaction)?;
            let refund_recipient = transaction
                .arg(refund_recipient)
                .map_err(SchedulerError::transaction)?;
            let result = transaction
                .call_target(
                    scheduler_binding::new_default_task_v2_target,
                    vec![
                        registry,
                        leader_registry,
                        execution,
                        prepayment,
                        refund_recipient,
                        occurrence_budget_mist,
                        failure_policy,
                    ],
                )
                .map_err(SchedulerError::transaction)?;
            created_task(transaction, result, ResolvedAuthority::Address)
        }
        (
            TaskOperation::AgentSkill { agent_id, .. },
            PreparedFunding::Address {
                prepay_amount_mist,
                refund_recipient,
            },
            Some(agent),
        ) if agent.object_id() == *agent_id => {
            let agent_argument = agent
                .clone()
                .immutable_ptb_argument(transaction)
                .map_err(SchedulerError::transaction)?;
            let prepayment = transaction
                .withdraw_sui_coin(*prepay_amount_mist)
                .map_err(SchedulerError::transaction)?;
            let refund_recipient = transaction
                .arg(refund_recipient)
                .map_err(SchedulerError::transaction)?;
            let result = transaction
                .call_target(
                    scheduler_binding::new_user_task_v2_target,
                    vec![
                        registry,
                        leader_registry,
                        agent_argument,
                        execution,
                        prepayment,
                        refund_recipient,
                        occurrence_budget_mist,
                        failure_policy,
                    ],
                )
                .map_err(SchedulerError::transaction)?;
            created_task(transaction, result, ResolvedAuthority::Address)
        }
        (
            TaskOperation::AgentSkill { agent_id, .. },
            PreparedFunding::Agent { prepay_amount_mist },
            Some(agent),
        ) if agent.object_id() == *agent_id => {
            let agent_argument = agent
                .clone()
                .mutable_ptb_argument(transaction)
                .map_err(SchedulerError::transaction)?;
            let prepay_amount_mist = transaction
                .arg(prepay_amount_mist)
                .map_err(SchedulerError::transaction)?;
            let result = transaction
                .call_target(
                    scheduler_binding::new_agent_task_v2_target,
                    vec![
                        registry,
                        leader_registry,
                        agent_argument,
                        execution,
                        prepay_amount_mist,
                        occurrence_budget_mist,
                        failure_policy,
                    ],
                )
                .map_err(SchedulerError::transaction)?;
            created_task(transaction, result, ResolvedAuthority::Agent(agent.clone()))
        }
        (TaskOperation::DefaultDag { .. }, PreparedFunding::Agent { .. }, _) => Err(
            incompatible_funding("a default DAG Task cannot use Agent-vault funding"),
        ),
        (TaskOperation::DefaultDag { .. }, PreparedFunding::Address { .. }, Some(_)) => Err(
            incompatible_funding("a default DAG Task must not resolve an Agent object"),
        ),
        (TaskOperation::AgentSkill { .. }, _, None) => Err(incompatible_funding(
            "an Agent-skill Task requires its Agent object",
        )),
        (TaskOperation::AgentSkill { .. }, _, Some(_)) => Err(incompatible_funding(
            "the resolved Agent does not match the Task operation",
        )),
    }
}

fn created_task(
    transaction: &NexusPtbBuilder,
    result: Argument,
    authority: ResolvedAuthority,
) -> Result<CreatedTask, SchedulerError> {
    let task = transaction
        .nested_result(result, 0)
        .map_err(SchedulerError::transaction)?;
    let pointer = transaction
        .nested_result(result, 1)
        .map_err(SchedulerError::transaction)?;
    Ok(CreatedTask {
        task,
        pointer,
        authority,
    })
}

fn shared_task_arg(
    transaction: &mut NexusPtbBuilder,
    task: &sui::types::ObjectReference,
) -> Result<Argument, SchedulerError> {
    transaction
        .shared_object(task, true)
        .map_err(SchedulerError::transaction)
}

pub(super) fn append_occurrence(
    transaction: &mut NexusPtbBuilder,
    task: Argument,
    authority: &ResolvedAuthority,
    occurrence: &PreparedOccurrence,
) -> Result<(), SchedulerError> {
    let (start_time_ms, deadline_ms, priority_fee_percentage) =
        occurrence_args(transaction, occurrence)?;
    let leader_registry_ref = transaction.objects().leader_registry.clone();
    let leader_registry = transaction
        .shared_object(&leader_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let clock = transaction.clock().map_err(SchedulerError::transaction)?;
    authority.call(
        transaction,
        scheduler_binding::schedule_target,
        scheduler_binding::schedule_as_agent_target,
        task,
        vec![
            start_time_ms,
            deadline_ms,
            priority_fee_percentage,
            leader_registry,
            clock,
        ],
    )?;
    Ok(())
}

pub(super) fn append_recurrence(
    transaction: &mut NexusPtbBuilder,
    task: Argument,
    authority: &ResolvedAuthority,
    recurrence: &PreparedRecurrence,
) -> Result<(), SchedulerError> {
    let (start_time_ms, deadline_ms, interval_ms, occurrences, priority_fee_percentage) =
        recurrence_args(transaction, recurrence)?;
    let leader_registry_ref = transaction.objects().leader_registry.clone();
    let leader_registry = transaction
        .shared_object(&leader_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let clock = transaction.clock().map_err(SchedulerError::transaction)?;
    authority.call(
        transaction,
        scheduler_binding::set_recurrence_target,
        scheduler_binding::set_recurrence_as_agent_target,
        task,
        vec![
            start_time_ms,
            deadline_ms,
            interval_ms,
            occurrences,
            priority_fee_percentage,
            leader_registry,
            clock,
        ],
    )?;
    Ok(())
}

pub(super) fn append_schedule(
    transaction: &mut NexusPtbBuilder,
    task: Argument,
    authority: &ResolvedAuthority,
    schedule: &PreparedSchedule,
) -> Result<(), SchedulerError> {
    for occurrence in &schedule.occurrences {
        append_occurrence(transaction, task, authority, occurrence)?;
    }
    if let Some(recurrence) = &schedule.recurrence {
        append_recurrence(transaction, task, authority, recurrence)?;
    }
    Ok(())
}

pub(super) fn finish_task(
    transaction: &mut NexusPtbBuilder,
    task: Argument,
    pointer: Argument,
    pointer_owner: sui::types::Address,
) -> Result<(), SchedulerError> {
    transaction
        .call_target(scheduler_binding::share_target, vec![task])
        .map_err(SchedulerError::transaction)?;
    let pointer_owner = transaction
        .arg(&pointer_owner)
        .map_err(SchedulerError::transaction)?;
    transaction
        .transfer_objects(vec![pointer], pointer_owner)
        .map_err(SchedulerError::transaction)?;
    Ok(())
}

pub(crate) fn create_task_ptb(
    objects: &NexusObjects,
    task: &PreparedTask,
    pointer_owner: sui::types::Address,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let created = create_unshared_task(transaction, task)?;
        finish_task(transaction, created.task, created.pointer, pointer_owner)
    })
}

pub(crate) fn schedule_task_ptb(
    objects: &NexusObjects,
    task: &PreparedTask,
    schedule: &PreparedSchedule,
    pointer_owner: sui::types::Address,
) -> Result<ProgrammableTransaction, SchedulerError> {
    if schedule.is_empty() {
        return Err(ScheduleError::EmptySchedule.into());
    }
    ptb(objects, |transaction| {
        let created = create_unshared_task(transaction, task)?;
        append_schedule(transaction, created.task, &created.authority, schedule)?;
        finish_task(transaction, created.task, created.pointer, pointer_owner)
    })
}

pub(crate) fn add_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
    occurrence: &PreparedOccurrence,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        append_occurrence(transaction, task, authority, occurrence)
    })
}

pub(crate) fn set_recurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
    recurrence: &PreparedRecurrence,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        append_recurrence(transaction, task, authority, recurrence)
    })
}

pub(crate) fn clear_recurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        let leader_registry = transaction
            .shared_object(&objects.leader_registry, false)
            .map_err(SchedulerError::transaction)?;
        let clock = transaction.clock().map_err(SchedulerError::transaction)?;
        authority.call(
            transaction,
            scheduler_binding::clear_recurrence_target,
            scheduler_binding::clear_recurrence_as_agent_target,
            task,
            vec![leader_registry, clock],
        )?;
        Ok(())
    })
}

pub(crate) fn pause_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        authority.call(
            transaction,
            scheduler_binding::pause_target,
            scheduler_binding::pause_as_agent_target,
            task,
            vec![],
        )?;
        Ok(())
    })
}

pub(crate) fn resume_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        let leader_registry = transaction
            .shared_object(&objects.leader_registry, false)
            .map_err(SchedulerError::transaction)?;
        let clock = transaction.clock().map_err(SchedulerError::transaction)?;
        authority.call(
            transaction,
            scheduler_binding::resume_target,
            scheduler_binding::resume_as_agent_target,
            task,
            vec![leader_registry, clock],
        )?;
        Ok(())
    })
}

pub(crate) fn cancel_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        authority.call(
            transaction,
            scheduler_binding::cancel_target,
            scheduler_binding::cancel_as_agent_target,
            task,
            vec![],
        )?;
        Ok(())
    })
}

pub(crate) fn refill_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
    amount_mist: u64,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        let leader_registry = transaction
            .shared_object(&objects.leader_registry, false)
            .map_err(SchedulerError::transaction)?;
        let clock = transaction.clock().map_err(SchedulerError::transaction)?;

        authority.lower(
            transaction,
            |transaction| {
                let funds = transaction
                    .withdraw_sui_coin(amount_mist)
                    .map_err(SchedulerError::transaction)?;
                transaction
                    .call_target(
                        scheduler_binding::refill_target,
                        vec![task, funds, leader_registry, clock],
                    )
                    .map_err(SchedulerError::transaction)
            },
            |transaction, agent| {
                let agent = agent
                    .mutable_ptb_argument(transaction)
                    .map_err(SchedulerError::transaction)?;
                let amount_mist = transaction
                    .arg(&amount_mist)
                    .map_err(SchedulerError::transaction)?;
                transaction
                    .call_target(
                        scheduler_binding::refill_from_agent_target,
                        vec![task, agent, amount_mist, leader_registry, clock],
                    )
                    .map_err(SchedulerError::transaction)
            },
        )?;
        Ok(())
    })
}

pub(crate) fn close_task_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    authority: &ResolvedAuthority,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        let registry = transaction
            .shared_object(&objects.agent_registry, true)
            .map_err(SchedulerError::transaction)?;
        authority.call_mutably(
            transaction,
            scheduler_binding::close_target,
            scheduler_binding::close_as_agent_target,
            task,
            vec![registry],
        )?;
        Ok(())
    })
}

pub(crate) fn expire_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    occurrence_id: u64,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        append_expire_occurrence(transaction, task, occurrence_id)
    })
}

/// Builds an occurrence expiration that reimburses the leader submission gas charge.
pub fn expire_occurrence_with_gas_charge_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    occurrence_id: u64,
    gas_charge: u64,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let task = shared_task_arg(transaction, task)?;
        let leader_registry_ref = transaction.objects().leader_registry.clone();
        let leader_registry = transaction
            .shared_object(&leader_registry_ref, false)
            .map_err(SchedulerError::transaction)?;
        let leader_cap = transaction
            .shared_object(leader_cap, false)
            .map_err(SchedulerError::transaction)?;
        let occurrence_id = transaction
            .arg(&occurrence_id)
            .map_err(SchedulerError::transaction)?;
        let gas_charge = transaction
            .arg(&gas_charge)
            .map_err(SchedulerError::transaction)?;
        let clock = transaction.clock().map_err(SchedulerError::transaction)?;
        transaction
            .call_target(
                scheduler_binding::expire_with_gas_charge_target,
                vec![
                    task,
                    leader_registry,
                    leader_cap,
                    occurrence_id,
                    gas_charge,
                    clock,
                ],
            )
            .map_err(SchedulerError::transaction)?;
        Ok(())
    })
}

pub(crate) fn append_expire_occurrence(
    transaction: &mut NexusPtbBuilder,
    task: &sui::types::ObjectReference,
    occurrence_id: u64,
) -> Result<(), SchedulerError> {
    let task = shared_task_arg(transaction, task)?;
    let occurrence_id = transaction
        .arg(&occurrence_id)
        .map_err(SchedulerError::transaction)?;
    let leader_registry_ref = transaction.objects().leader_registry.clone();
    let leader_registry = transaction
        .shared_object(&leader_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let clock = transaction.clock().map_err(SchedulerError::transaction)?;
    transaction
        .call_target(
            scheduler_binding::expire_target,
            vec![task, occurrence_id, leader_registry, clock],
        )
        .map_err(SchedulerError::transaction)?;
    Ok(())
}

pub(crate) fn append_dispatch_occurrence(
    transaction: &mut NexusPtbBuilder,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    occurrence_id: u64,
    gas_charge: u64,
    tool_cashiers: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> Result<(), SchedulerError> {
    append_dispatch_occurrence_(
        transaction,
        task,
        dag,
        leader_cap,
        occurrence_id,
        gas_charge,
        tool_cashiers,
    )
}

/// Builds an occurrence dispatch with an explicit leader submission gas charge.
pub fn dispatch_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    occurrence_id: u64,
    gas_charge: u64,
    tool_cashiers: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        append_dispatch_occurrence_(
            transaction,
            task,
            dag,
            leader_cap,
            occurrence_id,
            gas_charge,
            tool_cashiers,
        )
    })
}

fn append_dispatch_occurrence_(
    transaction: &mut NexusPtbBuilder,
    task: &sui::types::ObjectReference,
    dag: &sui::types::ObjectReference,
    leader_cap: &sui::types::ObjectReference,
    occurrence_id: u64,
    gas_charge: u64,
    tool_cashiers: &HashSet<(sui::types::Address, sui::types::Version)>,
) -> Result<(), SchedulerError> {
    let protocol_ref = transaction.objects().protocol.clone();
    let protocol = transaction
        .shared_object(&protocol_ref, false)
        .map_err(SchedulerError::transaction)?;
    let task = shared_task_arg(transaction, task)?;
    let dag = transaction
        .shared_object(dag, false)
        .map_err(SchedulerError::transaction)?;
    let agent_registry_ref = transaction.objects().agent_registry.clone();
    let agent_registry = transaction
        .shared_object(&agent_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let tool_registry_ref = transaction.objects().tool_registry.clone();
    let tool_registry = transaction
        .shared_object(&tool_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let leader_registry_ref = transaction.objects().leader_registry.clone();
    let leader_registry = transaction
        .shared_object(&leader_registry_ref, false)
        .map_err(SchedulerError::transaction)?;
    let leader_cap = transaction
        .shared_object(leader_cap, false)
        .map_err(SchedulerError::transaction)?;
    let occurrence_id = transaction
        .arg(&occurrence_id)
        .map_err(SchedulerError::transaction)?;
    let gas_charge = transaction
        .arg(&gas_charge)
        .map_err(SchedulerError::transaction)?;
    let clock = transaction.clock().map_err(SchedulerError::transaction)?;
    let execution = transaction
        .call_target(
            scheduler_binding::dispatch_next_target,
            vec![
                protocol,
                task,
                dag,
                agent_registry,
                tool_registry,
                leader_registry,
                leader_cap,
                occurrence_id,
                gas_charge,
                clock,
            ],
        )
        .map_err(SchedulerError::transaction)?;

    transaction
        .call_target(
            tool_cashier_adapter_binding::snapshot_dag_invocation_costs_target,
            vec![tool_registry, execution, dag],
        )
        .map_err(SchedulerError::transaction)?;

    let mut tool_cashiers = tool_cashiers.iter().copied().collect::<Vec<_>>();
    tool_cashiers.sort_unstable();
    for (address, version) in tool_cashiers {
        let tool_cashier = transaction
            .shared_object_by_id(address, version, true)
            .map_err(SchedulerError::transaction)?;
        transaction
            .call_target(
                tool_cashier_adapter_binding::lock_payment_state_for_tool_target,
                vec![tool_cashier, dag, execution],
            )
            .map_err(SchedulerError::transaction)?;
    }
    transaction
        .call_target(
            execution_entries_binding::start_and_share_target,
            vec![dag, execution, leader_registry, clock],
        )
        .map_err(SchedulerError::transaction)?;
    Ok(())
}

pub(crate) fn settle_occurrence_ptb(
    objects: &NexusObjects,
    task: &sui::types::ObjectReference,
    execution: &sui::types::ObjectReference,
) -> Result<ProgrammableTransaction, SchedulerError> {
    ptb(objects, |transaction| {
        let execution = transaction
            .shared_object(execution, true)
            .map_err(SchedulerError::transaction)?;
        let leader_registry = transaction
            .shared_object(&objects.leader_registry, false)
            .map_err(SchedulerError::transaction)?;
        let clock = transaction.clock().map_err(SchedulerError::transaction)?;
        append_settle_occurrence(transaction, task, execution, leader_registry, clock)
    })
}

pub(crate) fn append_settle_occurrence(
    transaction: &mut NexusPtbBuilder,
    task: &sui::types::ObjectReference,
    execution: Argument,
    leader_registry: Argument,
    clock: Argument,
) -> Result<(), SchedulerError> {
    let task = shared_task_arg(transaction, task)?;
    transaction
        .call_target(
            scheduler_binding::settle_target,
            vec![task, execution, leader_registry, clock],
        )
        .map_err(SchedulerError::transaction)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_boundary::NexusPtbBuilder,
            test_utils::sui_mocks::{mock_nexus_objects, object_ref_for_id},
            transactions::scheduler::compose::TaskDraftCompiler,
        },
        std::collections::BTreeMap,
        sui_sdk_types::{Argument, Command, Input, MoveCall},
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn task() -> PreparedTask {
        PreparedTask {
            operation: TaskOperation::default_dag(address("0x42")),
            agent: None,
            entry_group: "main".to_owned(),
            inputs: BTreeMap::new(),
            funding: PreparedFunding::Address {
                prepay_amount_mist: 10_000,
                refund_recipient: address("0x44"),
            },
            occurrence_budget_mist: 1_000,
            failure_policy: FailurePolicy::Continue,
        }
    }

    fn schedule() -> PreparedSchedule {
        PreparedSchedule::new(
            vec![
                PreparedOccurrence::new(100, None, 20),
                PreparedOccurrence::new(200, Some(250), 30),
            ],
            Some(PreparedRecurrence::new(
                PreparedOccurrence::new(300, None, 40),
                100,
                Some(3),
            )),
        )
    }

    fn move_calls(transaction: &ProgrammableTransaction) -> impl Iterator<Item = &MoveCall> {
        transaction
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) => Some(call),
                _ => None,
            })
    }

    fn pure_u64(transaction: &ProgrammableTransaction, argument: Argument) -> u64 {
        let Argument::Input(index) = argument else {
            panic!("expected pure input argument");
        };
        let Input::Pure(bytes) = &transaction.inputs[usize::from(index)] else {
            panic!("expected pure input");
        };
        u64::from_le_bytes(bytes.as_slice().try_into().expect("u64 input"))
    }

    fn assert_shared_argument(
        transaction: &ProgrammableTransaction,
        argument: Argument,
        expected: &sui::types::ObjectReference,
        expected_mutable: bool,
    ) {
        let Argument::Input(index) = argument else {
            panic!("expected input argument");
        };
        let Input::Shared(shared) = &transaction.inputs[usize::from(index)] else {
            panic!("expected shared object input");
        };
        assert_eq!(shared.object_id(), *expected.object_id());
        assert_eq!(shared.version(), expected.version());
        assert_eq!(shared.mutability().is_mutable(), expected_mutable);
    }

    fn scheduler_sequence(transaction: &ProgrammableTransaction) -> Vec<&str> {
        const STRUCTURAL_CALLS: &[&str] = &[
            "new_default_agent_execution_config_v2",
            "new_agent_execution_config_v2",
            "new_default_task_v2",
            "new_user_task_v2",
            "new_agent_task_v2",
            "schedule",
            "schedule_as_agent",
            "set_recurrence",
            "set_recurrence_as_agent",
            "share",
        ];

        move_calls(transaction)
            .map(|call| call.function.as_str())
            .filter(|function| STRUCTURAL_CALLS.contains(function))
            .collect()
    }

    #[test]
    fn complete_schedule_has_one_structural_command_path() {
        let transaction =
            schedule_task_ptb(&mock_nexus_objects(), &task(), &schedule(), address("0x46"))
                .expect("complete Task compiles");

        assert_eq!(
            scheduler_sequence(&transaction),
            [
                "new_default_agent_execution_config_v2",
                "new_default_task_v2",
                "schedule",
                "schedule",
                "set_recurrence",
                "share",
            ]
        );
    }

    #[test]
    fn empty_creation_is_composable() {
        let objects = mock_nexus_objects();
        let transaction =
            create_task_ptb(&objects, &task(), address("0x46")).expect("empty Task compiles");

        assert_eq!(
            scheduler_sequence(&transaction),
            [
                "new_default_agent_execution_config_v2",
                "new_default_task_v2",
                "share",
            ]
        );
        let constructor = move_calls(&transaction)
            .find(|call| call.function.as_str() == "new_default_task_v2")
            .expect("default Task constructor");
        assert_shared_argument(
            &transaction,
            constructor.arguments[1],
            &objects.leader_registry,
            false,
        );
    }

    #[test]
    fn composer_and_complete_compiler_are_identical() {
        let objects = mock_nexus_objects();
        let task = task();
        let schedule = schedule();
        let pointer_owner = address("0x46");
        let complete =
            schedule_task_ptb(&objects, &task, &schedule, pointer_owner).expect("complete compile");

        let mut builder = NexusPtbBuilder::new(std::sync::Arc::new(objects.clone()));
        TaskDraftCompiler::create(&mut builder, &task)
            .expect("draft creation")
            .schedule(&schedule)
            .expect("draft scheduling")
            .share(pointer_owner)
            .expect("draft sharing");
        let composed = builder.finish();

        assert_eq!(composed, complete);
    }

    #[test]
    fn agent_authority_selects_the_agent_suffix_once() {
        let agent_id = address("0x45");
        let agent = AgentInput::Shared(object_ref_for_id(agent_id));
        let task = PreparedTask {
            operation: TaskOperation::agent_skill(agent_id, 7, None, vec![]),
            agent: Some(agent),
            entry_group: "main".to_owned(),
            inputs: BTreeMap::new(),
            funding: PreparedFunding::Agent {
                prepay_amount_mist: 10_000,
            },
            occurrence_budget_mist: 1_000,
            failure_policy: FailurePolicy::Continue,
        };
        let transaction = schedule_task_ptb(
            &mock_nexus_objects(),
            &task,
            &PreparedSchedule::new(vec![PreparedOccurrence::new(100, None, 20)], None),
            address("0x46"),
        )
        .expect("Agent Task compiles");

        assert_eq!(
            scheduler_sequence(&transaction),
            [
                "new_agent_execution_config_v2",
                "new_agent_task_v2",
                "schedule_as_agent",
                "share",
            ]
        );
    }

    #[test]
    fn creation_transfers_the_pointer_result_to_its_owner() {
        let transaction = create_task_ptb(&mock_nexus_objects(), &task(), address("0x46"))
            .expect("Task compiles");
        let new_task_index = transaction
            .commands
            .iter()
            .position(|command| {
                matches!(
                    command,
                    Command::MoveCall(call) if call.function.as_str() == "new_default_task_v2"
                )
            })
            .expect("Task constructor");
        let (objects, recipient) = transaction
            .commands
            .iter()
            .find_map(|command| match command {
                Command::TransferObjects(transfer) => Some((&transfer.objects, transfer.address)),
                _ => None,
            })
            .expect("TaskPointer transfer");

        assert_eq!(
            objects,
            &[Argument::NestedResult(
                new_task_index.try_into().expect("command index"),
                1,
            )]
        );
        assert!(matches!(recipient, Argument::Input(_)));
    }

    #[test]
    fn dispatch_uses_the_protocol_as_its_first_argument() {
        let objects = mock_nexus_objects();
        let task = object_ref_for_id(address("0x50"));
        let dag = object_ref_for_id(address("0x51"));
        let leader_cap = object_ref_for_id(address("0x52"));
        let mut builder = NexusPtbBuilder::new(std::sync::Arc::new(objects.clone()));

        append_dispatch_occurrence(
            &mut builder,
            &task,
            &dag,
            &leader_cap,
            7,
            0,
            &HashSet::new(),
        )
        .expect("dispatch compiles");
        let transaction = builder.finish();
        let dispatch = move_calls(&transaction)
            .find(|call| call.function.as_str() == "dispatch_next")
            .expect("dispatch call");

        assert_eq!(dispatch.arguments.len(), 10);
        assert_eq!(pure_u64(&transaction, dispatch.arguments[8]), 0);
        let Argument::Input(protocol_index) = dispatch.arguments[0] else {
            panic!("expected protocol input argument");
        };
        let Input::Shared(protocol) = &transaction.inputs[usize::from(protocol_index)] else {
            panic!("expected shared protocol input");
        };
        assert_eq!(protocol.object_id(), *objects.protocol.object_id());
        assert_eq!(protocol.version(), objects.protocol.version());
        assert!(!protocol.mutability().is_mutable());
    }

    #[test]
    fn dispatch_serializes_the_submission_gas_charge() {
        let objects = mock_nexus_objects();
        let task = object_ref_for_id(address("0x50"));
        let dag = object_ref_for_id(address("0x51"));
        let leader_cap = object_ref_for_id(address("0x52"));

        let transaction =
            dispatch_occurrence_ptb(&objects, &task, &dag, &leader_cap, 7, 42, &HashSet::new())
                .expect("charged dispatch compiles");
        let dispatch = move_calls(&transaction)
            .find(|call| call.function.as_str() == "dispatch_next")
            .expect("charged dispatch call");

        assert_eq!(dispatch.arguments.len(), 10);
        assert_eq!(pure_u64(&transaction, dispatch.arguments[8]), 42);
    }

    #[test]
    fn charged_expiration_serializes_the_submission_gas_charge() {
        let objects = mock_nexus_objects();
        let task = object_ref_for_id(address("0x50"));
        let leader_cap = object_ref_for_id(address("0x52"));

        let transaction =
            expire_occurrence_with_gas_charge_ptb(&objects, &task, &leader_cap, 7, 42)
                .expect("charged expiration compiles");
        let expiration = move_calls(&transaction)
            .find(|call| call.function.as_str() == "expire_with_gas_charge")
            .expect("charged expiration call");

        assert_eq!(expiration.arguments.len(), 6);
        assert_eq!(pure_u64(&transaction, expiration.arguments[4]), 42);
    }
}
