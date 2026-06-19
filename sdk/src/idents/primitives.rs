//! Identifiers for the `nexus_primitives` Move package.
//!
//! The per-module unit structs (`Authorization`, `Data`, `Event`, …) and their
//! `ModuleAndNameIdent` constants are generated at build time from
//! `generated/ir/primitives.json`. This module keeps the hand-written
//! `NexusData` construction helpers and the `TypeTag` helper on top of them.

use crate::{
    idents::{move_std, ModuleAndNameIdent},
    sui,
};

include!(concat!(env!("OUT_DIR"), "/idents_primitives.rs"));

impl Data {
    /// Create NexusData with inline storage from a [serde_json::Value].
    pub fn nexus_data_inline_from_json<T: serde::Serialize>(
        tx: &mut sui::tx::TransactionBuilder,
        primitives_pkg_id: sui::types::Address,
        json: &T,
    ) -> anyhow::Result<sui::tx::Argument> {
        Self::nexus_data_from_json(
            tx,
            primitives_pkg_id,
            json,
            &Self::INLINE_ONE,
            &Self::INLINE_MANY,
        )
    }

    /// Create NexusData with Walrus storage from a [serde_json::Value].
    pub fn nexus_data_walrus_from_json<T: serde::Serialize>(
        tx: &mut sui::tx::TransactionBuilder,
        primitives_pkg_id: sui::types::Address,
        json: &T,
    ) -> anyhow::Result<sui::tx::Argument> {
        Self::nexus_data_from_json(
            tx,
            primitives_pkg_id,
            json,
            &Self::WALRUS_ONE,
            &Self::WALRUS_MANY,
        )
    }

    /// Internal helper to create NexusData from a [serde_json::Value].
    fn nexus_data_from_json<T: serde::Serialize>(
        tx: &mut sui::tx::TransactionBuilder,
        primitives_pkg_id: sui::types::Address,
        json: &T,
        one: &ModuleAndNameIdent,
        many: &ModuleAndNameIdent,
    ) -> anyhow::Result<sui::tx::Argument> {
        if let serde_json::Value::Array(arr) = serde_json::to_value(json)? {
            let type_params = vec![sui::types::TypeTag::Vector(Box::new(
                sui::types::TypeTag::U8,
            ))];

            let vec = tx.move_call(
                sui::tx::Function::new(
                    move_std::PACKAGE_ID,
                    move_std::Vector::EMPTY.module,
                    move_std::Vector::EMPTY.name,
                )
                .with_type_args(type_params.clone()),
                vec![],
            );

            for data in arr {
                // `bytes: vector<u8>`
                let data_bytes = tx.pure(&serde_json::to_vec(&data)?);

                // `vector<vector<u8>>::push_back`
                tx.move_call(
                    sui::tx::Function::new(
                        move_std::PACKAGE_ID,
                        move_std::Vector::PUSH_BACK.module,
                        move_std::Vector::PUSH_BACK.name,
                    )
                    .with_type_args(type_params.clone()),
                    vec![vec, data_bytes],
                );
            }

            return Ok(tx.move_call(
                sui::tx::Function::new(primitives_pkg_id, many.module.clone(), many.name.clone()),
                vec![vec],
            ));
        }

        let json = tx.pure(&serde_json::to_vec(json)?);

        Ok(tx.move_call(
            sui::tx::Function::new(primitives_pkg_id, one.module.clone(), one.name.clone()),
            vec![json],
        ))
    }
}

/// Helper to turn a `ModuleAndNameIdent` into a `sui::types::TypeTag`. Useful for
/// creating generic types.
pub fn into_type_tag(
    primitives_pkg_id: sui::types::Address,
    ident: ModuleAndNameIdent,
) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        primitives_pkg_id,
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
                sui::types::Identifier::from_static("foo"),
                sui::types::Identifier::from_static("bar"),
                vec![],
            )))
        );
    }

    #[test]
    fn authorization_idents_use_authorization_module() {
        assert_eq!(
            Authorization::PROVEN_VALUE.module,
            sui::types::Identifier::from_static("authorization")
        );
        assert_eq!(
            Authorization::PROVEN_VALUE.name,
            sui::types::Identifier::from_static("ProvenValue")
        );
    }
}
