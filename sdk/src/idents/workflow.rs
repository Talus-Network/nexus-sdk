//! Identifiers for the `nexus_workflow` Move package.
//!
//! The per-module unit structs (`Dag`, `Gas`, `GasExtension`, …) and their
//! `ModuleAndNameIdent` constants are generated at build time from
//! `generated/ir/workflow.json`. This module keeps the hand-written `Dag`
//! argument helpers and the `TypeTag` helper on top of them.
//!
//! Note: verifier-related identifiers live in the `nexus_interface` package
//! (see [`crate::idents::tap::Verifier`]); the `Dag` verifier helpers below
//! reference them there and take the interface package id at call time.

use crate::{
    idents::ModuleAndNameIdent,
    sui,
    types::{
        EdgeKind,
        FailureEvidenceKind,
        PostFailureAction,
        RuntimeVertex,
        VerifierConfig,
        VerifierMode,
    },
    ToolFqn,
};

include!(concat!(env!("OUT_DIR"), "/idents_workflow.rs"));

impl Dag {
    /// Create an EntryGroup from a string.
    pub fn entry_group_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::ENTRY_GROUP_FROM_STRING.module,
                Self::ENTRY_GROUP_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    /// Create an InputPort from a string.
    pub fn input_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::INPUT_PORT_FROM_STRING.module,
                Self::INPUT_PORT_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    /// Create an OutputPort from a string.
    pub fn output_port_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::OUTPUT_PORT_FROM_STRING.module,
                Self::OUTPUT_PORT_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    /// Create an OutputVariant from a string.
    pub fn output_variant_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::OUTPUT_VARIANT_FROM_STRING.module,
                Self::OUTPUT_VARIANT_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    /// Create a Vertex from a string.
    pub fn vertex_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, str)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_FROM_STRING.module,
                Self::VERTEX_FROM_STRING.name,
            ),
            vec![str],
        ))
    }

    /// Create a new off-chain NodeIdent from a string.
    pub fn off_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_OFF_CHAIN.module,
                Self::VERTEX_OFF_CHAIN.name,
            ),
            vec![str],
        ))
    }

    pub fn on_chain_vertex_kind_from_fqn(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        fqn: &ToolFqn,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = super::move_std::Ascii::ascii_string_from_str(tx, fqn.to_string())?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                workflow_pkg_id,
                Self::VERTEX_ON_CHAIN.module,
                Self::VERTEX_ON_CHAIN.name,
            ),
            vec![str],
        ))
    }

    /// Create an edge kind from an enum variant.
    pub fn edge_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
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
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    /// Create a post-failure action from an enum variant.
    pub fn post_failure_action_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        action: &PostFailureAction,
    ) -> sui::tx::Argument {
        let ident = match action {
            PostFailureAction::Terminate => Self::POST_FAILURE_ACTION_TERMINATE,
            PostFailureAction::TransientContinue => Self::POST_FAILURE_ACTION_TRANSIENT_CONTINUE,
        };

        tx.move_call(
            sui::tx::Function::new(workflow_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    /// Create a failure evidence kind from an enum variant.
    ///
    /// The `FailureEvidenceKind` constructors live in the `nexus_interface`
    /// package, so this takes the interface package id.
    pub fn failure_evidence_kind_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        evidence_kind: &FailureEvidenceKind,
    ) -> sui::tx::Argument {
        let ident = match evidence_kind {
            FailureEvidenceKind::ToolEvidence => {
                super::tap::Verifier::FAILURE_EVIDENCE_KIND_TOOL_EVIDENCE
            }
            FailureEvidenceKind::LeaderEvidence => {
                super::tap::Verifier::FAILURE_EVIDENCE_KIND_LEADER_EVIDENCE
            }
        };

        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    /// Create a verifier mode from an enum variant.
    ///
    /// The `VerifierMode` constructors live in the `nexus_interface` package.
    pub fn verifier_mode_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        mode: &VerifierMode,
    ) -> sui::tx::Argument {
        let ident = match mode {
            VerifierMode::None => super::tap::Verifier::VERIFIER_MODE_NONE,
            VerifierMode::LeaderRegisteredKey | VerifierMode::LeaderNautilusEnclave => {
                super::tap::Verifier::VERIFIER_MODE_AUTHENTICATED_COMMUNICATION
            }
            VerifierMode::ToolVerifierContract => {
                super::tap::Verifier::VERIFIER_MODE_TOOL_VERIFIER_CONTRACT
            }
        };

        tx.move_call(
            sui::tx::Function::new(interface_pkg_id, ident.module, ident.name),
            vec![],
        )
    }

    /// Create a verifier config value from the Rust mirror.
    ///
    /// The `verifier_config` constructor lives in the `nexus_interface` package.
    pub fn verifier_config(
        tx: &mut sui::tx::TransactionBuilder,
        interface_pkg_id: sui::types::Address,
        config: &VerifierConfig,
    ) -> anyhow::Result<sui::tx::Argument> {
        let mode = Self::verifier_mode_from_enum(tx, interface_pkg_id, &config.mode);
        let method = super::move_std::Ascii::ascii_string_from_str(tx, &config.method)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                interface_pkg_id,
                super::tap::Verifier::VERIFIER_CONFIG.module,
                super::tap::Verifier::VERIFIER_CONFIG.name,
            ),
            vec![mode, method],
        ))
    }

    /// Create a runtime vertex from an enum variant
    pub fn runtime_vertex_from_enum(
        tx: &mut sui::tx::TransactionBuilder,
        workflow_pkg_id: sui::types::Address,
        runtime_vertex: &RuntimeVertex,
    ) -> anyhow::Result<sui::tx::Argument> {
        match runtime_vertex {
            RuntimeVertex::Plain { vertex } => {
                let name = super::move_std::Ascii::ascii_string_from_str(tx, &vertex.name)?;

                Ok(tx.move_call(
                    sui::tx::Function::new(
                        workflow_pkg_id,
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
                        workflow_pkg_id,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.module,
                        Self::RUNTIME_VERTEX_WITH_ITERATOR_FROM_STRING.name,
                    ),
                    vec![name, iteration, out_of],
                ))
            }
        }
    }
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
