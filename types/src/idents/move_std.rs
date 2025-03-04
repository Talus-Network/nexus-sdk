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
