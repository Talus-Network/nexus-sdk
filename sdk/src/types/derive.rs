//! Helper for deriving Sui object IDs.

use {
    crate::{
        idents::{move_std, sui_framework},
        sui::{self, traits::ToBcs},
        ToolFqn,
    },
    serde::Serialize,
};

/// Generic function for deriving an object ID given a parent, type tag, and key.
pub fn derive_object_id<T: Serialize>(
    parent: sui::types::Address,
    tag: &sui::types::TypeTag,
    key: &T,
) -> anyhow::Result<sui::types::Address> {
    Ok(parent.derive_object_id(tag, &key.to_bcs()?))
}

/// Derives the object ID for a Tool given the tool registry and tool FQN.
pub fn derive_tool_id(
    tool_registry: sui::types::Address,
    tool_fqn: &ToolFqn,
) -> anyhow::Result<sui::types::Address> {
    let tag = move_std::into_type_tag(move_std::Ascii::STRING_TYPE);

    derive_object_id(tool_registry, &tag, &tool_fqn)
}

/// Derives the object ID for a ToolGas object given the gas service and tool FQN.
pub fn derive_tool_gas_id(
    gas_service: sui::types::Address,
    tool_fqn: &ToolFqn,
) -> anyhow::Result<sui::types::Address> {
    let tag = move_std::into_type_tag(move_std::Ascii::STRING_TYPE);

    derive_object_id(gas_service, &tag, &tool_fqn)
}

/// Derives the object ID for an InvokerGas object given the gas service and invoker address.
pub fn derive_invoker_gas_id(
    gas_service: sui::types::Address,
    invoker: sui::types::Address,
) -> anyhow::Result<sui::types::Address> {
    let tag = sui::types::TypeTag::Address;

    derive_object_id(gas_service, &tag, &invoker)
}

/// Derives the object ID for an ExecutionGas object given the gas service and execution ID.
pub fn derive_execution_gas_id(
    gas_service: sui::types::Address,
    execution: sui::types::Address,
) -> anyhow::Result<sui::types::Address> {
    let tag = sui_framework::into_type_tag(sui_framework::Object::ID);

    derive_object_id(gas_service, &tag, &execution)
}
