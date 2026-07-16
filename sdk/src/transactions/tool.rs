use {
    crate::{
        move_bindings::{
            registry::{
                registered_key_verifier as registered_key_verifier_binding,
                tool_registry as tool_registry_binding,
            },
            sui_framework::transfer as transfer_binding,
            workflow::gas as gas_binding,
        },
        move_boundary,
        sui,
        types::{NexusObjects, ToolMeta},
        ToolFqn,
    },
    anyhow::{bail, Context as _},
    std::{collections::HashSet, time::Duration},
    sui::types::{Argument, ProgrammableTransaction},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalVerifierObjectInput {
    pub object_ref: sui::types::ObjectReference,
    pub object_type: sui::types::TypeTag,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalVerifierRegistrationInput {
    pub package_id: sui::types::Address,
    pub module_name: String,
    pub function_name: String,
    /// Ordered immutable shared objects; object zero is the verifier witness.
    pub verifier_objects: Vec<ExternalVerifierObjectInput>,
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
}

impl ToolCollateral<'_> {
    fn ptb_argument(self, tx: &mut move_boundary::NexusPtbBuilder<'_>) -> anyhow::Result<Argument> {
        match self {
            Self::Coin(coin) => Ok(tx.owned_object(coin)?),
            Self::AddressBalance(amount) => Ok(tx.withdraw_sui_coin(amount)?),
        }
    }

    fn return_remainder(
        self,
        tx: &mut move_boundary::NexusPtbBuilder<'_>,
        coin: Argument,
        recipient: sui::types::Address,
    ) -> anyhow::Result<()> {
        if matches!(self, Self::AddressBalance(_)) {
            tx.send_sui_to_address_balance(coin, recipient)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct RegisteredToolArguments {
    tool: Argument,
    owner_cap_over_tool: Argument,
    owner_cap_over_gas: Argument,
}

fn timeout_millis(timeout: Duration) -> anyhow::Result<u64> {
    u64::try_from(timeout.as_millis()).context("tool timeout milliseconds do not fit in u64")
}

fn configure_registration(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    register_result: Argument,
    invocation_cost_mist: u64,
) -> anyhow::Result<RegisteredToolArguments> {
    let objects = tx.objects();
    let tool = tx.nested_result(register_result, 0)?;
    let owner_cap_over_tool = tx.nested_result(register_result, 1)?;
    let owner_cap_over_gas = tx.call_target(
        gas_binding::deescalate_target,
        vec![tool, owner_cap_over_tool],
    )?;
    let gas_service = tx.shared_object(&objects.gas_service, true)?;
    let invocation_cost_mist = tx.arg(&invocation_cost_mist)?;
    tx.call_target(
        gas_binding::create_tool_gas_and_share_target,
        vec![gas_service, tool, owner_cap_over_gas, invocation_cost_mist],
    )?;

    Ok(RegisteredToolArguments {
        tool,
        owner_cap_over_tool,
        owner_cap_over_gas,
    })
}

fn finish_registrations(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
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
                registration.owner_cap_over_gas,
            ]
        })
        .collect();
    let owner = tx.arg(&owner)?;
    tx.transfer_objects(capabilities, owner)?;
    Ok(())
}

fn register_off_chain_tool(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_registry: Argument,
    meta: &ToolMeta,
    pay_with: Argument,
    clock: Argument,
) -> anyhow::Result<Argument> {
    let fqn = tx.ascii_string(meta.fqn.to_string())?;
    let url = tx.arg(&meta.url.as_bytes().to_vec())?;
    let description = tx.arg(&meta.description.as_bytes().to_vec())?;
    let input_schema = tx.arg(&meta.input_schema)?;
    let output_schema = tx.arg(&meta.output_schema)?;
    let timeout_ms = timeout_millis(meta.timeout)?;
    let timeout_ms = tx.arg(&timeout_ms)?;

    tx.call_target(
        tool_registry_binding::register_off_chain_tool_target,
        vec![
            tool_registry,
            fqn,
            url,
            description,
            input_schema,
            output_schema,
            timeout_ms,
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
    objects: &NexusObjects,
    meta: &ToolMeta,
    address: sui::types::Address,
    collateral_coin: &sui::types::ObjectReference,
    invocation_cost_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    register_off_chain_for_self_with_collateral_ptb(
        objects,
        meta,
        address,
        ToolCollateral::Coin(collateral_coin),
        invocation_cost_mist,
    )
}

/// Builds a [`ProgrammableTransaction`] that registers an off chain tool using
/// collateral from the sender's address balance.
///
/// This is the address balance counterpart to
/// [`register_off_chain_for_self_ptb`].
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
pub fn register_off_chain_for_self_with_address_balance_ptb(
    objects: &NexusObjects,
    meta: &ToolMeta,
    address: sui::types::Address,
    collateral_mist: u64,
    invocation_cost_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    register_off_chain_for_self_with_collateral_ptb(
        objects,
        meta,
        address,
        ToolCollateral::AddressBalance(collateral_mist),
        invocation_cost_mist,
    )
}

fn register_off_chain_for_self_with_collateral_ptb(
    objects: &NexusObjects,
    meta: &ToolMeta,
    address: sui::types::Address,
    collateral: ToolCollateral<'_>,
    invocation_cost_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_object(&objects.tool_registry, true)?;
        let pay_with = collateral.ptb_argument(tx)?;
        let clock = tx.clock()?;
        let register_result = register_off_chain_tool(tx, tool_registry, meta, pay_with, clock)?;
        let registration = configure_registration(tx, register_result, invocation_cost_mist)?;
        finish_registrations(tx, &[registration], address)?;
        collateral.return_remainder(tx, pay_with, address)
    })
}

fn batch_collateral_mist(
    registrations: &[OffChainToolRegistration],
    collateral_per_tool_mist: u64,
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
    collateral_per_tool_mist
        .checked_mul(tool_count)
        .context("aggregate tool collateral overflows u64")
}

fn compose_off_chain_registration(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool_registry: Argument,
    pay_with: Argument,
    clock: Argument,
    registration: &OffChainToolRegistration,
) -> anyhow::Result<RegisteredToolArguments> {
    let register_result =
        register_off_chain_tool(tx, tool_registry, &registration.meta, pay_with, clock)?;
    let registered =
        configure_registration(tx, register_result, registration.invocation_cost_mist)?;
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
/// The transaction uses one withdrawal from the owner's address balance.
///
/// # Errors
///
/// Returns an error if the batch is empty, contains duplicate [`ToolFqn`]
/// values, requires more collateral than fits in `u64`, contains an invalid
/// timeout, or the transaction cannot be built.
pub fn register_off_chain_batch_for_self_with_address_balance_ptb(
    objects: &NexusObjects,
    registrations: &[OffChainToolRegistration],
    owner: sui::types::Address,
    collateral_per_tool_mist: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    let collateral_mist = batch_collateral_mist(registrations, collateral_per_tool_mist)?;

    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_object(&objects.tool_registry, true)?;
        let pay_with = tx.withdraw_sui_coin(collateral_mist)?;
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
        tx.send_sui_to_address_balance(pay_with, owner)?;
        Ok(())
    })
}

/// Builds a [`ProgrammableTransaction`] that registers an on chain Nexus tool
/// using an owned coin as collateral.
///
/// `workflow_authorization_cap_first` selects whether workflow vertex
/// authorization capability metadata is expected first.
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
#[allow(clippy::too_many_arguments)]
pub fn register_on_chain_for_self_with_workflow_authorization_cap_ptb(
    objects: &NexusObjects,
    package_address: sui::types::Address,
    module_name: &str,
    fqn: &ToolFqn,
    description: &str,
    input_schema: &str,
    output_schema: &str,
    timeout: Duration,
    tool_witness_id: sui::types::Address,
    collateral_coin: &sui::types::ObjectReference,
    address: sui::types::Address,
    workflow_authorization_cap_first: bool,
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
        ToolCollateral::Coin(collateral_coin),
        address,
        workflow_authorization_cap_first,
    )
}

/// Builds a [`ProgrammableTransaction`] that registers an on chain tool using
/// collateral from the sender's address balance.
///
/// This is the address balance counterpart to
/// [`register_on_chain_for_self_with_workflow_authorization_cap_ptb`].
///
/// # Errors
///
/// Returns an error if the timeout does not fit in `u64` milliseconds or the
/// transaction cannot be built.
#[allow(clippy::too_many_arguments)]
pub fn register_on_chain_for_self_with_address_balance_ptb(
    objects: &NexusObjects,
    package_address: sui::types::Address,
    module_name: &str,
    fqn: &ToolFqn,
    description: &str,
    input_schema: &str,
    output_schema: &str,
    timeout: Duration,
    tool_witness_id: sui::types::Address,
    collateral_mist: u64,
    address: sui::types::Address,
    workflow_authorization_cap_first: bool,
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
        ToolCollateral::AddressBalance(collateral_mist),
        address,
        workflow_authorization_cap_first,
    )
}

#[allow(clippy::too_many_arguments)]
fn register_on_chain_for_self_with_collateral_ptb(
    objects: &NexusObjects,
    package_address: sui::types::Address,
    module_name: &str,
    fqn: &ToolFqn,
    description: &str,
    input_schema: &str,
    output_schema: &str,
    timeout: Duration,
    tool_witness_id: sui::types::Address,
    collateral: ToolCollateral<'_>,
    address: sui::types::Address,
    workflow_authorization_cap_first: bool,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_object(&objects.tool_registry, true)?;
        let package_addr = tx.arg(&package_address)?;
        let module_name = tx.ascii_string(module_name)?;
        let fqn = tx.ascii_string(fqn.to_string())?;
        let description = tx.arg(&description.as_bytes().to_vec())?;
        let input_schema = tx.arg(&input_schema.as_bytes().to_vec())?;
        let output_schema = tx.arg(&output_schema.as_bytes().to_vec())?;
        let timeout_ms = timeout_millis(timeout)?;
        let timeout_ms = tx.arg(&timeout_ms)?;
        let tool_witness_id = tx.object_id(tool_witness_id)?;
        let pay_with = collateral.ptb_argument(tx)?;
        let clock = tx.clock()?;

        let target = if workflow_authorization_cap_first {
            tool_registry_binding::register_on_chain_tool_with_workflow_authorization_cap_target()?
        } else {
            tool_registry_binding::register_on_chain_tool_target()?
        };
        let register_result = tx.call_target(
            || Ok(target),
            vec![
                tool_registry,
                package_addr,
                module_name,
                fqn,
                description,
                input_schema,
                output_schema,
                timeout_ms,
                tool_witness_id,
                pay_with,
                clock,
            ],
        )?;

        let registration = configure_registration(tx, register_result, 0)?;
        finish_registrations(tx, &[registration], address)?;
        collateral.return_remainder(tx, pay_with, address)
    })
}

/// PTB template for setting the invocation cost of a Nexus Tool.
pub fn set_invocation_cost_ptb(
    objects: &NexusObjects,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    invocation_cost: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let gas_service = tx.shared_object(&objects.gas_service, true)?;
        let tool = tx.shared_object(tool, false)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let single_invocation_cost_mist = tx.arg(&invocation_cost)?;

        tx.call_target(
            gas_binding::set_single_invocation_cost_mist_target,
            vec![gas_service, tool, owner_cap, single_invocation_cost_mist],
        )?;
        Ok(())
    })
}

/// PTB template for unregistering a Nexus Tool.
pub fn unregister_ptb(
    objects: &NexusObjects,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool = tx.shared_object(tool, true)?;
        let registry = tx.shared_object(&objects.tool_registry, true)?;
        let verifier_registry = tx.shared_object(&objects.verifier_registry, true)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let clock = tx.clock()?;

        tx.call_target(
            tool_registry_binding::unregister_target,
            vec![tool, registry, verifier_registry, owner_cap, clock],
        )?;
        Ok(())
    })
}

/// Configure a Tool for the built-in two-signature RegisteredKey verifier.
pub fn configure_registered_key_verifier_ptb(
    objects: &NexusObjects,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    tool_key_binding: &sui::types::ObjectReference,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let registry = tx.shared_object(&objects.tool_registry, true)?;
        let tool = tx.shared_object(tool, false)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let network_auth = tx.shared_object(&objects.network_auth, false)?;
        let tool_key_binding = tx.shared_object(tool_key_binding, false)?;
        tx.call_target(
            registered_key_verifier_binding::configure_tool_target,
            vec![registry, tool, owner_cap, network_auth, tool_key_binding],
        )?;
        Ok(())
    })
}

/// Register one Tool-bound external verifier and its immutable shared object arguments.
pub fn register_external_verifier_ptb(
    objects: &NexusObjects,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    input: &ExternalVerifierRegistrationInput,
) -> anyhow::Result<ProgrammableTransaction> {
    let witness = input
        .verifier_objects
        .first()
        .ok_or_else(|| anyhow::anyhow!("external verifier requires a witness at object zero"))?;
    if input
        .verifier_objects
        .iter()
        .any(|object| *object.object_ref.object_id() == sui::types::Address::ZERO)
    {
        anyhow::bail!("external verifier object IDs must not be zero");
    }
    let mut unique_ids = std::collections::HashSet::new();
    if input
        .verifier_objects
        .iter()
        .any(|object| !unique_ids.insert(*object.object_ref.object_id()))
    {
        anyhow::bail!("external verifier objects must be unique");
    }

    move_boundary::ptb(objects, |tx| {
        let registry = tx.shared_object(&objects.tool_registry, true)?;
        let verifier_registry = tx.shared_object(&objects.verifier_registry, true)?;
        let tool_arg = tx.shared_object(tool, false)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let tool_id = tx.object_id(*tool.object_id())?;
        let package_id = tx.object_id(input.package_id)?;
        let module_name = tx.ascii_string(&input.module_name)?;
        let function_name = tx.ascii_string(&input.function_name)?;
        let witness_arg = tx.shared_object(&witness.object_ref, false)?;
        let registration = tx.call_function_with_type_args(
            objects.registry_pkg_id,
            "verifier_registry",
            "new_external_registration",
            vec![witness.object_type.clone()],
            vec![tool_id, package_id, module_name, function_name, witness_arg],
        )?;

        for object in input.verifier_objects.iter().skip(1) {
            let object_arg = tx.shared_object(&object.object_ref, false)?;
            tx.call_function_with_type_args(
                objects.registry_pkg_id,
                "verifier_registry",
                "add_obj",
                vec![object.object_type.clone()],
                vec![registration, object_arg],
            )?;
        }

        tx.call_target(
            tool_registry_binding::register_external_verifier_target,
            vec![
                registry,
                verifier_registry,
                tool_arg,
                owner_cap,
                registration,
            ],
        )?;
        Ok(())
    })
}

/// PTB template for claiming collateral for a Nexus Tool. The funds are
/// transferred to the tx sender.
pub fn claim_collateral_for_self_ptb(
    objects: &NexusObjects,
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

/// PTB template for updating a tool's timeout.
pub fn update_tool_timeout_ptb(
    objects: &NexusObjects,
    tool: &sui::types::ObjectReference,
    owner_cap: &sui::types::ObjectReference,
    new_timeout: Duration,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool = tx.shared_object(tool, false)?;
        let registry = tx.shared_object(&objects.tool_registry, true)?;
        let owner_cap = tx.owned_object(owner_cap)?;
        let timeout_ms = timeout_millis(new_timeout)?;
        let timeout_ms = tx.arg(&timeout_ms)?;

        tx.call_target(
            tool_registry_binding::update_tool_timeout_target,
            vec![tool, registry, owner_cap, timeout_ms],
        )?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::sui_framework::sui::SUI,
            test_utils::sui_mocks,
            types::DefaultDagExecutorTarget,
        },
        sui::types::{Command, WithdrawFrom},
        sui_move_call::CallArg,
    };

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

    fn nexus_objects() -> NexusObjects {
        NexusObjects {
            workflow_pkg_id: addr("0x1"),
            scheduler_pkg_id: addr("0x11"),
            primitives_pkg_id: addr("0x2"),
            interface_pkg_id: addr("0x3"),
            network_id: addr("0x4"),
            registry_pkg_id: addr("0x5"),
            tool_registry: object_ref("0x6", 1, 6),
            verifier_registry: object_ref("0x7", 1, 7),
            network_auth: object_ref("0x8", 1, 8),
            agent_registry: object_ref("0xc", 1, 12),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: addr("0xa1"),
                skill_id: 177,
            },
            gas_service: object_ref("0xd", 1, 13),
            leader_registry: object_ref("0xe", 1, 14),
            priority_fee_vault: object_ref("0xf", 1, 15),

            priority_fee_vault_owner_cap: object_ref("0x10", 1, 16),
            us_token: crate::types::UsTokenConfig {
                package_id: addr("0x11"),
                protected_treasury: None,
                metadata: None,
            },
            workflow_original_pkg_id: None,
            scheduler_original_pkg_id: None,
        }
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

    fn assert_sui_address_balance_withdrawal(
        objects: &NexusObjects,
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
        assert_eq!(
            withdrawal.coin_type(),
            &crate::move_bindings::type_tag::<SUI>(objects)
        );
        assert!(ptb.commands.iter().any(|command| {
            matches!(
                command,
                Command::MoveCall(call)
                    if call.package == sui::types::Address::from_static("0x2")
                        && call.module.as_str() == "coin"
                        && call.function.as_str() == "redeem_funds"
            )
        }));
        assert!(ptb.commands.iter().any(|command| {
            matches!(
                command,
                Command::MoveCall(call)
                    if call.package == sui::types::Address::TWO
                        && call.module.as_str() == "coin"
                        && call.function.as_str() == "send_funds"
            )
        }));
    }

    fn batch_registration(name: &str, key_byte: u8) -> OffChainToolRegistration {
        OffChainToolRegistration {
            meta: ToolMeta {
                fqn: format!("xyz.taluslabs.{name}@1").parse().unwrap(),
                url: format!("https://example.com/{name}"),
                description: name.to_string(),
                timeout: Duration::from_secs(1),
                input_schema: b"{}".to_vec(),
                output_schema: b"{}".to_vec(),
            },
            public_key: [key_byte; 32],
            pop_signature: [key_byte; 64],
            invocation_cost_mist: u64::from(key_byte),
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
    fn unregister_updates_tool_and_verifier_registries_atomically() {
        let objects = nexus_objects();
        let ptb = unregister_ptb(
            &objects,
            &object_ref("0x20", 2, 20),
            &object_ref("0x21", 3, 21),
        )
        .unwrap();
        let calls = move_calls(&ptb);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].module.as_str(), "tool_registry");
        assert_eq!(calls[0].function.as_str(), "unregister");
        assert_eq!(calls[0].arguments.len(), 5);
    }

    #[test]
    fn registered_key_configuration_uses_current_five_object_abi() {
        let objects = nexus_objects();
        let ptb = configure_registered_key_verifier_ptb(
            &objects,
            &object_ref("0x20", 2, 20),
            &object_ref("0x21", 3, 21),
            &object_ref("0x22", 4, 22),
        )
        .unwrap();
        let calls = move_calls(&ptb);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].module.as_str(), "registered_key_verifier");
        assert_eq!(calls[0].function.as_str(), "configure_tool");
        assert_eq!(calls[0].arguments.len(), 5);
    }

    #[test]
    fn external_registration_keeps_witness_first_and_appends_objects_in_order() {
        let objects = nexus_objects();
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
        let ptb = register_external_verifier_ptb(
            &objects,
            &object_ref("0x20", 2, 20),
            &object_ref("0x21", 3, 21),
            &input,
        )
        .unwrap();
        let calls = move_calls(&ptb)
            .into_iter()
            .filter(|call| {
                matches!(
                    call.function.as_str(),
                    "new_external_registration" | "add_obj" | "register_external_verifier"
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            calls
                .iter()
                .map(|call| call.function.as_str())
                .collect::<Vec<_>>(),
            vec![
                "new_external_registration",
                "add_obj",
                "register_external_verifier",
            ]
        );
        assert_eq!(calls[0].type_arguments, vec![witness_type]);
        assert_eq!(calls[1].type_arguments, vec![config_type]);
        assert_eq!(calls[0].arguments.len(), 5);
        assert_eq!(calls[1].arguments.len(), 2);
        assert_eq!(calls[2].arguments.len(), 5);
    }

    #[test]
    fn external_registration_requires_nonzero_unique_witness_and_objects() {
        let objects = nexus_objects();
        let tool = object_ref("0x20", 2, 20);
        let owner_cap = object_ref("0x21", 3, 21);
        let base = ExternalVerifierRegistrationInput {
            package_id: addr("0x41"),
            module_name: "verifier".to_string(),
            function_name: "verify".to_string(),
            verifier_objects: vec![],
        };

        assert!(
            register_external_verifier_ptb(&objects, &tool, &owner_cap, &base)
                .unwrap_err()
                .to_string()
                .contains("witness at object zero")
        );

        let object_type = object_type("0x40", "state", "Witness");
        let zero = ExternalVerifierRegistrationInput {
            verifier_objects: vec![ExternalVerifierObjectInput {
                object_ref: object_ref("0x0", 1, 1),
                object_type: object_type.clone(),
            }],
            ..base.clone()
        };
        assert!(
            register_external_verifier_ptb(&objects, &tool, &owner_cap, &zero)
                .unwrap_err()
                .to_string()
                .contains("must not be zero")
        );

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
        assert!(
            register_external_verifier_ptb(&objects, &tool, &owner_cap, &duplicate)
                .unwrap_err()
                .to_string()
                .contains("must be unique")
        );
    }

    #[test]
    fn single_off_chain_registration_retains_its_lifecycle_commands() {
        let objects = sui_mocks::mock_nexus_objects();
        let owner = sui_mocks::mock_sui_address();
        let registration = batch_registration("single", 6);
        let ptb = register_off_chain_for_self_with_address_balance_ptb(
            &objects,
            &registration.meta,
            owner,
            7,
            registration.invocation_cost_mist,
        )
        .unwrap();

        for (module, function) in [
            ("tool_registry", "register_off_chain_tool"),
            ("gas", "deescalate"),
            ("gas", "create_tool_gas_and_share"),
            ("transfer", "public_share_object"),
        ] {
            assert_eq!(move_call_indices(&ptb, module, function).len(), 1);
        }
        let transfer = ptb
            .commands
            .iter()
            .find_map(|command| match command {
                Command::TransferObjects(transfer) => Some(transfer),
                _ => None,
            })
            .expect("single registration must transfer both capabilities");
        assert_eq!(transfer.objects.len(), 2);
        assert_sui_address_balance_withdrawal(&objects, &ptb, 7);
    }

    #[test]
    fn atomic_batch_rejects_an_empty_catalog() {
        let objects = sui_mocks::mock_nexus_objects();
        let owner = sui_mocks::mock_sui_address();

        let error =
            register_off_chain_batch_for_self_with_address_balance_ptb(&objects, &[], owner, 1)
                .unwrap_err();
        assert!(error.to_string().contains("must not be empty"));
    }

    #[test]
    fn atomic_batch_rejects_duplicate_tool_names() {
        let objects = sui_mocks::mock_nexus_objects();
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
        let objects = sui_mocks::mock_nexus_objects();
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
        let objects = sui_mocks::mock_nexus_objects();
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
        assert_sui_address_balance_withdrawal(&objects, &first, 14);
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
            *objects.tool_registry.object_id(),
            *objects.gas_service.object_id(),
            *objects.network_auth.object_id(),
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
        let objects = sui_mocks::mock_nexus_objects();
        let address = sui_mocks::mock_sui_address();
        let meta = ToolMeta {
            fqn: "xyz.taluslabs.example@1".parse().unwrap(),
            url: "https://example.com".into(),
            description: "example".into(),
            timeout: Duration::from_secs(1),
            input_schema: b"{}".to_vec(),
            output_schema: b"{}".to_vec(),
        };

        let ptb =
            register_off_chain_for_self_with_address_balance_ptb(&objects, &meta, address, 42, 7)
                .unwrap();

        assert_sui_address_balance_withdrawal(&objects, &ptb, 42);
    }

    #[test]
    fn on_chain_registration_can_source_collateral_from_address_balance() {
        let objects = sui_mocks::mock_nexus_objects();
        let address = sui_mocks::mock_sui_address();
        let package = sui_mocks::mock_sui_address();
        let witness = sui_mocks::mock_sui_address();
        let fqn = "xyz.taluslabs.example@1".parse().unwrap();

        let ptb = register_on_chain_for_self_with_address_balance_ptb(
            &objects,
            package,
            "example",
            &fqn,
            "example",
            "{}",
            "{}",
            Duration::from_secs(1),
            witness,
            42,
            address,
            false,
        )
        .unwrap();

        assert_sui_address_balance_withdrawal(&objects, &ptb, 42);
    }

    #[test]
    fn on_chain_registration_rejects_timeout_that_does_not_fit_in_milliseconds() {
        let objects = sui_mocks::mock_nexus_objects();
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
            "{}",
            "{}",
            Duration::MAX,
            witness,
            42,
            address,
            false,
        )
        .unwrap_err();

        assert!(error.to_string().contains("timeout milliseconds"));
    }

    #[test]
    fn timeout_update_rejects_timeout_that_does_not_fit_in_milliseconds() {
        let objects = sui_mocks::mock_nexus_objects();
        let tool = sui_mocks::mock_sui_object_ref();
        let owner_cap = sui_mocks::mock_sui_object_ref();

        let error =
            update_tool_timeout_ptb(&objects, &tool, &owner_cap, Duration::MAX).unwrap_err();

        assert!(error.to_string().contains("timeout milliseconds"));
    }
}
