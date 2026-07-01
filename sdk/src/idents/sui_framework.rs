//! Identifiers for the `sui` (`0x2`) Move package.
//!
//! The per-module unit structs (`Address`, `Object`, `Coin`, `Transfer`, …) and
//! their `ModuleAndNameIdent` constants are generated at build time from
//! `generated/ir/sui_framework.json`. This module keeps the stable framework
//! object addresses and the hand-written argument helpers on top of them.

use crate::{idents::ModuleAndNameIdent, sui};

/// The `sui` framework package is published at the fixed address `0x2`.
pub const PACKAGE_ID: sui::types::Address = sui::types::Address::from_static("0x2");
/// The shared `Clock` object lives at the fixed address `0x6`.
pub const CLOCK_OBJECT_ID: sui::types::Address = sui::types::Address::from_static("0x6");
/// The shared `Random` object lives at the fixed address `0x8`.
pub const RANDOM_OBJECT_ID: sui::types::Address = sui::types::Address::from_static("0x8");

include!(concat!(env!("OUT_DIR"), "/idents_sui_framework.rs"));

impl Address {
    /// Convert a [`sui::types::Address`] into a [`sui::tx::Argument`].
    pub fn address_from_type(
        tx: &mut sui::tx::TransactionBuilder,
        address: sui::types::Address,
    ) -> anyhow::Result<sui::tx::Argument> {
        Ok(tx.pure(&address))
    }
}

impl Object {
    /// Convert an object ID to a Move ID.
    pub fn id_from_object_id(
        tx: &mut sui::tx::TransactionBuilder,
        object_id: sui::types::Address,
    ) -> anyhow::Result<sui::tx::Argument> {
        let address = Address::address_from_type(tx, object_id)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                PACKAGE_ID,
                Self::ID_FROM_ADDRESS.module,
                Self::ID_FROM_ADDRESS.name,
            ),
            vec![address],
        ))
    }
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::types::TypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(ident: ModuleAndNameIdent) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        PACKAGE_ID,
        ident.module,
        ident.name,
        vec![],
    )))
}
