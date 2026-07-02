use {
    crate::{
        move_bindings::{
            registry::tool_registry as tool_registry_binding,
            sui_framework::transfer as transfer_binding, workflow::gas as gas_binding,
        },
        move_boundary, sui,
        types::{NexusObjects, ToolMeta},
        ToolFqn,
    },
    std::time::Duration,
    sui::types::{Argument, ProgrammableTransaction},
};

fn finish_registration(
    tx: &mut move_boundary::NexusPtbBuilder<'_>,
    register_result: Argument,
    address: sui::types::Address,
    invocation_cost: u64,
) -> anyhow::Result<()> {
    let objects = tx.objects();
    let tool = tx.nested_result(register_result, 0)?;
    let owner_cap_over_tool = tx.nested_result(register_result, 1)?;

    let owner_cap_over_gas = tx.call_target(
        gas_binding::deescalate_target,
        vec![tool, owner_cap_over_tool],
    )?;

    let gas_service = tx.shared_object(&objects.gas_service, true)?;
    let single_invocation_cost_mist = tx.arg(&invocation_cost)?;
    tx.call_target(
        gas_binding::create_tool_gas_and_share_target,
        vec![
            gas_service,
            tool,
            owner_cap_over_gas,
            single_invocation_cost_mist,
        ],
    )?;

    tx.call_target(
        transfer_binding::public_share_object_target::<tool_registry_binding::Tool>,
        vec![tool],
    )?;

    let recipient = tx.arg(&address)?;
    tx.transfer_objects(vec![owner_cap_over_tool, owner_cap_over_gas], recipient)?;

    Ok(())
}

/// PTB template for registering a new Nexus Tool.
pub fn register_off_chain_for_self_ptb(
    objects: &NexusObjects,
    meta: &ToolMeta,
    address: sui::types::Address,
    collateral_coin: &sui::types::ObjectReference,
    invocation_cost: u64,
) -> anyhow::Result<ProgrammableTransaction> {
    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_object(&objects.tool_registry, true)?;
        let fqn = tx.ascii_string(meta.fqn.to_string())?;
        let url = tx.arg(&meta.url.to_string().into_bytes())?;
        let description = tx.arg(&meta.description.as_bytes().to_vec())?;
        let input_schema = tx.arg(&meta.input_schema)?;
        let output_schema = tx.arg(&meta.output_schema)?;
        let timeout_ms = tx.arg(&(meta.timeout.as_millis() as u64))?;
        let pay_with = tx.owned_object(collateral_coin)?;
        let clock = tx.clock()?;

        let register_result = tx.call_target(
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
        )?;

        finish_registration(tx, register_result, address, invocation_cost)
    })
}

/// PTB template for registering a new onchain Nexus Tool with optional
/// workflow vertex authorization cap-first metadata.
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
    move_boundary::ptb(objects, |tx| {
        let tool_registry = tx.shared_object(&objects.tool_registry, true)?;
        let package_addr = tx.arg(&package_address)?;
        let module_name = tx.ascii_string(module_name)?;
        let fqn = tx.ascii_string(fqn.to_string())?;
        let description = tx.arg(&description.as_bytes().to_vec())?;
        let input_schema = tx.arg(&input_schema.as_bytes().to_vec())?;
        let output_schema = tx.arg(&output_schema.as_bytes().to_vec())?;
        let timeout_ms = tx.arg(&(timeout.as_millis() as u64))?;
        let tool_witness_id = tx.object_id(tool_witness_id)?;
        let pay_with = tx.owned_object(collateral_coin)?;
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

        finish_registration(tx, register_result, address, 0)
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
        let owner_cap = tx.owned_object(owner_cap)?;
        let clock = tx.clock()?;

        tx.call_target(
            tool_registry_binding::unregister_target,
            vec![tool, owner_cap, clock],
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
        let timeout_ms = tx.arg(&(new_timeout.as_millis() as u64))?;

        tx.call_target(
            tool_registry_binding::update_tool_timeout_target,
            vec![tool, registry, owner_cap, timeout_ms],
        )?;
        Ok(())
    })
}
