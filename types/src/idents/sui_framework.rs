use crate::{idents::ModuleAndNameIdent, sui};

// == `sui::address` ==

pub struct Address;

const ADDRESS_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("address");

impl Address {
    /// `sui::address::from_ascii_bytes`
    pub const FROM_ASCII_BYTES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: ADDRESS_MODULE,
        name: sui::move_ident_str!("from_ascii_bytes"),
    };

    /// Convert a string to a Move ASCII string.
    pub fn address_from_str<T: AsRef<str>>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        str: T,
    ) -> anyhow::Result<sui::Argument> {
        let str = tx.pure(str.as_ref().as_bytes())?;

        Ok(tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            Self::FROM_ASCII_BYTES.module.into(),
            Self::FROM_ASCII_BYTES.name.into(),
            vec![],
            vec![str],
        ))
    }
}

// == `sui::object` ==

pub struct Object;

const OBJECT_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("object");

impl Object {
    /// `sui::object::id_from_address`
    pub const ID_FROM_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OBJECT_MODULE,
        name: sui::move_ident_str!("id_from_address"),
    };

    /// Convert an object ID to a Move ID.
    pub fn id_from_object_id(
        tx: &mut sui::ProgrammableTransactionBuilder,
        object_id: sui::ObjectID,
    ) -> anyhow::Result<sui::Argument> {
        let with_prefix = false;
        let address = Address::address_from_str(tx, object_id.to_canonical_string(with_prefix))?;

        Ok(tx.programmable_move_call(
            sui::FRAMEWORK_PACKAGE_ID,
            Self::ID_FROM_ADDRESS.module.into(),
            Self::ID_FROM_ADDRESS.name.into(),
            vec![],
            vec![address],
        ))
    }
}

// == `sui::vec_set` ==

pub struct VecSet;

const VEC_SET_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("vec_set");

impl VecSet {
    /// `sui::vec_set::empty`
    pub const EMPTY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_SET_MODULE,
        name: sui::move_ident_str!("empty"),
    };
    /// `sui::vec_set::insert`
    pub const INSERT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_SET_MODULE,
        name: sui::move_ident_str!("insert"),
    };
}

// == `sui::vec_map` ==

pub struct VecMap;

const VEC_MAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("vec_map");

impl VecMap {
    /// `sui::vec_map::empty`
    pub const EMPTY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_MAP_MODULE,
        name: sui::move_ident_str!("empty"),
    };
    /// `sui::vec_map::insert`
    pub const INSERT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_MAP_MODULE,
        name: sui::move_ident_str!("insert"),
    };
}

// == `sui::transfer` ==

pub struct Transfer;

const TRANSFER_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("transfer");

impl Transfer {
    /// `sui::transfer::public_share_object`
    pub const PUBLIC_SHARE_OBJECT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TRANSFER_MODULE,
        name: sui::move_ident_str!("public_share_object"),
    };
}
