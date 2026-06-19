use crate::{idents::ModuleAndNameIdent, sui};

// == `nexus_scheduler::scheduler` ==

pub struct Scheduler;

pub const SCHEDULER_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("scheduler");

impl Scheduler {
    /// Enqueue a new occurrence for a task with explicit deadline.
    ///
    /// `nexus_scheduler::scheduler::add_occurrence_absolute_for_task`
    pub const ADD_OCCURRENCE_ABSOLUTE_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("add_occurrence_absolute_for_task"),
    };
    /// Enqueue a new occurrence relative to the current time.
    ///
    /// `nexus_scheduler::scheduler::add_occurrence_relative_for_task`
    pub const ADD_OCCURRENCE_RELATIVE_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("add_occurrence_relative_for_task"),
    };
    /// Cancel scheduling for a task.
    ///
    /// `nexus_scheduler::scheduler::cancel`
    pub const CANCEL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("cancel"),
    };
    /// Cancel scheduling for an explicit-agent task.
    ///
    /// `nexus_scheduler::scheduler::cancel_with_agent`
    pub const CANCEL_WITH_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("cancel_with_agent"),
    };
    /// Run scheduler checks to consume the next periodic occurrence.
    ///
    /// `nexus_scheduler::scheduler::check_periodic_occurrence`
    pub const CHECK_PERIODIC_OCCURRENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("check_periodic_occurrence"),
    };
    /// Run scheduler checks to consume the next queue occurrence.
    ///
    /// `nexus_scheduler::scheduler::check_queue_occurrence`
    pub const CHECK_QUEUE_OCCURRENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("check_queue_occurrence"),
    };
    /// Collect an idle agent-funded scheduled payment reserve back to the agent vault.
    ///
    /// `nexus_scheduler::scheduler::collect_idle_agent_funded_scheduled_payment_reserve_to_vault`
    pub const COLLECT_IDLE_AGENT_FUNDED_SCHEDULED_PAYMENT_RESERVE_TO_VAULT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "collect_idle_agent_funded_scheduled_payment_reserve_to_vault",
            ),
        };
    /// Disable periodic scheduling for a task.
    ///
    /// `nexus_scheduler::scheduler::disable_periodic_for_task`
    pub const DISABLE_PERIODIC_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("disable_periodic_for_task"),
    };
    /// Execute the task policy advancing logic.
    ///
    /// `nexus_scheduler::scheduler::execute`
    pub const EXECUTE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("execute"),
    };
    /// Finalize a task run ensuring policies reached accepting states.
    ///
    /// `nexus_scheduler::scheduler::finish`
    pub const FINISH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("finish"),
    };
    /// Creates a new task with metadata and policies.
    ///
    /// `nexus_scheduler::scheduler::new`
    pub const NEW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new"),
    };
    /// Creates an agent-owned scheduled task with agent-funded payment reserve components.
    ///
    /// `nexus_scheduler::scheduler::new_agent_funded_task`
    pub const NEW_AGENT_FUNDED_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_agent_funded_task"),
    };
    /// Creates the default constraints policy.
    ///
    /// `nexus_scheduler::scheduler::new_constraints_policy`
    pub const NEW_CONSTRAINTS_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_constraints_policy"),
    };
    /// Creates a scheduled task for the registry-owned default agent.
    ///
    /// `nexus_scheduler::scheduler::new_default_agent_task`
    pub const NEW_DEFAULT_AGENT_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_default_agent_task"),
    };
    /// Creates the default execution policy.
    ///
    /// `nexus_scheduler::scheduler::new_execution_policy`
    pub const NEW_EXECUTION_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_execution_policy"),
    };
    /// Creates a sender-owned invoker-funded scheduled task with payment reserve components.
    ///
    /// `nexus_scheduler::scheduler::new_invoker_funded_agent_task`
    pub const NEW_INVOKER_FUNDED_AGENT_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_invoker_funded_agent_task"),
    };
    /// Creates a sender-owned default-agent scheduled task with invoker-funded payment reserve components.
    ///
    /// `nexus_scheduler::scheduler::new_invoker_funded_default_agent_task`
    pub const NEW_INVOKER_FUNDED_DEFAULT_AGENT_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_invoker_funded_default_agent_task"),
    };
    /// Creates a metadata container from key/value pairs.
    ///
    /// `nexus_scheduler::scheduler::new_metadata`
    pub const NEW_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_metadata"),
    };
    /// Configure or update periodic scheduling for a task.
    ///
    /// `nexus_scheduler::scheduler::new_or_modify_periodic_for_task`
    pub const NEW_OR_MODIFY_PERIODIC_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_or_modify_periodic_for_task"),
    };
    /// Create a periodic generator state.
    ///
    /// `nexus_scheduler::scheduler::new_periodic_generator_state`
    pub const NEW_PERIODIC_GENERATOR_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_periodic_generator_state"),
    };
    /// Create a queue generator state.
    ///
    /// `nexus_scheduler::scheduler::new_queue_generator_state`
    pub const NEW_QUEUE_GENERATOR_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_queue_generator_state"),
    };
    /// Pause the scheduler for a task.
    ///
    /// `nexus_scheduler::scheduler::pause`
    pub const PAUSE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("pause"),
    };
    /// Pause the scheduler for an explicit-agent task.
    ///
    /// `nexus_scheduler::scheduler::pause_with_agent`
    pub const PAUSE_WITH_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("pause_with_agent"),
    };
    /// The PeriodicGeneratorState struct type.
    ///
    /// `nexus_scheduler::scheduler::PeriodicGeneratorState`
    pub const PERIODIC_GENERATOR_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("PeriodicGeneratorState"),
    };
    /// Witness type registered for periodic generators.
    ///
    /// `nexus_scheduler::scheduler::PeriodicGeneratorWitness`
    pub const PERIODIC_GENERATOR_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("PeriodicGeneratorWitness"),
    };
    /// Prepare a registered DAG execution from durable scheduled payment.
    ///
    /// `nexus_scheduler::scheduler::prepare_agent_execution_from_scheduled_payment`
    pub const PREPARE_AGENT_EXECUTION_FROM_SCHEDULED_PAYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "prepare_agent_execution_from_scheduled_payment",
            ),
        };
    /// Prepare default DAG execution from durable scheduled payment.
    ///
    /// `nexus_scheduler::scheduler::prepare_default_agent_execution_from_scheduled_payment`
    pub const PREPARE_DEFAULT_AGENT_EXECUTION_FROM_SCHEDULED_PAYMENT: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "prepare_default_agent_execution_from_scheduled_payment",
            ),
        };
    /// Prepare default DAG execution using an immediate payment coin.
    ///
    /// `nexus_scheduler::scheduler::prepare_default_agent_execution_from_scheduler`
    pub const PREPARE_DEFAULT_AGENT_EXECUTION_FROM_SCHEDULER: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "prepare_default_agent_execution_from_scheduler",
            ),
        };
    /// The QueueGeneratorState struct type.
    ///
    /// `nexus_scheduler::scheduler::QueueGeneratorState`
    pub const QUEUE_GENERATOR_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("QueueGeneratorState"),
    };
    /// Witness type registered for queue-based generators.
    ///
    /// `nexus_scheduler::scheduler::QueueGeneratorWitness`
    pub const QUEUE_GENERATOR_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("QueueGeneratorWitness"),
    };
    /// Register the workflow registered-agent execution config on the execution policy.
    ///
    /// `nexus_scheduler::scheduler::register_begin_agent_execution`
    pub const REGISTER_BEGIN_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("register_begin_agent_execution"),
    };
    /// Register the workflow default DAG execution config on the execution policy.
    ///
    /// `nexus_scheduler::scheduler::register_begin_default_agent_execution`
    pub const REGISTER_BEGIN_DEFAULT_AGENT_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("register_begin_default_agent_execution"),
    };
    /// Register the periodic generator state on the constraints policy.
    ///
    /// `nexus_scheduler::scheduler::register_periodic_generator`
    pub const REGISTER_PERIODIC_GENERATOR: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("register_periodic_generator"),
    };
    /// Register the queue generator state on the constraints policy.
    ///
    /// `nexus_scheduler::scheduler::register_queue_generator`
    pub const REGISTER_QUEUE_GENERATOR: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("register_queue_generator"),
    };
    /// Resume the scheduler for a task.
    ///
    /// `nexus_scheduler::scheduler::resume`
    pub const RESUME: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("resume"),
    };
    /// Resume the scheduler for an explicit-agent task.
    ///
    /// `nexus_scheduler::scheduler::resume_with_agent`
    pub const RESUME_WITH_AGENT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("resume_with_agent"),
    };
    /// Settle an address-funded scheduled TAP execution payment if the execution is finished.
    ///
    /// `nexus_scheduler::scheduler::settle_finished_scheduled_tap_execution_payment_if_ready`
    pub const SETTLE_FINISHED_SCHEDULED_EXECUTION_PAYMENT_IF_READY: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: SCHEDULER_MODULE,
            name: sui::types::Identifier::from_static(
                "settle_finished_scheduled_execution_payment_if_ready",
            ),
        };
    /// The Task struct type. Mostly used for creating generic types.
    ///
    /// `nexus_scheduler::scheduler::Task`
    pub const TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("Task"),
    };
    /// The TimeConstraint struct type.
    ///
    /// `nexus_scheduler::scheduler::TimeConstraint`
    pub const TIME_CONSTRAINT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("TimeConstraint"),
    };
    /// Updates task metadata with the provided values.
    ///
    /// `nexus_scheduler::scheduler::update_metadata`
    pub const UPDATE_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("update_metadata"),
    };
}

/// Helper to turn a scheduler `ModuleAndNameIdent` into a `sui::types::TypeTag`.
pub fn into_type_tag(
    scheduler_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        scheduler_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}
