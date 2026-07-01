//! Identifiers for the `nexus_interface` Move package.
//!
//! The per-module unit structs (`Dag`, `Graph`, `Verifier`, `Agent`, …) and
//! their `ModuleAndNameIdent` constants are generated at build time from
//! `generated/ir/interface.json`. This module keeps the hand-written `Graph`
//! construction helpers and the `TypeTag` helper on top of them.

use crate::{
    idents::ModuleAndNameIdent,
    sui,
    types::{
        interface::graph::EdgeKind,
        FailureEvidenceKind,
        PostFailureAction,
        RuntimeVertex,
        VerifierConfig,
        VerifierMode,
    },
    ToolFqn,
};

include!(concat!(env!("OUT_DIR"), "/idents_interface.rs"));

impl Verifier {
    pub fn failure_evidence_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        evidence_kind: &FailureEvidenceKind,
    ) -> sui::tx::Argument {
        let ident = match evidence_kind {
            FailureEvidenceKind::ToolEvidence => Self::FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE,
            FailureEvidenceKind::LeaderEvidence => Self::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE,
        };
        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    pub fn verifier_mode_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        mode: &VerifierMode,
    ) -> sui::tx::Argument {
        let ident = match mode {
            VerifierMode::None => Self::VERIFIER_MODE_NONE,
            VerifierMode::LeaderRegisteredKey => Self::VERIFIER_MODE_AUTHENTICATED_COMMUNICATION,
            VerifierMode::ToolVerifierContract => Self::VERIFIER_MODE_TOOL_VERIFIER_CONTRACT,
        };
        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    pub fn verifier_config(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        config: &VerifierConfig,
    ) -> anyhow::Result<sui::tx::Argument> {
        let mode = Self::verifier_mode_from_enum(tx, interface_pkg_id, &config.mode);
        let method = super::move_std::Ascii::str_to_argument(tx, &config.method)?;
        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                Self::VERIFIER_CONFIG.module,
                Self::VERIFIER_CONFIG.name,
            ),
            vec![mode, method],
        ))
    }
}

impl Graph {
    pub fn entry_group_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::str_to_argument(tx, str)?;
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
        let str = super::move_std::Ascii::str_to_argument(tx, str)?;
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
        let str = super::move_std::Ascii::str_to_argument(tx, str)?;
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
        let str = super::move_std::Ascii::str_to_argument(tx, str)?;
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
        let str = super::move_std::Ascii::str_to_argument(tx, str)?;
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
        let str = super::move_std::Ascii::str_to_argument(tx, fqn.to_string())?;
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
        let str = super::move_std::Ascii::str_to_argument(tx, fqn.to_string())?;
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
                let name = super::move_std::Ascii::str_to_argument(tx, vertex.name.as_ref())?;
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
                let name = super::move_std::Ascii::str_to_argument(tx, vertex.name.as_ref())?;
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
