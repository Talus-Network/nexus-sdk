use crate::{
    idents::ModuleAndNameIdent,
    sui,
    types::{EdgeKind, PostFailureAction, RuntimeVertex},
    ToolFqn,
};

// == `nexus_interface::{dag, graph}` ==

const DAG_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("dag");
const GRAPH_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("graph");
const VERIFIER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("verifier");

pub struct Dag;

impl Dag {
    /// The DAG struct. Mostly used for creating generic types.
    ///
    /// `nexus_interface::dag::DAG`
    pub const DAG: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("DAG"),
    };
    /// Create a new DAG object.
    ///
    /// `nexus_interface::dag::new`
    pub const NEW: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("new"),
    };
    /// Configure the DAG-wide default leader verifier policy.
    ///
    /// `nexus_interface::dag::with_default_leader_verifier`
    pub const WITH_DEFAULT_LEADER_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_leader_verifier"),
    };
    /// Configure the DAG-wide default tool verifier policy.
    ///
    /// `nexus_interface::dag::with_default_tool_verifier`
    pub const WITH_DEFAULT_TOOL_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_tool_verifier"),
    };
    /// Add a default value to a DAG.
    ///
    /// `nexus_interface::dag::with_default_value`
    pub const WITH_DEFAULT_VALUE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_default_value"),
    };
    /// Add an edge to a DAG.
    ///
    /// `nexus_interface::dag::with_edge`
    pub const WITH_EDGE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_edge"),
    };
    /// Mark a vertex as an entry vertex and assign it to a group.
    ///
    /// `nexus_interface::dag::with_entry_in_group`
    pub const WITH_ENTRY_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_in_group"),
    };
    /// Add an input port as an entry input port and assign it to a group.
    ///
    /// `nexus_interface::dag::with_entry_port_in_group`
    pub const WITH_ENTRY_PORT_IN_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_entry_port_in_group"),
    };
    /// Add an output to a DAG.
    ///
    /// `nexus_interface::dag::with_output`
    pub const WITH_OUTPUT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_output"),
    };
    /// Configure a DAG-level default post-failure action.
    ///
    /// `nexus_interface::dag::with_post_failure_action`
    pub const WITH_POST_FAILURE_ACTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_post_failure_action"),
    };
    /// Add a vertex to a DAG.
    ///
    /// `nexus_interface::dag::with_vertex`
    pub const WITH_VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex"),
    };
    /// Configure the vertex-level leader verifier policy.
    ///
    /// `nexus_interface::dag::with_vertex_leader_verifier`
    pub const WITH_VERTEX_LEADER_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_leader_verifier"),
    };
    /// Configure a vertex-level post-failure action override.
    ///
    /// `nexus_interface::dag::with_vertex_post_failure_action`
    pub const WITH_VERTEX_POST_FAILURE_ACTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_post_failure_action"),
    };
    /// Configure the vertex-level tool verifier policy.
    ///
    /// `nexus_interface::dag::with_vertex_tool_verifier`
    pub const WITH_VERTEX_TOOL_VERIFIER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DAG_MODULE,
        name: sui::types::Identifier::from_static("with_vertex_tool_verifier"),
    };
}

pub struct Verifier;

impl Verifier {
    /// Build external verifier submission evidence.
    ///
    /// `nexus_interface::verifier::new_external_verifier_submit_evidence_v1`
    pub const NEW_EXTERNAL_VERIFIER_SUBMIT_EVIDENCE_V1: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VERIFIER_MODULE,
        name: sui::types::Identifier::from_static("new_external_verifier_submit_evidence_v1"),
    };
    /// Build an external-verifier off-chain proof.
    ///
    /// `nexus_interface::verifier::new_off_chain_verifier_proof_external_verifier_v1`
    pub const NEW_OFF_CHAIN_VERIFIER_PROOF_EXTERNAL_VERIFIER_V1: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: VERIFIER_MODULE,
            name: sui::types::Identifier::from_static(
                "new_off_chain_verifier_proof_external_verifier_v1",
            ),
        };
    /// Build a registered-key off-chain proof.
    ///
    /// `nexus_interface::verifier::new_off_chain_verifier_proof_registered_key_v1`
    pub const NEW_OFF_CHAIN_VERIFIER_PROOF_REGISTERED_KEY_V1: ModuleAndNameIdent =
        ModuleAndNameIdent {
            module: VERIFIER_MODULE,
            name: sui::types::Identifier::from_static(
                "new_off_chain_verifier_proof_registered_key_v1",
            ),
        };
}
pub struct Graph;

impl Graph {
    /// `nexus_interface::graph::EdgeKind` constructors and type tags.
    pub const EDGE_KIND_BREAK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_break"),
    };
    pub const EDGE_KIND_COLLECT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_collect"),
    };
    pub const EDGE_KIND_DO_WHILE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_do_while"),
    };
    pub const EDGE_KIND_FOR_EACH: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_for_each"),
    };
    pub const EDGE_KIND_NORMAL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_normal"),
    };
    pub const EDGE_KIND_STATIC: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("edge_kind_static"),
    };
    pub const ENTRY_GROUP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("EntryGroup"),
    };
    pub const ENTRY_GROUP_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("entry_group_from_string"),
    };
    pub const INPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("InputPort"),
    };
    pub const INPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("input_port_from_string"),
    };
    pub const OUTPUT_PORT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("OutputPort"),
    };
    pub const OUTPUT_PORT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("output_port_from_string"),
    };
    pub const OUTPUT_VARIANT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("OutputVariant"),
    };
    pub const OUTPUT_VARIANT_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("output_variant_from_string"),
    };
    pub const POST_FAILURE_ACTION_TERMINATE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("post_failure_action_terminate"),
    };
    pub const POST_FAILURE_ACTION_TRANSIENT_CONTINUE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("post_failure_action_transient_continue"),
    };
    pub const RUNTIME_VERTEX_PLAIN_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("runtime_vertex_plain_from_string"),
    };
    pub const RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("runtime_vertex_with_iterator_from_string"),
    };
    pub const TAGGED_OUTPUT_TO_DAG_TYPES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("tagged_output_to_dag_types"),
    };
    pub const VERTEX: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("Vertex"),
    };
    pub const VERTEX_FROM_STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("vertex_from_string"),
    };
    pub const VERTEX_OFF_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("vertex_off_chain"),
    };
    pub const VERTEX_ON_CHAIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: GRAPH_MODULE,
        name: sui::types::Identifier::from_static("vertex_on_chain"),
    };

    pub fn entry_group_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::ENTRY_GROUP_FROM_STRING.module,
                Self::ENTRY_GROUP_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    pub fn input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::INPUT_PORT_FROM_STRING.module,
                Self::INPUT_PORT_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    pub fn output_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::OUTPUT_PORT_FROM_STRING.module,
                Self::OUTPUT_PORT_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    pub fn output_variant_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::OUTPUT_VARIANT_FROM_STRING.module,
                Self::OUTPUT_VARIANT_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    pub fn vertex_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::VERTEX_FROM_STRING.module,
                Self::VERTEX_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    pub fn off_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::VERTEX_OFF_CHAIN.module,
                Self::VERTEX_OFF_CHAIN.name,
            ),
            vec![str],
        ))
    }

    pub fn on_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::VERTEX_ON_CHAIN.module,
                Self::VERTEX_ON_CHAIN.name,
            ),
            vec![str],
        ))
    }

    pub fn edge_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        edge_kind: &EdgeKind,
    ) -> sui::tx::Argument {
        let ident = match edge_kind {
            EdgeKind::Normal => Self::EDGE_KIND_NORMAL,
            EdgeKind::ForEach => Self::EDGE_KIND_FOR_EACH,
            EdgeKind::Collect => Self::EDGE_KIND_COLLECT,
            EdgeKind::DoWhile => Self::EDGE_KIND_DO_WHILE,
            EdgeKind::Break => Self::EDGE_KIND_BREAK,
            EdgeKind::Static => Self::EDGE_KIND_STATIC,
        };
        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    pub fn post_failure_action_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        action: &PostFailureAction,
    ) -> sui::tx::Argument {
        let ident = match action {
            PostFailureAction::Terminate => Self::POST_FAILURE_ACTION_TERMINATE,
            PostFailureAction::TransientContinue => Self::POST_FAILURE_ACTION_TRANSIENT_CONTINUE,
        };
        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    pub fn runtime_vertex_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        runtime_vertex: &RuntimeVertex,
    ) -> anyhow::Result<sui::tx::Argument> {
        match runtime_vertex {
            RuntimeVertex::Plain { vertex } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;
                Ok(tx.move_call(
                    sui::tx::Function::new(
                        interface_pkg_id,
                        Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_PLAIN_FROM_STRING.name,
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
                let iteration = tx.pure(iteration);
                let out_of = tx.pure(out_of);
                Ok(tx.move_call(
                    sui::tx::Function::new(
                        interface_pkg_id,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name,
                    ),
                    vec![name, iteration, out_of],
                ))
            }
        }
    }
}

/// Helper to turn an interface `ModuleAndNameIdent` into a `sui::types::TypeTag`.
pub fn into_type_tag(
    interface_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        interface_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}
