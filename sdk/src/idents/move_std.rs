use crate::{
    idents::{move_std, pure_arg, ModuleAndNameIdent},
    sui,
};

pub const PACKAGE_ID: sui::types::Address = sui::types::Address::from_static("0x1");

// == `std::option` ==

pub struct Option;

const OPTION_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("option");

impl Option {
    /// `std::option::none`
    pub const NONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OPTION_MODULE,
        name: sui::types::Identifier::from_static("none"),
    };
    /// `std::option::Option`
    pub const OPTION: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OPTION_MODULE,
        name: sui::types::Identifier::from_static("Option"),
    };
    /// `std::option::some`
    pub const SOME: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OPTION_MODULE,
        name: sui::types::Identifier::from_static("some"),
    };

    pub fn type_tag(element: sui::types::TypeTag) -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            PACKAGE_ID,
            Self::OPTION.module,
            Self::OPTION.name,
            vec![element],
        )))
    }

    pub fn none(
        tx: &mut sui::tx::TransactionBuilder,
        element: sui::types::TypeTag,
    ) -> sui::types::Argument {
        tx.move_call(
            sui::tx::Function::new(
                PACKAGE_ID,
                Self::NONE.module,
                Self::NONE.name,
                vec![element],
            ),
            vec![],
        )
    }

    pub fn some(
        tx: &mut sui::tx::TransactionBuilder,
        element: sui::types::TypeTag,
        value: sui::types::Argument,
    ) -> sui::types::Argument {
        tx.move_call(
            sui::tx::Function::new(
                PACKAGE_ID,
                Self::SOME.module,
                Self::SOME.name,
                vec![element],
            ),
            vec![value],
        )
    }
}

// == `std::ascii` ==

pub struct Ascii;

const ASCII_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("ascii");

impl Ascii {
    /// `std::ascii::string`
    pub const STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: ASCII_MODULE,
        name: sui::types::Identifier::from_static("string"),
    };
    /// `std::ascii::String`
    pub const STRING_TYPE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: ASCII_MODULE,
        name: sui::types::Identifier::from_static("String"),
    };

    /// Convert a string to a Move ASCII string.
    pub fn ascii_string_from_str<T: AsRef<str>>(
        tx: &mut sui::tx::TransactionBuilder,
        str: T,
    ) -> anyhow::Result<sui::types::Argument> {
        let str = tx.input(pure_arg(&str.as_ref().to_string())?);

        Ok(tx.move_call(
            sui::tx::Function::new(
                move_std::PACKAGE_ID,
                Self::STRING.module,
                Self::STRING.name,
                vec![],
            ),
            vec![str],
        ))
    }
}

// == `std::vector` ==

pub struct Vector;

const VECTOR_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("vector");

impl Vector {
    /// `std::vector::empty`
    pub const EMPTY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VECTOR_MODULE,
        name: sui::types::Identifier::from_static("empty"),
    };
    /// `std::vector::push_back`
    pub const PUSH_BACK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VECTOR_MODULE,
        name: sui::types::Identifier::from_static("push_back"),
    };
    /// `std::vector::singleton`
    pub const SINGLETON: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VECTOR_MODULE,
        name: sui::types::Identifier::from_static("singleton"),
    };
}

// == `std::string` ==

pub struct StdString;

const STRING_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("string");

impl StdString {
    /// `std::string::String`
    pub const STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STRING_MODULE,
        name: sui::types::Identifier::from_static("String"),
    };

    /// Convenience helper to build the `std::string::String` type tag.
    pub fn type_tag() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            move_std::PACKAGE_ID,
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
        let element = StdString::type_tag();
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
        let value = tx.input(pure_arg(&7_u64).expect("pure arg"));
        let element = sui::types::TypeTag::U64;
        let result = Option::some(&mut tx, element.clone(), value);

        let tx = crate::test_utils::sui_mocks::mock_finish_transaction(tx);
        let call = move_call(tx, 0);
        assert_eq!(result, sui::types::Argument::Result(0));
        assert_eq!(call.package, PACKAGE_ID);
        assert_eq!(call.module, Option::SOME.module);
        assert_eq!(call.function, Option::SOME.name);
        assert_eq!(call.type_arguments, vec![element]);
        assert_eq!(call.arguments, vec![value]);
    }
}
