//! Programmable transaction builders for `nexus_registry::network_auth`.

use {
    crate::{
        move_bindings::{
            registry::network_auth as network_auth_binding,
            sui_framework::transfer as transfer_binding,
        },
        move_boundary, sui,
        types::NexusObjects,
    },
    sui::types::{Argument, ProgrammableTransaction},
};

fn description_option(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    description: Option<Vec<u8>>,
) -> anyhow::Result<Argument> {
    let description = description
        .as_ref()
        .map(|description| tx.arg(description))
        .transpose()?;
    Ok(tx.option::<Vec<u8>>(description)?)
}

fn proof_for_offchain_tool(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    tool: Argument,
    owner_cap: Argument,
) -> anyhow::Result<Argument> {
    Ok(tx.call_target(
        network_auth_binding::prove_offchain_tool_target,
        vec![tool, owner_cap],
    )?)
}

fn proof_for_leader(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    leader_cap: Argument,
) -> anyhow::Result<Argument> {
    Ok(tx.call_target(network_auth_binding::prove_leader_target, vec![leader_cap])?)
}

fn register_key(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    binding: Argument,
    proof: Argument,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
) -> anyhow::Result<()> {
    let public_key = tx.arg(&public_key.to_vec())?;
    let signature = tx.arg(&pop_signature.to_vec())?;
    let proof_of_key = tx.call_target(
        network_auth_binding::new_proof_of_key_target,
        vec![binding, proof, public_key, signature],
    )?;
    let clock = tx.clock()?;

    tx.call_target(
        network_auth_binding::register_key_target,
        vec![binding, proof, proof_of_key, clock],
    )?;
    Ok(())
}

fn create_binding(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    proof: Argument,
    description: Option<Vec<u8>>,
) -> anyhow::Result<Argument> {
    let objects = tx.objects();
    let network_auth = tx.shared_object(&objects.network_auth, true)?;
    let description = description_option(tx, description)?;

    Ok(tx.call_target(
        network_auth_binding::create_binding_target,
        vec![network_auth, proof, description],
    )?)
}

fn share_binding(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    binding: Argument,
) -> anyhow::Result<()> {
    tx.call_target(
        transfer_binding::public_share_object_target::<network_auth_binding::KeyBinding>,
        vec![binding],
    )?;
    Ok(())
}

/// Create a new off-chain tool key binding and register the first key.
///
/// This is used when the binding object does not yet exist.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_tool_binding_and_register_key_ptb(
    objects: &NexusObjects,
    tool: &sui::types::ObjectReference,
    owner_cap_over_tool: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
    description: Option<Vec<u8>>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool = tx.shared_object(tool, false)?;
        let owner_cap = tx.owned_object(owner_cap_over_tool)?;

        let proof_for_binding = proof_for_offchain_tool(tx, tool, owner_cap)?;
        let binding = create_binding(tx, proof_for_binding, description)?;

        let proof_for_key = proof_for_offchain_tool(tx, tool, owner_cap)?;
        register_key(tx, binding, proof_for_key, public_key, pop_signature)?;
        share_binding(tx, binding)
    })
}

/// Register a new key on an existing off-chain tool key binding.
///
/// This is used for rotation when the `KeyBinding` already exists.
#[allow(clippy::too_many_arguments)]
pub(crate) fn register_tool_key_on_existing_binding_ptb(
    objects: &NexusObjects,
    binding: &sui::types::ObjectReference,
    tool: &sui::types::ObjectReference,
    owner_cap_over_tool: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let binding = tx.shared_object(binding, true)?;
        let tool = tx.shared_object(tool, false)?;
        let owner_cap = tx.owned_object(owner_cap_over_tool)?;

        let proof = proof_for_offchain_tool(tx, tool, owner_cap)?;
        register_key(tx, binding, proof, public_key, pop_signature)
    })
}

/// Create a new leader key binding and register the first key.
///
/// This is used when the binding object does not yet exist.
#[allow(clippy::too_many_arguments)]
pub fn create_leader_binding_and_register_key_ptb(
    objects: &NexusObjects,
    leader_cap_over_network: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
    description: Option<Vec<u8>>,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let leader_cap = tx.shared_object(leader_cap_over_network, false)?;

        let proof_for_binding = proof_for_leader(tx, leader_cap)?;
        let binding = create_binding(tx, proof_for_binding, description)?;

        let proof_for_key = proof_for_leader(tx, leader_cap)?;
        register_key(tx, binding, proof_for_key, public_key, pop_signature)?;
        share_binding(tx, binding)
    })
}

/// Register a new key on an existing leader key binding.
///
/// This is used for rotation when the `KeyBinding` already exists.
#[allow(clippy::too_many_arguments)]
pub fn register_leader_key_on_existing_binding_ptb(
    objects: &NexusObjects,
    binding: &sui::types::ObjectReference,
    leader_cap_over_network: &sui::types::ObjectReference,
    public_key: [u8; 32],
    pop_signature: [u8; 64],
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let binding = tx.shared_object(binding, true)?;
        let leader_cap = tx.shared_object(leader_cap_over_network, false)?;

        let proof = proof_for_leader(tx, leader_cap)?;
        register_key(tx, binding, proof, public_key, pop_signature)
    })
}
