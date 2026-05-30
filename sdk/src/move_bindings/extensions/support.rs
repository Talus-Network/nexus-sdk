//! Shared SDK helpers for generated Move binding types.
//!
//! This module holds small projections that are not specific to one Move package, such as reading
//! a [`ToolFqn`] from a generated vertex kind or listing declared input port names. General Move
//! and Sui behavior belongs in the Move binding code generator; code here must stay tied to Nexus
//! domain meaning.

use crate::ToolFqn;

impl crate::move_bindings::interface::graph::VertexKind {
    pub fn tool_fqn(&self) -> anyhow::Result<ToolFqn> {
        let value = match self {
            Self::OnChain { tool_fqn, .. } | Self::OffChain { tool_fqn, .. } => tool_fqn.as_str(),
        };
        value
            .parse::<ToolFqn>()
            .map_err(|error| anyhow::anyhow!("DAG BCS tool FQN '{value}' did not parse: {error}"))
    }
}

impl crate::move_bindings::interface::graph::VertexInfo {
    pub fn declared_input_port_names(&self) -> Vec<String> {
        let mut ports = self
            .input_ports
            .contents
            .iter()
            .map(|port| port.name.as_str().to_owned())
            .collect::<Vec<_>>();
        ports.sort();
        ports
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            move_bindings::{
                move_std::{
                    ascii::String as MoveString,
                    option::Option as MoveOption,
                    type_name::TypeName,
                },
                sui_framework::{
                    bag::Bag,
                    object::{ID, UID},
                    object_bag::ObjectBag,
                    priority_queue::PriorityQueue,
                    table::Table as MoveTable,
                    vec_set::VecSet,
                },
            },
            sui,
        },
        serde::{Deserialize, Serialize},
        std::marker::PhantomData,
        sui_move::{MoveStruct, MoveType},
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn id(value: &'static str) -> ID {
        ID {
            bytes: address(value),
        }
    }

    fn uid_for_test(value: &'static str) -> crate::move_bindings::sui_framework::object::UID {
        crate::move_bindings::sui_framework::object::UID {
            id: crate::move_bindings::sui_framework::object::ID {
                bytes: address(value),
            },
        }
    }

    #[test]
    fn move_string_helpers_cover_string_and_bcs() {
        let value = MoveString::from("nexus");

        assert_eq!(value.as_str(), "nexus");
        assert_eq!(value.as_ref(), "nexus");
        assert_eq!(String::from(value.clone()), "nexus");
        assert_eq!(MoveString::from(String::from("sdk")).as_str(), "sdk");
        assert_eq!(
            format!("{value:?}"),
            "String { bytes: [110, 101, 120, 117, 115] }"
        );

        let bytes = bcs::to_bytes(&value).unwrap();
        assert_eq!(bcs::from_bytes::<MoveString>(&bytes).unwrap(), value);
    }

    #[test]
    fn type_name_bcs_roundtrip() {
        let value = TypeName::new("0xa5::scheduler::QueueGeneratorWitness");
        assert_eq!(
            bcs::from_bytes::<TypeName>(&bcs::to_bytes(&value).unwrap()).unwrap(),
            value
        );
    }

    #[test]
    fn id_and_uid_helpers_return_addresses_and_display_id() {
        let id = id("0x123");
        let uid = UID { id };

        assert_eq!(format!("{id}"), id.bytes.to_string());
        assert_eq!(sui::types::Address::from(id), id.bytes);
        assert_eq!(sui::types::Address::from(uid), id.bytes);
    }

    #[test]
    fn id_bcs_roundtrip() {
        let value = id("0xda6");
        let bytes = bcs::to_bytes(&value).unwrap();
        assert_eq!(bcs::from_bytes::<ID>(&bytes).unwrap(), value);
    }

    #[test]
    fn framework_types_consume_bcs_layout_bytes() {
        use crate::move_bindings::sui_framework::{
            balance,
            object_table,
            sui as sui_module,
            table_vec,
            vec_map,
        };

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct BalanceThenSentinel {
            balance: balance::Balance<sui_module::SUI>,
            sentinel: u8,
        }

        let balance = BalanceThenSentinel {
            balance: balance::Balance {
                value: 42,
                phantom_t0: PhantomData,
            },
            sentinel: 7,
        };
        let balance_bytes = bcs::to_bytes(&balance).unwrap();
        assert_eq!(
            bcs::from_bytes::<BalanceThenSentinel>(&balance_bytes).unwrap(),
            balance
        );

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct VecMapThenSentinel {
            map: vec_map::VecMap<u8, u64>,
            sentinel: u8,
        }

        let map = VecMapThenSentinel {
            map: vec_map::VecMap {
                contents: vec![vec_map::Entry { key: 1, value: 9 }],
            },
            sentinel: 8,
        };
        let map_bytes = bcs::to_bytes(&map).unwrap();
        assert_eq!(
            bcs::from_bytes::<VecMapThenSentinel>(&map_bytes).unwrap(),
            map
        );

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct ObjectTableThenSentinel {
            table: object_table::ObjectTable<ID, UID>,
            sentinel: u8,
        }

        let object_table = ObjectTableThenSentinel {
            table: object_table::ObjectTable {
                id: uid_for_test("0x456"),
                size: 3,
                phantom_t0: PhantomData,
                phantom_t1: PhantomData,
            },
            sentinel: 9,
        };
        let object_table_bytes = bcs::to_bytes(&object_table).unwrap();
        assert_eq!(
            bcs::from_bytes::<ObjectTableThenSentinel>(&object_table_bytes).unwrap(),
            object_table
        );

        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct TableVecThenSentinel {
            table_vec: table_vec::TableVec<u64>,
            sentinel: u8,
        }

        let table_vec = TableVecThenSentinel {
            table_vec: table_vec::TableVec {
                contents: MoveTable::new(address("0x789"), 4),
                phantom_t0: PhantomData,
            },
            sentinel: 10,
        };
        let table_vec_bytes = bcs::to_bytes(&table_vec).unwrap();
        assert_eq!(
            bcs::from_bytes::<TableVecThenSentinel>(&table_vec_bytes).unwrap(),
            table_vec
        );
    }

    #[test]
    fn support_move_tags_match_sui_framework_types() {
        fn assert_struct_type<T>()
        where
            T: MoveStruct + MoveType,
        {
            assert!(matches!(
                T::type_tag_static(),
                sui::types::TypeTag::Struct(_)
            ));
        }

        assert_struct_type::<ID>();
        assert_struct_type::<UID>();
        assert_struct_type::<Bag>();
        assert_struct_type::<ObjectBag>();
        assert_struct_type::<PriorityQueue<ID>>();
        assert_struct_type::<MoveString>();
        assert_struct_type::<MoveOption<ID>>();
        assert_struct_type::<VecSet<ID>>();
        assert_struct_type::<MoveTable<ID, UID>>();
        assert_struct_type::<TypeName>();

        assert_eq!(ID::struct_tag_static().module().as_str(), "object");
        assert_eq!(UID::struct_tag_static().name().as_str(), "UID");
        assert_eq!(Bag::struct_tag_static().module().as_str(), "bag");
        assert_eq!(
            ObjectBag::struct_tag_static().module().as_str(),
            "object_bag"
        );
        assert_eq!(
            PriorityQueue::<ID>::struct_tag_static().type_params(),
            &[ID::type_tag_static()]
        );
        assert_eq!(MoveString::struct_tag_static().module().as_str(), "ascii");
        assert_eq!(
            MoveOption::<ID>::struct_tag_static().type_params(),
            &[ID::type_tag_static()]
        );
        assert_eq!(
            VecSet::<ID>::struct_tag_static().type_params(),
            &[ID::type_tag_static()]
        );
        assert_eq!(
            MoveTable::<ID, UID>::struct_tag_static().type_params(),
            &[ID::type_tag_static(), UID::type_tag_static()]
        );
        assert_eq!(TypeName::struct_tag_static().module().as_str(), "type_name");
    }

    #[test]
    fn support_structs_round_trip_through_bcs() {
        let bag = Bag {
            id: uid_for_test("0x456"),
            size: 9,
        };
        let object_bag = ObjectBag {
            id: uid_for_test("0x789"),
            size: 11,
        };

        assert_eq!(
            bcs::from_bytes::<Bag>(&bcs::to_bytes(&bag).unwrap()).unwrap(),
            bag
        );
        assert_eq!(
            bcs::from_bytes::<ObjectBag>(&bcs::to_bytes(&object_bag).unwrap()).unwrap(),
            object_bag
        );
    }
}
