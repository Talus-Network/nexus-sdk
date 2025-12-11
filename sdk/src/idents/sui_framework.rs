use crate::{
    idents::{pure_arg, ModuleAndNameIdent},
    sui,
};

pub const PACKAGE_ID: sui::types::Address = sui::types::Address::from_static("0x2");
pub const CLOCK_OBJECT_ID: sui::types::Address = sui::types::Address::from_static("0x6");

// == `sui::types::Address` ==

pub struct Address;

const ADDRESS_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("address");

impl Address {
    /// `sui::types::Address::from_ascii_bytes`
    pub const FROM_ASCII_BYTES: ModuleAndNameIdent = ModuleAndNameIdent {
        module: ADDRESS_MODULE,
        name: sui::types::Identifier::from_static("from_ascii_bytes"),
    };

    /// Covert [`sui::types::Address`] into a [`sui::types::Argument`].
    pub fn address_from_type(
        tx: &mut sui::tx::TransactionBuilder,
        address: sui::types::Address,
    ) -> anyhow::Result<sui::types::Argument> {
        Ok(tx.input(pure_arg(&address)?))
    }
}

// == `sui::object` ==

pub struct Object;

const OBJECT_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("object");

impl Object {
    /// `sui::object::id_from_address`
    pub const ID_FROM_ADDRESS: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OBJECT_MODULE,
        name: sui::types::Identifier::from_static("id_from_address"),
    };

    /// Convert an object ID to a Move ID.
    pub fn id_from_object_id(
        tx: &mut sui::tx::TransactionBuilder,
        object_id: sui::types::Address,
    ) -> anyhow::Result<sui::types::Argument> {
        let address = Address::address_from_type(tx, object_id)?;

        Ok(tx.move_call(
            sui::tx::Function::new(
                PACKAGE_ID,
                Self::ID_FROM_ADDRESS.module,
                Self::ID_FROM_ADDRESS.name,
                vec![],
            ),
            vec![address],
        ))
    }
}

// == `sui::vec_set` ==

pub struct VecSet;

const VEC_SET_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("vec_set");

impl VecSet {
    /// `sui::vec_set::empty`
    pub const EMPTY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_SET_MODULE,
        name: sui::types::Identifier::from_static("empty"),
    };
    /// `sui::vec_set::insert`
    pub const INSERT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_SET_MODULE,
        name: sui::types::Identifier::from_static("insert"),
    };
}

// == `sui::vec_map` ==

pub struct VecMap;

const VEC_MAP_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("vec_map");

impl VecMap {
    /// `sui::vec_map::empty`
    pub const EMPTY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_MAP_MODULE,
        name: sui::types::Identifier::from_static("empty"),
    };
    /// `sui::vec_map::insert`
    pub const INSERT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_MAP_MODULE,
        name: sui::types::Identifier::from_static("insert"),
    };
    /// `sui::vec_map::VecMap`
    pub const VEC_MAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: VEC_MAP_MODULE,
        name: sui::types::Identifier::from_static("VecMap"),
    };
}

// == `sui::transfer` ==

pub struct Transfer;

const TRANSFER_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("transfer");

impl Transfer {
    /// `sui::transfer::public_share_object`
    pub const PUBLIC_SHARE_OBJECT: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TRANSFER_MODULE,
        name: sui::types::Identifier::from_static("public_share_object"),
    };
    /// `sui::transfer::public_transfer`
    pub const PUBLIC_TRANSFER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: TRANSFER_MODULE,
        name: sui::types::Identifier::from_static("public_transfer"),
    };
}

// == `sui::coin` ==

pub struct Coin;

const COIN_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("coin");

impl Coin {
    /// `sui::coin::Coin`
    pub const COIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: COIN_MODULE,
        name: sui::types::Identifier::from_static("Coin"),
    };
    /// `sui::coin::from_balance`
    pub const FROM_BALANCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: COIN_MODULE,
        name: sui::types::Identifier::from_static("from_balance"),
    };
    /// `sui::coin::into_balance`
    pub const INTO_BALANCE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: COIN_MODULE,
        name: sui::types::Identifier::from_static("into_balance"),
    };
    /// `sui::coin::join`
    pub const JOIN: ModuleAndNameIdent = ModuleAndNameIdent {
        module: COIN_MODULE,
        name: sui::types::Identifier::from_static("join"),
    };
}

// == `sui::sui` ==

pub struct Sui;

const SUI_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("sui");

impl Sui {
    /// `sui::sui::SUI`
    pub const SUI: ModuleAndNameIdent = ModuleAndNameIdent {
        module: SUI_MODULE,
        name: sui::types::Identifier::from_static("SUI"),
    };
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
