//! Identifiers for the `std` (`0x1`) Move package.
//!
//! The per-module unit structs (`Option`, `Ascii`, `Vector`, `String`, …) and
//! their `ModuleAndNameIdent` constants are generated at build time from
//! `generated/ir/move_std.json`. This module keeps the stable `0x1` package
//! address and the hand-written argument/`TypeTag` helpers on top of them.

use crate::{idents::ModuleAndNameIdent, sui};

/// The `std` package is published at the fixed framework address `0x1`.
pub const PACKAGE_ID: sui::types::Address = sui::types::Address::from_static("0x1");

include!(concat!(env!("OUT_DIR"), "/idents_move_std.rs"));

impl Option {
    /// Build the `std::option::Option<element>` type tag.
    pub fn type_tag(element: sui::types::TypeTag) -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            PACKAGE_ID,
            Self::OPTION.module,
            Self::OPTION.name,
            vec![element],
        )))
    }

    /// Emit a `std::option::none<element>()` call.
    pub fn none(
        tx: &mut sui::tx::TransactionBuilder,
        element: sui::types::TypeTag,
    ) -> sui::tx::Argument {
        tx.move_call(
            sui::tx::Function::new(PACKAGE_ID, Self::NONE.module, Self::NONE.name)
                .with_type_args(vec![element]),
            vec![],
        )
    }

    /// Emit a `std::option::some<element>(value)` call.
    pub fn some(
        tx: &mut sui::tx::TransactionBuilder,
        element: sui::types::TypeTag,
        value: sui::tx::Argument,
    ) -> sui::tx::Argument {
        tx.move_call(
            sui::tx::Function::new(PACKAGE_ID, Self::SOME.module, Self::SOME.name)
                .with_type_args(vec![element]),
            vec![value],
        )
    }
}

impl Ascii {
    /// Convert a string to a Move ASCII string.
    pub fn str_to_argument<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        str: T,
    ) -> anyhow::Result<sui::tx::Argument> {
        let str = tx.pure(&str.as_ref().to_string());

        Ok(tx.move_call(
            sui::tx::Function::new(PACKAGE_ID, Self::STRING.module, Self::STRING.name),
            vec![str],
        ))
    }
}

impl String {
    /// Convenience helper to build the `std::string::String` type tag.
    pub fn type_tag() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            PACKAGE_ID,
            Self::STRING.module,
            Self::STRING.name,
            vec![],
        )))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn move_call(tx: sui::types::Transaction, index: usize) -> sui::types::MoveCall {
        let sui::types::TransactionKind::ProgrammableTransaction(programmable) = tx.kind else {
            panic!("expected programmable transaction");
        };
        match programmable.commands.get(index) {
            Some(sui::types::Command::MoveCall(call)) => call.clone(),
            other => panic!("expected move call, got {other:?}"),
        }
    }

    #[test]
    fn option_type_tag_wraps_element_type() {
        let element = String::type_tag();
        let tag = Option::type_tag(element.clone());

        let sui::types::TypeTag::Struct(tag) = tag else {
            panic!("expected struct type tag");
        };
        assert_eq!(*tag.address(), PACKAGE_ID);
        assert_eq!(*tag.module(), Option::OPTION.module);
        assert_eq!(*tag.name(), Option::OPTION.name);
        assert_eq!(tag.type_params(), &[element]);
    }

    #[test]
    fn option_some_builds_std_option_call() {
        let mut tx = sui::tx::TransactionBuilder::new();
        let value = tx.pure(&7_u64);
        let element = sui::types::TypeTag::U64;
        let _result = Option::some(&mut tx, element.clone(), value);

        let tx = crate::test_utils::sui_mocks::mock_finish_transaction(tx);
        let call = move_call(tx, 0);
        assert_eq!(call.package, PACKAGE_ID);
        assert_eq!(call.module, Option::SOME.module);
        assert_eq!(call.function, Option::SOME.name);
        assert_eq!(call.type_arguments, vec![element]);
        assert_eq!(call.arguments, vec![sui::types::Argument::Input(0)]);
    }
}
