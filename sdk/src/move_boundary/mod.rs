//! SDK boundary helpers for generated Move bindings.
//!
//! Generated package bindings carry the package IDs from committed IR. Production SDK code scopes
//! those bindings with the deployment specific package IDs from [`NexusObjects`] before creating
//! call targets or type tags.

#[cfg(feature = "transactions")]
use crate::move_bindings::primitives::data::NexusData;
use crate::sui;
#[cfg(feature = "transactions")]
use crate::{
    move_bindings::{interface, move_std, primitives, sui_framework},
    types::NexusObjects,
};
#[cfg(feature = "transactions")]
use std::ops::{Deref, DerefMut};
#[cfg(feature = "transactions")]
use sui_move_call::CallTarget;
#[cfg(feature = "transactions")]
pub use sui_move_ptb::CLOCK_OBJECT_ID;
#[cfg(feature = "transactions")]
use sui_move_ptb::{BuildError, PtbBuilder};
#[cfg(feature = "transactions")]
use sui_sdk_types::Argument;

#[cfg(feature = "transactions")]
const MAX_PURE_INPUT_BYTES: usize = 16_384;
#[cfg(feature = "transactions")]
const MAX_NEXUS_DATA_ARRAY_CHUNK_ARGS: usize = 64;

/// Normalize package dependency IDs for Sui publish commands.
///
/// Sui publish rejects an empty dependency list. If the compiler reports no
/// explicit storage dependencies, include the fixed framework packages.
#[cfg_attr(not(feature = "move_publish"), allow(dead_code))]
pub(crate) fn publish_dependency_ids_or_framework_defaults(
    dependency_ids: impl IntoIterator<Item = sui::types::Address>,
) -> Vec<sui::types::Address> {
    let dependency_ids = dependency_ids.into_iter().collect::<Vec<_>>();
    if dependency_ids.is_empty() {
        vec![
            sui::types::Address::from_static("0x1"),
            sui::types::Address::from_static("0x2"),
        ]
    } else {
        dependency_ids
    }
}

/// Nexus scoped PTB builder.
///
/// This is the single transaction building form inside the SDK: it carries the canonical
/// `sui_move_ptb::PtbBuilder` together with the Nexus deployment object/package scope.
/// Generic PTB input/command operations come from `PtbBuilder`; this type only adds
/// Nexus scoped generated calls and domain constructors.
#[cfg(feature = "transactions")]
pub struct NexusPtbBuilder<'a> {
    objects: &'a NexusObjects,
    tx: PtbBuilder,
}

#[cfg(feature = "transactions")]
impl<'a> NexusPtbBuilder<'a> {
    fn new(objects: &'a NexusObjects) -> Self {
        Self {
            objects,
            tx: PtbBuilder::new(),
        }
    }

    /// Deployment object/package IDs associated with this PTB.
    pub fn objects(&self) -> &'a NexusObjects {
        self.objects
    }

    /// Add a generated Move call target to this PTB.
    pub fn call_target(
        &mut self,
        target: impl FnOnce() -> Result<CallTarget, sui_move_call::CallSpecError>,
        arguments: Vec<Argument>,
    ) -> anyhow::Result<Argument> {
        Ok(self.tx.call_target(target()?, arguments)?)
    }

    /// Add an already built Move call target to this PTB.
    fn call_raw_target(
        &mut self,
        target: CallTarget,
        arguments: Vec<Argument>,
    ) -> Result<Argument, BuildError> {
        self.tx.call_target(target, arguments)
    }

    /// Add a dynamic Move call target to this PTB.
    ///
    /// This is for runtime owned ABI edges, such as user tool entrypoints stored in Nexus
    /// metadata. Nexus protocol calls should use generated targets instead.
    pub fn call_function(
        &mut self,
        package: sui::types::Address,
        module: impl AsRef<str>,
        function: impl AsRef<str>,
        arguments: Vec<Argument>,
    ) -> anyhow::Result<Argument> {
        Ok(self.tx.call_target(
            CallTarget::new(package, module.as_ref(), function.as_ref())?,
            arguments,
        )?)
    }

    /// Build a Move `0x1::ascii::String` from bytes.
    pub fn ascii_string(&mut self, value: impl AsRef<str>) -> Result<Argument, BuildError> {
        ascii_string(&mut self.tx, value)
    }

    /// Build a Move `0x2::object::ID` from an address/object ID.
    pub fn object_id(&mut self, object_id: sui::types::Address) -> Result<Argument, BuildError> {
        let address = self.tx.arg(&object_id)?;
        self.tx.call_target(
            sui_framework::object::id_from_address_target()?,
            vec![address],
        )
    }

    fn call_target_with_ascii(
        &mut self,
        value: impl AsRef<str>,
        target: impl FnOnce() -> Result<CallTarget, sui_move_call::CallSpecError>,
    ) -> anyhow::Result<Argument> {
        let value = self.ascii_string(value)?;
        self.call_target(target, vec![value])
    }

    /// Build a generated `interface::graph::Vertex`.
    pub(crate) fn graph_vertex(&mut self, value: impl AsRef<str>) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(value, interface::graph::vertex_from_string_target)
    }

    /// Build a generated `interface::graph::InputPort`.
    pub(crate) fn graph_input_port(&mut self, value: impl AsRef<str>) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(value, interface::graph::input_port_from_string_target)
    }

    /// Build a generated `interface::graph::OutputPort`.
    pub(crate) fn graph_output_port(&mut self, value: impl AsRef<str>) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(value, interface::graph::output_port_from_string_target)
    }

    /// Build a generated `interface::graph::OutputVariant`.
    pub(crate) fn graph_output_variant(
        &mut self,
        value: impl AsRef<str>,
    ) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(value, interface::graph::output_variant_from_string_target)
    }

    /// Build a generated `interface::graph::EntryGroup`.
    pub(crate) fn graph_entry_group(&mut self, value: impl AsRef<str>) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(value, interface::graph::entry_group_from_string_target)
    }

    /// Build a generated on chain `interface::graph::VertexKind`.
    pub(crate) fn graph_vertex_kind_on_chain(
        &mut self,
        tool_fqn: impl AsRef<str>,
    ) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(tool_fqn, interface::graph::vertex_on_chain_target)
    }

    /// Build a generated off chain `interface::graph::VertexKind`.
    pub(crate) fn graph_vertex_kind_off_chain(
        &mut self,
        tool_fqn: impl AsRef<str>,
    ) -> anyhow::Result<Argument> {
        self.call_target_with_ascii(tool_fqn, interface::graph::vertex_off_chain_target)
    }

    /// Build a generated `interface::graph::PostFailureAction`.
    pub(crate) fn graph_post_failure_action(
        &mut self,
        action: &interface::graph::PostFailureAction,
    ) -> anyhow::Result<Argument> {
        let target = match action {
            interface::graph::PostFailureAction::Terminate => {
                interface::graph::post_failure_action_terminate_target
            }
            interface::graph::PostFailureAction::TransientContinue => {
                interface::graph::post_failure_action_transient_continue_target
            }
        };
        self.call_target(target, vec![])
    }

    /// Build a generated `interface::graph::EdgeKind`.
    pub(crate) fn graph_edge_kind(
        &mut self,
        edge_kind: &interface::graph::EdgeKind,
    ) -> anyhow::Result<Argument> {
        let target = match edge_kind {
            interface::graph::EdgeKind::Normal => interface::graph::edge_kind_normal_target,
            interface::graph::EdgeKind::ForEach => interface::graph::edge_kind_for_each_target,
            interface::graph::EdgeKind::Collect => interface::graph::edge_kind_collect_target,
            interface::graph::EdgeKind::DoWhile => interface::graph::edge_kind_do_while_target,
            interface::graph::EdgeKind::Break => interface::graph::edge_kind_break_target,
            interface::graph::EdgeKind::Static => interface::graph::edge_kind_static_target,
        };
        self.call_target(target, vec![])
    }

    /// Build a generated `interface::verifier::VerifierMode`.
    pub(crate) fn verifier_mode(
        &mut self,
        mode: &interface::verifier::VerifierMode,
    ) -> anyhow::Result<Argument> {
        let target = match mode {
            interface::verifier::VerifierMode::None => {
                interface::verifier::verifier_mode_none_target
            }
            interface::verifier::VerifierMode::LeaderRegisteredKey => {
                interface::verifier::verifier_mode_authenticated_communication_target
            }
            interface::verifier::VerifierMode::ToolVerifierContract => {
                interface::verifier::verifier_mode_tool_verifier_contract_target
            }
        };
        self.call_target(target, vec![])
    }

    /// Build a generated `interface::verifier::VerifierConfig`.
    pub(crate) fn verifier_config(
        &mut self,
        config: &interface::verifier::VerifierConfig,
    ) -> anyhow::Result<Argument> {
        let mode = self.verifier_mode(&config.mode)?;
        let method = self.ascii_string(&config.method)?;
        self.call_target(
            interface::verifier::verifier_config_target,
            vec![mode, method],
        )
    }

    /// Build a generated `interface::verifier::FailureEvidenceKind`.
    pub(crate) fn failure_evidence_kind(
        &mut self,
        evidence_kind: &interface::verifier::FailureEvidenceKind,
    ) -> anyhow::Result<Argument> {
        let target = match evidence_kind {
            interface::verifier::FailureEvidenceKind::ToolEvidence => {
                interface::verifier::failure_evidence_kind_tool_evidence_target
            }
            interface::verifier::FailureEvidenceKind::LeaderEvidence => {
                interface::verifier::failure_evidence_kind_leader_evidence_target
            }
        };
        self.call_target(target, vec![])
    }

    /// Build a generated `interface::verifier::VerificationSubmissionKind`.
    pub(crate) fn verification_submission_kind(
        &mut self,
        submission_kind: &interface::verifier::VerificationSubmissionKind,
    ) -> anyhow::Result<Argument> {
        let target = match submission_kind {
            interface::verifier::VerificationSubmissionKind::Success => {
                interface::verifier::verification_submission_kind_success_target
            }
            interface::verifier::VerificationSubmissionKind::ErrEval => {
                interface::verifier::verification_submission_kind_err_eval_target
            }
        };
        self.call_target(target, vec![])
    }

    /// Build a generated `interface::verifier::VerifierDecision`.
    pub(crate) fn verifier_decision(
        &mut self,
        decision: &interface::verifier::VerifierDecision,
    ) -> anyhow::Result<Argument> {
        let target = match decision {
            interface::verifier::VerifierDecision::Accept => {
                interface::verifier::verifier_decision_accept_target
            }
            interface::verifier::VerifierDecision::Reject => {
                interface::verifier::verifier_decision_reject_target
            }
        };
        self.call_target(target, vec![])
    }

    /// Build a generated `primitives::data::NexusData`.
    pub(crate) fn nexus_data(&mut self, value: &NexusData) -> anyhow::Result<Argument> {
        let element_type = sui::types::TypeTag::Vector(Box::new(sui::types::TypeTag::U8));
        let (one_target, many_target) = match value.storage.as_slice() {
            b"inline" => (
                primitives::data::inline_one_target
                    as fn() -> Result<CallTarget, sui_move_call::CallSpecError>,
                primitives::data::inline_many_target
                    as fn() -> Result<CallTarget, sui_move_call::CallSpecError>,
            ),
            b"walrus" => (
                primitives::data::walrus_one_target
                    as fn() -> Result<CallTarget, sui_move_call::CallSpecError>,
                primitives::data::walrus_many_target
                    as fn() -> Result<CallTarget, sui_move_call::CallSpecError>,
            ),
            storage => anyhow::bail!(
                "unsupported NexusData storage tag: {}",
                hex::encode(storage)
            ),
        };

        if !value.many.is_empty() || value.one.is_empty() {
            let array = if value.many.is_empty() {
                self.tx
                    .make_move_vector(Some(element_type.clone()), vec![])?
            } else {
                let mut chunks = value
                    .many
                    .iter()
                    .cloned()
                    .try_fold(Vec::<Vec<Vec<u8>>>::new(), |mut chunks, value| {
                        let mut candidate = chunks.pop().unwrap_or_default();
                        candidate.push(value);

                        if candidate.len() > MAX_NEXUS_DATA_ARRAY_CHUNK_ARGS
                            || bcs::to_bytes(&candidate)?.len() > MAX_PURE_INPUT_BYTES
                        {
                            let last = candidate.pop().expect("candidate should not be empty");

                            if candidate.is_empty() {
                                anyhow::bail!(
                                    "single nexus data array element exceeds pure input size limit"
                                );
                            }

                            chunks.push(candidate);
                            chunks.push(vec![last]);
                        } else {
                            chunks.push(candidate);
                        }

                        Ok::<_, anyhow::Error>(chunks)
                    })?
                    .into_iter();
                let first = chunks
                    .next()
                    .expect("non empty values should yield a first chunk");
                let first_args = first
                    .into_iter()
                    .map(|value| self.tx.arg(&value))
                    .collect::<Result<Vec<_>, _>>()?;
                let array = self
                    .tx
                    .make_move_vector(Some(element_type.clone()), first_args)?;

                for chunk in chunks {
                    let chunk_args = chunk
                        .into_iter()
                        .map(|value| self.tx.arg(&value))
                        .collect::<Result<Vec<_>, _>>()?;
                    let chunk = self
                        .tx
                        .make_move_vector(Some(element_type.clone()), chunk_args)?;
                    let mut target = CallTarget::new(
                        sui::types::Address::from_static("0x1"),
                        "vector",
                        "append",
                    )?;
                    target.type_arguments.push(element_type.clone());
                    self.call_raw_target(target, vec![array, chunk])?;
                }

                array
            };

            return self.call_target(many_target, vec![array]);
        }

        let one = self.tx.arg(&value.one)?;
        self.call_target(one_target, vec![one])
    }

    /// Build a Move `0x1::option::Option<T>` from an optional PTB argument.
    pub fn option<T>(&mut self, value: Option<Argument>) -> Result<Argument, BuildError>
    where
        T: sui_move::MoveType,
    {
        option::<T>(&mut self.tx, value)
    }

    /// Finish and return the canonical programmable transaction.
    pub fn finish(self) -> sui::types::ProgrammableTransaction {
        self.tx.finish()
    }
}

/// Build a Nexus scoped programmable transaction.
///
/// This is the whole PTB form for SDK transaction construction. The closure receives the same
/// scoped builder type used by reusable fragments, while this function owns package scoping and
/// finalization.
#[cfg(feature = "transactions")]
pub fn ptb<'a>(
    objects: &'a NexusObjects,
    build: impl FnOnce(&mut NexusPtbBuilder<'a>) -> anyhow::Result<()>,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    let mut tx = NexusPtbBuilder::new(objects);
    crate::move_bindings::with_nexus_scope(objects, || build(&mut tx))?;
    Ok(tx.finish())
}

#[cfg(feature = "transactions")]
impl Deref for NexusPtbBuilder<'_> {
    type Target = PtbBuilder;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

#[cfg(feature = "transactions")]
impl DerefMut for NexusPtbBuilder<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

/// Build a Move `0x1::ascii::String` from bytes.
#[cfg(feature = "transactions")]
fn ascii_string(tx: &mut PtbBuilder, value: impl AsRef<str>) -> Result<Argument, BuildError> {
    let bytes = tx.arg(&value.as_ref().as_bytes().to_vec())?;
    tx.call_target(move_std::ascii::string_target()?, vec![bytes])
}

/// Build a Move `0x1::option::Option<T>` from an optional PTB argument.
#[cfg(feature = "transactions")]
fn option<T>(tx: &mut PtbBuilder, value: Option<Argument>) -> Result<Argument, BuildError>
where
    T: sui_move::MoveType,
{
    match value {
        Some(value) => tx.call_target(move_std::option::some_target::<T>()?, vec![value]),
        None => tx.call_target(move_std::option::none_target::<T>()?, vec![]),
    }
}

#[cfg(all(test, feature = "transactions"))]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::workflow::gas::{deescalate_target, GasService},
            sui,
            types::DefaultDagExecutorTarget,
        },
        sui_move::MoveStruct,
    };

    fn addr(byte: u8) -> sui::types::Address {
        sui::types::Address::new([byte; 32])
    }

    fn obj(byte: u8) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(addr(byte), 1, sui::types::Digest::new([byte; 32]))
    }

    #[test]
    fn derives_tool_object_ids_match_snapshots() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        let fqn = crate::fqn!("xyz.taluslabs.math.i64.add@1");
        assert_eq!(
            crate::move_bindings::derive_tool_id(registry_id, &fqn).unwrap(),
            sui::types::Address::from_static(
                "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35"
            )
        );
        assert_eq!(
            crate::move_bindings::derive_tool_gas_id(registry_id, &fqn).unwrap(),
            sui::types::Address::from_static(
                "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35"
            )
        );

        let fqn = crate::fqn!("xyz.taluslabs.math.i64.mul@1");
        assert_eq!(
            crate::move_bindings::derive_tool_id(registry_id, &fqn).unwrap(),
            sui::types::Address::from_static(
                "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da"
            )
        );
        assert_eq!(
            crate::move_bindings::derive_tool_gas_id(registry_id, &fqn).unwrap(),
            sui::types::Address::from_static(
                "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da"
            )
        );
    }

    #[test]
    fn derives_network_auth_binding_id_matches_snapshot() {
        let registry_pkg_id = "0x1b7beaf7c749f48e8746b2ee2803eaad6303bd353ad967c3e23db50317919beb"
            .parse()
            .unwrap();
        let network_auth_object_id =
            "0x47fc1741e0f9d0c3a8f573f82fc5c632bc3f3068c325bff24ecb76e4d685b696"
                .parse()
                .unwrap();
        let leader_cap_id = "0x1b7b4eeb8a11033f52b9394b6e284abd6dc33a2a22ff18f678b65d7a909b6eb7"
            .parse()
            .unwrap();

        assert_eq!(
            crate::move_bindings::derive_network_auth_binding_id(
                registry_pkg_id,
                network_auth_object_id,
                &crate::move_bindings::registry::network_auth::IdentityKey::leader(leader_cap_id),
            )
            .unwrap(),
            "0xcd2e634ec159ea299824d23a437992dba70c2a2239cfb7cd16a8ee767b17c040"
                .parse()
                .unwrap()
        );
    }

    fn objects() -> NexusObjects {
        NexusObjects {
            workflow_pkg_id: addr(0x44),
            scheduler_pkg_id: addr(0x55),
            primitives_pkg_id: addr(0x11),
            interface_pkg_id: addr(0x22),
            network_id: addr(0x77),
            registry_pkg_id: addr(0x33),
            tool_registry: obj(1),
            verifier_registry: obj(2),
            network_auth: obj(3),
            agent_registry: obj(4),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: addr(5),
                skill_id: 1,
            },
            gas_service: obj(6),
            leader_registry: obj(7),
            workflow_original_pkg_id: Some(addr(0x40)),
            scheduler_original_pkg_id: Some(addr(0x50)),
        }
    }

    #[test]
    fn scopes_call_package_and_type_package_separately() {
        let objects = objects();

        let (target, tag) = crate::move_bindings::with_nexus_scope(&objects, || {
            (
                deescalate_target().unwrap(),
                GasService::struct_tag_static(),
            )
        });

        assert_eq!(target.package, objects.workflow_pkg_id);
        assert_eq!(*tag.address(), objects.workflow_type_origin_pkg_id());
    }
}
