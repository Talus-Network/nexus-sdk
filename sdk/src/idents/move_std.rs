use crate::{idents::ModuleAndNameIdent, sui};

// == `std::ascii` ==

pub struct Ascii;

const ASCII_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("ascii");

impl Ascii {
    /// `std::ascii::string`
    pub const STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: ASCII_MODULE,
        name: sui::move_ident_str!("string"),
    };

    /// Convert a string to a Move ASCII string.
    pub fn ascii_string_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = tx.pure(str.as_ref().as_bytes())?;

        Ok(tx.programmable_move_call(
            sui::MOVE_STDLIB_PACKAGE_ID,
            Self::STRING.module.into(),
            Self::STRING.name.into(),
            vec![],
            vec![str],
        ))
    }
}

// == `std::vector` ==

pub struct Vector;

const VECTOR_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("vector");

impl Vector {
    /// `std::vector::empty`
    pub const EMPTY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VECTOR_MODULE,
        name: sui::move_ident_str!("empty"),
    };
    /// `std::vector::push_back`
    pub const PUSH_BACK: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VECTOR_MODULE,
        name: sui::move_ident_str!("push_back"),
    };
    /// `std::vector::singleton`
    pub const SINGLETON: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VECTOR_MODULE,
        name: sui::move_ident_str!("singleton"),
    };
}

// == `std::string` ==

pub struct StdString;

const STRING_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("string");

impl StdString {
    /// `std::string::String`
    pub const STRING: ModuleAndNameIdent = ModuleAndNameIdent {
        module: STRING_MODULE,
        name: sui::move_ident_str!("String"),
    };

    /// Convenience helper to build the `std::string::String` type tag.
    pub fn type_tag() -> sui::MoveTypeTag {
        sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
            address: *sui::MOVE_STDLIB_PACKAGE_ID,
            module: Self::STRING.module.into(),
            name: Self::STRING.name.into(),
            type_params: vec![],
        }))
    }
}
