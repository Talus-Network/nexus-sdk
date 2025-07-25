use crate::{idents::ModuleAndNameIdent, sui};

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

    /// Create NexusData from a [serde_json::Value].
    pub fn nexus_data_from_json<T: serde::Serialize>(
        tx: &mut sui::ProgrammableTransactionBuilder,
        primitives_pkg_id: sui::ObjectID,
        json: &T,
        encrypted: bool,
    ) -> anyhow::Result<sui::Argument> {
        let json = tx.pure(serde_json::to_string(json)?.into_bytes())?;

        if encrypted {
            return Ok(tx.programmable_move_call(
                primitives_pkg_id,
                Self::INLINE_ONE_ENCRYPTED.module.into(),
                Self::INLINE_ONE_ENCRYPTED.name.into(),
                vec![],
                vec![json],
            ));
        }

        Ok(tx.programmable_move_call(
            primitives_pkg_id,
            Self::INLINE_ONE.module.into(),
            Self::INLINE_ONE.name.into(),
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
