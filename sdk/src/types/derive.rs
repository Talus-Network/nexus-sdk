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

#[cfg(test)]
mod tests {
    use {super::*, crate::fqn};

    #[test]
    fn test_derive_tool_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        // 1
        let fqn = fqn!("xyz.taluslabs.math.i64.add@1");
        let expected_id = sui::types::Address::from_static(
            "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35",
        );
        let derived_id = derive_tool_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);

        // 2
        let fqn = fqn!("xyz.taluslabs.math.i64.mul@1");
        let expected_id = sui::types::Address::from_static(
            "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da",
        );
        let derived_id = derive_tool_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_tool_gas_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        // 1
        let fqn = fqn!("xyz.taluslabs.math.i64.add@1");
        let expected_id = sui::types::Address::from_static(
            "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35",
        );
        let derived_id = derive_tool_gas_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);

        // 2
        let fqn = fqn!("xyz.taluslabs.math.i64.mul@1");
        let expected_id = sui::types::Address::from_static(
            "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da",
        );
        let derived_id = derive_tool_gas_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_invoker_gas_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );
        let address = sui::types::Address::from_static(
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        );
        let expected_id = sui::types::Address::from_static(
            "0x62000f053c3d54fa76229a93d255b5d40584c374b9c84aefee95bfd91a9d6bb1",
        );
        let derived_id = derive_invoker_gas_id(registry_id, address).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_execution_gas_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );
        let execution_id = sui::types::Address::from_static(
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        );
        let expected_id = sui::types::Address::from_static(
            "0x6cccb4844b7b37cfc085b976b9bb84f46763df213deea769072804164ffdb875",
        );
        let derived_id = derive_execution_gas_id(registry_id, execution_id).unwrap();
        assert_eq!(derived_id, expected_id);
    }
}
