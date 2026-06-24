//! Standard TAP agent interface helpers.
//!
//! The raw `ModuleAndNameIdent` constants for the agent interface are generated
//! per package — see [`crate::idents::interface`] (`Agent`, `Authorization`,
//! `Payment`, `Version`, …) and [`crate::idents::registry`]
//! (`AgentRegistry`). This module keeps only the module identifiers and the
//! hand-written `TypeTag` builders that span those packages and are resolved at
//! runtime.

use crate::sui;

pub const STANDARD_AGENT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("agent");
pub const STANDARD_AUTHORIZATION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("authorization");
pub const STANDARD_PAYMENT_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("payment");
pub const INTERFACE_VERSION_MODULE: sui::types::Identifier =
    sui::types::Identifier::from_static("version");

pub fn interface_version_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        INTERFACE_VERSION_MODULE,
        sui::types::Identifier::from_static("InterfaceVersion"),
        vec![],
    )))
}

pub fn interface_revision_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    interface_version_type(package_id)
}

pub fn scheduled_vertex_authorization_template_type(
    package_id: sui::types::Address,
) -> sui::types::TypeTag {
    agent_vertex_authorization_template_type(package_id)
}

pub fn agent_vertex_authorization_template_type(
    package_id: sui::types::Address,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_AUTHORIZATION_MODULE,
        sui::types::Identifier::from_static("AgentVertexAuthorizationTemplate"),
        vec![],
    )))
}

pub fn agent_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_AGENT_MODULE,
        sui::types::Identifier::from_static("Agent"),
        vec![],
    )))
}

pub fn agent_payment_vault_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_AGENT_MODULE,
        sui::types::Identifier::from_static("AgentPaymentVault"),
        vec![],
    )))
}

pub fn execution_payment_receipt_type(package_id: sui::types::Address) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        package_id,
        STANDARD_PAYMENT_MODULE,
        sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_tags_use_supplied_package() {
        let package = sui::types::Address::from_static("0x42");

        for (tag, expected_module, expected_name) in [
            (
                interface_revision_type(package),
                INTERFACE_VERSION_MODULE,
                sui::types::Identifier::from_static("InterfaceVersion"),
            ),
            (
                scheduled_vertex_authorization_template_type(package),
                STANDARD_AUTHORIZATION_MODULE,
                sui::types::Identifier::from_static("AgentVertexAuthorizationTemplate"),
            ),
            (
                execution_payment_receipt_type(package),
                STANDARD_PAYMENT_MODULE,
                sui::types::Identifier::from_static("ExecutionPaymentReceipt"),
            ),
        ] {
            let sui::types::TypeTag::Struct(tag) = tag else {
                panic!("expected struct type tag");
            };
            assert_eq!(*tag.address(), package);
            assert_eq!(*tag.module(), expected_module);
            assert_eq!(*tag.name(), expected_name);
            assert!(tag.type_params().is_empty());
        }
    }
}
