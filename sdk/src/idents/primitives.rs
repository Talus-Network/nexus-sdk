use crate::{
    idents::{move_std, ModuleAndNameIdent},
    sui,
};

// == `nexus_primitives::data` ==

pub struct Data;

const DATA_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("data");

impl Data {
    /// Create NexusData from a vector of vectors of bytes.
    ///
    /// `nexus_primitives::data::inline_many`
    pub const INLINE_MANY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("inline_many"),
    };
    /// Create NexusData from a vector of vectors of bytes and mark it as
    /// encrypted.
    ///
    /// `nexus_primitives::data::inline_many_encrypted`
    pub const INLINE_MANY_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("inline_many_encrypted"),
    };
    /// Create NexusData from a vector of bytes.
    ///
    /// `nexus_primitives::data::inline_one`
    pub const INLINE_ONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("inline_one"),
    };
    /// Create NexusData from a vector of bytes and mark it as encrypted.
    ///
    /// `nexus_primitives::data::inline_one_encrypted`
    pub const INLINE_ONE_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("inline_one_encrypted"),
    };
    /// NexusData struct. Mostly used for creating generic types.
    ///
    /// `nexus_primitives::data::NexusData`
    pub const NEXUS_DATA: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("NexusData"),
    };
    /// Create NexusData from a vector of vectors of bytes that are stored on
    /// Walrus.
    ///
    /// `nexus_primitives::data::walrus_many`
    pub const WALRUS_MANY: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("walrus_many"),
    };
    /// Create NexusData from a vector of vectors of bytes that are stored on
    /// Walrus and mark it as encrypted.
    ///
    /// `nexus_primitives::data::walrus_many_encrypted`
    pub const WALRUS_MANY_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("walrus_many_encrypted"),
    };
    /// Create NexusData from a vector of bytes that are stored on Walrus.
    ///
    /// `nexus_primitives::data::walrus_one`
    pub const WALRUS_ONE: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("walrus_one"),
    };
    /// Create NexusData from a vector of bytes that are stored on Walrus and
    /// mark it as encrypted.
    ///
    /// `nexus_primitives::data::walrus_one_encrypted`
    pub const WALRUS_ONE_ENCRYPTED: ModuleAndNameIdent = ModuleAndNameIdent {
        module: DATA_MODULE,
        name: sui::move_ident_str!("walrus_one_encrypted"),
    };

    /// Create NexusData with inline storage from a [serde_json::Value].
    pub fn nexus_data_inline_from_json<T: serde::Serialize>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        primitives_pkg_id: sui::ObjectID,
        json: &T,
    ) -> anyhow::Result<sui::Argument> {
        Self::nexus_data_from_json(
            tx,
            primitives_pkg_id,
            json,
            &Self::INLINE_ONE,
            &Self::INLINE_MANY,
        )
    }

    /// Create NexusData with inline encrypted storage from a [serde_json::Value].
    pub fn nexus_data_inline_encrypted_from_json<T: serde::Serialize>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        primitives_pkg_id: sui::ObjectID,
        json: &T,
    ) -> anyhow::Result<sui::Argument> {
        Self::nexus_data_from_json(
            tx,
            primitives_pkg_id,
            json,
            &Self::INLINE_ONE_ENCRYPTED,
            &Self::INLINE_MANY_ENCRYPTED,
        )
    }

    /// Create NexusData with Walrus storage from a [serde_json::Value].
    pub fn nexus_data_walrus_from_json<T: serde::Serialize>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        primitives_pkg_id: sui::ObjectID,
        json: &T,
    ) -> anyhow::Result<sui::Argument> {
        Self::nexus_data_from_json(
            tx,
            primitives_pkg_id,
            json,
            &Self::WALRUS_ONE,
            &Self::WALRUS_MANY,
        )
    }

    /// Create NexusData with Walrus encrypted storage from a [serde_json::Value].
    pub fn nexus_data_walrus_encrypted_from_json<T: serde::Serialize>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        primitives_pkg_id: sui::ObjectID,
        json: &T,
    ) -> anyhow::Result<sui::Argument> {
        Self::nexus_data_from_json(
            tx,
            primitives_pkg_id,
            json,
            &Self::WALRUS_ONE_ENCRYPTED,
            &Self::WALRUS_MANY_ENCRYPTED,
        )
    }

    /// Internal helper to create NexusData from a [serde_json::Value].
    fn nexus_data_from_json<T: serde::Serialize>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        primitives_pkg_id: sui::ObjectID,
        json: &T,
        one: &ModuleAndNameIdent,
        many: &ModuleAndNameIdent,
    ) -> anyhow::Result<sui::Argument> {
        if let serde_json::Value::Array(arr) = serde_json::to_value(json)? {
            let type_params = vec![sui::MoveTypeTag::Vector(Box::new(sui::MoveTypeTag::U8))];

            let vec = tx.programmable_move_call(
                sui::MOVE_STDLIB_PACKAGE_ID,
                move_std::Vector::EMPTY.module.into(),
                move_std::Vector::EMPTY.name.into(),
                type_params.clone(),
                vec![],
            );

            for data in arr {
                let data_bytes = tx.pure(serde_json::to_string(&data)?.into_bytes())?;
                tx.programmable_move_call(
                    sui::MOVE_STDLIB_PACKAGE_ID,
                    move_std::Vector::PUSH_BACK.module.into(),
                    move_std::Vector::PUSH_BACK.name.into(),
                    type_params.clone(),
                    vec![vec, data_bytes],
                );
            }

            return Ok(tx.programmable_move_call(
                primitives_pkg_id,
                many.module.into(),
                many.name.into(),
                vec![],
                vec![vec],
            ));
        }

        let json = tx.pure(serde_json::to_string(json)?.into_bytes())?;

        Ok(tx.programmable_move_call(
            primitives_pkg_id,
            one.module.into(),
            one.name.into(),
            vec![],
            vec![json],
        ))
    }
}

// == `nexus_primitives::event` ==

pub struct Event;

const EVENT_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("event");

impl Event {
    /// All events fired by the on-chain part of Nexus are wrapped in the
    /// generic argument of this struct.
    ///
    /// `nexus_primitives::event::EventWrapper`
    pub const EVENT_WRAPPER: ModuleAndNameIdent = ModuleAndNameIdent {
        module: EVENT_MODULE,
        name: sui::move_ident_str!("EventWrapper"),
    };
}

// == `nexus_primitives::owner_cap` ==

pub struct OwnerCap;

const OWNER_CAP_MODULE: &sui::MoveIdentStr = sui::move_ident_str!("owner_cap");

impl OwnerCap {
    /// This is used to fetch owner caps for the configured addresses. Each
    /// owner cap can authorize transactions that notify the chain about DAG
    /// execution results. N owner caps allow for N parallel requests.
    ///
    /// `nexus_primitives::owner_cap::CloneableOwnerCap`
    pub const CLONEABLE_OWNER_CAP: ModuleAndNameIdent = ModuleAndNameIdent {
        module: OWNER_CAP_MODULE,
        name: sui::move_ident_str!("CloneableOwnerCap"),
    };
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::MoveTypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    workflow_pkg_id: sui::ObjectID,
    ident: ModuleAndNameIdent,
) -> sui::MoveTypeTag {
    sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *workflow_pkg_id,
        module: ident.module.into(),
        name: ident.name.into(),
        type_params: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_into_type_tag() {
        let primitives_pkg_id = sui::ObjectID::random();
        let ident = ModuleAndNameIdent {
            module: sui::move_ident_str!("foo"),
            name: sui::move_ident_str!("bar"),
        };

        let tag = into_type_tag(primitives_pkg_id, ident);
        assert_eq!(
            tag,
            sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                address: *primitives_pkg_id,
                module: sui::move_ident_str!("foo").into(),
                name: sui::move_ident_str!("bar").into(),
                type_params: vec![],
            }))
        );
    }
}
