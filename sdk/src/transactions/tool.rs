use {
    crate::{
        move_bindings::{
            sui_framework::transfer as transfer_binding,
            tool::tool_registry as tool_registry_binding,
        },
        move_boundary,
        sui,
        types::{NexusContext, OnchainToolMode, PackageRole, ToolMeta},
        ToolFqn,
    },
    anyhow::{bail, Context as _},
    std::{collections::HashSet, time::Duration},
    sui::types::{Argument, ProgrammableTransaction},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalVerifierObjectInput {
    /// Current shared object reference.
    pub object_ref: sui::types::ObjectReference,
    /// Exact concrete Move type required by the verifier ABI.
    pub object_type: sui::types::TypeTag,
}

/// Validated external verifier definition used during Tool registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalVerifierRegistrationInput {
    /// Package that publishes the verifier function.
    pub package_id: sui::types::Address,
    /// Module containing the verifier function.
    pub module_name: String,
    /// Public verifier function name.
    pub function_name: String,
    /// Ordered immutable shared objects; object zero is the verifier witness.
    pub verifier_objects: Vec<ExternalVerifierObjectInput>,
}

/// Immutable verifier contract selected when an off chain Tool is registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolVerifierContractInput {
    /// The Tool does not support verification.
    None,
    /// The Tool uses the registered key verifier.
    RegisteredKey,
    /// The Tool uses one validated external verifier.
    External(ExternalVerifierRegistrationInput),
}

#[derive(Clone, Copy)]
enum ToolCollateral<'a> {
    Coin(&'a sui::types::ObjectReference),
    AddressBalance(u64),
}

/// Registration data for an off chain [`ToolMeta`] and its initial network
/// authorization key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OffChainToolRegistration {
    /// Metadata written to the tool registry.
    pub meta: ToolMeta,
    /// Initial Ed25519 public key bytes.
    pub public_key: [u8; 32],
    /// Proof that the initial key belongs to the tool identity.
    pub pop_signature: [u8; 64],
    /// Cost charged for one invocation in MIST.
    pub invocation_cost_mist: u64,
    /// Immutable verifier behavior for the Tool definition.
    pub verifier_contract: ToolVerifierContractInput,
}

impl ToolCollateral<'_> {
    fn ptb_argument(self, tx: &mut move_boundary::NexusPtbBuilder) -> anyhow::Result<Argument> {
        match self {
            Self::Coin(coin) => Ok(tx.owned_object(coin)?),
            Self::AddressBalance(amount) => {
                let us_type = tx.objects().us_token.type_tag();
                Ok(tx.withdraw_coin_from_address_balance(us_type, amount)?)
            }
        }
    }

    fn return_remainder(
        self,
        tx: &mut move_boundary::NexusPtbBuilder,
        coin: Argument,
        recipient: sui::types::Address,
    ) -> anyhow::Result<()> {
        if matches!(self, Self::AddressBalance(_)) {
            let us_type = tx.objects().us_token.type_tag();
            tx.send_coin_to_address_balance(us_type, coin, recipient)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct RegisteredToolArguments {
    tool: Argument,
    owner_cap_over_tool: Argument,
    cashier_admin_cap: Argument,
}

fn timeout_millis(timeout: Duration) -> anyhow::Result<u64> {
    u64::try_from(timeout.as_millis()).context("tool timeout milliseconds do not fit in u64")
}

fn configure_registration(
    tx: &mut move_boundary::NexusPtbBuilder,
    register_result: Argument,
) -> anyhow::Result<RegisteredToolArguments> {
    let tool = tx.nested_result(register_result, 0)?;
    let owner_cap_over_tool = tx.nested_result(register_result, 1)?;
    let cashier_admin_cap = tx.nested_result(register_result, 2)?;

    Ok(RegisteredToolArguments {
        tool,
        owner_cap_over_tool,
        cashier_admin_cap,
    })
}

fn build_verifier_contract(
    tx: &mut move_boundary::NexusPtbBuilder,
    tool_id: sui::types::Address,
    input: &ToolVerifierContractInput,
) -> anyhow::Result<Argument> {
    match input {
        ToolVerifierContractInput::None => {
            tx.call_target(tool_registry_binding::verifier_contract_none_target, vec![])
        }
        ToolVerifierContractInput::RegisteredKey => tx.call_target(
            tool_registry_binding::verifier_contract_registered_key_target,
            vec![],
        ),
        ToolVerifierContractInput::External(input) => {
            let registration = build_external_verifier_registration(tx, tool_id, input)?;
            tx.call_target(
                tool_registry_binding::verifier_contract_external_target,
                vec![registration],
            )
        }
    }
}

fn build_external_verifier_registration(
    tx: &mut move_boundary::NexusPtbBuilder,
    tool_id: sui::types::Address,
    input: &ExternalVerifierRegistrationInput,
) -> anyhow::Result<Argument> {
    let witness = input
        .verifier_objects
        .first()
        .ok_or_else(|| anyhow::anyhow!("external verifier requires a witness at object zero"))?;
    if input
        .verifier_objects
        .iter()
        .any(|object| *object.object_ref.object_id() == sui::types::Address::ZERO)
    {
        bail!("external verifier object IDs must not be zero");
    }
    let mut unique_ids = HashSet::with_capacity(input.verifier_objects.len());
    if input
        .verifier_objects
        .iter()
        .any(|object| !unique_ids.insert(*object.object_ref.object_id()))
    {
        bail!("external verifier objects must be unique");
    }

    let tool_package = tx.context().require_package(PackageRole::Tool)?.storage_id;
    let tool_id = tx.object_id(tool_id)?;
    let package_id = tx.object_id(input.package_id)?;
    let module_name = tx.ascii_string(&input.module_name)?;
    let function_name = tx.ascii_string(&input.function_name)?;
    let witness_argument = tx.shared_object(&witness.object_ref, false)?;
    let registration = tx.call_function_with_type_args(
        tool_package,
        "external_verifier",
        "new",
        vec![witness.object_type.clone()],
        vec![
            tool_id,
            package_id,
            module_name,
            function_name,
            witness_argument,
        ],
    )?;

    for object in input.verifier_objects.iter().skip(1) {
        let object_argument = tx.shared_object(&object.object_ref, false)?;
        tx.call_function_with_type_args(
            tool_package,
            "external_verifier",
            "add_object",
            vec![object.object_type.clone()],
            vec![registration, object_argument],
        )?;
    }

    Ok(registration)
}

fn finish_registrations(
    tx: &mut move_boundary::NexusPtbBuilder,
    registrations: &[RegisteredToolArguments],
    owner: sui::types::Address,
) -> anyhow::Result<()> {
    for registration in registrations {
        tx.call_target(
            transfer_binding::public_share_object_target::<tool_registry_binding::Tool>,
            vec![registration.tool],
        )?;
    }

    let capabilities = registrations
        .iter()
        .flat_map(|registration| {
            [
                registration.owner_cap_over_tool,
                registration.cashier_admin_cap,
            ]
        })
        .collect();
    let owner = tx.arg(&owner)?;
    tx.transfer_objects(capabilities, owner)?;
    Ok(())
}

fn register_off_chain_tool(
    tx: &mut move_boundary::NexusPtbBuilder,
    tool_registry: Argument,
    meta: &ToolMeta,
    verifier_contract: &ToolVerifierContractInput,
    invocation_cost_mist: u64,
    pay_with: Argument,
    clock: Argument,
) -> anyhow::Result<Argument> {
    let fqn = tx.ascii_string(meta.fqn.to_string())?;
    let url = tx.arg(&meta.url.as_bytes().to_vec())?;
    let description = tx.arg(&meta.description.as_bytes().to_vec())?;
    let meta_schema = tx.meta_schema(&meta.meta_schema()?)?;
    let timeout_ms = timeout_millis(meta.timeout)?;
    let timeout_ms = tx.arg(&timeout_ms)?;
    let tool_id =
        crate::move_bindings::derive_tool_id(tx.objects().tool_registry.object_id(), &meta.fqn)?;
    let verifier_contract = build_verifier_contract(tx, tool_id, verifier_contract)?;
    let invocation_cost_mist = tx.arg(&invocation_cost_mist)?;

    tx.call_target(
        tool_registry_binding::register_off_chain_tool_target,
        vec![
            tool_registry,
            fqn,
            url,
            description,
            meta_schema,
            timeout_ms,
            verifier_contract,
            invocation_cost_mist,
            pay_with,
            clock,
        ],
    )
}

/// Builds a [`ProgrammableTransaction`] that registers an off chain tool using
/// an owned coin as collateral.
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
pub fn register_off_chain_for_self_ptb(
    objects: &NexusContext,
    meta: &ToolMeta,
    verifier_contract: &ToolVerifierContractInput,
    address: sui::types::Address,
    collateral_coin: &sui::types::ObjectReference,
    invocation_cost_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    register_off_chain_for_self_with_collateral_ptb(
        objects,
        meta,
        verifier_contract,
        address,
        ToolCollateral::Coin(collateral_coin),
        invocation_cost_mist,
    )
}

/// Builds a [`ProgrammableTransaction`] that registers an off chain tool using
/// `$US` collateral from the sender address balance.
///
/// This is the address balance counterpart to
/// [`register_off_chain_for_self_ptb`].
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
pub fn register_off_chain_for_self_with_address_balance_ptb(
    objects: &NexusContext,
    meta: &ToolMeta,
    verifier_contract: &ToolVerifierContractInput,
    address: sui::types::Address,
    collateral_us: u64,
    invocation_cost_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    register_off_chain_for_self_with_collateral_ptb(
        objects,
        meta,
        verifier_contract,
        address,
        ToolCollateral::AddressBalance(collateral_us),
        invocation_cost_mist,
    )
}

fn register_off_chain_for_self_with_collateral_ptb(
    objects: &NexusContext,
    meta: &ToolMeta,
    verifier_contract: &ToolVerifierContractInput,
    address: sui::types::Address,
    collateral: ToolCollateral<'_>,
    invocation_cost_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_root(&objects.tool_registry, true)?;
        let pay_with = collateral.ptb_argument(tx)?;
        let clock = tx.clock()?;
        let register_result = register_off_chain_tool(
            tx,
            tool_registry,
            meta,
            verifier_contract,
            invocation_cost_mist,
            pay_with,
            clock,
        )?;
        let registration = configure_registration(tx, register_result)?;
        finish_registrations(tx, &[registration], address)?;
        collateral.return_remainder(tx, pay_with, address)
    })
}

fn batch_collateral_us(
    registrations: &[OffChainToolRegistration],
    collateral_per_tool_us: u64,
) -> anyhow::Result<u64> {
    if registrations.is_empty() {
        bail!("off chain tool registration batch must not be empty");
    }

    let mut fqns = HashSet::with_capacity(registrations.len());
    for (index, registration) in registrations.iter().enumerate() {
        if !fqns.insert(&registration.meta.fqn) {
            bail!(
                "registration index {index} repeats tool FQN '{}'",
                registration.meta.fqn
            );
        }
    }

    let tool_count = u64::try_from(registrations.len())
        .context("tool registration count does not fit in u64")?;
    collateral_per_tool_us
        .checked_mul(tool_count)
        .context("aggregate tool collateral overflows u64")
}

fn compose_off_chain_registration(
    tx: &mut move_boundary::NexusPtbBuilder,
    tool_registry: Argument,
    pay_with: Argument,
    clock: Argument,
    registration: &OffChainToolRegistration,
) -> anyhow::Result<RegisteredToolArguments> {
    let register_result = register_off_chain_tool(
        tx,
        tool_registry,
        &registration.meta,
        &registration.verifier_contract,
        registration.invocation_cost_mist,
        pay_with,
        clock,
    )?;
    let registered = configure_registration(tx, register_result)?;
    super::network_auth::create_tool_binding_and_register_key(
        tx,
        registered.tool,
        registered.owner_cap_over_tool,
        registration.public_key,
        registration.pop_signature,
        None,
    )?;
    Ok(registered)
}

/// Builds one [`ProgrammableTransaction`] that atomically registers off chain
/// [`OffChainToolRegistration`] values and their initial network authorization
/// keys.
///
/// The transaction uses one `$US` withdrawal from the owner address balance.
///
/// # Errors
///
/// Returns an error if the batch is empty, contains duplicate [`ToolFqn`]
/// values, requires more collateral than fits in `u64`, contains an invalid
/// timeout, or the transaction cannot be built.
pub fn register_off_chain_batch_for_self_with_address_balance_ptb(
    objects: &NexusContext,
    registrations: &[OffChainToolRegistration],
    owner: sui::types::Address,
    collateral_per_tool_us: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    let collateral_us = batch_collateral_us(registrations, collateral_per_tool_us)?;
    let collateral = ToolCollateral::AddressBalance(collateral_us);

    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_root(&objects.tool_registry, true)?;
        let pay_with = collateral.ptb_argument(tx)?;
        let clock = tx.clock()?;
        let mut registered_tools = Vec::with_capacity(registrations.len());

        for (index, registration) in registrations.iter().enumerate() {
            let registered =
                compose_off_chain_registration(tx, tool_registry, pay_with, clock, registration)
                    .with_context(|| {
                        format!(
                            "build registration index {index} for '{}'",
                            registration.meta.fqn
                        )
                    })?;
            registered_tools.push(registered);
        }

        finish_registrations(tx, &registered_tools, owner)?;
        collateral.return_remainder(tx, pay_with, owner)
    })
}

/// Builds a [`ProgrammableTransaction`] that registers an onchain Nexus Tool
/// using an owned coin as collateral.
///
/// [`OnchainToolMode`] selects the matching registry entrypoint.
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
#[allow(clippy::too_many_arguments)]
pub fn register_on_chain_for_self_ptb(
    objects: &NexusContext,
    package_address: sui::types::Address,
    module_name: &str,
    fqn: &ToolFqn,
    description: &str,
    input_schema: &str,
    output_schema: &str,
    timeout: Duration,
    tool_witness_id: sui::types::Address,
    invocation_cost_mist: u64,
    collateral_coin: &sui::types::ObjectReference,
    address: sui::types::Address,
    mode: OnchainToolMode,
) -> anyhow::Result<ProgrammableTransaction> {
    register_on_chain_for_self_with_collateral_ptb(
        objects,
        package_address,
        module_name,
        fqn,
        description,
        input_schema,
        output_schema,
        timeout,
        tool_witness_id,
        invocation_cost_mist,
        ToolCollateral::Coin(collateral_coin),
        address,
        mode,
    )
}

/// Builds a [`ProgrammableTransaction`] that registers an onchain Tool using
/// `$US` collateral from the sender address balance.
///
/// This is the address balance counterpart to
/// [`register_on_chain_for_self_ptb`].
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
#[allow(clippy::too_many_arguments)]
pub fn register_on_chain_for_self_with_address_balance_ptb(
    objects: &NexusContext,
    package_address: sui::types::Address,
    module_name: &str,
    fqn: &ToolFqn,
    description: &str,
    input_schema: &str,
    output_schema: &str,
    timeout: Duration,
    tool_witness_id: sui::types::Address,
    invocation_cost_mist: u64,
    collateral_us: u64,
    address: sui::types::Address,
    mode: OnchainToolMode,
) -> anyhow::Result<ProgrammableTransaction> {
    register_on_chain_for_self_with_collateral_ptb(
        objects,
        package_address,
        module_name,
        fqn,
        description,
        input_schema,
        output_schema,
        timeout,
        tool_witness_id,
        invocation_cost_mist,
        ToolCollateral::AddressBalance(collateral_us),
        address,
        mode,
    )
}

#[allow(clippy::too_many_arguments)]
fn register_on_chain_for_self_with_collateral_ptb(
    objects: &NexusContext,
    package_address: sui::types::Address,
    module_name: &str,
    fqn: &ToolFqn,
    description: &str,
    input_schema: &str,
    output_schema: &str,
    timeout: Duration,
    tool_witness_id: sui::types::Address,
    invocation_cost_mist: u64,
    collateral: ToolCollateral<'_>,
    address: sui::types::Address,
    mode: OnchainToolMode,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_root(&objects.tool_registry, true)?;
        let package_addr = tx.arg(&package_address)?;
        let module_name = tx.ascii_string(module_name)?;
        let fqn = tx.ascii_string(fqn.to_string())?;
        let description = tx.arg(&description.as_bytes().to_vec())?;
        let meta_schema =
            crate::move_bindings::interface::meta_schema::MetaSchema::from_onchain_json_schemas(
                input_schema,
                output_schema,
            )?;
        let meta_schema = tx.meta_schema(&meta_schema)?;
        let timeout_ms = timeout_millis(timeout)?;
        let timeout_ms = tx.arg(&timeout_ms)?;
        let tool_witness_id = tx.object_id(tool_witness_id)?;
        let invocation_cost_mist = tx.arg(&invocation_cost_mist)?;
        let pay_with = collateral.ptb_argument(tx)?;
        let clock = tx.clock()?;

        let arguments = vec![
            tool_registry,
            package_addr,
            module_name,
            fqn,
            description,
            meta_schema,
            timeout_ms,
            tool_witness_id,
            invocation_cost_mist,
            pay_with,
            clock,
        ];
        let register_result = match mode {
            OnchainToolMode::Standard => tx.call_target(
                tool_registry_binding::register_on_chain_tool_target,
                arguments,
            )?,
            OnchainToolMode::WorkflowAuthorization => tx.call_target(
                tool_registry_binding::register_on_chain_tool_with_workflow_authorization_cap_target,
                arguments,
            )?,
        };

        let registration = configure_registration(tx, register_result)?;
        finish_registrations(tx, &[registration], address)?;
        collateral.return_remainder(tx, pay_with, address)
    })
}

/// Builds a transaction that closes a [`Tool`](tool_registry_binding::Tool).
///
/// Closing removes the Tool from open membership while preserving its
/// immutable definition and outstanding settlement state.
pub fn close_ptb(
    context: &NexusContext,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(context, |tx| {
        let tool = tx.shared_object(tool, true)?;
        let registry = tx.shared_root(&context.tool_registry, true)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let clock = tx.clock()?;
        tx.call_target(
            tool_registry_binding::close_target,
            vec![tool, registry, owner_cap, clock],
        )?;
        Ok(())
    })
}

/// PTB template for claiming collateral for a Nexus Tool. The funds are
/// transferred to the tx sender.
pub fn claim_collateral_for_self_ptb(
    objects: &NexusContext,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool = tx.shared_object(tool, true)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let clock = tx.clock()?;

        tx.call_target(
            tool_registry_binding::claim_collateral_for_self_target,
            vec![tool, owner_cap, clock],
        )?;
        Ok(())
    })
}

/// Builds a transaction that retires a closed and drained
/// [`Tool`](tool_registry_binding::Tool).
pub fn retire_ptb(
    context: &NexusContext,
    tool: &sui::types::ObjectReference,
    cashier: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(context, |tx| {
        let tool = tx.shared_object(tool, true)?;
        let registry = tx.shared_root(&context.tool_registry, false)?;
        let cashier = tx.shared_object(cashier, false)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let clock = tx.clock()?;
        tx.call_target(
            tool_registry_binding::retire_target,
            vec![tool, registry, cashier, owner_cap, clock],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{test_utils::sui_mocks, types::PackageRole},
        sui::types::{Command, WithdrawFrom},
        sui_move_call::CallArg,
    };

    const OFFCHAIN_INPUT_SCHEMA: &[u8] = br#"{"type":"object","properties":{}}"#;
    const OFFCHAIN_OUTPUT_SCHEMA: &[u8] = br#"{"oneOf":[{"const":"Ok"}]}"#;
    const ONCHAIN_INPUT_SCHEMA: &str = "{}";
    const ONCHAIN_OUTPUT_SCHEMA: &str = r#"{"Ok":{"fields":{}}}"#;

    fn addr(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn object_ref(value: &'static str, version: u64, digest: u8) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(
            addr(value),
            version,
            sui::types::Digest::from([digest; 32]),
        )
    }

    fn nexus_objects() -> NexusContext {
        sui_mocks::mock_nexus_context()
    }

    fn move_calls(ptb: &ProgrammableTransaction) -> Vec<&sui::types::MoveCall> {
        ptb.commands
            .iter()
            .filter_map(|command| match command {
                Command::MoveCall(call) => Some(call),
                _ => None,
            })
            .collect()
    }

    fn assert_us_address_balance_withdrawal(
        objects: &NexusContext,
        ptb: &ProgrammableTransaction,
        expected_amount: u64,
    ) {
        let withdrawal = ptb
            .inputs
            .iter()
            .find_map(|input| match input {
                CallArg::FundsWithdrawal(withdrawal) => Some(withdrawal),
                _ => None,
            })
            .expect("registration must withdraw its collateral");

        assert_eq!(withdrawal.source(), WithdrawFrom::Sender);
        assert_eq!(withdrawal.amount(), Some(expected_amount));
        let us_type = objects.us_token.type_tag();
        assert_eq!(withdrawal.coin_type(), &us_type);

        for function in ["redeem_funds", "send_funds"] {
            let call = ptb
                .commands
                .iter()
                .find_map(|command| match command {
                    Command::MoveCall(call)
                        if call.package == sui::types::Address::TWO
                            && call.module.as_str() == "coin"
                            && call.function.as_str() == function =>
                    {
                        Some(call)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("expected coin::{function} call"));
            assert_eq!(call.type_arguments, vec![us_type.clone()]);
        }
    }

    fn batch_registration(name: &str, key_byte: u8) -> OffChainToolRegistration {
        OffChainToolRegistration {
            meta: ToolMeta {
                fqn: format!("xyz.taluslabs.{name}@1").parse().unwrap(),
                url: format!("https://example.com/{name}"),
                description: name.to_string(),
                timeout: Duration::from_secs(1),
                input_schema: OFFCHAIN_INPUT_SCHEMA.to_vec(),
                output_schema: OFFCHAIN_OUTPUT_SCHEMA.to_vec(),
            },
            public_key: [key_byte; 32],
            pop_signature: [key_byte; 64],
            invocation_cost_mist: u64::from(key_byte),
            verifier_contract: ToolVerifierContractInput::RegisteredKey,
        }
    }

    fn move_call_indices(
        ptb: &ProgrammableTransaction,
        module: &str,
        function: &str,
    ) -> Vec<usize> {
        ptb.commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| match command {
                Command::MoveCall(call)
                    if call.module.as_str() == module && call.function.as_str() == function =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect()
    }

    fn object_type(
        address: &'static str,
        module: &'static str,
        name: &'static str,
    ) -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            addr(address),
            sui::types::Identifier::from_static(module),
            sui::types::Identifier::from_static(name),
            vec![],
        )))
    }

    #[test]
    fn registered_key_verifier_is_fixed_during_registration() {
        let objects = nexus_objects();
        let registration = batch_registration("registered-key", 1);
        let ptb = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::RegisteredKey,
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap();
        let calls = move_calls(&ptb);
        assert!(calls.iter().any(|call| {
            call.module.as_str() == "tool_registry"
                && call.function.as_str() == "verifier_contract_registered_key"
        }));
    }

    #[test]
    fn no_verifier_is_fixed_during_registration() {
        let objects = nexus_objects();
        let registration = batch_registration("no-verifier", 1);
        let ptb = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::None,
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap();
        let calls = move_calls(&ptb);
        assert!(calls.iter().any(|call| {
            call.module.as_str() == "tool_registry"
                && call.function.as_str() == "verifier_contract_none"
        }));
    }

    #[test]
    fn close_uses_the_tool_and_current_registry() {
        let objects = nexus_objects();
        let tool = object_ref("0x20", 2, 20);
        let owner_cap = object_ref("0x21", 3, 21);
        let ptb = close_ptb(&objects, &tool, &owner_cap).unwrap();

        let calls = move_calls(&ptb);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].module.as_str(), "tool_registry");
        assert_eq!(calls[0].function.as_str(), "close");
        assert_eq!(calls[0].arguments.len(), 4);
        let tool_input = ptb
            .inputs
            .iter()
            .find_map(|input| match input {
                CallArg::Shared(shared) if shared.object_id() == *tool.object_id() => Some(shared),
                _ => None,
            })
            .expect("close must use the mutable Tool");
        assert!(tool_input.mutability().is_mutable());
        assert!(ptb.inputs.iter().any(|input| {
            matches!(input, CallArg::Shared(shared) if shared.object_id() == objects.tool_registry.object_id())
        }));
    }

    #[test]
    fn retire_uses_tool_registry_cashier_and_owner_authority() {
        let objects = nexus_objects();
        let ptb = retire_ptb(
            &objects,
            &object_ref("0x20", 2, 20),
            &object_ref("0x21", 3, 21),
            &object_ref("0x22", 4, 22),
        )
        .unwrap();
        let calls = move_calls(&ptb);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].module.as_str(), "tool_registry");
        assert_eq!(calls[0].function.as_str(), "retire");
        assert_eq!(calls[0].arguments.len(), 5);
    }

    #[test]
    fn external_registration_keeps_witness_first_and_appends_objects_in_order() {
        let objects = nexus_objects();
        let registration = batch_registration("external", 1);
        let witness_type = object_type("0x40", "state", "Witness");
        let config_type = object_type("0x40", "state", "Config");
        let input = ExternalVerifierRegistrationInput {
            package_id: addr("0x41"),
            module_name: "verifier".to_string(),
            function_name: "verify".to_string(),
            verifier_objects: vec![
                ExternalVerifierObjectInput {
                    object_ref: object_ref("0x42", 5, 42),
                    object_type: witness_type.clone(),
                },
                ExternalVerifierObjectInput {
                    object_ref: object_ref("0x43", 6, 43),
                    object_type: config_type.clone(),
                },
            ],
        };
        let ptb = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::External(input),
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap();
        let calls = move_calls(&ptb)
            .into_iter()
            .filter(|call| {
                (call.module.as_str() == "external_verifier"
                    && matches!(call.function.as_str(), "new" | "add_object"))
                    || (call.module.as_str() == "tool_registry"
                        && call.function.as_str() == "verifier_contract_external")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls
                .iter()
                .map(|call| (call.module.as_str(), call.function.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("external_verifier", "new"),
                ("external_verifier", "add_object"),
                ("tool_registry", "verifier_contract_external"),
            ]
        );
        assert_eq!(calls[0].type_arguments, vec![witness_type]);
        assert_eq!(calls[1].type_arguments, vec![config_type]);
        assert_eq!(calls[0].arguments.len(), 5);
        assert_eq!(calls[1].arguments.len(), 2);
        assert_eq!(calls[2].arguments.len(), 1);
    }

    #[test]
    fn external_registration_requires_nonzero_unique_witness_and_objects() {
        let objects = nexus_objects();
        let registration = batch_registration("invalid-external", 1);
        let base = ExternalVerifierRegistrationInput {
            package_id: addr("0x41"),
            module_name: "verifier".to_string(),
            function_name: "verify".to_string(),
            verifier_objects: vec![],
        };

        assert!(register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::External(base.clone()),
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap_err()
        .to_string()
        .contains("witness at object zero"));

        let object_type = object_type("0x40", "state", "Witness");
        let zero = ExternalVerifierRegistrationInput {
            verifier_objects: vec![ExternalVerifierObjectInput {
                object_ref: object_ref("0x0", 1, 1),
                object_type: object_type.clone(),
            }],
            ..base.clone()
        };
        assert!(register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::External(zero),
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap_err()
        .to_string()
        .contains("must not be zero"));

        let duplicate_ref = object_ref("0x42", 5, 42);
        let duplicate = ExternalVerifierRegistrationInput {
            verifier_objects: vec![
                ExternalVerifierObjectInput {
                    object_ref: duplicate_ref.clone(),
                    object_type: object_type.clone(),
                },
                ExternalVerifierObjectInput {
                    object_ref: duplicate_ref,
                    object_type,
                },
            ],
            ..base
        };
        assert!(register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::External(duplicate),
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap_err()
        .to_string()
        .contains("must be unique"));
    }

    #[test]
    fn single_off_chain_registration_retains_its_lifecycle_commands() {
        let objects = sui_mocks::mock_nexus_context();
        let owner = sui_mocks::mock_sui_address();
        let registration = batch_registration("single", 6);
        let ptb = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &registration.verifier_contract,
            owner,
            7,
            registration.invocation_cost_mist,
        )
        .unwrap();

        for (module, function) in [
            ("tool_registry", "register_off_chain_tool"),
            ("transfer", "public_share_object"),
        ] {
            assert_eq!(move_call_indices(&ptb, module, function).len(), 1);
        }
        let register = move_calls(&ptb)
            .into_iter()
            .find(|call| {
                call.module.as_str() == "tool_registry"
                    && call.function.as_str() == "register_off_chain_tool"
            })
            .expect("single registration must create the Tool and both capabilities");
        assert_eq!(register.arguments.len(), 10);
        let transfer = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::TransferObjects(transfer) => Some(transfer),
                _ => None,
            })
            .expect("single registration must transfer both capabilities");
        assert_eq!(transfer.objects.len(), 2);
        assert_us_address_balance_withdrawal(&objects, &ptb, 7);
    }

    #[test]
    fn atomic_batch_rejects_an_empty_catalog() {
        let objects = sui_mocks::mock_nexus_context();
        let owner = sui_mocks::mock_sui_address();

        let error =
            register_off_chain_batch_for_self_with_address_balance_ptb(&objects, &[], owner, 1)
                .unwrap_err();
        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn atomic_batch_rejects_duplicate_tool_names() {
        let objects = sui_mocks::mock_nexus_context();
        let owner = sui_mocks::mock_sui_address();

        let duplicate = batch_registration("duplicate", 1);
        let error = register_off_chain_batch_for_self_with_address_balance_ptb(
            &objects,
            &[duplicate.clone(), duplicate],
            owner,
            1,
        )
        .unwrap_err();
        assert!(error.to_string().contains("index 1"));
        assert!(error.to_string().contains("xyz.taluslabs.duplicate@1"));
    }

    #[test]
    fn atomic_batch_rejects_collateral_overflow() {
        let objects = sui_mocks::mock_nexus_context();
        let owner = sui_mocks::mock_sui_address();

        let error = register_off_chain_batch_for_self_with_address_balance_ptb(
            &objects,
            &[
                batch_registration("overflow_a", 2),
                batch_registration("overflow_b", 3),
            ],
            owner,
            u64::MAX,
        )
        .unwrap_err();
        assert!(error.to_string().contains("collateral"));
    }

    #[test]
    fn atomic_batch_uses_one_withdrawal_and_finishes_after_every_registration() {
        let objects = sui_mocks::mock_nexus_context();
        let owner = sui_mocks::mock_sui_address();
        let registrations = [
            batch_registration("atomic_a", 4),
            batch_registration("atomic_b", 5),
        ];

        let first = register_off_chain_batch_for_self_with_address_balance_ptb(
            &objects,
            &registrations,
            owner,
            7,
        )
        .unwrap();
        let second = register_off_chain_batch_for_self_with_address_balance_ptb(
            &objects,
            &registrations,
            owner,
            7,
        )
        .unwrap();

        assert_eq!(first, second);
        assert_us_address_balance_withdrawal(&objects, &first, 14);
        assert_eq!(
            first
                .inputs
                .iter()
                .filter(|input| matches!(input, CallArg::FundsWithdrawal(_)))
                .count(),
            1
        );
        assert_eq!(
            move_call_indices(&first, "tool_registry", "register_off_chain_tool").len(),
            2
        );
        let key_calls = move_call_indices(&first, "network_auth", "register_key");
        assert_eq!(key_calls.len(), 2);
        let last_key_call = *key_calls.last().expect("batch must register every key");

        let tool_type = crate::move_bindings::type_tag::<tool_registry_binding::Tool>(&objects);
        let tool_share_calls = first
            .commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| match command {
                Command::MoveCall(call)
                    if call.module.as_str() == "transfer"
                        && call.function.as_str() == "public_share_object"
                        && call.type_arguments.as_slice() == std::slice::from_ref(&tool_type) =>
                {
                    Some(index)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(tool_share_calls.len(), 2);
        assert!(tool_share_calls.iter().all(|index| *index > last_key_call));
        let last_tool_share_call = *tool_share_calls
            .last()
            .expect("batch must share every tool");

        let transfer = first
            .commands
            .iter()
            .enumerate()
            .find_map(|(index, command)| match command {
                Command::TransferObjects(transfer) => Some((index, transfer)),
                _ => None,
            })
            .expect("batch must transfer capabilities");
        assert_eq!(transfer.1.objects.len(), 4);
        assert!(transfer.0 > last_tool_share_call);

        for object_id in [
            objects.tool_registry.object_id(),
            objects.network_auth.object_id(),
            move_boundary::CLOCK_OBJECT_ID,
        ] {
            assert_eq!(
                first
                    .inputs
                    .iter()
                    .filter(|input| {
                        matches!(input, CallArg::Shared(shared) if shared.object_id() == object_id)
                    })
                    .count(),
                1
            );
        }
    }

    #[test]
    fn off_chain_registration_can_source_collateral_from_address_balance() {
        let objects = sui_mocks::mock_nexus_context();
        let address = sui_mocks::mock_sui_address();
        let meta = ToolMeta {
            fqn: "xyz.taluslabs.example@1".parse().unwrap(),
            url: "https://example.com".into(),
            description: "example".into(),
            timeout: Duration::from_secs(1),
            input_schema: OFFCHAIN_INPUT_SCHEMA.to_vec(),
            output_schema: OFFCHAIN_OUTPUT_SCHEMA.to_vec(),
        };

        let ptb = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &meta,
            &ToolVerifierContractInput::None,
            address,
            42,
            7,
        )
        .unwrap();

        assert_us_address_balance_withdrawal(&objects, &ptb, 42);
    }

    #[test]
    fn on_chain_registration_scopes_generated_targets_and_uses_address_balance() {
        let objects = sui_mocks::mock_nexus_context();
        let address = sui_mocks::mock_sui_address();
        let package = sui_mocks::mock_sui_address();
        let witness = sui_mocks::mock_sui_address();
        let fqn = "xyz.taluslabs.example@1".parse().unwrap();

        for (mode, expected_function) in [
            (
                crate::types::OnchainToolMode::Standard,
                "register_on_chain_tool",
            ),
            (
                crate::types::OnchainToolMode::WorkflowAuthorization,
                "register_on_chain_tool_with_workflow_authorization_cap",
            ),
        ] {
            let ptb = register_on_chain_for_self_with_address_balance_ptb(
                &objects,
                package,
                "example",
                &fqn,
                "example",
                ONCHAIN_INPUT_SCHEMA,
                ONCHAIN_OUTPUT_SCHEMA,
                Duration::from_secs(1),
                witness,
                7,
                42,
                address,
                mode,
            )
            .unwrap();

            assert_us_address_balance_withdrawal(&objects, &ptb, 42);
            let registration = move_calls(&ptb)
                .into_iter()
                .find(|call| {
                    call.module.as_str() == "tool_registry"
                        && call.function.as_str() == expected_function
                })
                .expect("on chain registration call");
            assert_eq!(
                registration.package,
                objects
                    .require_package(PackageRole::Tool)
                    .unwrap()
                    .storage_id
            );
            assert_eq!(registration.arguments.len(), 11);
        }
    }

    #[test]
    fn on_chain_registration_rejects_timeout_that_does_not_fit_in_milliseconds() {
        let objects = sui_mocks::mock_nexus_context();
        let address = sui_mocks::mock_sui_address();
        let package = sui_mocks::mock_sui_address();
        let witness = sui_mocks::mock_sui_address();
        let fqn = "xyz.taluslabs.example@1".parse().unwrap();

        let error = register_on_chain_for_self_with_address_balance_ptb(
            &objects,
            package,
            "example",
            &fqn,
            "example",
            ONCHAIN_INPUT_SCHEMA,
            ONCHAIN_OUTPUT_SCHEMA,
            Duration::MAX,
            witness,
            7,
            42,
            address,
            OnchainToolMode::Standard,
        )
        .unwrap_err();

        assert!(error.to_string().contains("timeout milliseconds"));
    }

    #[test]
    fn off_chain_registration_rejects_timeout_that_does_not_fit_in_milliseconds() {
        let objects = sui_mocks::mock_nexus_context();
        let mut registration = batch_registration("timeout", 1);
        registration.meta.timeout = Duration::MAX;

        let error = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            &ToolVerifierContractInput::None,
            addr("0x20"),
            1,
            registration.invocation_cost_mist,
        )
        .unwrap_err();

        assert!(error.to_string().contains("timeout milliseconds"));
    }
}
