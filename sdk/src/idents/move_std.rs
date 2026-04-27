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
