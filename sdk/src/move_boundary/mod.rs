//! SDK boundary helpers for generated Move bindings.
//!
//! Generated package bindings carry the package IDs from committed IR. Production SDK code scopes
//! those bindings with the deployment specific package IDs from [`NexusObjects`] before creating
//! call targets or type tags.

#[cfg(feature = "transactions")]
use crate::move_bindings::interface::meta_schema::{
    MetaSchema,
    OutputVariantSchema,
    PortSchema,
    ValueKind,
};
#[cfg(feature = "transactions")]
use crate::move_bindings::primitives::data::NexusValue;
use crate::sui;
#[cfg(feature = "transactions")]
use crate::{
    move_bindings::{interface, move_std, primitives, sui_framework},
    types::{NexusData, NexusObjects},
};
#[cfg(feature = "transactions")]
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
#[cfg(feature = "transactions")]
use sui_move_call::CallTarget;
#[cfg(feature = "transactions")]
pub use sui_move_ptb::CLOCK_OBJECT_ID;
#[cfg(feature = "transactions")]
use sui_move_ptb::{BuildError, PtbBuilder};
#[cfg(feature = "transactions")]
use sui_sdk_types::Argument;

#[cfg(feature = "transactions")]
pub const RANDOM_OBJECT_ID: sui::types::Address = sui::types::Address::from_static("0x8");
#[cfg(feature = "transactions")]
const MAX_PURE_INPUT_BYTES: usize = 16_384;
#[cfg(feature = "transactions")]
const BYTE_VECTOR_CHUNK_BYTES: usize = MAX_PURE_INPUT_BYTES - 384;

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

/// A PTB builder bound to one Nexus deployment.
///
/// The builder owns the canonical [`PtbBuilder`] and the [`NexusObjects`] that
/// define package identity. Generated call targets and generic Move types are
/// resolved through that same deployment.
#[cfg(feature = "transactions")]
pub struct NexusPtbBuilder {
    objects: Arc<NexusObjects>,
    tx: PtbBuilder,
}

#[cfg(feature = "transactions")]
impl NexusPtbBuilder {
    pub(crate) fn new(objects: Arc<NexusObjects>) -> Self {
        Self {
            objects,
            tx: PtbBuilder::new(),
        }
    }

    /// Deployment object/package IDs associated with this PTB.
    pub fn objects(&self) -> &NexusObjects {
        self.objects.as_ref()
    }

    /// Add a generated Move call target to this PTB.
    pub fn call_target(
        &mut self,
        target: impl FnOnce() -> Result<CallTarget, sui_move_call::CallSpecError>,
        arguments: Vec<Argument>,
    ) -> anyhow::Result<Argument> {
        let target = crate::move_bindings::with_nexus_scope(self.objects.as_ref(), target)?;
        Ok(self.tx.call_target(target, arguments)?)
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

    /// Add a runtime owned Move call with already validated type arguments.
    pub fn call_function_with_type_args(
        &mut self,
        package: sui::types::Address,
        module: impl AsRef<str>,
        function: impl AsRef<str>,
        type_arguments: Vec<sui::types::TypeTag>,
        arguments: Vec<Argument>,
    ) -> anyhow::Result<Argument> {
        let mut target = CallTarget::new(package, module.as_ref(), function.as_ref())?;
        target.type_arguments = type_arguments;
        Ok(self.tx.call_target(target, arguments)?)
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

    /// Withdraws the requested asset from the sender address balance and
    /// redeems it as a coin of the same type.
    ///
    /// Callers must use this same type when returning any remaining balance.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] if the withdrawal input or redemption call cannot
    /// be built.
    pub fn withdraw_coin_from_address_balance(
        &mut self,
        coin_type: sui::types::TypeTag,
        amount: u64,
    ) -> Result<Argument, BuildError> {
        self.tx.funds_withdrawal_coin(coin_type, amount)
    }

    /// Withdraws SUI from the sender address balance and redeems it as a coin.
    ///
    /// # Errors
    ///
    /// Returns [`BuildError`] if the withdrawal input or redemption call cannot
    /// be built.
    pub fn withdraw_sui_coin(&mut self, amount: u64) -> Result<Argument, BuildError> {
        let sui_type =
            crate::move_bindings::type_tag::<sui_framework::sui::SUI>(self.objects.as_ref());
        self.withdraw_coin_from_address_balance(sui_type, amount)
    }

    /// Consumes a coin and credits its value to the recipient address balance
    /// using the same asset type.
    ///
    /// # Errors
    ///
    /// Returns an error if the recipient input or transfer call cannot be
    /// built.
    pub fn send_coin_to_address_balance(
        &mut self,
        coin_type: sui::types::TypeTag,
        coin: Argument,
        recipient: sui::types::Address,
    ) -> anyhow::Result<()> {
        let recipient = self.tx.arg(&recipient)?;
        let mut target = CallTarget::new(sui::types::Address::TWO, "coin", "send_funds")?;
        target.type_arguments = vec![coin_type];
        self.call_raw_target(target, vec![coin, recipient])?;
        Ok(())
    }

    /// Consumes a SUI coin and credits its value to an address balance.
    ///
    /// # Errors
    ///
    /// Returns an error if the recipient input or transfer call cannot be
    /// built.
    pub fn send_sui_to_address_balance(
        &mut self,
        coin: Argument,
        recipient: sui::types::Address,
    ) -> anyhow::Result<()> {
        let sui_type =
            crate::move_bindings::type_tag::<sui_framework::sui::SUI>(self.objects.as_ref());
        self.send_coin_to_address_balance(sui_type, coin, recipient)
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

    /// Build a generated `interface::verifier::ToolVerifierMode`.
    pub(crate) fn tool_verifier_mode(
        &mut self,
        mode: &interface::verifier::ToolVerifierMode,
    ) -> anyhow::Result<Argument> {
        let target = match mode {
            interface::verifier::ToolVerifierMode::None => {
                interface::verifier::verifier_mode_none_target
            }
            interface::verifier::ToolVerifierMode::RegisteredKey => {
                interface::verifier::verifier_mode_registered_key_target
            }
            interface::verifier::ToolVerifierMode::External => {
                interface::verifier::verifier_mode_external_target
            }
        };
        self.call_target(target, vec![])
    }

    /// Build one generated `primitives::data::NexusValue` witness.
    pub(crate) fn nexus_value(&mut self, value: &NexusValue) -> anyhow::Result<Argument> {
        if !value.is_well_formed() {
            anyhow::bail!("cannot build malformed NexusValue witness");
        }
        match value {
            NexusValue::Object { id } => {
                let id = self.object_id(id.address())?;
                self.call_target(primitives::data::object_value_target, vec![id])
            }
            NexusValue::InlineData { bytes } => {
                let bytes = self.byte_vector(bytes)?;
                self.call_target(primitives::data::inline_data_value_target, vec![bytes])
            }
            NexusValue::WalrusData {
                storage_key,
                content_digest,
            } => {
                let storage_key = self.byte_vector(storage_key)?;
                let content_digest = self.byte_vector(content_digest)?;
                self.call_target(
                    primitives::data::walrus_data_value_target,
                    vec![storage_key, content_digest],
                )
            }
        }
    }

    /// Build a schema port's exact ordered `NexusValue` witness group.
    pub(crate) fn nexus_value_witnesses(&mut self, value: &NexusData) -> anyhow::Result<Argument> {
        if !value.is_well_formed() {
            anyhow::bail!("cannot build malformed NexusData witnesses");
        }
        let values = value.values()?;
        let values = values
            .iter()
            .map(|value| self.nexus_value(value))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(self.move_vector::<NexusValue>(values)?)
    }

    /// Build a generated immutable `interface::meta_schema::MetaSchema`.
    pub(crate) fn meta_schema(&mut self, schema: &MetaSchema) -> anyhow::Result<Argument> {
        let input_ports = schema
            .input_ports
            .iter()
            .map(|port| self.port_schema(port))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let input_ports = self.move_vector::<PortSchema>(input_ports)?;
        let output_variants = schema
            .output_variants
            .iter()
            .map(|variant| {
                let name = self.tx.arg(&variant.variant_name)?;
                let ports = variant
                    .ports
                    .iter()
                    .map(|port| self.port_schema(port))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let ports = self.move_vector::<PortSchema>(ports)?;
                self.call_target(
                    interface::meta_schema::output_variant_schema_target,
                    vec![name, ports],
                )
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let output_variants = self.move_vector::<OutputVariantSchema>(output_variants)?;
        self.call_target(
            interface::meta_schema::new_target,
            vec![input_ports, output_variants],
        )
    }

    fn port_schema(&mut self, schema: &PortSchema) -> anyhow::Result<Argument> {
        let port_name = self.tx.arg(&schema.port_name)?;
        let is_many = self.tx.arg(&schema.is_many)?;
        let value_kind = self.value_kind(schema.value_kind)?;
        self.call_target(
            interface::meta_schema::port_schema_target,
            vec![port_name, is_many, value_kind],
        )
    }

    /// Build a `vector<u8>` without exceeding Sui's per-pure-input byte limit.
    fn byte_vector(&mut self, bytes: &[u8]) -> anyhow::Result<Argument> {
        let mut chunks = bytes.chunks(BYTE_VECTOR_CHUNK_BYTES);
        let first = chunks.next().unwrap_or_default().to_vec();
        let vector = self.tx.arg(&first)?;

        for chunk in chunks {
            let suffix = self.tx.arg(&chunk.to_vec())?;
            let mut target =
                CallTarget::new(sui::types::Address::from_static("0x1"), "vector", "append")?;
            target.type_arguments.push(sui::types::TypeTag::U8);
            self.call_raw_target(target, vec![vector, suffix])?;
        }

        Ok(vector)
    }

    fn value_kind(&mut self, kind: ValueKind) -> anyhow::Result<Argument> {
        let target = match kind {
            ValueKind::Object => interface::meta_schema::value_kind_object_target,
            ValueKind::Data => interface::meta_schema::value_kind_data_target,
        };
        self.call_target(target, vec![])
    }

    /// Build a typed Move `vector<T>` from existing PTB arguments.
    pub fn move_vector<T>(&mut self, elements: Vec<Argument>) -> Result<Argument, BuildError>
    where
        T: sui_move::MoveType,
    {
        let element_type = crate::move_bindings::type_tag::<T>(self.objects.as_ref());
        self.tx.make_move_vector(Some(element_type), elements)
    }

    /// Build a Move `0x1::option::Option<T>` from an optional PTB argument.
    pub fn option<T>(&mut self, value: Option<Argument>) -> Result<Argument, BuildError>
    where
        T: sui_move::MoveType,
    {
        crate::move_bindings::with_nexus_scope(self.objects.as_ref(), || {
            option::<T>(&mut self.tx, value)
        })
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
pub fn ptb(
    objects: &NexusObjects,
    build: impl FnOnce(&mut NexusPtbBuilder) -> anyhow::Result<()>,
) -> anyhow::Result<sui::types::ProgrammableTransaction> {
    let mut tx = NexusPtbBuilder::new(Arc::new(objects.clone()));
    crate::move_bindings::with_nexus_scope(objects, || build(&mut tx))?;
    Ok(tx.finish())
}

#[cfg(feature = "transactions")]
impl Deref for NexusPtbBuilder {
    type Target = PtbBuilder;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

#[cfg(feature = "transactions")]
impl DerefMut for NexusPtbBuilder {
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
            move_bindings::{
                interface::agent::FixedTool,
                tool::tool_cashier::{settle_payment_vertex_target, ToolCashier},
            },
            sui,
            types::{DefaultDagExecutorTarget, UsTokenConfig},
        },
        sui_move::MoveStruct,
        sui_move_call::CallArg,
    };

    fn addr(byte: u8) -> sui::types::Address {
        sui::types::Address::new([byte; 32])
    }

    fn obj(byte: u8) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(addr(byte), 1, sui::types::Digest::new([byte; 32]))
    }

    #[test]
    fn derives_tool_and_payment_ids_from_their_respective_parents() {
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
        let add_tool = crate::move_bindings::derive_tool_id(registry_id, &fqn).unwrap();
        let add_payment =
            crate::move_bindings::derive_tool_cashier_id(addr(0xa7), add_tool).unwrap();
        assert_ne!(add_payment, add_tool);

        let fqn = crate::fqn!("xyz.taluslabs.math.i64.mul@1");
        assert_eq!(
            crate::move_bindings::derive_tool_id(registry_id, &fqn).unwrap(),
            sui::types::Address::from_static(
                "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da"
            )
        );
        let mul_tool = crate::move_bindings::derive_tool_id(registry_id, &fqn).unwrap();
        let mul_payment =
            crate::move_bindings::derive_tool_cashier_id(addr(0xa7), mul_tool).unwrap();
        assert_ne!(mul_payment, mul_tool);
        assert_ne!(mul_payment, add_payment);
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
            protocol_version: 1,
            protocol: obj(10),
            packages: crate::types::NexusPackages {
                primitives: crate::types::PackageVersion::new(
                    addr(0x10),
                    addr(0x11),
                    2,
                    Default::default(),
                ),
                interface: crate::types::PackageVersion::new(
                    addr(0x20),
                    addr(0x22),
                    2,
                    Default::default(),
                ),
                tool: crate::types::PackageVersion::new(
                    addr(0x80),
                    addr(0x88),
                    2,
                    Default::default(),
                ),
                registry: crate::types::PackageVersion::new(
                    addr(0x30),
                    addr(0x33),
                    2,
                    Default::default(),
                ),
                workflow: crate::types::PackageVersion::new(
                    addr(0x40),
                    addr(0x44),
                    2,
                    Default::default(),
                ),
                scheduler: crate::types::PackageVersion::new(
                    addr(0x50),
                    addr(0x55),
                    2,
                    Default::default(),
                ),
            },
            config_hash: vec![0; 32],
            network_id: addr(0x77),
            tool_registry: obj(1),
            network_auth: obj(3),
            agent_registry: obj(4),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: addr(5),
                skill_id: 1,
            },
            leader_registry: obj(7),
            priority_fee_vault: obj(8),
            priority_fee_vault_owner_cap: obj(9),
            us_token: UsTokenConfig::new(addr(0x66)),
        }
    }

    fn nexus_value_constructors(value: &NexusData) -> Vec<String> {
        let mut transaction = NexusPtbBuilder::new(Arc::new(objects()));
        transaction.nexus_value_witnesses(value).unwrap();
        transaction
            .finish()
            .commands
            .into_iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call.function.to_string()),
                _ => None,
            })
            .collect()
    }

    fn assert_sui_protocol_limits(transaction: &sui::types::ProgrammableTransaction) {
        const MAX_PROGRAMMABLE_TX_COMMANDS: usize = 1_024;
        const MAX_PROGRAMMABLE_TX_BYTES_WITH_HEADROOM: usize = 120 * 1_024;

        assert!(transaction.commands.len() <= MAX_PROGRAMMABLE_TX_COMMANDS);
        for input in &transaction.inputs {
            if let sui::types::Input::Pure(bytes) = input {
                assert!(bytes.len() <= MAX_PURE_INPUT_BYTES);
            }
        }
        assert!(
            bcs::to_bytes(transaction).unwrap().len() <= MAX_PROGRAMMABLE_TX_BYTES_WITH_HEADROOM
        );
    }

    #[test]
    fn nexus_data_one_builds_one_inline_witness() {
        let value = NexusData::inline_data(b"one").expect("fixture is bounded");

        assert_eq!(nexus_value_constructors(&value), ["inline_data_value"]);
    }

    #[test]
    fn nexus_data_many_builds_each_inline_witness() {
        let value = NexusData::inline_data_many([b"one".to_vec(), b"two".to_vec()])
            .expect("fixture shape matches");

        assert_eq!(
            nexus_value_constructors(&value),
            ["inline_data_value", "inline_data_value"]
        );
    }

    #[test]
    fn nexus_data_empty_many_builds_no_ptb_inputs_or_commands() {
        let value = NexusData::new(b"nexus_value".to_vec(), Vec::new(), Vec::new());
        let mut transaction = NexusPtbBuilder::new(Arc::new(objects()));

        assert!(transaction
            .nexus_value_witnesses(&value)
            .unwrap_err()
            .to_string()
            .contains("cannot build malformed NexusData witnesses"));
        let transaction = transaction.finish();
        assert!(transaction.inputs.is_empty());
        assert!(transaction.commands.is_empty());
    }

    #[test]
    fn nexus_data_builds_object_and_walrus_values() {
        let mut transaction = NexusPtbBuilder::new(Arc::new(objects()));
        let object = NexusData::object(addr(0x99));
        let walrus = NexusData::walrus_data(b"blob-key", vec![7; 32]).expect("fixture is valid");

        transaction
            .nexus_value_witnesses(&object)
            .expect("typed Object should build");
        transaction
            .nexus_value_witnesses(&walrus)
            .expect("typed Walrus Data should build");
        let functions = transaction
            .finish()
            .commands
            .into_iter()
            .filter_map(|command| match command {
                sui::types::Command::MoveCall(call) => Some(call.function.to_string()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(functions.iter().any(|name| name == "object_value"));
        assert!(functions.iter().any(|name| name == "walrus_data_value"));
    }

    #[test]
    fn maximal_valid_inline_one_is_sui_ptb_buildable() {
        let value = NexusData::inline_data(vec![0; 61_440]).unwrap();
        let mut builder = NexusPtbBuilder::new(Arc::new(objects()));

        builder.nexus_value_witnesses(&value).unwrap();

        assert_sui_protocol_limits(&builder.finish());
    }

    #[test]
    fn aggregate_limit_inline_many_is_sui_ptb_buildable() {
        let value = NexusData::inline_data_many((0..128).map(|_| vec![0; 500])).unwrap();
        let mut builder = NexusPtbBuilder::new(Arc::new(objects()));

        builder.nexus_value_witnesses(&value).unwrap();

        assert_sui_protocol_limits(&builder.finish());
    }

    #[test]
    fn maximal_count_walrus_many_is_sui_ptb_buildable() {
        let value =
            NexusData::walrus_data_many((0..128).map(|_| (vec![0; 400], vec![0; 32]))).unwrap();
        let mut builder = NexusPtbBuilder::new(Arc::new(objects()));

        builder.nexus_value_witnesses(&value).unwrap();

        assert_sui_protocol_limits(&builder.finish());
    }

    #[test]
    fn maximal_meta_schema_is_sui_ptb_buildable() {
        let input_ports = (0..32)
            .map(|index| {
                PortSchema::new(
                    format!("input-{index}").into_bytes(),
                    false,
                    ValueKind::Data,
                )
            })
            .collect();
        let output_variants = (0..8)
            .map(|variant| {
                OutputVariantSchema::new(
                    format!("variant-{variant}").into_bytes(),
                    (0..16)
                        .map(|port| {
                            PortSchema::new(
                                format!("port-{port}").into_bytes(),
                                true,
                                ValueKind::Data,
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        let schema = MetaSchema::new(input_ports, output_variants);
        schema.validate_for_tool(false).unwrap();
        let mut builder = NexusPtbBuilder::new(Arc::new(objects()));

        builder.meta_schema(&schema).unwrap();
        let transaction = builder.finish();

        assert_eq!(transaction.commands.len(), 339);
        assert_sui_protocol_limits(&transaction);
    }

    #[test]
    fn scopes_call_package_and_type_package_separately() {
        let objects = objects();

        let (target, tag) = crate::move_bindings::with_nexus_scope(&objects, || {
            (
                settle_payment_vertex_target().unwrap(),
                ToolCashier::struct_tag_static(),
            )
        });

        assert_eq!(target.package, objects.tool_pkg_id());
        assert_eq!(*tag.address(), objects.tool_cashier_type_origin_pkg_id());
    }

    #[test]
    fn scopes_generated_generic_types() {
        let objects = objects();

        let mut transaction = NexusPtbBuilder::new(Arc::new(objects.clone()));
        transaction.move_vector::<FixedTool>(vec![]).unwrap();
        transaction.option::<FixedTool>(None).unwrap();
        let transaction = transaction.finish();

        let sui::types::Command::MakeMoveVector(vector) = &transaction.commands[0] else {
            panic!("expected a Move vector command");
        };
        let Some(sui::types::TypeTag::Struct(element_type)) = &vector.type_ else {
            panic!("expected a generated struct element type");
        };

        assert_eq!(
            *element_type.address(),
            objects.interface_type_origin_pkg_id()
        );

        let sui::types::Command::MoveCall(option) = &transaction.commands[1] else {
            panic!("expected an Option constructor call");
        };
        let sui::types::TypeTag::Struct(element_type) = &option.type_arguments[0] else {
            panic!("expected a generated Option element type");
        };

        assert_eq!(
            *element_type.address(),
            objects.interface_type_origin_pkg_id()
        );
    }

    #[test]
    fn verifier_modes_call_the_matching_move_constructors() {
        use crate::move_bindings::interface::verifier::ToolVerifierMode;

        for (mode, expected_function) in [
            (ToolVerifierMode::None, "verifier_mode_none"),
            (
                ToolVerifierMode::RegisteredKey,
                "verifier_mode_registered_key",
            ),
            (ToolVerifierMode::External, "verifier_mode_external"),
        ] {
            let transaction = ptb(&objects(), |tx| {
                tx.tool_verifier_mode(&mode)?;
                Ok(())
            })
            .unwrap();
            let call = transaction
                .commands
                .iter()
                .find_map(|command| match command {
                    sui::types::Command::MoveCall(call) => Some(call),
                    _ => None,
                })
                .expect("mode constructor emits one Move call");
            assert_eq!(call.function.as_str(), expected_function);
        }
    }

    #[test]
    fn withdraw_sui_coin_uses_sender_address_balance() {
        let objects = objects();

        let ptb = ptb(&objects, |tx| {
            tx.withdraw_sui_coin(42)?;
            Ok(())
        })
        .unwrap();

        let CallArg::FundsWithdrawal(withdrawal) = &ptb.inputs[0] else {
            panic!("expected funds withdrawal input");
        };
        assert_eq!(withdrawal.amount(), Some(42));
        assert_eq!(withdrawal.source(), sui::types::WithdrawFrom::Sender);
        assert_eq!(
            withdrawal.coin_type(),
            &crate::move_bindings::type_tag::<crate::move_bindings::sui_framework::sui::SUI>(
                &objects
            )
        );
        let sui::types::Command::MoveCall(redeem) = &ptb.commands[0] else {
            panic!("expected redeem funds call");
        };
        assert_eq!(redeem.package, sui::types::Address::from_static("0x2"));
        assert_eq!(redeem.module.as_str(), "coin");
        assert_eq!(redeem.function.as_str(), "redeem_funds");
    }

    #[test]
    fn send_sui_to_address_balance_consumes_the_coin() {
        let objects = objects();
        let recipient = addr(9);

        let ptb = ptb(&objects, |tx| {
            let coin = tx.withdraw_sui_coin(42)?;
            tx.send_sui_to_address_balance(coin, recipient)?;
            Ok(())
        })
        .unwrap();

        let sui::types::Command::MoveCall(send) = &ptb.commands[1] else {
            panic!("expected send_funds call");
        };
        assert_eq!(send.package, sui::types::Address::TWO);
        assert_eq!(send.module.as_str(), "coin");
        assert_eq!(send.function.as_str(), "send_funds");
    }
}
