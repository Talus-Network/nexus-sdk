use crate::{
    idents::{pure_arg, sui_framework::Address, ModuleAndNameIdent},
    sui,
    types::{EdgeKind, RuntimeVertex},
    ToolFqn,
};

// == `nexus_workflow::default_tap` ==

pub struct DefaultTap;

const DEFAULT_TAP_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("default_tap");

impl DefaultTap {
    /// This function is called when a DAG is to be executed using the default
    /// TAP implementation.
    ///
    /// `nexus_workflow::default_tap::begin_dag_execution`
    pub const BEGIN_DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::types::Identifier::from_static("begin_dag_execution"),
    };
    /// The witness type needed to register DAG execution.
    ///
    /// `nexus_workflow::default_tap::BeginDagExecutionWitness`
    pub const BEGIN_DAG_EXECUTION_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::types::Identifier::from_static("BeginDagExecutionWitness"),
    };
    /// Scheduler entry point to invoke DAG execution via the default TAP.
    ///
    /// `nexus_workflow::default_tap::dag_begin_execution_from_scheduler`
    pub const DAG_BEGIN_EXECUTION_FROM_SCHEDULER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::types::Identifier::from_static("dag_begin_execution_from_scheduler"),
    };
    /// The DefaultTAP struct type.
    ///
    /// `nexus_workflow::default_tap::DefaultTAP`
    pub const DEFAULT_TAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::types::Identifier::from_static("DefaultTAP"),
    };
    /// Register DAG execution configuration on the execution policy.
    ///
    /// `nexus_workflow::default_tap::register_begin_execution`
    pub const REGISTER_BEGIN_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::types::Identifier::from_static("register_begin_execution"),
    };
}

// == `nexus_workflow::scheduler` ==

pub struct Scheduler;

const SCHEDULER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("scheduler");

impl Scheduler {
    /// Enqueue a new occurrence for a task with explicit deadline.
    ///
    /// `nexus_workflow::scheduler::add_occurrence_absolute_for_task`
    pub const ADD_OCCURRENCE_ABSOLUTE_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("add_occurrence_absolute_for_task"),
    };
    /// Enqueue a new occurrence relative to the current time.
    ///
    /// `nexus_workflow::scheduler::add_occurrence_relative_for_task`
    pub const ADD_OCCURRENCE_RELATIVE_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("add_occurrence_relative_for_task"),
    };
    /// Cancel scheduling for a task.
    ///
    /// `nexus_workflow::scheduler::cancel_time_constraint_for_task`
    pub const CANCEL_TIME_CONSTRAINT_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("cancel_time_constraint_for_task"),
    };
    /// Run scheduler checks to consume the next periodic occurrence.
    ///
    /// `nexus_workflow::scheduler::check_periodic_occurrence`
    pub const CHECK_PERIODIC_OCCURRENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("check_periodic_occurrence"),
    };
    /// Run scheduler checks to consume the next queue occurrence.
    ///
    /// `nexus_workflow::scheduler::check_queue_occurrence`
    pub const CHECK_QUEUE_OCCURRENCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("check_queue_occurrence"),
    };
    /// Disable periodic scheduling for a task.
    ///
    /// `nexus_workflow::scheduler::disable_periodic_for_task`
    pub const DISABLE_PERIODIC_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("disable_periodic_for_task"),
    };
    /// Execute the DAG witness advancing logic.
    ///
    /// `nexus_workflow::scheduler::execute`
    pub const EXECUTE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("execute"),
    };
    /// Finalize a task run ensuring policies reached accepting states.
    ///
    /// `nexus_workflow::scheduler::finish`
    pub const FINISH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("finish"),
    };
    /// Creates a new task with metadata and policies.
    ///
    /// `nexus_workflow::scheduler::new`
    pub const NEW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new"),
    };
    /// Creates the default constraints policy.
    ///
    /// `nexus_workflow::scheduler::new_constraints_policy`
    pub const NEW_CONSTRAINTS_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_constraints_policy"),
    };
    /// Creates the default execution policy.
    ///
    /// `nexus_workflow::scheduler::new_execution_policy`
    pub const NEW_EXECUTION_POLICY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_execution_policy"),
    };
    /// Creates a metadata container from key/value pairs.
    ///
    /// `nexus_workflow::scheduler::new_metadata`
    pub const NEW_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_metadata"),
    };
    /// Configure or update periodic scheduling for a task.
    ///
    /// `nexus_workflow::scheduler::new_or_modify_periodic_for_task`
    pub const NEW_OR_MODIFY_PERIODIC_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_or_modify_periodic_for_task"),
    };
    /// Create a periodic generator state.
    ///
    /// `nexus_workflow::scheduler::new_periodic_generator_state`
    pub const NEW_PERIODIC_GENERATOR_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_periodic_generator_state"),
    };
    /// Create a queue generator state.
    ///
    /// `nexus_workflow::scheduler::new_queue_generator_state`
    pub const NEW_QUEUE_GENERATOR_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("new_queue_generator_state"),
    };
    /// Pause the scheduler for a task.
    ///
    /// `nexus_workflow::scheduler::pause_time_constraint_for_task`
    pub const PAUSE_TIME_CONSTRAINT_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("pause_time_constraint_for_task"),
    };
    /// Witness type registered for periodic generators.
    ///
    /// `nexus_workflow::scheduler::PeriodicGeneratorWitness`
    pub const PERIODIC_GENERATOR_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("PeriodicGeneratorWitness"),
    };
    /// Witness type registered for queue-based generators.
    ///
    /// `nexus_workflow::scheduler::QueueGeneratorWitness`
    pub const QUEUE_GENERATOR_WITNESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("QueueGeneratorWitness"),
    };
    /// Register the periodic generator state on the constraints policy.
    ///
    /// `nexus_workflow::scheduler::register_periodic_generator`
    pub const REGISTER_PERIODIC_GENERATOR: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("register_periodic_generator"),
    };
    /// Register the queue generator state on the constraints policy.
    ///
    /// `nexus_workflow::scheduler::register_queue_generator`
    pub const REGISTER_QUEUE_GENERATOR: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("register_queue_generator"),
    };
    /// Resume the scheduler for a task.
    ///
    /// `nexus_workflow::scheduler::resume_time_constraint_for_task`
    pub const RESUME_TIME_CONSTRAINT_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("resume_time_constraint_for_task"),
    };
    /// The Task struct type. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::scheduler::Task`
    pub const TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("Task"),
    };
    /// The TimeConstraint struct type.
    ///
    /// `nexus_workflow::scheduler::TimeConstraint`
    pub const TIME_CONSTRAINT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("TimeConstraint"),
    };
    /// Updates task metadata with the provided values.
    ///
    /// `nexus_workflow::scheduler::update_metadata`
    pub const UPDATE_METADATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::types::Identifier::from_static("update_metadata"),
    };
}
// == `nexus_workflow::dag` ==

pub struct Dag;

const DAG_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("dag");

impl Dag {
    /// The DAG struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::DAG`
    pub const DAG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("DAG"),
    };
    /// The DAGExecution struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::DAGExecution`
    pub const DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("DAGExecution"),
    };
    /// Create a break edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_break`
    pub const EDGE_KIND_BREAK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_break"),
    };
    /// Create a collect edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_collect`
    pub const EDGE_KIND_COLLECT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_collect"),
    };
    /// Create a do-while edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_do_while`
    pub const EDGE_KIND_DO_WHILE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_do_while"),
    };
    /// Create a for-each edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_for_each`
    pub const EDGE_KIND_FOR_EACH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_for_each"),
    };
    /// Create a normal edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_normal`
    pub const EDGE_KIND_NORMAL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_normal"),
    };
    /// Create an encrypted InputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::encrypted_input_port_from_string`
    pub const ENCRYPTED_INPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("encrypted_input_port_from_string"),
    };
    /// The EntryGroup struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::EntryGroup`
    pub const ENTRY_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("EntryGroup"),
    };
    /// Create an EntryGroup from an ASCII string.
    ///
    /// `nexus_workflow::dag::entry_group_from_string`
    pub const ENTRY_GROUP_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("entry_group_from_string"),
    };
    /// The InputPort struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::InputPort`
    pub const INPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("InputPort"),
    };
    /// Create an InputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::input_port_from_string`
    pub const INPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("input_port_from_string"),
    };
    /// Create a new DAG object.
    ///
    /// `nexus_workflow::dag::new`
    pub const NEW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("new"),
    };
    /// Create a new DAG execution config value.
    ///
    /// `nexus_workflow::dag::new_dag_execution_config`
    pub const NEW_DAG_EXECUTION_CONFIG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("new_dag_execution_config"),
    };
    /// The OutputPort struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::OutputPort`
    pub const OUTPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("OutputPort"),
    };
    /// Create an OutputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::output_port_from_string`
    pub const OUTPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("output_port_from_string"),
    };
    /// The OutputVariant struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::OutputVariant`
    pub const OUTPUT_VARIANT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("OutputVariant"),
    };
    /// Create an OutputVariant from an ASCII string.
    ///
    /// `nexus_workflow::dag::output_variant_from_string`
    pub const OUTPUT_VARIANT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("output_variant_from_string"),
    };
    /// Function to call to continue to the next vertex in the given walk.
    ///
    /// `nexus_workflow::dag::request_network_to_execute_walks`
    pub const REQUEST_NETWORK_TO_EXECUTE_WALKS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("request_network_to_execute_walks"),
    };
    /// Returns a new hot potato object RequestWalkExecution.
    ///
    /// `nexus_workflow::dag::request_walk_execution`
    pub const REQUEST_WALK_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("request_walk_execution"),
    };
    /// Create a `RuntimeVertex::Plain` from a string.
    ///
    /// `nexus_workflow::dag::runtime_vertex_plain_from_string`
    pub const RUNTIME_VERTEX_PLAIN_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("runtime_vertex_plain_from_string"),
    };
    /// Create a `RuntimeVertex::WithIterator` from a string.
    ///
    /// `nexus_workflow::dag::runtime_vertex_with_iterator_from_string`
    pub const RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("runtime_vertex_with_iterator_from_string"),
    };
    /// One of the functions to call when an off-chain tool is evaluated to submit
    /// its result to the workflow.
    ///
    /// `nexus_workflow::dag::submit_off_chain_tool_eval_for_walk`
    pub const SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("submit_off_chain_tool_eval_for_walk"),
    };
    /// One of the functions to call when an on-chain tool is evaluated to submit
    /// its result to the workflow.
    ///
    /// `nexus_workflow::dag::submit_on_chain_tool_eval_for_walk`
    // TODO: <https://github.com/Talus-Network/nexus-next/issues/30>
    pub const SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("submit_on_chain_tool_eval_for_walk"),
    };
    /// The Vertex struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::Vertex`
    pub const VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("Vertex"),
    };
    /// Create a Vertex from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_from_string`
    pub const VERTEX_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("vertex_from_string"),
    };
    /// Create a new off-chain NodeIdent from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_off_chain`
    pub const VERTEX_OFF_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("vertex_off_chain"),
    };
    /// Create a new onchain NodeIdent from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_on_chain`
    pub const VERTEX_ON_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("vertex_on_chain"),
    };
    /// Add a default value to a DAG. Default value is a Vertex + InputPort pair
    /// with NexusData as the value.
    ///
    /// `nexus_workflow::dag::with_default_value`
    pub const WITH_DEFAULT_VALUE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_value"),
    };
    /// Add an Edge to a DAG.
    ///
    /// `nexus_workflow::dag::with_edge`
    pub const WITH_EDGE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_edge"),
    };
    /// Add an encrypted Edge to a DAG.
    ///
    /// `nexus_workflow::dag::with_encrypted_edge`
    pub const WITH_ENCRYPTED_EDGE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_encrypted_edge"),
    };
    /// Add an encrypted output to a DAG.
    ///
    /// `nexus_workflow::dag::with_encrypted_output`
    pub const WITH_ENCRYPTED_OUTPUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_encrypted_output"),
    };
    /// Mark a vertex as an entry vertex and assign it to a group.
    ///
    /// `nexus_workflow::dag::with_entry_in_group`
    pub const WITH_ENTRY_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_in_group"),
    };
    /// Add an input port as an entry input port and assign it to a group.
    ///
    /// `nexus_workflow::dag::with_entry_port_in_group`
    pub const WITH_ENTRY_PORT_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_port_in_group"),
    };
    /// Add an entry vertex to a DAG. Entry vertex is just a Vertex with its
    /// required InputPorts specified.
    ///
    /// `nexus_workflow::dag::with_entry_vertex`
    pub const WITH_ENTRY_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_vertex"),
    };
    /// Add an output to a DAG.
    ///
    /// `nexus_workflow::dag::with_output`
    pub const WITH_OUTPUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_output"),
    };
    /// Add a Vertex to a DAG.
    ///
    /// `nexus_workflow::dag::with_vertex`
    pub const WITH_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex"),
    };

    /// Create an EntryGroup from a string.
    pub fn entry_group_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::ENTRY_GROUP_FROM_STRING.module,
                Self::ENTRY_GROUP_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an InputPort from a string.
    pub fn input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::INPUT_PORT_FROM_STRING.module,
                Self::INPUT_PORT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an encrypted InputPort from a string.
    pub fn encrypted_input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::ENCRYPTED_INPUT_PORT_FROM_STRING.module,
                Self::ENCRYPTED_INPUT_PORT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an OutputPort from a string.
    pub fn output_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::OUTPUT_PORT_FROM_STRING.module,
                Self::OUTPUT_PORT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create an OutputVariant from a string.
    pub fn output_variant_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::OUTPUT_VARIANT_FROM_STRING.module,
                Self::OUTPUT_VARIANT_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create a Vertex from a string.
    pub fn vertex_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_FROM_STRING.module,
                Self::VERTEX_FROM_STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }

    /// Create a new off-chain NodeIdent from a string.
    pub fn off_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_OFF_CHAIN.module,
                Self::VERTEX_OFF_CHAIN.name,
                vec![],
            ),
            vec![str],
        ))
    }

    pub fn on_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        tool_registry_id: &sui::types::ObjectReference,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;
        let tool_registry_id = tx.input(sui::tx::Input::shared(
            *tool_registry_id.object_id(),
            tool_registry_id.version(),
            false,
        ));

        let witness_id = tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                ToolRegistry::ONCHAIN_TOOL_WITNESS_ID.module,
                ToolRegistry::ONCHAIN_TOOL_WITNESS_ID.name,
                vec![],
            ),
            vec![tool_registry_id, str],
        );

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_ON_CHAIN.module,
                Self::VERTEX_ON_CHAIN.name,
                vec![],
            ),
            vec![str, witness_id],
        ))
    }

    /// Create an edge kind from an enum variant.
    pub fn edge_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        edge_kind: &EdgeKind,
    ) -> sui::types::Argument {
        let ident = match edge_kind {
            EdgeKind::Normal => Self::EDGE_KIND_NORMAL,
            EdgeKind::ForEach => Self::EDGE_KIND_FOR_EACH,
            EdgeKind::Collect => Self::EDGE_KIND_COLLECT,
            EdgeKind::DoWhile => Self::EDGE_KIND_DO_WHILE,
            EdgeKind::Break => Self::EDGE_KIND_BREAK,
        };

        tx.move_call(
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name, vec![]),
            vec![],
        )
    }

    /// Create a runtime vertex from an enum variant
    pub fn runtime_vertex_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        runtime_vertex: &RuntimeVertex,
    ) -> anyhow::Result<sui::types::Argument> {
        match runtime_vertex {
            RuntimeVertex::Plain { vertex } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;

                Ok(tx.move_call(
                    sui::tx::Function::new(
                        workflow_pkg_id,
                        Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.name,
                        vec![],
                    ),
                    vec![name],
                ))
            }
            RuntimeVertex::WithIterator {
                vertex,
                iteration,
                out_of,
            } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;

                let iteration = tx.input(pure_arg(iteration)?);
                let out_of = tx.input(pure_arg(out_of)?);

                Ok(tx.move_call(
                    sui::tx::Function::new(
                        workflow_pkg_id,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name,
                        vec![],
                    ),
                    vec![name, iteration, out_of],
                ))
            }
        }
    }
}

// == `nexus_workflow::tool_output` ==

pub struct ToolOutput;

const TOOL_OUTPUT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("tool_output");

impl ToolOutput {
    /// Convert ToolOutput to DAG types.
    ///
    /// `nexus_workflow::tool_output::to_dag_types`
    pub const TO_DAG_TYPES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_OUTPUT_MODULE,
        name: sui::types::Identifier::from_static("to_dag_types"),
    };
}

// == `nexus_workflow::tool_registry` ==

pub struct ToolRegistry;

const TOOL_REGISTRY_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("tool_registry");

impl ToolRegistry {
    /// Add an address to the allowlist for tool registration.
    /// Only callable by the holder of OverSlashing cap.
    ///
    /// `nexus_workflow::tool_registry::add_allowed_owner`
    pub const ADD_ALLOWED_OWNER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("add_allowed_owner"),
    };
    /// Claim collateral for a tool and transfer the balance to the tx sender.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral_for_self`
    pub const CLAIM_COLLATERAL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("claim_collateral_for_self"),
    };
    /// Claim collateral for a tool. The function call returns Balance<SUI>.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral_for_tool`
    pub const CLAIM_COLLATERAL_FOR_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("claim_collateral_for_tool"),
    };
    /// Get the witness ID for an onchain tool.
    ///
    /// `nexus_workflow::tool_registry::onchain_tool_witness_id`
    pub const ONCHAIN_TOOL_WITNESS_ID: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("onchain_tool_witness_id"),
    };
    /// OverSlashing struct type. Used to fetch caps for slashing tools.
    ///
    /// `nexus_workflow::tool_registry::OverSlashing`
    pub const OVER_SLASHING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("OverSlashing"),
    };
    /// OverTool struct type. Used for fetching tool owner caps.
    ///
    /// `nexus_workflow::tool_registry::OverTool`
    pub const OVER_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("OverTool"),
    };
    /// Register an off-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_workflow::tool_registry::register_off_chain_tool`
    pub const REGISTER_OFF_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_off_chain_tool"),
    };
    /// Register an off-chain tool and transfer the tool's owner cap to the ctx
    /// sender.
    ///
    /// `nexus_workflow::tool_registry::register_off_chain_tool_for_self`
    pub const REGISTER_OFF_CHAIN_TOOL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_off_chain_tool_for_self"),
    };
    /// Register an on-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_workflow::tool_registry::register_on_chain_tool`
    pub const REGISTER_ON_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_on_chain_tool"),
    };
    /// Register an on-chain tool and transfer the tool's owner cap to the ctx
    /// sender.
    ///
    /// `nexus_workflow::tool_registry::register_on_chain_tool_for_self`
    pub const REGISTER_ON_CHAIN_TOOL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("register_on_chain_tool_for_self"),
    };
    /// Remove an address from the allowlist for tool registration.
    /// Only callable by the holder of OverSlashing cap.
    ///
    /// `nexus_workflow::tool_registry::remove_allowed_owner`
    pub const REMOVE_ALLOWED_OWNER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("remove_allowed_owner"),
    };
    /// The ToolRegistry struct type.
    ///
    /// `nexus_workflow::tool_registry::ToolRegistry`
    pub const TOOL_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("ToolRegistry"),
    };
    /// Unregister a tool.
    ///
    /// `nexus_workflow::tool_registry::unregister_tool`
    pub const UNREGISTER_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::types::Identifier::from_static("unregister_tool"),
    };
}

// == `nexus_workflow::leader_cap` ==

pub struct LeaderCap;

const LEADER_CAP_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("leader_cap");

impl LeaderCap {
    /// Create N leader caps for self and the provided addresses.
    ///
    /// `nexus_workflow::leader_cap::create_for_self_and_addresses`
    pub const CREATE_FOR_SELF_AND_ADDRESSES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::types::Identifier::from_static("create_for_self_and_addresses"),
    };
    /// This is used as a generic argument for
    /// [crate::idents::primitives::OwnerCap::CLONEABLE_OWNER_CAP].
    ///
    /// `nexus_workflow::leader_cap::OverNetwork`
    pub const OVER_NETWORK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::types::Identifier::from_static("OverNetwork"),
    };
}

// == `nexus_workflow::gas` ==

pub struct Gas;

const GAS_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("gas");

impl Gas {
    /// Add Balance<SUI> to the tx sender's gas budget.
    ///
    /// `nexus_workflow::gas::add_gas_budget`
    pub const ADD_GAS_BUDGET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("add_gas_budget"),
    };
    /// Claim leader gas for this evaluation.
    ///
    /// `nexus_workflow::gas::claim_leader_gas`
    pub const CLAIM_LEADER_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("claim_leader_gas"),
    };
    /// Claim leader gas against an invoker scope (no DAG execution object).
    ///
    /// `nexus_workflow::gas::claim_leader_gas_for_invoker`
    pub const CLAIM_LEADER_GAS_FOR_INVOKER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("claim_leader_gas_for_invoker"),
    };
    /// Claim leader gas specifically for pre-key handshakes.
    ///
    /// `nexus_workflow::gas::claim_leader_gas_for_pre_key`
    pub const CLAIM_LEADER_GAS_FOR_PRE_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("claim_leader_gas_for_pre_key"),
    };
    /// De-escalate an OverTool owner cap into OverGas.
    ///
    /// `nexus_workflow::gas::deescalate`
    pub const DEESCALATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("deescalate"),
    };
    /// GasService type for lookups.
    ///
    /// `nexus_workflow::gas::GasService`
    pub const GAS_SERVICE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("GasService"),
    };
    /// OverGas owner cap generic.
    ///
    /// `nexus_workflow::gas::OverGas`
    pub const OVER_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("OverGas"),
    };
    /// Create an Execution scope.
    ///
    /// `nexus_workflow::gas::scope_execution`
    pub const SCOPE_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_execution"),
    };
    /// Create an InvokerAddress scope.
    ///
    /// `nexus_workflow::gas::scope_invoker_address`
    pub const SCOPE_INVOKER_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_invoker_address"),
    };
    /// Create a WorksheetType scope.
    ///
    /// `nexus_workflow::gas::scope_worksheet_type`
    pub const SCOPE_WORKSHEET_TYPE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("scope_worksheet_type"),
    };
    /// Set a tool invocation cost in MIST.
    ///
    /// `nexus_workflow::gas::set_single_invocation_cost_mist`
    pub const SET_SINGLE_INVOCATION_COST_MIST: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("set_single_invocation_cost_mist"),
    };
    /// Sync gas for the vertices in the current execution object.
    ///
    /// `nexus_workflow::gas::sync_gas_state`
    pub const SYNC_GAS_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::types::Identifier::from_static("sync_gas_state"),
    };

    /// Convert an object ID to an InvokerAddress scope.
    pub fn scope_invoker_address_from_object_id(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        object_id: sui::types::Address,
    ) -> anyhow::Result<sui::types::Argument> {
        let address = Address::address_from_type(tx, object_id)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::SCOPE_INVOKER_ADDRESS.module,
                Self::SCOPE_INVOKER_ADDRESS.name,
                vec![],
            ),
            vec![address],
        ))
    }
}

// == `nexus_workflow::gas_extension` ==

pub struct GasExtension;

const GAS_EXTENSION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("gas_extension");

impl GasExtension {
    /// Buy an expiry gas extension ticket.
    ///
    /// `nexus_workflow::gas_extension::buy_expiry_gas_ticket`
    pub const BUY_EXPIRY_GAS_TICKET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("buy_expiry_gas_ticket"),
    };
    /// Buy a limited invocations gas extension ticket.
    ///
    /// `nexus_workflow::gas_extension::buy_limited_invocations_gas_ticket`
    pub const BUY_LIMITED_INVOCATIONS_GAS_TICKET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("buy_limited_invocations_gas_ticket"),
    };
    /// Disable expiry gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::disable_expiry`
    pub const DISABLE_EXPIRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("disable_expiry"),
    };
    /// Disable limited invocations gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::disable_limited_invocations`
    pub const DISABLE_LIMITED_INVOCATIONS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("disable_limited_invocations"),
    };
    /// Enable expiry gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::enable_expiry`
    pub const ENABLE_EXPIRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("enable_expiry"),
    };
    /// Enable limited invocations gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::enable_limited_invocations`
    pub const ENABLE_LIMITED_INVOCATIONS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::types::Identifier::from_static("enable_limited_invocations"),
    };
}

// == `nexus_workflow::pre_key_vault` ==

pub struct PreKeyVault;

// == `nexus_workflow::network_auth` ==

pub struct NetworkAuth;

const NETWORK_AUTH_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("network_auth");

impl NetworkAuth {
    /// Create a new key binding for an identity.
    ///
    /// `nexus_workflow::network_auth::create_binding`
    pub const CREATE_BINDING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("create_binding"),
    };
    /// Move type `nexus_workflow::network_auth::IdentityKey`.
    pub const IDENTITY_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("IdentityKey"),
    };
    /// Construct a proof-of-possession for a key registration slot.
    ///
    /// `nexus_workflow::network_auth::new_proof_of_key`
    pub const NEW_PROOF_OF_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("new_proof_of_key"),
    };
    /// Create proof-of-identity for a leader (sender), using a leader capability.
    ///
    /// `nexus_workflow::network_auth::prove_leader`
    pub const PROVE_LEADER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("prove_leader"),
    };
    /// Create proof-of-identity for an off-chain tool (checked against ToolRegistry).
    ///
    /// `nexus_workflow::network_auth::prove_offchain_tool`
    pub const PROVE_OFFCHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("prove_offchain_tool"),
    };
    /// Register a new key on an existing binding and set it active.
    ///
    /// `nexus_workflow::network_auth::register_key`
    pub const REGISTER_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: NETWORK_AUTH_MODULE,
        name: sui::types::Identifier::from_static("register_key"),
    };
}

const PRE_KEY_VAULT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("pre_key_vault");

impl PreKeyVault {
    /// Associate a pre key with the sender and fire an initial message.
    ///
    /// `nexus_workflow::pre_key_vault::associate_pre_key`
    pub const ASSOCIATE_PRE_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::types::Identifier::from_static("associate_pre_key"),
    };
    /// Claim a pre key for the tx sender.
    ///
    /// `nexus_workflow::pre_key_vault::claim_pre_key_for_self`
    pub const CLAIM_PRE_KEY_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::types::Identifier::from_static("claim_pre_key_for_self"),
    };
    /// Fulfill a requested pre key for a user.
    ///
    /// `nexus_workflow::pre_key_vault::fulfill_pre_key_for_user`
    pub const FULFILL_PRE_KEY_FOR_USER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::types::Identifier::from_static("fulfill_pre_key_for_user"),
    };
    /// OverCrypto owner cap generic.
    ///
    /// `nexus_workflow::pre_key_vault::OverCrypto`
    pub const OVER_CRYPTO: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::types::Identifier::from_static("OverCrypto"),
    };
    /// PreKeyVault type for lookups.
    ///
    /// `nexus_workflow::pre_key_vault::PreKeyVault`
    pub const PRE_KEY_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::types::Identifier::from_static("PreKeyVault"),
    };
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::types::TypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    workflow_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        workflow_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_type_tag() {
        let rng = &mut rand::thread_rng();
        let workflow_pkg_id = sui::types::Address::generate(rng);
        let ident = ModuleAndNameIdent {
            module: sui::types::Identifier::from_static("foo"),
            name: sui::types::Identifier::from_static("bar"),
        };

        let tag = into_type_tag(workflow_pkg_id, ident);

        assert_eq!(
            tag,
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                workflow_pkg_id,
                sui::types::Identifier::from_static("foo"),
                sui::types::Identifier::from_static("bar"),
                vec![],
            )))
        )
    }
}
