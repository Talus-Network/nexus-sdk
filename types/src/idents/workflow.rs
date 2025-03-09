use crate::{idents::ModuleAndNameIdent, sui, ToolFqn};

// == `nexus_workflow::default_sap` ==

pub struct DefaultSap;

const DEFAULT_SAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("default_sap");

impl DefaultSap {
    /// This function is called when a DAG is to be executed using the default
    /// SAP implementation.
    ///
    /// `nexus_workflow::default_sap::begin_dag_execution`
    pub const BEGIN_DAG_EXECUTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DEFAULT_SAP_MODULE,
        name: sui::move_ident_str!("begin_dag_execution"),
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
    /// Add an entry vertex to a DAG. Entry vertex is just a Vertex with its
    /// required InputPorts specified.
    ///
    /// `nexus_workflow::dag::with_entry_vertex`
    pub const WITH_ENTRY_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_entry_vertex"),
    };
    /// Add an entry vertex that is in a specific group to a DAG.
    ///
    /// `nexus_workflow::dag::with_entry_vertex_in_group`
    pub const WITH_ENTRY_VERTEX_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::move_ident_str!("with_entry_vertex_in_group"),
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
}

// == `nexus_workflow::tool_registry` ==

pub struct ToolRegistry;

const TOOL_REGISTRY_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("tool_registry");

impl ToolRegistry {
    /// Claim collateral for a tool.
    ///
    /// `nexus_workflow::tool_registry::claim_collateral_for_tool`
    pub const CLAIM_COLLATERAL_FOR_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        // TODO: This will likely be renamed to `claim_collateral_for_tool`.
        name: sui::move_ident_str!("claim_collateral_for_off_chain_tool"),
    };
    /// Register an off-chain tool.
    ///
    /// `nexus_workflow::tool_registry::register_off_chain_tool`
    pub const REGISTER_OFF_CHAIN_TOOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TOOL_REGISTRY_MODULE,
        name: sui::move_ident_str!("register_off_chain_tool"),
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
    /// This is used as a generic argument for
    /// [crate::idents::primitives::OwnerCap::CLONEABLE_OWNER_CAP].
    ///
    /// `nexus_workflow::leader_cap::OverNetwork`
    pub const OVER_NETWORK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: LEADER_CAP_MODULE,
        name: sui::move_ident_str!("OverNetwork"),
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
