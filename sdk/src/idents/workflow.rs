use crate::{
    idents::{sui_framework::Address, ModuleAndNameIdent},
    sui,
    types::{EdgeKind, RuntimeVertex},
    ToolFqn,
};

// == `nexus_workflow::default_tap` ==

pub struct DefaultTap;

const DEFAULT_TAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("default_tap");

impl DefaultTap {
    /// This function is called when a DAG is to be executed using the default
    /// TAP implementation.
    ///
    /// `nexus_workflow::default_tap::begin_dag_execution`
    pub const BEGIN_DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::move_ident_str!("begin_dag_execution"),
    };
    /// Scheduler entry point to invoke DAG execution via the default TAP.
    ///
    /// `nexus_workflow::default_tap::dag_begin_execution_from_scheduler`
    pub const DAG_BEGIN_EXECUTION_FROM_SCHEDULER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::move_ident_str!("dag_begin_execution_from_scheduler"),
    };
    /// The DefaultTap struct type.
    ///
    /// `nexus_workflow::default_tap::DefaultTAP`
    pub const DEFAULT_TAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_TAP_MODULE,
        name: sui::move_ident_str!("DefaultTAP"),
    };
}

// == `nexus_workflow::scheduler` ==

pub struct Scheduler;

const SCHEDULER_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("scheduler");

impl Scheduler {
    /// Enqueue a new occurrence for a task with explicit deadline.
    ///
    /// `nexus_workflow::scheduler::add_occurrence_absolute_for_task`
    pub const ADD_OCCURRENCE_ABSOLUTE_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("add_occurrence_absolute_for_task"),
    };
    /// Enqueue a new occurrence with deadline offset.
    ///
    /// `nexus_workflow::scheduler::add_occurrence_with_offset_for_task`
    pub const ADD_OCCURRENCE_WITH_OFFSET_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("add_occurrence_with_offset_for_task"),
    };
    /// Cancel scheduling for a task.
    ///
    /// `nexus_workflow::scheduler::cancel_time_constraint_for_task`
    pub const CANCEL_TIME_CONSTRAINT_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("cancel_time_constraint_for_task"),
    };
    /// Run scheduler checks to consume the next occurrence.
    ///
    /// `nexus_workflow::scheduler::check_time_constraint`
    pub const CHECK_TIME_CONSTRAINT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("check_time_constraint"),
    };
    /// Disable periodic scheduling for a task.
    ///
    /// `nexus_workflow::scheduler::disable_periodic_for_task`
    pub const DISABLE_PERIODIC_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("disable_periodic_for_task"),
    };
    /// Modify periodic schedule parameters for a task.
    ///
    /// `nexus_workflow::scheduler::modify_periodic_for_task`
    pub const MODIFY_PERIODIC_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("modify_periodic_for_task"),
    };
    /// Pause the scheduler for a task.
    ///
    /// `nexus_workflow::scheduler::pause_time_constraint_for_task`
    pub const PAUSE_TIME_CONSTRAINT_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("pause_time_constraint_for_task"),
    };
    /// Resume the scheduler for a task.
    ///
    /// `nexus_workflow::scheduler::resume_time_constraint_for_task`
    pub const RESUME_TIME_CONSTRAINT_FOR_TASK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SCHEDULER_MODULE,
        name: sui::move_ident_str!("resume_time_constraint_for_task"),
    };
}

// == `nexus_workflow::dag` ==

pub struct Dag;

const DAG_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("dag");

impl Dag {
    /// The DAG struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::DAG`
    pub const DAG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("DAG"),
    };
    /// The DAGExecution struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::DAGExecution`
    pub const DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("DAGExecution"),
    };
    /// Create a break edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_break`
    pub const EDGE_KIND_BREAK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("edge_kind_break"),
    };
    /// Create a collect edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_collect`
    pub const EDGE_KIND_COLLECT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("edge_kind_collect"),
    };
    /// Create a do-while edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_do_while`
    pub const EDGE_KIND_DO_WHILE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("edge_kind_do_while"),
    };
    /// Create a for-each edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_for_each`
    pub const EDGE_KIND_FOR_EACH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("edge_kind_for_each"),
    };
    /// Create a normal edge kind.
    ///
    /// `nexus_workflow::dag::edge_kind_normal`
    pub const EDGE_KIND_NORMAL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("edge_kind_normal"),
    };
    /// Create an encrypted InputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::encrypted_input_port_from_string`
    pub const ENCRYPTED_INPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("encrypted_input_port_from_string"),
    };
    /// The EntryGroup struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::EntryGroup`
    pub const ENTRY_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("EntryGroup"),
    };
    /// Create an EntryGroup from an ASCII string.
    ///
    /// `nexus_workflow::dag::entry_group_from_string`
    pub const ENTRY_GROUP_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("entry_group_from_string"),
    };
    /// The InputPort struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::InputPort`
    pub const INPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("InputPort"),
    };
    /// Create an InputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::input_port_from_string`
    pub const INPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("input_port_from_string"),
    };
    /// Create a new DAG object.
    ///
    /// `nexus_workflow::dag::new`
    pub const NEW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("new"),
    };
    /// The OutputPort struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::OutputPort`
    pub const OUTPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("OutputPort"),
    };
    /// Create an OutputPort from an ASCII string.
    ///
    /// `nexus_workflow::dag::output_port_from_string`
    pub const OUTPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("output_port_from_string"),
    };
    /// The OutputVariant struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::OutputVariant`
    pub const OUTPUT_VARIANT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("OutputVariant"),
    };
    /// Create an OutputVariant from an ASCII string.
    ///
    /// `nexus_workflow::dag::output_variant_from_string`
    pub const OUTPUT_VARIANT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("output_variant_from_string"),
    };
    /// Function to call to continue to the next vertex in the given walk.
    ///
    /// `nexus_workflow::dag::request_network_to_execute_walks`
    pub const REQUEST_NETWORK_TO_EXECUTE_WALKS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("request_network_to_execute_walks"),
    };
    /// Returns a new hot potato object RequestWalkExecution.
    ///
    /// `nexus_workflow::dag::request_walk_execution`
    pub const REQUEST_WALK_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("request_walk_execution"),
    };
    /// Create a `RuntimeVertex::Plain` from a string.
    ///
    /// `nexus_workflow::dag::runtime_vertex_plain_from_string`
    pub const RUNTIME_VERTEX_PLAIN_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("runtime_vertex_plain_from_string"),
    };
    /// Create a `RuntimeVertex::WithIterator` from a string.
    ///
    /// `nexus_workflow::dag::runtime_vertex_with_iterator_from_string`
    pub const RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("runtime_vertex_with_iterator_from_string"),
    };
    /// One of the functions to call when an off-chain tool is evaluated to submit
    /// its result to the workflow.
    ///
    /// `nexus_workflow::dag::submit_off_chain_tool_eval_for_walk`
    pub const SUBMIT_OFF_CHAIN_TOOL_EVAL_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("submit_off_chain_tool_eval_for_walk"),
    };
    /// One of the functions to call when an on-chain tool is evaluated to submit
    /// its result to the workflow.
    ///
    /// `nexus_workflow::dag::submit_on_chain_tool_eval_for_walk`
    // TODO: <https://github.com/Talus-Network/nexus-next/issues/30>
    pub const SUBMIT_ON_CHAIN_TOOL_EVAL_FOR_WALK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("submit_on_chain_tool_eval_for_walk"),
    };
    /// The Vertex struct. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::dag::Vertex`
    pub const VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("Vertex"),
    };
    /// Create a Vertex from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_from_string`
    pub const VERTEX_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("vertex_from_string"),
    };
    /// Create a new off-chain NodeIdent from an ASCII string.
    ///
    /// `nexus_workflow::dag::vertex_off_chain`
    pub const VERTEX_OFF_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("vertex_off_chain"),
    };
    /// Add a default value to a DAG. Default value is a Vertex + InputPort pair
    /// with NexusData as the value.
    ///
    /// `nexus_workflow::dag::with_default_value`
    pub const WITH_DEFAULT_VALUE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_default_value"),
    };
    /// Add an Edge to a DAG.
    ///
    /// `nexus_workflow::dag::with_edge`
    pub const WITH_EDGE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_edge"),
    };
    /// Add an encrypted Edge to a DAG.
    ///
    /// `nexus_workflow::dag::with_encrypted_edge`
    pub const WITH_ENCRYPTED_EDGE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_encrypted_edge"),
    };
    /// Add an encrypted output to a DAG.
    ///
    /// `nexus_workflow::dag::with_encrypted_output`
    pub const WITH_ENCRYPTED_OUTPUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_encrypted_output"),
    };
    /// Mark a vertex as an entry vertex and assign it to a group.
    ///
    /// `nexus_workflow::dag::with_entry_in_group`
    pub const WITH_ENTRY_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_entry_in_group"),
    };
    /// Add an input port as an entry input port and assign it to a group.
    ///
    /// `nexus_workflow::dag::with_entry_port_in_group`
    pub const WITH_ENTRY_PORT_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_entry_port_in_group"),
    };
    /// Add an entry vertex to a DAG. Entry vertex is just a Vertex with its
    /// required InputPorts specified.
    ///
    /// `nexus_workflow::dag::with_entry_vertex`
    pub const WITH_ENTRY_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_entry_vertex"),
    };
    /// Add an output to a DAG.
    ///
    /// `nexus_workflow::dag::with_output`
    pub const WITH_OUTPUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_output"),
    };
    /// Add a Vertex to a DAG.
    ///
    /// `nexus_workflow::dag::with_vertex`
    pub const WITH_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_vertex"),
    };

    /// Create an EntryGroup from a string.
    pub fn entry_group_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::ENTRY_GROUP_FROM_STRING.module.into(),
            Self::ENTRY_GROUP_FROM_STRING.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create an InputPort from a string.
    pub fn input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::INPUT_PORT_FROM_STRING.module.into(),
            Self::INPUT_PORT_FROM_STRING.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create an encrypted InputPort from a string.
    pub fn encrypted_input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::ENCRYPTED_INPUT_PORT_FROM_STRING.module.into(),
            Self::ENCRYPTED_INPUT_PORT_FROM_STRING.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create an OutputPort from a string.
    pub fn output_port_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::OUTPUT_PORT_FROM_STRING.module.into(),
            Self::OUTPUT_PORT_FROM_STRING.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create an OutputVariant from a string.
    pub fn output_variant_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::OUTPUT_VARIANT_FROM_STRING.module.into(),
            Self::OUTPUT_VARIANT_FROM_STRING.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create a Vertex from a string.
    pub fn vertex_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::VERTEX_FROM_STRING.module.into(),
            Self::VERTEX_FROM_STRING.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create a new off-chain NodeIdent from a string.
    pub fn off_chain_vertex_kind_from_fqn(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::VERTEX_OFF_CHAIN.module.into(),
            Self::VERTEX_OFF_CHAIN.name.into(),
            vec![],
            vec![str],
        ))
    }

    /// Create an edge kind from an enum variant.
    pub fn edge_kind_from_enum(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        edge_kind: &EdgeKind,
    ) -> sui::Argument {
        let ident = match edge_kind {
            EdgeKind::Normal => Self::EDGE_KIND_NORMAL,
            EdgeKind::ForEach => Self::EDGE_KIND_FOR_EACH,
            EdgeKind::Collect => Self::EDGE_KIND_COLLECT,
            EdgeKind::DoWhile => Self::EDGE_KIND_DO_WHILE,
            EdgeKind::Break => Self::EDGE_KIND_BREAK,
        };

        tx.programmable_move_call(
            workflow_pkg_id,
            ident.module.into(),
            ident.name.into(),
            vec![],
            vec![],
        )
    }

    /// Create a runtime vertex from an enum variant
    pub fn runtime_vertex_from_enum(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        runtime_vertex: &RuntimeVertex,
    ) -> anyhow::Result<sui::Argument> {
        match runtime_vertex {
            RuntimeVertex::Plain { vertex } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;

                Ok(tx.programmable_move_call(
                    workflow_pkg_id,
                    Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.module.into(),
                    Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.name.into(),
                    vec![],
                    vec![name],
                ))
            }
            RuntimeVertex::WithIterator {
                vertex,
                iteration,
                out_of,
            } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;
                let iteration = tx.pure(*iteration)?;
                let out_of = tx.pure(*out_of)?;

                Ok(tx.programmable_move_call(
                    workflow_pkg_id,
                    Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module.into(),
                    Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name.into(),
                    vec![],
                    vec![name, iteration, out_of],
                ))
            }
        }
    }
}

// == `nexus_workflow::tool_registry` ==

pub struct ToolRegistry;

const TOOL_REGISTRY_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("tool_registry");

impl ToolRegistry {
    /// Add an address to the allowlist for tool registration.
    /// Only callable by the holder of OverSlashing cap.
    ///
    /// `nexus_workflow::tool_registry::add_allowed_owner`
    pub const ADD_ALLOWED_OWNER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("add_allowed_owner"),
    };
    /// Claim collateral for a tool and transfer the balance to the tx sender.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral_for_self`
    pub const CLAIM_COLLATERAL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("claim_collateral_for_self"),
    };
    /// Claim collateral for a tool. The function call returns Balance<SUI>.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral_for_tool`
    pub const CLAIM_COLLATERAL_FOR_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        // TODO: This will likely be renamed to `claim_collateral_for_tool`.
        name: sui::move_ident_str!("claim_collateral_for_off_chain_tool"),
    };
    /// OverSlashing struct type. Used to fetch caps for slashing tools.
    ///
    /// `nexus_workflow::tool_registry::OverSlashing`
    pub const OVER_SLASHING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("OverSlashing"),
    };
    /// OverTool struct type. Used for fetching tool owner caps.
    ///
    /// `nexus_workflow::tool_registry::OverTool`
    pub const OVER_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("OverTool"),
    };
    /// Register an off-chain tool. This returns the tool's owner cap.
    ///
    /// `nexus_workflow::tool_registry::register_off_chain_tool`
    pub const REGISTER_OFF_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("register_off_chain_tool"),
    };
    /// Register an off-chain tool and transfer the tool's owner cap to the ctx
    /// sender.
    ///
    /// `nexus_workflow::tool_registry::register_off_chain_tool_for_self`
    pub const REGISTER_OFF_CHAIN_TOOL_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("register_off_chain_tool_for_self"),
    };
    /// Remove an address from the allowlist for tool registration.
    /// Only callable by the holder of OverSlashing cap.
    ///
    /// `nexus_workflow::tool_registry::remove_allowed_owner`
    pub const REMOVE_ALLOWED_OWNER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("remove_allowed_owner"),
    };
    /// The ToolRegistry struct type.
    ///
    /// `nexus_workflow::tool_registry::ToolRegistry`
    pub const TOOL_REGISTRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("ToolRegistry"),
    };
    /// Unregister an tool.
    ///
    /// `nexus_workflow::tool_registry::unregister_tool`
    pub const UNREGISTER_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        // TODO: This will likely be renamed to `unregister_tool`.
        name: sui::move_ident_str!("unregister_off_chain_tool"),
    };
}

// == `nexus_workflow::leader_cap` ==

pub struct LeaderCap;

const LEADER_CAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("leader_cap");

impl LeaderCap {
    /// Create N leader caps for self and the provided addresses.
    ///
    /// `nexus_workflow::leader_cap::create_for_self_and_addresses`
    pub const CREATE_FOR_SELF_AND_ADDRESSES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::move_ident_str!("create_for_self_and_addresses"),
    };
    /// This is used as a generic argument for
    /// [crate::idents::primitives::OwnerCap::CLONEABLE_OWNER_CAP].
    ///
    /// `nexus_workflow::leader_cap::OverNetwork`
    pub const OVER_NETWORK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::move_ident_str!("OverNetwork"),
    };
}

// == `nexus_workflow::gas` ==

pub struct Gas;

const GAS_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("gas");

impl Gas {
    /// Add Balance<SUI> to the tx sender's gas budget.
    ///
    /// `nexus_workflow::gas::add_gas_budget`
    pub const ADD_GAS_BUDGET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("add_gas_budget"),
    };
    /// Claim leader gas for this evaluation.
    ///
    /// `nexus_workflow::gas::claim_leader_gas`
    pub const CLAIM_LEADER_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("claim_leader_gas"),
    };
    /// De-escalate an OverTool owner cap into OverGas.
    ///
    /// `nexus_workflow::gas::deescalate`
    pub const DEESCALATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("deescalate"),
    };
    /// GasService type for lookups.
    ///
    /// `nexus_workflow::gas::GasService`
    pub const GAS_SERVICE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("GasService"),
    };
    /// OverGas owner cap generic.
    ///
    /// `nexus_workflow::gas::OverGas`
    pub const OVER_GAS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("OverGas"),
    };
    /// Create an Execution scope.
    ///
    /// `nexus_workflow::gas::scope_execution`
    pub const SCOPE_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("scope_execution"),
    };
    /// Create an InvokerAddress scope.
    ///
    /// `nexus_workflow::gas::scope_invoker_address`
    pub const SCOPE_INVOKER_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("scope_invoker_address"),
    };
    /// Create a WorksheetType scope.
    ///
    /// `nexus_workflow::gas::scope_worksheet_type`
    pub const SCOPE_WORKSHEET_TYPE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("scope_worksheet_type"),
    };
    /// Set a tool invocation cost in MIST.
    ///
    /// `nexus_workflow::gas::set_single_invocation_cost_mist`
    pub const SET_SINGLE_INVOCATION_COST_MIST: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("set_single_invocation_cost_mist"),
    };
    /// Sync gas for the vertices in the current execution object.
    ///
    /// `nexus_workflow::gas::sync_gas_state`
    pub const SYNC_GAS_STATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_MODULE,
        name: sui::move_ident_str!("sync_gas_state"),
    };

    /// Convert an object ID to an InvokerAddress scope.
    pub fn scope_invoker_address_from_object_id(
        tx: &mut sui::ProgrammableTransactionBuilder,
        workflow_pkg_id: sui::ObjectID,
        object_id: sui::ObjectID,
    ) -> anyhow::Result<sui::Argument> {
        let with_prefix = false;
        let address = Address::address_from_str(tx, object_id.to_canonical_string(with_prefix))?;

        Ok(tx.programmable_move_call(
            workflow_pkg_id,
            Self::SCOPE_INVOKER_ADDRESS.module.into(),
            Self::SCOPE_INVOKER_ADDRESS.name.into(),
            vec![],
            vec![address],
        ))
    }
}

// == `nexus_workflow::gas_extension` ==

pub struct GasExtension;

const GAS_EXTENSION_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("gas_extension");

impl GasExtension {
    /// Buy an expiry gas extension ticket.
    ///
    /// `nexus_workflow::gas_extension::buy_expiry_gas_ticket`
    pub const BUY_EXPIRY_GAS_TICKET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::move_ident_str!("buy_expiry_gas_ticket"),
    };
    /// Buy a limited invocations gas extension ticket.
    ///
    /// `nexus_workflow::gas_extension::buy_limited_invocations_gas_ticket`
    pub const BUY_LIMITED_INVOCATIONS_GAS_TICKET: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::move_ident_str!("buy_limited_invocations_gas_ticket"),
    };
    /// Disable expiry gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::disable_expiry`
    pub const DISABLE_EXPIRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::move_ident_str!("disable_expiry"),
    };
    /// Disable limited invocations gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::disable_limited_invocations`
    pub const DISABLE_LIMITED_INVOCATIONS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::move_ident_str!("disable_limited_invocations"),
    };
    /// Enable expiry gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::enable_expiry`
    pub const ENABLE_EXPIRY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::move_ident_str!("enable_expiry"),
    };
    /// Enable limited invocations gas extension for a tool.
    ///
    /// `nexus_workflow::gas_extension::enable_limited_invocations`
    pub const ENABLE_LIMITED_INVOCATIONS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GAS_EXTENSION_MODULE,
        name: sui::move_ident_str!("enable_limited_invocations"),
    };
}

// == `nexus_workflow::pre_key_vault` ==

pub struct PreKeyVault;

const PRE_KEY_VAULT_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("pre_key_vault");

impl PreKeyVault {
    /// Associate a pre key with the sender and fire an initial message.
    ///
    /// `nexus_workflow::pre_key_vault::associate_pre_key`
    pub const ASSOCIATE_PRE_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::move_ident_str!("associate_pre_key"),
    };
    /// Claim a pre key for the tx sender.
    ///
    /// `nexus_workflow::pre_key_vault::claim_pre_key_for_self`
    pub const CLAIM_PRE_KEY_FOR_SELF: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::move_ident_str!("claim_pre_key_for_self"),
    };
    /// PreKey struct type. Mostly used for creating generic types.
    ///
    /// `nexus_workflow::pre_key_vault::PreKey`
    pub const PRE_KEY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::move_ident_str!("PreKey"),
    };
    /// Create a new pre key from bytes.
    ///
    /// `nexus_workflow::pre_key_vault::pre_key_from_bytes`
    pub const PRE_KEY_FROM_BYTES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::move_ident_str!("pre_key_from_bytes"),
    };
    /// PreKeyVault type for lookups.
    ///
    /// `nexus_workflow::pre_key_vault::PreKeyVault`
    pub const PRE_KEY_VAULT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::move_ident_str!("PreKeyVault"),
    };
    /// Replenish the pre key vault with public pre_keys.
    ///
    /// `nexus_workflow::pre_key_vault::replenish_pre_keys`
    pub const REPLENISH_PRE_KEYS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: PRE_KEY_VAULT_MODULE,
        name: sui::move_ident_str!("replenish_pre_keys"),
    };
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::MoveTypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    workflow_pkg_id: sui::ObjectID,
    ident: ModuleAndNameIdent,
) -> sui::MoveTypeTag {
    sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *workflow_pkg_id,
        module: ident.module.into(),
        name: ident.name.into(),
        type_params: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_type_tag() {
        let workflow_pkg_id = sui::ObjectID::random();
        let ident = ModuleAndNameIdent {
            module: sui::move_ident_str!("foo"),
            name: sui::move_ident_str!("bar"),
        };

        let tag = into_type_tag(workflow_pkg_id, ident);

        assert_eq!(
            tag,
            sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                address: *workflow_pkg_id,
                module: sui::move_ident_str!("foo").into(),
                name: sui::move_ident_str!("bar").into(),
                type_params: vec![],
            }))
        )
    }
}
