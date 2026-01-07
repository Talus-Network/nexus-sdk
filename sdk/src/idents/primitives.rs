use crate::{
    idents::{move_std, pure_arg, ModuleAndNameIdent},
    sui,
};

// == `nexus_primitives::data` ==

pub struct Data;

const DATA_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("data");

impl Data {
    /// Create NexusData from a vector of vectors of bytes.
    ///
    /// `nexus_primitives::data::inline_many`
    pub const INLINE_MANY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("inline_many"),
    };
    /// Create NexusData from a vector of vectors of bytes and mark it as
    /// encrypted.
    ///
    /// `nexus_primitives::data::inline_many_encrypted`
    pub const INLINE_MANY_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("inline_many_encrypted"),
    };
    /// Create NexusData from a vector of bytes.
    ///
    /// `nexus_primitives::data::inline_one`
    pub const INLINE_ONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("inline_one"),
    };
    /// Create NexusData from a vector of bytes and mark it as encrypted.
    ///
    /// `nexus_primitives::data::inline_one_encrypted`
    pub const INLINE_ONE_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("inline_one_encrypted"),
    };
    /// NexusData struct. Mostly used for creating generic types.
    ///
    /// `nexus_primitives::data::NexusData`
    pub const NEXUS_DATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("NexusData"),
    };
    /// Create NexusData from a vector of vectors of bytes that are stored on
    /// Walrus.
    ///
    /// `nexus_primitives::data::walrus_many`
    pub const WALRUS_MANY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("walrus_many"),
    };
    /// Create NexusData from a vector of vectors of bytes that are stored on
    /// Walrus and mark it as encrypted.
    ///
    /// `nexus_primitives::data::walrus_many_encrypted`
    pub const WALRUS_MANY_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("walrus_many_encrypted"),
    };
    /// Create NexusData from a vector of bytes that are stored on Walrus.
    ///
    /// `nexus_primitives::data::walrus_one`
    pub const WALRUS_ONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("walrus_one"),
    };
    /// Create NexusData from a vector of bytes that are stored on Walrus and
    /// mark it as encrypted.
    ///
    /// `nexus_primitives::data::walrus_one_encrypted`
    pub const WALRUS_ONE_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::types::Identifier::from_static("walrus_one_encrypted"),
    };

    /// Create NexusData with inline storage from a [serde_json::Value].
    pub fn nexus_data_inline_from_json<T: serde::Serialize>(
        tx: &mut sui::tx::TransactionBuilder,
        primitives_pkg_id: sui::types::Address,
        json: &T,
        encrypted: bool,
    ) -> anyhow::Result<sui::types::Argument> {
        let (one, many) = match encrypted {
            true => (&Self::INLINE_ONE_ENCRYPTED, &Self::INLINE_MANY_ENCRYPTED),
            false => (&Self::INLINE_ONE, &Self::INLINE_MANY),
        };

        Self::nexus_data_from_json(tx, primitives_pkg_id, json, one, many)
    }

    /// Create NexusData with Walrus storage from a [serde_json::Value].
    pub fn nexus_data_walrus_from_json<T: serde::Serialize>(
        tx: &mut sui::tx::TransactionBuilder,
        primitives_pkg_id: sui::types::Address,
        json: &T,
        encrypted: bool,
    ) -> anyhow::Result<sui::types::Argument> {
        let (one, many) = match encrypted {
            true => (&Self::WALRUS_ONE_ENCRYPTED, &Self::WALRUS_MANY_ENCRYPTED),
            false => (&Self::WALRUS_ONE, &Self::WALRUS_MANY),
        };

        Self::nexus_data_from_json(tx, primitives_pkg_id, json, one, many)
    }

    /// Internal helper to create NexusData from a [serde_json::Value].
    fn nexus_data_from_json<T: serde::Serialize>(
        tx: &mut sui::tx::TransactionBuilder,
        primitives_pkg_id: sui::types::Address,
        json: &T,
        one: &ModuleAndNameIdent,
        many: &ModuleAndNameIdent,
    ) -> anyhow::Result<sui::types::Argument> {
        if let serde_json::Value::Array(arr) = serde_json::to_value(json)? {
            let type_params = vec![sui::types::TypeTag::Vector(Box::new(
                sui::types::TypeTag::U8,
            ))];

            let vec = tx.move_call(
                sui::tx::Function::new(
                    move_std::PACKAGE_ID,
                    move_std::Vector::EMPTY.module,
                    move_std::Vector::EMPTY.name,
                    type_params.clone(),
                ),
                vec![],
            );

            for data in arr {
                // `bytes: vector<u8>`
                let data_bytes = tx.input(pure_arg(&serde_json::to_vec(&data)?)?);

                // `vector<vector<u8>>::push_back`
                tx.move_call(
                    sui::tx::Function::new(
                        move_std::PACKAGE_ID,
                        move_std::Vector::PUSH_BACK.module,
                        move_std::Vector::PUSH_BACK.name,
                        type_params.clone(),
                    ),
                    vec![vec, data_bytes],
                );
            }

            return Ok(tx.move_call(
                sui::tx::Function::new(
                    primitives_pkg_id,
                    many.module.clone(),
                    many.name.clone(),
                    vec![],
                ),
                vec![vec],
            ));
        }

        let json = tx.input(pure_arg(&serde_json::to_vec(json)?)?);

        Ok(tx.move_call(
            sui::tx::Function::new(
                primitives_pkg_id,
                one.module.clone(),
                one.name.clone(),
                vec![],
            ),
            vec![json],
        ))
    }
}

// == `nexus_primitives::event` ==

pub struct Event;

const EVENT_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("event");

impl Event {
    /// All events fired by the on-chain part of Nexus are wrapped in the
    /// generic argument of this struct.
    ///
    /// `nexus_primitives::event::EventWrapper`
    pub const EVENT_WRAPPER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EVENT_MODULE,
        name: sui::types::Identifier::from_static("EventWrapper"),
    };
}

// == `nexus_primitives::owner_cap` ==

pub struct OwnerCap;

const OWNER_CAP_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("owner_cap");

impl OwnerCap {
    /// This is used to fetch owner caps for the configured addresses. Each
    /// owner cap can authorize transactions that notify the chain about DAG
    /// execution results. N owner caps allow for N parallel requests.
    ///
    /// `nexus_primitives::owner_cap::CloneableOwnerCap`
    pub const CLONEABLE_OWNER_CAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OWNER_CAP_MODULE,
        name: sui::types::Identifier::from_static("CloneableOwnerCap"),
    };
}

// == `nexus_primitives::policy` ==

pub struct Policy;

const POLICY_MODULE: sui::types::Identifier = sui::types::Identifier::from_static("policy");

impl Policy {
    /// `nexus_primitives::policy::Symbol`
    pub const SYMBOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: POLICY_MODULE,
        name: sui::types::Identifier::from_static("Symbol"),
    };
    /// `nexus_primitives::policy::witness_symbol`
    pub const WITNESS_SYMBOL: ModuleAndNameIdent = ModuleAndNameIdent {
        module: POLICY_MODULE,
        name: sui::types::Identifier::from_static("witness_symbol"),
    };
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::types::TypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    workflow_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        workflow_pkg_id,
        ident.module,
        ident.name,
        vec![],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_type_tag() {
        let rng = &mut rand::thread_rng();
        let primitives_pkg_id = sui::types::Address::generate(rng);
        let ident = ModuleAndNameIdent {
            module: sui::types::Identifier::from_static("foo"),
            name: sui::types::Identifier::from_static("bar"),
        };

        let tag = into_type_tag(primitives_pkg_id, ident);
        assert_eq!(
            tag,
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                primitives_pkg_id,
                sui::types::Identifier::from_static("foo").into(),
                sui::types::Identifier::from_static("bar").into(),
                vec![],
            )))
        );
    }
}
