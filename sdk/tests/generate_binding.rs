use {
    std::path::Path,
    sui_move_call::SharedMoveObject,
    sui_move_codegen::{
        fetch_package,
        render::{render_package, RenderOptions},
        Address,
        Client,
    },
    sui_move_ptb::ptb,
};

#[tokio::test]
async fn test_generate_move_bindings() {
    let mut client = Client::new("https://grpc.ssfn.devnet.production.taluslabs.dev").unwrap();
    let package_id =
        Address::from_static("0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06");

    let package = fetch_package(&mut client, package_id).await.unwrap();

    let render = render_package(&package, &RenderOptions::default());

    tokio::fs::write(
        Path::new("/home/kouks/Code/talus/nexus-sdk/sdk/tests/workflow.rs"),
        render,
    )
    .await
    .unwrap();
}

pub mod primitives {
    /// Package address (the on-chain package object id).
    pub const PACKAGE: sui_move::prelude::Address = sui_move::prelude::Address::from_static(
        "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
    );
    pub mod automaton {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "automaton",
            abilities = "key, store",
            phantoms = "T0, T1",
            type_abilities = "T0: store, copy, drop; T1: store, copy, drop"
        )]
        pub struct ConfiguredAutomaton<T0, T1> {
            pub id: sm::types::UID,
            pub dfa: DeterministicAutomaton<T0, T1>,
        }
        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "automaton",
            abilities = "store",
            phantoms = "T0, T1",
            type_abilities = "T0: store, copy, drop; T1: store, copy, drop"
        )]
        pub struct DeterministicAutomaton<T0, T1> {
            pub states: sm::containers::TableVec<T0>,
            pub alphabet: sm::containers::TableVec<T1>,
            pub transition: sm::containers::TableVec<sm::containers::TableVec<u64>>,
            pub accepting: sm::containers::TableVec<bool>,
            pub start: u64,
        }
        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::TransitionConfigKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "automaton",
            abilities = "store, copy, drop",
            type_abilities = "T0: store, copy, drop; T1: store, copy, drop"
        )]
        pub struct TransitionConfigKey<T0, T1> {
            pub transition: TransitionKey<T0, T1>,
            pub config: sm::type_name::TypeName,
        }
        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::TransitionKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "automaton",
            abilities = "store, copy, drop",
            type_abilities = "T0: store, copy, drop; T1: store, copy, drop"
        )]
        pub struct TransitionKey<T0, T1> {
            pub state: sm::containers::MoveOption<T0>,
            pub symbol: T1,
        }
        /// Move: `public fun automaton::accepts<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &0x2::table_vec::TableVec<T1>): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn accepts<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: sm::containers::TableVec<T1>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "accepts")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::add_state<T0: store + copy + drop, T1: store + copy + drop>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: T0, arg2: bool, arg3: &0x2::table_vec::TableVec<T0>, arg4: &mut 0x2::tx_context::TxContext): u64`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_state<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
            arg2: bool,
            arg3: sm::containers::TableVec<T0>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "add_state")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::add_symbol<T0: store + copy + drop, T1: store + copy + drop>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: T1, arg2: &0x2::table_vec::TableVec<T0>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_symbol<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T1,
            arg2: sm::containers::TableVec<T0>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "add_symbol")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::alphabet<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>): &0x2::table_vec::TableVec<T1>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn alphabet<T0, T1>(arg0: DeterministicAutomaton<T0, T1>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "alphabet")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::borrow_core<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>): &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_core<T0, T1>(
            arg0: &impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_core")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::borrow_core_mut<T0: store + copy + drop, T1: store + copy + drop>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>): &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_core_mut<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_core_mut")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::borrow_mut_symbol_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T1): &mut T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_mut_symbol_config<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_mut_symbol_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::borrow_mut_transition_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T0, arg2: T1): &mut T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_mut_transition_config<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T0,
            arg2: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_mut_transition_config")
                    .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::borrow_symbol_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T1): &T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_symbol_config<T0, T1, T2>(
            arg0: &impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_symbol_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::borrow_transition_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T0, arg2: T1): &T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_transition_config<T0, T1, T2>(
            arg0: &impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T0,
            arg2: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_transition_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::delta<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T0, arg2: &T1): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn delta<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
            arg2: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "delta")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::delta_indexed<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: u64, arg2: u64): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn delta_indexed<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: u64,
            arg2: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "delta_indexed")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::expect_state_index_of<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T0): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn expect_state_index_of<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "expect_state_index_of")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::expect_symbol_index_of<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T1): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn expect_symbol_index_of<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "expect_symbol_index_of")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::has_symbol_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T1): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn has_symbol_config<T0, T1, T2>(
            arg0: &impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "has_symbol_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::is_accepting<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T0): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_accepting<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "is_accepting")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::new<T0: store + copy + drop, T1: store + copy + drop>(arg0: 0x2::table_vec::TableVec<T0>, arg1: 0x2::table_vec::TableVec<T1>, arg2: T0, arg3: 0x2::table_vec::TableVec<T0>, arg4: 0x2::table_vec::TableVec<0x2::table_vec::TableVec<T0>>, arg5: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new<T0, T1>(
            arg0: sm::containers::TableVec<T0>,
            arg1: sm::containers::TableVec<T1>,
            arg2: T0,
            arg3: sm::containers::TableVec<T0>,
            arg4: sm::containers::TableVec<sm::containers::TableVec<T0>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "new")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::new_configured<T0: store + copy + drop, T1: store + copy + drop>(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_configured<T0, T1>(arg0: DeterministicAutomaton<T0, T1>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "new_configured")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::register_symbol_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T1, arg2: T2)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_symbol_config<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T1,
            arg2: T2,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "register_symbol_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::register_transition_config<T0: store + copy + drop, T1: store + copy + drop, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<T0, T1>, arg1: T0, arg2: T1, arg3: T2)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_transition_config<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<ConfiguredAutomaton<T0, T1>>,
            arg1: T0,
            arg2: T1,
            arg3: T2,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "automaton", "register_transition_config")
                    .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::run<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &0x2::table_vec::TableVec<T1>): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn run<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: sm::containers::TableVec<T1>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "run")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::run_from<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T0, arg2: &0x2::table_vec::TableVec<T1>): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn run_from<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
            arg2: sm::containers::TableVec<T1>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "run_from")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::set_accepting_state<T0: store + copy + drop, T1: store + copy + drop>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T0, arg2: bool)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_accepting_state<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
            arg2: bool,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "set_accepting_state")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::set_transition<T0: store + copy + drop, T1: store + copy + drop>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T0, arg2: &T1, arg3: T0)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_transition<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T0,
            arg2: T1,
            arg3: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "set_transition")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::set_transition_indexed<T0: store + copy + drop, T1: store + copy + drop>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: u64, arg2: u64, arg3: u64)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_transition_indexed<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: u64,
            arg2: u64,
            arg3: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "set_transition_indexed")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::start_index<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn start_index<T0, T1>(arg0: DeterministicAutomaton<T0, T1>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "start_index")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::start_state<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn start_state<T0, T1>(arg0: DeterministicAutomaton<T0, T1>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "start_state")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::state_at<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: u64): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn state_at<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "state_at")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::states<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>): &0x2::table_vec::TableVec<T0>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn states<T0, T1>(arg0: DeterministicAutomaton<T0, T1>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "states")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun automaton::symbol_index_of<T0: store + copy + drop, T1: store + copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<T0, T1>, arg1: &T1): 0x1::option::Option<u64>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn symbol_index_of<T0, T1>(
            arg0: DeterministicAutomaton<T0, T1>,
            arg1: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
            T1: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "symbol_index_of")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
    }

    pub mod data {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "data",
            abilities = "store, copy, drop"
        )]
        pub struct NexusData {
            pub storage: Vec<u8>,
            pub one: Vec<u8>,
            pub many: Vec<Vec<u8>>,
            pub encryption_mode: u8,
        }
        /// Move: `public fun data::collect_ones(arg0: vector<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn collect_ones(arg0: Vec<NexusData>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "collect_ones")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::encryption_mode(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData): u8`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn encryption_mode(arg0: NexusData) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "encryption_mode")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::encryption_mode_limited_persistent(): u8`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn encryption_mode_limited_persistent() -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "data", "encryption_mode_limited_persistent")
                    .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun data::encryption_mode_plain(): u8`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn encryption_mode_plain() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "encryption_mode_plain")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun data::encryption_mode_standard(): u8`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn encryption_mode_standard() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "encryption_mode_standard")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun data::inline_many(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inline_many(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "inline_many")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::inline_many_encrypted(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inline_many_encrypted(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "inline_many_encrypted")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::inline_many_limited_persistent(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inline_many_limited_persistent(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "data", "inline_many_limited_persistent")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::inline_one(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inline_one(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "inline_one")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::inline_one_encrypted(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inline_one_encrypted(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "inline_one_encrypted")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::inline_one_limited_persistent(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inline_one_limited_persistent(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "inline_one_limited_persistent")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::split_many(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData): vector<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn split_many(arg0: NexusData) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "split_many")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::walrus_many(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn walrus_many(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "walrus_many")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::walrus_many_encrypted(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn walrus_many_encrypted(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "walrus_many_encrypted")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::walrus_many_limited_persistent(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn walrus_many_limited_persistent(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "data", "walrus_many_limited_persistent")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::walrus_one(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn walrus_one(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "walrus_one")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::walrus_one_encrypted(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn walrus_one_encrypted(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "walrus_one_encrypted")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun data::walrus_one_limited_persistent(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn walrus_one_limited_persistent(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "walrus_one_limited_persistent")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
    }

    pub mod event {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::event::EventWrapper`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "event",
            abilities = "copy, drop"
        )]
        pub struct EventWrapper<T0> {
            pub event: T0,
        }
        /// Move: `public fun event::emit<T0: copy + drop>(arg0: T0)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn emit<T0>(arg0: T0) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "event", "emit").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun event::inner<T0: copy + drop>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::event::EventWrapper<T0>): &T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inner<T0>(arg0: EventWrapper<T0>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "event", "inner").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
    }

    pub mod owner_cap {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "owner_cap",
            abilities = "key, store",
            phantoms = "T0"
        )]
        pub struct CloneableOwnerCap<T0> {
            pub id: sm::types::UID,
            pub what_for: sm::types::ID,
            pub inner: OwnerCap<T0>,
        }
        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::OwnerCap`.
        /// Abilities: `store, drop`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "owner_cap",
            abilities = "store, drop",
            phantoms = "T0"
        )]
        pub struct OwnerCap<T0> {
            pub unique: sm::types::ID,
        }
        /// Move: `public fun owner_cap::as_ref<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>): &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::OwnerCap<T0>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn as_ref<T0>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "as_ref")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::clone<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg1: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn clone<T0>(arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "clone")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::clone_for_receiver<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg1: address, arg2: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn clone_for_receiver<T0>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
            arg1: sm::prelude::Address,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "clone_for_receiver")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::clone_n<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): vector<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn clone_n<T0>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
            arg1: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "clone_n")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::destroy<T0>(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn destroy<T0>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "destroy")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::id<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::OwnerCap<T0>): 0x2::object::ID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn id<T0>(arg0: OwnerCap<T0>) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "owner_cap", "id").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::is_for<T0, T1: key>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg1: &T1): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_for<T0, T1>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
            arg1: &impl sm_call::ObjectArg<T1>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
            T1: sm::MoveStruct + sm::HasKey,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "is_for")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::is_for_id<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg1: 0x2::object::ID): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_for_id<T0>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
            arg1: sm::types::ID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "is_for_id")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::new_cloneable_drop<T0: drop>(arg0: T0, arg1: &0x2::object::UID, arg2: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_cloneable_drop<T0>(arg0: T0, arg1: sm::types::UID) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "new_cloneable_drop")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::new_cloneable_key<T0: key>(arg0: &T0, arg1: &0x2::object::UID, arg2: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_cloneable_key<T0>(
            arg0: &impl sm_call::ObjectArg<T0>,
            arg1: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveStruct + sm::HasKey,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "new_cloneable_key")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::new_uncloneable<T0>(arg0: &T0, arg1: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::OwnerCap<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_uncloneable<T0>(arg0: T0) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "new_uncloneable")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun owner_cap::what_for<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>): 0x2::object::ID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn what_for<T0>(
            arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "owner_cap", "what_for")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
    }

    pub mod policy {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "policy",
            abilities = "key, store",
            type_abilities = "T0: store"
        )]
        pub struct Policy<T0> {
            pub id: sm::types::UID,
            pub dfa: super::automaton::ConfiguredAutomaton<u64, Symbol>,
            pub state_index: u64,
            pub data: T0,
        }
        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum Symbol {
            Witness { pos0: sm::type_name::TypeName },
            Uid { pos0: sm::types::ID },
        }

        impl sm::MoveType for Symbol {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for Symbol {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("policy").expect("invalid module"),
                    sm::parse_identifier("Symbol").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for Symbol {}

        impl sm::HasCopy for Symbol {}

        impl sm::HasDrop for Symbol {}
        /// Move: `public fun policy::add_state<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: u64, arg2: bool, arg3: &0x2::table_vec::TableVec<u64>, arg4: &mut 0x2::tx_context::TxContext): u64`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_state<T0>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: u64,
            arg2: bool,
            arg3: sm::containers::TableVec<u64>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "add_state")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::add_symbol<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol, arg2: &0x2::table_vec::TableVec<u64>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_symbol<T0>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: Symbol,
            arg2: sm::containers::TableVec<u64>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "add_symbol")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::add_uid_symbol<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: &0x2::object::UID, arg2: &0x2::table_vec::TableVec<u64>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_uid_symbol<T0>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: sm::types::UID,
            arg2: sm::containers::TableVec<u64>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "add_uid_symbol")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::add_witness_symbol<T0: drop, T1: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>, arg1: &0x2::table_vec::TableVec<u64>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_witness_symbol<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T1>>,
            arg1: sm::containers::TableVec<u64>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "add_witness_symbol")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::advance_with_uid<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: &0x2::object::UID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn advance_with_uid<T0>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "advance_with_uid")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::advance_with_witness<T0: drop, T1: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>, arg1: T0)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn advance_with_witness<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T1>>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "advance_with_witness")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_config<T0: drop, T1: store, T2: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>): &T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_config<T0, T1, T2>(
            arg0: &impl sm_call::ObjectArg<Policy<T1>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_config_for_uid<T0: store, T1: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: &0x2::object::UID): &T1`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_config_for_uid<T0, T1>(
            arg0: &impl sm_call::ObjectArg<Policy<T0>>,
            arg1: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_config_for_uid")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_mut_config<T0: drop, T1: store, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>): &mut T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_mut_config<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T1>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_mut_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_mut_config_for_uid<T0: store, T1: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: &0x2::object::UID): &mut T1`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_mut_config_for_uid<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_mut_config_for_uid")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_mut_state_config<T0: drop, T1: store, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>, arg1: u64): &mut T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_mut_state_config<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T1>>,
            arg1: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_mut_state_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_mut_state_config_for_uid<T0: store, T1: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: u64, arg2: &0x2::object::UID): &mut T1`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_mut_state_config_for_uid<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: u64,
            arg2: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "borrow_mut_state_config_for_uid")
                    .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_state_config<T0: drop, T1: store, T2: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>, arg1: u64): &T2`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_state_config<T0, T1, T2>(
            arg0: &impl sm_call::ObjectArg<Policy<T1>>,
            arg1: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_state_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::borrow_state_config_for_uid<T0: store, T1: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: u64, arg2: &0x2::object::UID): &T1`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_state_config_for_uid<T0, T1>(
            arg0: &impl sm_call::ObjectArg<Policy<T0>>,
            arg1: u64,
            arg2: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_state_config_for_uid")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::configured_dfa<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::ConfiguredAutomaton<u64, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn configured_dfa<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "configured_dfa")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::data<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): &T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn data<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "data").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::data_mut<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): &mut T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn data_mut<T0>(arg0: &mut impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "data_mut")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::dfa<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<u64, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn dfa<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "dfa").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::has_uid_config<T0: store, T1: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: &0x2::object::UID): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn has_uid_config<T0, T1>(
            arg0: &impl sm_call::ObjectArg<Policy<T0>>,
            arg1: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "has_uid_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::has_witness_config<T0: drop, T1: store, T2: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn has_witness_config<T0, T1, T2>(
            arg0: &impl sm_call::ObjectArg<Policy<T1>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "has_witness_config")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::has_witness_symbol<T0: drop, T1: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn has_witness_symbol<T0, T1>(
            arg0: &impl sm_call::ObjectArg<Policy<T1>>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "has_witness_symbol")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::id<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): &0x2::object::UID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn id<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "id").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::is_accepting<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_accepting<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "is_accepting")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::new<T0: store>(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::automaton::DeterministicAutomaton<u64, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol>, arg1: T0, arg2: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new<T0>(
            arg0: super::automaton::DeterministicAutomaton<u64, Symbol>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "new").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::new_linear<T0: store>(arg0: &0x2::table_vec::TableVec<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol>, arg1: T0, arg2: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_linear<T0>(arg0: sm::containers::TableVec<Symbol>, arg1: T0) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "new_linear")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::register<T0: drop, T1: store, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>, arg1: T2)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T1>>,
            arg1: T2,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "register")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::register_for_state<T0: drop, T1: store, T2: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T1>, arg1: u64, arg2: T2)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_for_state<T0, T1, T2>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T1>>,
            arg1: u64,
            arg2: T2,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
            T1: sm::MoveType + sm::HasStore,
            T2: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "register_for_state")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_type_arg::<T2>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::register_uid<T0: store, T1: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: &0x2::object::UID, arg2: T1)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_uid<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: sm::types::UID,
            arg2: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "register_uid")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::register_uid_for_state<T0: store, T1: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: u64, arg2: &0x2::object::UID, arg3: T1)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_uid_for_state<T0, T1>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: u64,
            arg2: sm::types::UID,
            arg3: T1,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
            T1: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "register_uid_for_state")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_type_arg::<T1>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::reset<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn reset<T0>(arg0: &mut impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "reset").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::set_accepting<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: u64, arg2: bool)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_accepting<T0>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: u64,
            arg2: bool,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "set_accepting")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::set_transition<T0: store>(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>, arg1: u64, arg2: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol, arg3: u64)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_transition<T0>(
            arg0: &mut impl sm_call::ObjectArg<Policy<T0>>,
            arg1: u64,
            arg2: Symbol,
            arg3: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "set_transition")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::state<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn state<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "policy", "state").expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::state_index<T0: store>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<T0>): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn state_index<T0>(arg0: &impl sm_call::ObjectArg<Policy<T0>>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasStore,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "state_index")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::uid_symbol_from_uid(arg0: &0x2::object::UID): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn uid_symbol_from_uid(arg0: sm::types::UID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "uid_symbol_from_uid")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun policy::witness_symbol<T0: drop>(): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn witness_symbol<T0>() -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "witness_symbol")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec
        }
        /// Move: `public fun policy::witness_symbol_from_name(arg0: 0x1::type_name::TypeName): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn witness_symbol_from_name(arg0: sm::type_name::TypeName) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "witness_symbol_from_name")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
    }

    pub mod proof_of_uid {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`.
        /// Abilities: *(none)*.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "proof_of_uid"
        )]
        pub struct ProofOfUID {
            pub from_uid: sm::types::ID,
            pub from_type: sm::containers::MoveOption<sm::type_name::TypeName>,
            pub stamps: sm::vec_map::VecMap<sm::types::ID, Vec<u8>>,
        }
        /// Move: `public fun proof_of_uid::consume(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg1: &0x2::object::UID): 0x2::vec_map::VecMap<0x2::object::ID, vector<u8>>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn consume(arg0: ProofOfUID, arg1: sm::types::UID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "consume")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::created_from(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID): 0x2::object::ID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn created_from(arg0: ProofOfUID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "created_from")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::has_stamp(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg1: 0x2::object::ID): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn has_stamp(arg0: ProofOfUID, arg1: sm::types::ID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "has_stamp")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::new(arg0: &0x2::object::UID): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new(arg0: sm::types::UID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "new")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::new_with_type<T0: key>(arg0: &0x2::object::UID, arg1: &T0): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_with_type<T0>(
            arg0: sm::types::UID,
            arg1: &impl sm_call::ObjectArg<T0>,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveStruct + sm::HasKey,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "new_with_type")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::stamp(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg1: &0x2::object::UID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn stamp(arg0: ProofOfUID, arg1: sm::types::UID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "stamp")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::stamp_with_data(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg1: &0x2::object::UID, arg2: vector<u8>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn stamp_with_data(
            arg0: ProofOfUID,
            arg1: sm::types::UID,
            arg2: Vec<u8>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "stamp_with_data")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::stamps(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID): &0x2::vec_map::VecMap<0x2::object::ID, vector<u8>>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn stamps(arg0: ProofOfUID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "stamps")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::stamps_len(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn stamps_len(arg0: ProofOfUID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "stamps_len")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::type_name(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID): 0x1::option::Option<0x1::type_name::TypeName>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn type_name(arg0: ProofOfUID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "type_name")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proof_of_uid::unstamp(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg1: &0x2::object::UID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn unstamp(arg0: ProofOfUID, arg1: sm::types::UID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "unstamp")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
    }

    pub mod proven_value {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue`.
        /// Abilities: *(none)*.
        #[sm::move_struct(
            address = "0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f",
            module = "proven_value"
        )]
        pub struct ProvenValue<T0> {
            pub value: T0,
            pub by: sm::types::ID,
            pub recipient: sm::containers::MoveOption<sm::types::ID>,
        }
        /// Move: `public fun proven_value::by<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>): 0x2::object::ID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn by<T0>(arg0: ProvenValue<T0>) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "by")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proven_value::drop<T0: drop>(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn drop<T0>(arg0: ProvenValue<T0>) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "drop")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proven_value::recipient<T0>(arg0: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>): 0x1::option::Option<0x2::object::ID>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn recipient<T0>(arg0: ProvenValue<T0>) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "recipient")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proven_value::unwrap<T0>(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn unwrap<T0>(arg0: ProvenValue<T0>) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "unwrap")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun proven_value::unwrap_as_recipient<T0>(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>, arg1: &0x2::object::UID): T0`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn unwrap_as_recipient<T0>(
            arg0: ProvenValue<T0>,
            arg1: sm::types::UID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "unwrap_as_recipient")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun proven_value::wrap<T0>(arg0: &0x2::object::UID, arg1: T0): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn wrap<T0>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "wrap")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun proven_value::wrap_for_recipient<T0>(arg0: &0x2::object::UID, arg1: T0, arg2: 0x2::object::ID): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proven_value::ProvenValue<T0>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn wrap_for_recipient<T0>(
            arg0: sm::types::UID,
            arg1: T0,
            arg2: sm::types::ID,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "proven_value", "wrap_for_recipient")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
    }

    pub use {
        automaton::{
            ConfiguredAutomaton,
            DeterministicAutomaton,
            TransitionConfigKey,
            TransitionKey,
        },
        data::NexusData,
        event::EventWrapper,
        owner_cap::{CloneableOwnerCap, OwnerCap},
        policy::{Policy, Symbol},
        proof_of_uid::ProofOfUID,
        proven_value::ProvenValue,
    };
}

pub mod workflow {
    use super::primitives;

    /// Package address (the on-chain package object id).
    pub const PACKAGE: sui_move::prelude::Address = sui_move::prelude::Address::from_static(
        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
    );
    pub mod dag {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "key, store"
        )]
        pub struct DAG {
            pub id: sm::types::UID,
            pub vertices: sm::linked_table::LinkedTable<Vertex, VertexInfo>,
            pub entry_groups: sm::vec_map::VecMap<
                EntryGroup,
                sm::vec_map::VecMap<Vertex, sm::vec_set::VecSet<InputPort>>,
            >,
            pub edges: sm::containers::Table<Vertex, Vec<Edge>>,
            pub outputs: sm::containers::Table<Vertex, Vec<OutputVariantPort>>,
            pub defaults_to_input_ports:
                sm::containers::Table<VertexInputPort, super::primitives::data::NexusData>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGCreatedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct DAGCreatedEvent {
            pub dag: sm::types::ID,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGDefaultValueAddedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct DAGDefaultValueAddedEvent {
            pub dag: sm::types::ID,
            pub vertex: Vertex,
            pub port: InputPort,
            pub value: super::primitives::data::NexusData,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGEdgeAddedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct DAGEdgeAddedEvent {
            pub dag: sm::types::ID,
            pub from_vertex: Vertex,
            pub edge: Edge,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGEntryVertexInputPortAddedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct DAGEntryVertexInputPortAddedEvent {
            pub dag: sm::types::ID,
            pub vertex: Vertex,
            pub entry_port: InputPort,
            pub entry_group: EntryGroup,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "key, store"
        )]
        pub struct DAGExecution {
            pub id: sm::types::UID,
            pub dag: sm::types::ID,
            pub entry_group: EntryGroup,
            pub invoker: sm::prelude::Address,
            pub created_at: u64,
            pub priority_fee_per_gas_unit: u64,
            pub worksheet_from_type: sm::type_name::TypeName,
            pub last_request_for_execution_emitted_at_digest: Vec<u8>,
            pub network: sm::types::ID,
            pub evaluations: sm::object_table::ObjectTable<Vertex, VertexEvaluations>,
            pub walks: Vec<DAGWalk>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGOutputAddedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct DAGOutputAddedEvent {
            pub dag: sm::types::ID,
            pub vertex: Vertex,
            pub output: OutputVariantPort,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGVertexAddedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct DAGVertexAddedEvent {
            pub dag: sm::types::ID,
            pub vertex: Vertex,
            pub kind: VertexKind,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGWalk`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store"
        )]
        pub struct DAGWalk {
            pub vertex_to_invoke: RuntimeVertex,
            pub status: DAGWalkStatus,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGWalkStatus`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum DAGWalkStatus {
            Active,
            Successful,
            Failed,
            Consumed,
        }

        impl sm::MoveType for DAGWalkStatus {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for DAGWalkStatus {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("dag").expect("invalid module"),
                    sm::parse_identifier("DAGWalkStatus").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for DAGWalkStatus {}

        impl sm::HasCopy for DAGWalkStatus {}

        impl sm::HasDrop for DAGWalkStatus {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DagExecutionConfig`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct DagExecutionConfig {
            pub dag: sm::types::ID,
            pub network: sm::types::ID,
            pub entry_group: EntryGroup,
            pub inputs: sm::vec_map::VecMap<
                Vertex,
                sm::vec_map::VecMap<InputPort, super::primitives::data::NexusData>,
            >,
            pub invoker: sm::prelude::Address,
            pub priority_fee_per_gas_unit: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Edge`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct Edge {
            pub from: OutputVariantPort,
            pub to: VertexInputPort,
            pub kind: EdgeKind,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum EdgeKind {
            Normal,
            ForEach,
            Collect,
            DoWhile,
            Break,
        }

        impl sm::MoveType for EdgeKind {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for EdgeKind {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("dag").expect("invalid module"),
                    sm::parse_identifier("EdgeKind").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for EdgeKind {}

        impl sm::HasCopy for EdgeKind {}

        impl sm::HasDrop for EdgeKind {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EndStateReachedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct EndStateReachedEvent {
            pub dag: sm::types::ID,
            pub execution: sm::types::ID,
            pub walk_index: u64,
            pub vertex: RuntimeVertex,
            pub variant: OutputVariant,
            pub variant_ports_to_data:
                sm::vec_map::VecMap<OutputPort, super::primitives::data::NexusData>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct EntryGroup {
            pub name: sm::ascii::String,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::ExecutionFinishedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct ExecutionFinishedEvent {
            pub dag: sm::types::ID,
            pub execution: sm::types::ID,
            pub has_any_walk_failed: bool,
            pub has_any_walk_succeeded: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct InputPort {
            pub name: sm::ascii::String,
            pub encrypted: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct OutputPort {
            pub name: sm::ascii::String,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct OutputVariant {
            pub name: sm::ascii::String,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariantPort`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct OutputVariantPort {
            pub variant: OutputVariant,
            pub port: OutputPort,
            pub encrypted: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::PortData`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum PortData {
            Single {
                _variant_name: sm::ascii::String,
                data: super::primitives::data::NexusData,
            },
            Many {
                _variant_name: sm::ascii::String,
                data: sm::vec_map::VecMap<u64, super::primitives::data::NexusData>,
            },
        }

        impl sm::MoveType for PortData {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for PortData {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("dag").expect("invalid module"),
                    sm::parse_identifier("PortData").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for PortData {}

        impl sm::HasCopy for PortData {}

        impl sm::HasDrop for PortData {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution`.
        /// Abilities: *(none)*.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag"
        )]
        pub struct RequestWalkExecution {
            pub execution: sm::types::ID,
            pub for_walks_indices: Vec<u64>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecutionEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct RequestWalkExecutionEvent {
            pub dag: sm::types::ID,
            pub execution: sm::types::ID,
            pub walk_index: u64,
            pub next_vertex: RuntimeVertex,
            pub evaluations: sm::types::ID,
            pub worksheet_from_type: sm::type_name::TypeName,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum RuntimeVertex {
            Plain {
                _variant_name: sm::ascii::String,
                vertex: Vertex,
            },
            WithIterator {
                _variant_name: sm::ascii::String,
                vertex: Vertex,
                iteration: u64,
                out_of: u64,
            },
        }

        impl sm::MoveType for RuntimeVertex {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for RuntimeVertex {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("dag").expect("invalid module"),
                    sm::parse_identifier("RuntimeVertex").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for RuntimeVertex {}

        impl sm::HasCopy for RuntimeVertex {}

        impl sm::HasDrop for RuntimeVertex {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct Vertex {
            pub name: sm::ascii::String,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexEvaluations`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "key, store"
        )]
        pub struct VertexEvaluations {
            pub id: sm::types::UID,
            pub ports_to_data: sm::vec_map::VecMap<InputPort, PortData>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexInfo`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct VertexInfo {
            pub kind: VertexKind,
            pub input_ports: sm::vec_set::VecSet<InputPort>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexInputPort`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "store, copy, drop"
        )]
        pub struct VertexInputPort {
            pub vertex: Vertex,
            pub port: InputPort,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexKind`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum VertexKind {
            OnChain {
                _variant_name: sm::ascii::String,
                tool: sm::types::ID,
                tool_fqn: sm::ascii::String,
            },
            OffChain {
                _variant_name: sm::ascii::String,
                tool_fqn: sm::ascii::String,
            },
        }

        impl sm::MoveType for VertexKind {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for VertexKind {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("dag").expect("invalid module"),
                    sm::parse_identifier("VertexKind").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for VertexKind {}

        impl sm::HasCopy for VertexKind {}

        impl sm::HasDrop for VertexKind {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::WalkAdvancedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct WalkAdvancedEvent {
            pub dag: sm::types::ID,
            pub execution: sm::types::ID,
            pub walk_index: u64,
            pub vertex: RuntimeVertex,
            pub variant: OutputVariant,
            pub variant_ports_to_data:
                sm::vec_map::VecMap<OutputPort, super::primitives::data::NexusData>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::WalkFailedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "dag",
            abilities = "copy, drop"
        )]
        pub struct WalkFailedEvent {
            pub dag: sm::types::ID,
            pub execution: sm::types::ID,
            pub walk_index: u64,
            pub vertex: RuntimeVertex,
            pub reason: sm::ascii::String,
        }
        /// Move: `public fun dag::assert_execution_is_for(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_execution_is_for(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
            arg1: &impl sm_call::ObjectArg<DAG>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "assert_execution_is_for")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::assert_execution_matches_leader_cap(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_execution_matches_leader_cap(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "assert_execution_matches_leader_cap")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::assert_matches_worksheet(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_matches_worksheet(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
            arg1: super::primitives::proof_of_uid::ProofOfUID,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "assert_matches_worksheet")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::assert_request_walk_execution_is_for(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_request_walk_execution_is_for(
            arg0: RequestWalkExecution,
            arg1: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "assert_request_walk_execution_is_for")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::begin_execution(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg2: 0x2::object::ID, arg3: 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>>, arg4: u64, arg5: &0x2::clock::Clock, arg6: &mut 0x2::tx_context::TxContext): (0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn begin_execution(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: super::primitives::proof_of_uid::ProofOfUID,
            arg2: sm::types::ID,
            arg3: sm::vec_map::VecMap<
                Vertex,
                sm::vec_map::VecMap<InputPort, super::primitives::data::NexusData>,
            >,
            arg4: u64,
            arg5: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "begin_execution")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(arg5).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::begin_execution_of_entry_group(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg2: 0x2::object::ID, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup, arg4: 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>>, arg5: u64, arg6: &0x2::clock::Clock, arg7: &mut 0x2::tx_context::TxContext): (0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn begin_execution_of_entry_group(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: super::primitives::proof_of_uid::ProofOfUID,
            arg2: sm::types::ID,
            arg3: EntryGroup,
            arg4: sm::vec_map::VecMap<
                Vertex,
                sm::vec_map::VecMap<InputPort, super::primitives::data::NexusData>,
            >,
            arg5: u64,
            arg6: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "begin_execution_of_entry_group")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(arg6).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::begin_execution_with_config(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DagExecutionConfig, arg3: &0x2::clock::Clock, arg4: &mut 0x2::tx_context::TxContext): (0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn begin_execution_with_config(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: super::primitives::proof_of_uid::ProofOfUID,
            arg2: DagExecutionConfig,
            arg3: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "begin_execution_with_config")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::dag_runtime_vertex_tool_fqn(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn dag_runtime_vertex_tool_fqn(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: RuntimeVertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "dag_runtime_vertex_tool_fqn")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::dag_vertex_tool_fqn(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn dag_vertex_tool_fqn(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "dag_vertex_tool_fqn")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::default_entry_group(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn default_entry_group() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "default_entry_group")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::edge_kind_break(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn edge_kind_break() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "edge_kind_break")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::edge_kind_collect(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn edge_kind_collect() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "edge_kind_collect")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::edge_kind_do_while(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn edge_kind_do_while() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "edge_kind_do_while")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::edge_kind_for_each(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn edge_kind_for_each() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "edge_kind_for_each")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::edge_kind_normal(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn edge_kind_normal() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "edge_kind_normal")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::encrypted_input_port_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn encrypted_input_port_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "encrypted_input_port_from_string")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::entry_group_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn entry_group_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "entry_group_from_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::entry_group_into_string(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn entry_group_into_string(arg0: EntryGroup) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "entry_group_into_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_created_at(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): u64`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_created_at(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_created_at")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_entry_group(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_entry_group(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_entry_group")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_invoker(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): address`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_invoker(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_invoker")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_is_finished(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_is_finished(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_is_finished")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_is_vertex_invoked(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_is_vertex_invoked(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
            arg1: RuntimeVertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_is_vertex_invoked")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_network(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): 0x2::object::ID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_network(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_network")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_vertex_ports_to_data(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex): &0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::PortData>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_vertex_ports_to_data(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
            arg1: Vertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_vertex_ports_to_data")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::execution_worksheet_type_name(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): &0x1::type_name::TypeName`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execution_worksheet_type_name(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "execution_worksheet_type_name")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::input_port_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn input_port_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "input_port_from_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::input_port_into_string(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn input_port_into_string(arg0: InputPort) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "input_port_into_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::inputs_to_begin_execution(arg0: vector<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex>, arg1: vector<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort>, arg2: vector<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>): 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn inputs_to_begin_execution(
            arg0: Vec<Vertex>,
            arg1: Vec<InputPort>,
            arg2: Vec<super::primitives::data::NexusData>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "inputs_to_begin_execution")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::is_vertex_onchain_tool(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_vertex_onchain_tool(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "is_vertex_onchain_tool")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::new(arg0: &mut 0x2::tx_context::TxContext): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new() -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "new").expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun dag::new_dag_execution_config(arg0: 0x2::object::ID, arg1: 0x2::object::ID, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup, arg3: 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>>, arg4: u64, arg5: &mut 0x2::tx_context::TxContext): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DagExecutionConfig`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_dag_execution_config(
            arg0: sm::types::ID,
            arg1: sm::types::ID,
            arg2: EntryGroup,
            arg3: sm::vec_map::VecMap<
                Vertex,
                sm::vec_map::VecMap<InputPort, super::primitives::data::NexusData>,
            >,
            arg4: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "new_dag_execution_config")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::output_port_from_raw(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn output_port_from_raw(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "output_port_from_raw")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::output_port_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn output_port_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "output_port_from_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::output_port_into_string(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn output_port_into_string(arg0: OutputPort) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "output_port_into_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::output_variant_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn output_variant_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "output_variant_from_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::output_variant_into_string(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn output_variant_into_string(arg0: OutputVariant) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "output_variant_into_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::port_data_many(arg0: 0x2::vec_map::VecMap<u64, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::PortData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn port_data_many(
            arg0: sm::vec_map::VecMap<u64, super::primitives::data::NexusData>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "port_data_many")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::port_data_single(arg0: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::PortData`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn port_data_single(arg0: super::primitives::data::NexusData) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "port_data_single")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::rebuild(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0x2::tx_context::TxContext): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn rebuild(arg0: &impl sm_call::ObjectArg<DAG>) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "rebuild").expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::request_network_to_execute_walks(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution, arg3: &0x2::clock::Clock, arg4: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn request_network_to_execute_walks(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: &mut impl sm_call::ObjectArg<DAGExecution>,
            arg2: RequestWalkExecution,
            arg3: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "request_network_to_execute_walks")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg_mut(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::request_walk_execution(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn request_walk_execution(
            arg0: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "request_walk_execution")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::request_walk_execution_vertices(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg2: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution): vector<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn request_walk_execution_vertices(
            arg0: RequestWalkExecution,
            arg1: &impl sm_call::ObjectArg<DAG>,
            arg2: &impl sm_call::ObjectArg<DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "request_walk_execution_vertices")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::request_walk_execution_walks_indices(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution): &vector<u64>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn request_walk_execution_walks_indices(
            arg0: RequestWalkExecution,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "request_walk_execution_walks_indices")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::runtime_vertex_plain_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn runtime_vertex_plain_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "runtime_vertex_plain_from_string")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::runtime_vertex_plain_from_vertex(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn runtime_vertex_plain_from_vertex(arg0: Vertex) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "runtime_vertex_plain_from_vertex")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::runtime_vertex_with_iterator_from_string(arg0: 0x1::ascii::String, arg1: u64, arg2: u64): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn runtime_vertex_with_iterator_from_string(
            arg0: sm::ascii::String,
            arg1: u64,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "runtime_vertex_with_iterator_from_string")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::runtime_vertex_with_iterator_from_vertex(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg1: u64, arg2: u64): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn runtime_vertex_with_iterator_from_vertex(
            arg0: Vertex,
            arg1: u64,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "runtime_vertex_with_iterator_from_vertex")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::submit_off_chain_tool_eval_for_walk(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg2: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg3: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>, arg4: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution, arg5: u64, arg6: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex, arg7: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, arg8: 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>, arg9: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn submit_off_chain_tool_eval_for_walk(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: &mut impl sm_call::ObjectArg<DAGExecution>,
            arg2: super::primitives::proof_of_uid::ProofOfUID,
            arg3: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg4: RequestWalkExecution,
            arg5: u64,
            arg6: RuntimeVertex,
            arg7: OutputVariant,
            arg8: sm::vec_map::VecMap<OutputPort, super::primitives::data::NexusData>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "submit_off_chain_tool_eval_for_walk")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg_mut(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec.push_arg(&arg7).expect("encode arg");
            spec.push_arg(&arg8).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::submit_on_chain_tool_eval_for_walk(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg2: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID, arg3: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>, arg4: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution, arg5: u64, arg6: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex, arg7: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, arg8: 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>, arg9: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn submit_on_chain_tool_eval_for_walk(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: &mut impl sm_call::ObjectArg<DAGExecution>,
            arg2: super::primitives::proof_of_uid::ProofOfUID,
            arg3: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg4: RequestWalkExecution,
            arg5: u64,
            arg6: RuntimeVertex,
            arg7: OutputVariant,
            arg8: sm::vec_map::VecMap<OutputPort, super::primitives::data::NexusData>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "submit_on_chain_tool_eval_for_walk")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg_mut(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec.push_arg(&arg7).expect("encode arg");
            spec.push_arg(&arg8).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_from_string(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_from_string(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_from_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_input_port(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexInputPort`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_input_port(arg0: Vertex, arg1: InputPort) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_input_port")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_input_port_from_string(arg0: 0x1::ascii::String, arg1: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexInputPort`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_input_port_from_string(
            arg0: sm::ascii::String,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_input_port_from_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_into_string(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_into_string(arg0: Vertex) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_into_string")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_name(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_name(arg0: RuntimeVertex) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_name")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_off_chain(arg0: 0x1::ascii::String): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_off_chain(arg0: sm::ascii::String) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_off_chain")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertex_on_chain(arg0: 0x1::ascii::String, arg1: 0x2::object::ID): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexKind`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertex_on_chain(arg0: sm::ascii::String, arg1: sm::types::ID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "vertex_on_chain")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::vertices(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG): &0x2::linked_table::LinkedTable<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexInfo>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn vertices(arg0: &impl sm_call::ObjectArg<DAG>) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "dag", "vertices").expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_default_value(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, arg3: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_default_value(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: InputPort,
            arg3: super::primitives::data::NexusData,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_default_value")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_edge(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort, arg4: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg5: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, arg6: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_edge(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: OutputVariant,
            arg3: OutputPort,
            arg4: Vertex,
            arg5: InputPort,
            arg6: EdgeKind,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_edge")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_encrypted_edge(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort, arg4: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg5: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, arg6: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EdgeKind): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_encrypted_edge(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: OutputVariant,
            arg3: OutputPort,
            arg4: Vertex,
            arg5: InputPort,
            arg6: EdgeKind,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_encrypted_edge")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_encrypted_output(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_encrypted_output(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: OutputVariant,
            arg3: OutputPort,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_encrypted_output")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_entry(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_entry(arg0: &impl sm_call::ObjectArg<DAG>, arg1: Vertex) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_entry")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_entry_in_group(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_entry_in_group(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: EntryGroup,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_entry_in_group")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_entry_port(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_entry_port(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: InputPort,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_entry_port")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_entry_port_in_group(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_entry_port_in_group(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: InputPort,
            arg3: EntryGroup,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_entry_port_in_group")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_output(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_output(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: OutputVariant,
            arg3: OutputPort,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_output")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun dag::with_vertex(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::VertexKind): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_vertex(
            arg0: &impl sm_call::ObjectArg<DAG>,
            arg1: Vertex,
            arg2: VertexKind,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "dag", "with_vertex")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
    }

    pub mod default_tap {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::BeginDagExecutionWitness`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "default_tap",
            abilities = "drop"
        )]
        pub struct BeginDagExecutionWitness {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::DefaultTAP`.
        /// Abilities: `key`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "default_tap",
            abilities = "key"
        )]
        pub struct DefaultTAP {
        pub id: sm::types::UID,
        pub witness: sm::bag::Bag,
        pub iv: compile_error!(
            "sui-move-codegen: unknown external type `0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88::version::InterfaceVersion`; generate bindings for that package too"
        ),
    }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::DefaultTAPV1Witness`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "default_tap",
            abilities = "key, store"
        )]
        pub struct DefaultTAPV1Witness {
            pub id: sm::types::UID,
        }
        /// Move: `public fun default_tap::begin_dag_execution(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::DefaultTAP, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg2: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg3: 0x2::object::ID, arg4: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::EntryGroup, arg5: 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::Vertex, 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::InputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>>, arg6: u64, arg7: &0x2::clock::Clock, arg8: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn begin_dag_execution(
            arg0: &mut impl sm_call::ObjectArg<DefaultTAP>,
            arg1: &impl sm_call::ObjectArg<super::dag::DAG>,
            arg2: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg3: sm::types::ID,
            arg4: super::dag::EntryGroup,
            arg5: sm::vec_map::VecMap<
                super::dag::Vertex,
                sm::vec_map::VecMap<super::dag::InputPort, super::primitives::data::NexusData>,
            >,
            arg6: u64,
            arg7: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "default_tap", "begin_dag_execution")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg_mut(arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec.push_arg(arg7).expect("encode arg");
            spec
        }
        /// Move: `public fun default_tap::confirm_tool_eval_for_walk(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::DefaultTAP, arg1: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn confirm_tool_eval_for_walk(
            arg0: &mut impl sm_call::ObjectArg<DefaultTAP>,
            arg1: super::primitives::proof_of_uid::ProofOfUID,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "default_tap", "confirm_tool_eval_for_walk")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun default_tap::dag_begin_execution_from_scheduler(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::DefaultTAP, arg1: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg2: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg3: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg4: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>, arg5: u64, arg6: u64, arg7: &0x2::clock::Clock, arg8: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn dag_begin_execution_from_scheduler(
            arg0: &mut impl sm_call::ObjectArg<DefaultTAP>,
            arg1: &mut impl sm_call::ObjectArg<super::scheduler::Task>,
            arg2: &impl sm_call::ObjectArg<super::dag::DAG>,
            arg3: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg4: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg5: u64,
            arg6: u64,
            arg7: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(
                PACKAGE,
                "default_tap",
                "dag_begin_execution_from_scheduler",
            )
            .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg_mut(arg1).expect("encode arg");
            spec.push_arg(arg2).expect("encode arg");
            spec.push_arg_mut(arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec.push_arg(arg7).expect("encode arg");
            spec
        }
        /// Move: `public fun default_tap::register_begin_execution(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Execution>, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DagExecutionConfig)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_begin_execution(
            arg0: super::primitives::policy::Policy<super::scheduler::Execution>,
            arg1: super::dag::DagExecutionConfig,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "default_tap", "register_begin_execution")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun default_tap::worksheet(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::default_tap::DefaultTAP): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn worksheet(arg0: &impl sm_call::ObjectArg<DefaultTAP>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "default_tap", "worksheet")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
    }

    pub mod gas {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ClaimedGas`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "store, copy, drop"
        )]
        pub struct ClaimedGas {
            pub execution: u64,
            pub priority: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ExecutionGas`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "key, store"
        )]
        pub struct ExecutionGas {
            pub id: sm::types::UID,
            pub settled_vertices: sm::vec_set::VecSet<super::dag::RuntimeVertex>,
            pub tool_cost_snapshot: sm::containers::Table<sm::ascii::String, u64>,
            pub claimed_leader_gas: sm::containers::Table<Vec<u8>, ClaimedGas>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasBudgets`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "store"
        )]
        pub struct GasBudgets {
            pub inner: sm::bag::Bag,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService`.
        /// Abilities: `key`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "key"
        )]
        pub struct GasService {
            pub id: sm::types::UID,
            pub executions_gas: sm::object_table::ObjectTable<sm::types::ID, ExecutionGas>,
            pub tools_gas: sm::containers::Table<sm::ascii::String, ToolGas>,
            pub gas_budgets: GasBudgets,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasSettlementUpdateEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "copy, drop"
        )]
        pub struct GasSettlementUpdateEvent {
            pub execution: sm::types::ID,
            pub vertex: super::dag::RuntimeVertex,
            pub tool_fqn: sm::ascii::String,
            pub was_settled: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasTicket`.
        /// Abilities: `store, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "store, drop"
        )]
        pub struct GasTicket {
            pub created_at_ms: u64,
            pub modus_operandi: ModusOperandi,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::LeaderClaimedGasEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "copy, drop"
        )]
        pub struct LeaderClaimedGasEvent {
            pub network: sm::types::ID,
            pub amount: u64,
            pub purpose: sm::ascii::String,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ModusOperandi`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum ModusOperandi {
            Expiry {
                _variant_name: sm::ascii::String,
                valid_for_ms: u64,
            },
            LimitedInvocations {
                _variant_name: sm::ascii::String,
                total: u64,
                used: u64,
            },
            UponDiscretionOfTheTool {
                _variant_name: sm::ascii::String,
            },
        }

        impl sm::MoveType for ModusOperandi {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for ModusOperandi {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("gas").expect("invalid module"),
                    sm::parse_identifier("ModusOperandi").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for ModusOperandi {}

        impl sm::HasCopy for ModusOperandi {}

        impl sm::HasDrop for ModusOperandi {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "drop"
        )]
        pub struct OverGas {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum Scope {
            Execution { pos0: sm::types::ID },
            WorksheetType { pos0: sm::type_name::TypeName },
            InvokerAddress { pos0: sm::prelude::Address },
        }

        impl sm::MoveType for Scope {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for Scope {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("gas").expect("invalid module"),
                    sm::parse_identifier("Scope").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for Scope {}

        impl sm::HasCopy for Scope {}

        impl sm::HasDrop for Scope {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ToolGas`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas",
            abilities = "store"
        )]
        pub struct ToolGas {
            pub vault: sm::balance::Balance<sm::sui::SUI>,
            pub single_invocation_cost_mist: u64,
            pub settings: sm::bag::Bag,
            pub tickets: sm::bag::Bag,
        }
        /// Move: `public fun gas::add_gas_budget(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope, arg2: 0x2::balance::Balance<0x2::sui::SUI>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_gas_budget(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: Scope,
            arg2: sm::balance::Balance<sm::sui::SUI>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "add_gas_budget")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::add_gas_ticket(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: 0x1::ascii::String, arg4: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope, arg5: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ModusOperandi, arg6: &0x2::clock::Clock, arg7: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_gas_ticket(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
            arg4: Scope,
            arg5: ModusOperandi,
            arg6: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "add_gas_ticket")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(arg6).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::claim_gas(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg3: 0x1::ascii::String, arg4: &mut 0x2::tx_context::TxContext): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn claim_gas(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "claim_gas")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::claim_leader_gas(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>, arg3: u64, arg4: u64, arg5: &mut 0x2::tx_context::TxContext): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn claim_leader_gas(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::dag::DAGExecution>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: u64,
            arg4: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "claim_leader_gas")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::claim_leader_gas_for_invoker(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: address, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>, arg3: u64): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn claim_leader_gas_for_invoker(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: sm::prelude::Address,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "claim_leader_gas_for_invoker")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::claim_leader_gas_for_pre_key(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: address, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>, arg3: u64): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn claim_leader_gas_for_pre_key(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: sm::prelude::Address,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "claim_leader_gas_for_pre_key")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::deescalate(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg2: 0x1::ascii::String, arg3: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn deescalate(
            arg0: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "deescalate")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::donate_to_tool(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: 0x1::ascii::String, arg2: 0x2::balance::Balance<0x2::sui::SUI>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn donate_to_tool(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: sm::ascii::String,
            arg2: sm::balance::Balance<sm::sui::SUI>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "donate_to_tool")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::gas_budget_above_min(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope, arg2: u64): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn gas_budget_above_min(
            arg0: &impl sm_call::ObjectArg<GasService>,
            arg1: Scope,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "gas_budget_above_min")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::get_tool_gas_setting(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: 0x1::ascii::String): &0x2::bag::Bag`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn get_tool_gas_setting(
            arg0: &impl sm_call::ObjectArg<GasService>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "get_tool_gas_setting")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::get_tool_gas_setting_mut(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: 0x1::ascii::String, arg4: &mut 0x2::tx_context::TxContext): &mut 0x2::bag::Bag`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn get_tool_gas_setting_mut(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "get_tool_gas_setting_mut")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::is_execution_vertex_settled(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_execution_vertex_settled(
            arg0: &impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::dag::DAGExecution>,
            arg2: super::dag::RuntimeVertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "is_execution_vertex_settled")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::modus_operandi_expiry(arg0: u64): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ModusOperandi`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn modus_operandi_expiry(arg0: u64) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "modus_operandi_expiry")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::modus_operandi_limited_invocations(arg0: u64, arg1: u64): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ModusOperandi`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn modus_operandi_limited_invocations(arg0: u64, arg1: u64) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "gas", "modus_operandi_limited_invocations")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::modus_operandi_upon_discretion_of_the_tool(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::ModusOperandi`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn modus_operandi_upon_discretion_of_the_tool() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(
                PACKAGE,
                "gas",
                "modus_operandi_upon_discretion_of_the_tool",
            )
            .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun gas::refund_execution_gas_budget(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg2: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn refund_execution_gas_budget(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::dag::DAGExecution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "refund_execution_gas_budget")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::refund_invoker_gas_budget(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &mut 0x2::tx_context::TxContext): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn refund_invoker_gas_budget(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "refund_invoker_gas_budget")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::refund_worksheet_gas_budget<T0>(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &T0): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn refund_worksheet_gas_budget<T0>(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "refund_worksheet_gas_budget")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::revoke_gas_ticket(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: 0x1::ascii::String, arg4: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope, arg5: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn revoke_gas_ticket(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
            arg4: Scope,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "revoke_gas_ticket")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::scope_execution(arg0: 0x2::object::ID): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn scope_execution(arg0: sm::types::ID) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "scope_execution")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::scope_invoker_address(arg0: address): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn scope_invoker_address(arg0: sm::prelude::Address) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "scope_invoker_address")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::scope_worksheet_type(arg0: 0x1::type_name::TypeName): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::Scope`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn scope_worksheet_type(arg0: sm::type_name::TypeName) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "scope_worksheet_type")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::set_single_invocation_cost_mist(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: 0x1::ascii::String, arg4: u64, arg5: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_single_invocation_cost_mist(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
            arg4: u64,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "gas", "set_single_invocation_cost_mist")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::sync_gas_state(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg2: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg3: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RequestWalkExecution, arg4: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn sync_gas_state(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::dag::DAG>,
            arg2: &impl sm_call::ObjectArg<super::dag::DAGExecution>,
            arg3: super::dag::RequestWalkExecution,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "sync_gas_state")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas::sync_gas_state_for_vertex(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAG, arg2: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::DAGExecution, arg3: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::RuntimeVertex, arg4: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn sync_gas_state_for_vertex(
            arg0: &mut impl sm_call::ObjectArg<GasService>,
            arg1: &impl sm_call::ObjectArg<super::dag::DAG>,
            arg2: &impl sm_call::ObjectArg<super::dag::DAGExecution>,
            arg3: super::dag::RuntimeVertex,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas", "sync_gas_state_for_vertex")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
    }

    pub mod gas_extension {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas_extension::ExpiryCostPerMinuteKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas_extension",
            abilities = "store, copy, drop"
        )]
        pub struct ExpiryCostPerMinuteKey {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas_extension::ExpiryGasOwnerCapKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas_extension",
            abilities = "store, copy, drop"
        )]
        pub struct ExpiryGasOwnerCapKey {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas_extension::LimitedInvocationsCostPerInvocationKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas_extension",
            abilities = "store, copy, drop"
        )]
        pub struct LimitedInvocationsCostPerInvocationKey {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas_extension::LimitedInvocationsGasOwnerCapKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas_extension",
            abilities = "store, copy, drop"
        )]
        pub struct LimitedInvocationsGasOwnerCapKey {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas_extension::LimitedInvocationsMaxInvocationsKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas_extension",
            abilities = "store, copy, drop"
        )]
        pub struct LimitedInvocationsMaxInvocationsKey {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas_extension::LimitedInvocationsMinInvocationsKey`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "gas_extension",
            abilities = "store, copy, drop"
        )]
        pub struct LimitedInvocationsMinInvocationsKey {
            pub dummy_field: bool,
        }
        /// Move: `public fun gas_extension::buy_expiry_gas_ticket(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: 0x1::ascii::String, arg3: u64, arg4: &mut 0x2::coin::Coin<0x2::sui::SUI>, arg5: &0x2::clock::Clock, arg6: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn buy_expiry_gas_ticket(
            arg0: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: sm::ascii::String,
            arg3: u64,
            arg4: &mut impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
            arg5: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "gas_extension", "buy_expiry_gas_ticket")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg_mut(arg4).expect("encode arg");
            spec.push_arg(arg5).expect("encode arg");
            spec
        }
        /// Move: `public fun gas_extension::buy_limited_invocations_gas_ticket(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: 0x1::ascii::String, arg3: u64, arg4: &mut 0x2::coin::Coin<0x2::sui::SUI>, arg5: &0x2::clock::Clock, arg6: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn buy_limited_invocations_gas_ticket(
            arg0: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: sm::ascii::String,
            arg3: u64,
            arg4: &mut impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
            arg5: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(
                PACKAGE,
                "gas_extension",
                "buy_limited_invocations_gas_ticket",
            )
            .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg_mut(arg4).expect("encode arg");
            spec.push_arg(arg5).expect("encode arg");
            spec
        }
        /// Move: `public fun gas_extension::disable_expiry(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: 0x1::ascii::String, arg4: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn disable_expiry(
            arg0: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas_extension", "disable_expiry")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas_extension::disable_limited_invocations(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: 0x1::ascii::String, arg4: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn disable_limited_invocations(
            arg0: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "gas_extension", "disable_limited_invocations")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun gas_extension::enable_expiry(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: u64, arg4: 0x1::ascii::String, arg5: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn enable_expiry(
            arg0: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: u64,
            arg4: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "gas_extension", "enable_expiry")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun gas_extension::enable_limited_invocations(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg2: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::OverGas>, arg3: u64, arg4: u64, arg5: u64, arg6: 0x1::ascii::String, arg7: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn enable_limited_invocations(
            arg0: &mut impl sm_call::ObjectArg<super::gas::GasService>,
            arg1: &impl sm_call::ObjectArg<super::tool_registry::ToolRegistry>,
            arg2: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg3: u64,
            arg4: u64,
            arg5: u64,
            arg6: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "gas_extension", "enable_limited_invocations")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec
        }
    }

    pub mod leader_cap {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::FoundingLeaderCapCreatedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "leader_cap",
            abilities = "copy, drop"
        )]
        pub struct FoundingLeaderCapCreatedEvent {
            pub leader_cap: sm::types::ID,
            pub network: sm::types::ID,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "leader_cap",
            abilities = "drop"
        )]
        pub struct OverNetwork {
            pub dummy_field: bool,
        }
        /// Move: `public fun leader_cap::new(arg0: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::leader_cap::OverNetwork>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "leader_cap", "new")
                .expect("valid Move identifiers");
            spec
        }
    }

    pub mod main {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;
    }

    pub mod pre_key_vault {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::OverCrypto`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "drop"
        )]
        pub struct OverCrypto {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyAssociatedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "copy, drop"
        )]
        pub struct PreKeyAssociatedEvent {
            pub claimed_by: sm::prelude::Address,
            pub pre_key: Vec<u8>,
            pub initial_message: Vec<u8>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyClaim`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "store"
        )]
        pub struct PreKeyClaim {
            pub claimed_ms: u64,
            pub pre_key_bytes: Vec<u8>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyFulfilledEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "copy, drop"
        )]
        pub struct PreKeyFulfilledEvent {
            pub requested_by: sm::prelude::Address,
            pub pre_key_bytes: Vec<u8>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyRequestedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "copy, drop"
        )]
        pub struct PreKeyRequestedEvent {
            pub requested_by: sm::prelude::Address,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVault`.
        /// Abilities: `key`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "key"
        )]
        pub struct PreKeyVault {
            pub id: sm::types::UID,
            pub claimed: sm::containers::Table<sm::prelude::Address, PreKeyClaim>,
            pub mist_gas_budget_required_to_claim: u64,
            pub per_address_rate_limit_ms: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVaultCreatedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "pre_key_vault",
            abilities = "copy, drop"
        )]
        pub struct PreKeyVaultCreatedEvent {
            pub vault: sm::types::ID,
            pub crypto_cap: sm::types::ID,
        }
        /// Move: `public fun pre_key_vault::associate_pre_key(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVault, arg1: vector<u8>, arg2: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn associate_pre_key(
            arg0: &mut impl sm_call::ObjectArg<PreKeyVault>,
            arg1: Vec<u8>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "pre_key_vault", "associate_pre_key")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun pre_key_vault::claim_pre_key_for_self(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVault, arg1: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::gas::GasService, arg2: &0x2::clock::Clock, arg3: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn claim_pre_key_for_self(
            arg0: &mut impl sm_call::ObjectArg<PreKeyVault>,
            arg1: &impl sm_call::ObjectArg<super::gas::GasService>,
            arg2: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "pre_key_vault", "claim_pre_key_for_self")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec.push_arg(arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun pre_key_vault::fulfill_pre_key_for_user(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVault, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::OverCrypto>, arg2: address, arg3: vector<u8>, arg4: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn fulfill_pre_key_for_user(
            arg0: &mut impl sm_call::ObjectArg<PreKeyVault>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::prelude::Address,
            arg3: Vec<u8>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "pre_key_vault", "fulfill_pre_key_for_user")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun pre_key_vault::set_mist_gas_budget_required_to_claim(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVault, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::OverCrypto>, arg2: u64)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_mist_gas_budget_required_to_claim(
            arg0: &mut impl sm_call::ObjectArg<PreKeyVault>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(
                PACKAGE,
                "pre_key_vault",
                "set_mist_gas_budget_required_to_claim",
            )
            .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun pre_key_vault::set_per_address_rate_limit_ms(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::PreKeyVault, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::pre_key_vault::OverCrypto>, arg2: u64)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_per_address_rate_limit_ms(
            arg0: &mut impl sm_call::ObjectArg<PreKeyVault>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "pre_key_vault", "set_per_address_rate_limit_ms")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
    }

    pub mod scheduler {

        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;
        use {
            super::PACKAGE,
            crate::primitives::{policy, Symbol},
        };

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Constraints`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, copy, drop"
        )]
        pub struct Constraints {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Execution`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, copy, drop"
        )]
        pub struct Execution {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Metadata`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, copy, drop"
        )]
        pub struct Metadata {
        pub values: sm::vec_map::VecMap<
            compile_error!(
                "sui-move-codegen: unknown external type `0x1::string::String`; generate bindings for that package too"
            ),
            compile_error!(
                "sui-move-codegen: unknown external type `0x1::string::String`; generate bindings for that package too"
            ),
        >,
    }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::MissedOccurrenceEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct MissedOccurrenceEvent {
            pub task: sm::types::ID,
            pub start_time_ms: u64,
            pub deadline_ms: sm::containers::MoveOption<u64>,
            pub pruned_at: u64,
            pub priority_fee_per_gas_unit: u64,
            pub generator: super::primitives::policy::Symbol,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Occurrence`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, copy, drop"
        )]
        pub struct Occurrence {
            pub start_time_ms: u64,
            pub deadline_ms: sm::containers::MoveOption<u64>,
            pub priority_fee_per_gas_unit: u64,
            pub generator: super::primitives::policy::Symbol,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::OccurrenceConsumedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct OccurrenceConsumedEvent {
            pub task: sm::types::ID,
            pub start_time_ms: u64,
            pub deadline_ms: sm::containers::MoveOption<u64>,
            pub priority_fee_per_gas_unit: u64,
            pub generator: super::primitives::policy::Symbol,
            pub executed_at: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::OccurrenceScheduledEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct OccurrenceScheduledEvent {
            pub task: sm::types::ID,
            pub generator: super::primitives::policy::Symbol,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::PeriodicGeneratorState`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store"
        )]
        pub struct PeriodicGeneratorState {
            pub next_start_ms: sm::containers::MoveOption<u64>,
            pub period_ms: u64,
            pub deadline_offset_ms: sm::containers::MoveOption<u64>,
            pub max_iterations: sm::containers::MoveOption<u64>,
            pub generated: u64,
            pub last_emitted_start_ms: sm::containers::MoveOption<u64>,
            pub priority_fee_per_gas_unit: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::PeriodicGeneratorWitness`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, copy, drop"
        )]
        pub struct PeriodicGeneratorWitness {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::PeriodicScheduleConfiguredEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct PeriodicScheduleConfiguredEvent {
            pub task: sm::types::ID,
            pub period_ms: sm::containers::MoveOption<u64>,
            pub deadline_offset_ms: sm::containers::MoveOption<u64>,
            pub max_iterations: sm::containers::MoveOption<u64>,
            pub generated: sm::containers::MoveOption<u64>,
            pub priority_fee_per_gas_unit: sm::containers::MoveOption<u64>,
            pub last_generated_start_ms: sm::containers::MoveOption<u64>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::QueueEntry`.
        /// Abilities: `store, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, drop"
        )]
        pub struct QueueEntry {
            pub occurrence: Occurrence,
            pub sequence: u64,
            pub request_ms: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::QueueGeneratorState`.
        /// Abilities: `store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store"
        )]
        pub struct QueueGeneratorState {
        pub pending: compile_error!(
            "sui-move-codegen: unknown external type `0x2::priority_queue::PriorityQueue`; generate bindings for that package too"
        ),
        pub len: u64,
        pub next_sequence: u64,
        pub active: sm::containers::MoveOption<QueueEntry>,
    }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::QueueGeneratorWitness`.
        /// Abilities: `store, copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "store, copy, drop"
        )]
        pub struct QueueGeneratorWitness {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::RequestScheduledExecution`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop",
            type_abilities = "T0: copy, drop"
        )]
        pub struct RequestScheduledExecution<T0> {
            pub request: T0,
            pub priority: u64,
            pub request_ms: u64,
            pub start_ms: u64,
            pub deadline_ms: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::State`.
        /// Abilities: `store, copy, drop`.
        #[derive(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum State {
            Active,
            Paused,
            Canceled,
        }

        impl sm::MoveType for State {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for State {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("scheduler").expect("invalid module"),
                    sm::parse_identifier("State").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for State {}

        impl sm::HasCopy for State {}

        impl sm::HasDrop for State {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "key, store"
        )]
        pub struct Task {
            pub id: sm::types::UID,
            pub owner: sm::prelude::Address,
            pub metadata: Metadata,
            pub constraints: super::primitives::policy::Policy<super::scheduler::Execution>,
            pub execution: super::primitives::policy::Policy<super::scheduler::Execution>,
            pub state: State,
            pub data: sm::bag::Bag,
            pub objects: sm::object_bag::ObjectBag,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::TaskCanceledEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct TaskCanceledEvent {
            pub task: sm::types::ID,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::TaskCreatedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct TaskCreatedEvent {
            pub task: sm::types::ID,
            pub owner: sm::prelude::Address,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::TaskPausedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct TaskPausedEvent {
            pub task: sm::types::ID,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::TaskResumedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "scheduler",
            abilities = "copy, drop"
        )]
        pub struct TaskResumedEvent {
            pub task: sm::types::ID,
        }
        /// Move: `public fun scheduler::add_occurrence_absolute_for_task(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: u64, arg2: 0x1::option::Option<u64>, arg3: u64, arg4: &0x2::clock::Clock, arg5: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_occurrence_absolute_for_task(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: u64,
            arg2: sm::containers::MoveOption<u64>,
            arg3: u64,
            arg4: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "add_occurrence_absolute_for_task")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::add_occurrence_relative_for_task(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: u64, arg2: 0x1::option::Option<u64>, arg3: u64, arg4: &0x2::clock::Clock, arg5: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_occurrence_relative_for_task(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: u64,
            arg2: sm::containers::MoveOption<u64>,
            arg3: u64,
            arg4: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "add_occurrence_relative_for_task")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::advance_constraints_with_witness<T0: drop>(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: T0)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn advance_constraints_with_witness<T0>(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "advance_constraints_with_witness")
                    .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::advance_execution_with_witness<T0: drop>(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: T0)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn advance_execution_with_witness<T0>(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "advance_execution_with_witness")
                    .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::borrow_constraint(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task): &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Constraints>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_constraint(arg0: &impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "borrow_constraint")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::borrow_execution(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task): &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Execution>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn borrow_execution(arg0: &impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "borrow_execution")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::cancel(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn cancel(arg0: &mut impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "cancel")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::check_periodic_occurrence(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: &0x2::clock::Clock): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn check_periodic_occurrence(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "check_periodic_occurrence")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::check_queue_occurrence(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: &0x2::clock::Clock): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn check_queue_occurrence(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "check_queue_occurrence")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::disable_periodic_for_task(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn disable_periodic_for_task(
            arg0: &mut impl sm_call::ObjectArg<Task>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "disable_periodic_for_task")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::execute(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn execute(arg0: &mut impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "execute")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::finish(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::proof_of_uid::ProofOfUID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn finish(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: super::primitives::proof_of_uid::ProofOfUID,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "finish")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::for_task(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: address)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn for_task(
            arg0: &impl sm_call::ObjectArg<Task>,
            arg1: sm::prelude::Address,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "for_task")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::is_time_canceled(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_time_canceled(arg0: &impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "is_time_canceled")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::is_time_paused(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn is_time_paused(arg0: &impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "is_time_paused")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::make_occurrence(arg0: u64, arg1: 0x1::option::Option<u64>, arg2: u64, arg3: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Occurrence`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn make_occurrence(
            arg0: u64,
            arg1: sm::containers::MoveOption<u64>,
            arg2: u64,
            arg3: super::primitives::policy::Symbol,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "make_occurrence")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::new(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Metadata, arg1: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Constraints>, arg2: 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Execution>, arg3: &mut 0x2::tx_context::TxContext): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new(
            arg0: Metadata,
            arg1: super::primitives::policy::Policy<super::scheduler::Execution>,
            arg2: super::primitives::policy::Policy<super::scheduler::Execution>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "new")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::new_constraints_policy(arg0: &0x2::table_vec::TableVec<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol>, arg1: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Constraints>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_constraints_policy(
            arg0: sm::containers::TableVec<super::primitives::policy::Symbol>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "new_constraints_policy")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::new_execution_policy(arg0: &0x2::table_vec::TableVec<0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Symbol>, arg1: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Execution>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_execution_policy(
            arg0: sm::containers::TableVec<super::primitives::policy::Symbol>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "new_execution_policy")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::new_metadata(arg0: 0x2::vec_map::VecMap<0x1::string::String, 0x1::string::String>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Metadata`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_metadata(
            arg0: sm::vec_map::VecMap<
            compile_error!(
                "sui-move-codegen: unknown external type `0x1::string::String`; generate bindings for that package too"
            ),
            compile_error!(
                "sui-move-codegen: unknown external type `0x1::string::String`; generate bindings for that package too"
            ),
        >,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "new_metadata")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::new_or_modify_periodic_for_task(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: u64, arg2: u64, arg3: 0x1::option::Option<u64>, arg4: 0x1::option::Option<u64>, arg5: u64, arg6: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_or_modify_periodic_for_task(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: u64,
            arg2: u64,
            arg3: sm::containers::MoveOption<u64>,
            arg4: sm::containers::MoveOption<u64>,
            arg5: u64,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "new_or_modify_periodic_for_task")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::new_periodic_generator_state(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::PeriodicGeneratorState`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_periodic_generator_state() -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "new_periodic_generator_state")
                    .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun scheduler::new_queue_generator_state(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::QueueGeneratorState`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_queue_generator_state() -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "new_queue_generator_state")
                    .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun scheduler::new_scheduled_execution_request<T0: copy + drop>(arg0: T0, arg1: u64, arg2: u64, arg3: u64, arg4: u64): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::RequestScheduledExecution<T0>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new_scheduled_execution_request<T0>(
            arg0: T0,
            arg1: u64,
            arg2: u64,
            arg3: u64,
            arg4: u64,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
        {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "new_scheduled_execution_request")
                    .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::pause(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn pause(arg0: &mut impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "pause")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::register_periodic_generator(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Constraints>, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::PeriodicGeneratorState)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_periodic_generator(
            arg0: super::primitives::policy::Policy<super::scheduler::Execution>,
            arg1: PeriodicGeneratorState,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "scheduler", "register_periodic_generator")
                    .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::register_queue_generator(arg0: &mut 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::policy::Policy<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Constraints>, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::QueueGeneratorState)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_queue_generator(
            arg0: super::primitives::policy::Policy<super::scheduler::Execution>,
            arg1: QueueGeneratorState,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "register_queue_generator")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::resume(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn resume(arg0: &mut impl sm_call::ObjectArg<Task>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "resume")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun scheduler::update_metadata(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Task, arg1: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::scheduler::Metadata, arg2: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn update_metadata(
            arg0: &mut impl sm_call::ObjectArg<Task>,
            arg1: Metadata,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "scheduler", "update_metadata")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
    }

    pub mod tool_output {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue`.
        /// Abilities: `store, drop`.
        #[derive(
            ::core::fmt::Debug,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            sm::__private::serde::Serialize,
            sm::__private::serde::Deserialize,
        )]
        #[serde(crate = "sui_move::__private::serde")]
        pub enum OutputValue {
            Raw { pos0: Vec<u8> },
            Number { pos0: Vec<u8> },
            String { pos0: Vec<u8> },
            Bool { pos0: Vec<u8> },
            Address { pos0: Vec<u8> },
        }

        impl sm::MoveType for OutputValue {
            fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
                sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                    <Self as sm::MoveStruct>::struct_tag_static(),
                ))
            }
        }

        impl sm::MoveStruct for OutputValue {
            fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
                sm::__private::sui_sdk_types::StructTag::new(
                    sm::parse_address(
                        "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
                    )
                    .expect("invalid address literal"),
                    sm::parse_identifier("tool_output").expect("invalid module"),
                    sm::parse_identifier("OutputValue").expect("invalid struct name"),
                    vec![],
                )
            }
        }

        impl sm::HasStore for OutputValue {}

        impl sm::HasDrop for OutputValue {}
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_output",
            abilities = "drop"
        )]
        pub struct ToolOutput {
            pub variant: sm::ascii::String,
            pub ports: sm::vec_map::VecMap<sm::ascii::String, OutputValue>,
        }
        /// Move: `public fun tool_output::address_value(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn address_value(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "address_value")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::bool_value(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn bool_value(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "bool_value")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::err(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn err(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "err")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::new(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn new(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "new")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::number_value(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn number_value(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "number_value")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::ok(): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn ok() -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "ok")
                .expect("valid Move identifiers");
            spec
        }
        /// Move: `public fun tool_output::raw_value(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn raw_value(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "raw_value")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::string_value(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn string_value(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "string_value")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::to_dag_types(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput): (0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputVariant, 0x2::vec_map::VecMap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::dag::OutputPort, 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData>)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn to_dag_types(arg0: ToolOutput) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "to_dag_types")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::variant(arg0: vector<u8>): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn variant(arg0: Vec<u8>) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "variant")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_output::with_field(arg0: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput, arg1: vector<u8>, arg2: 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::OutputValue): 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_output::ToolOutput`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn with_field(arg0: ToolOutput, arg1: Vec<u8>, arg2: OutputValue) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_output", "with_field")
                .expect("valid Move identifiers");
            spec.push_arg(&arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
    }

    pub mod tool_registry {

        use super::PACKAGE;
        #[allow(unused_imports)]
        use sui_move as sm;
        #[allow(unused_imports)]
        use sui_move_call as sm_call;

        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::AllowedOwnerAddedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct AllowedOwnerAddedEvent {
            pub owner: sm::prelude::Address,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::AllowedOwnerRemovedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct AllowedOwnerRemovedEvent {
            pub owner: sm::prelude::Address,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OffChainTool`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "key, store"
        )]
        pub struct OffChainTool {
            pub id: sm::types::UID,
            pub url: Vec<u8>,
            pub description: Vec<u8>,
            pub input_schema: Vec<u8>,
            pub output_schema: Vec<u8>,
            pub vault: sm::balance::Balance<sm::sui::SUI>,
            pub lock_duration_ms: u64,
            pub registered_at_ms: u64,
            pub unregistered_at_ms: sm::containers::MoveOption<u64>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OffChainToolRegisteredEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct OffChainToolRegisteredEvent {
            pub registry: sm::types::ID,
            pub tool: sm::types::ID,
            pub registered_at_ms: u64,
            pub fqn: sm::ascii::String,
            pub url: Vec<u8>,
            pub description: Vec<u8>,
            pub input_schema: Vec<u8>,
            pub output_schema: Vec<u8>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OnChainTool`.
        /// Abilities: `key, store`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "key, store"
        )]
        pub struct OnChainTool {
            pub id: sm::types::UID,
            pub package_address: sm::prelude::Address,
            pub module_name: sm::ascii::String,
            pub witness_id: sm::types::ID,
            pub description: Vec<u8>,
            pub input_schema: Vec<u8>,
            pub output_schema: Vec<u8>,
            pub vault: sm::balance::Balance<sm::sui::SUI>,
            pub lock_duration_ms: u64,
            pub registered_at_ms: u64,
            pub unregistered_at_ms: sm::containers::MoveOption<u64>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OnChainToolRegisteredEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct OnChainToolRegisteredEvent {
            pub registry: sm::types::ID,
            pub tool: sm::types::ID,
            pub registered_at_ms: u64,
            pub fqn: sm::ascii::String,
            pub package_address: sm::prelude::Address,
            pub module_name: sm::ascii::String,
            pub witness_id: sm::types::ID,
            pub input_schema: Vec<u8>,
            pub output_schema: Vec<u8>,
            pub description: Vec<u8>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverSlashing`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "drop"
        )]
        pub struct OverSlashing {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool`.
        /// Abilities: `drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "drop"
        )]
        pub struct OverTool {
            pub dummy_field: bool,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry`.
        /// Abilities: `key`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "key"
        )]
        pub struct ToolRegistry {
            pub id: sm::types::UID,
            pub tools: sm::object_bag::ObjectBag,
            pub mist_collateral_to_lock: u64,
            pub lock_duration_ms: u64,
            pub allowed_owners: sm::vec_set::VecSet<sm::prelude::Address>,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistryCreatedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct ToolRegistryCreatedEvent {
            pub registry: sm::types::ID,
            pub slashing_cap: sm::types::ID,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolSlashedEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct ToolSlashedEvent {
            pub tool: sm::types::ID,
            pub fqn: sm::ascii::String,
            pub amount: u64,
        }
        /// Move type: `0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolUnregisteredEvent`.
        /// Abilities: `copy, drop`.
        #[sm::move_struct(
            address = "0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06",
            module = "tool_registry",
            abilities = "copy, drop"
        )]
        pub struct ToolUnregisteredEvent {
            pub tool: sm::types::ID,
            pub fqn: sm::ascii::String,
        }
        /// Move: `public fun tool_registry::add_allowed_owner(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverSlashing>, arg2: address)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn add_allowed_owner(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::prelude::Address,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "add_allowed_owner")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::assert_onchain_tool_owner(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg2: 0x1::ascii::String)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_onchain_tool_owner(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "assert_onchain_tool_owner")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::assert_onchain_tool_owner_unchecked_generic<T0>(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg2: 0x1::ascii::String)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_onchain_tool_owner_unchecked_generic<T0>(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(
                PACKAGE,
                "tool_registry",
                "assert_onchain_tool_owner_unchecked_generic",
            )
            .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::assert_onchain_tool_witness_id(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String, arg2: 0x2::object::ID)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_onchain_tool_witness_id(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
            arg2: sm::types::ID,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "assert_onchain_tool_witness_id")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::assert_tool_owner(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg2: 0x1::ascii::String)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_tool_owner(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "assert_tool_owner")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::assert_tool_owner_unchecked_generic<T0>(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>, arg2: 0x1::ascii::String)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_tool_owner_unchecked_generic<T0>(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType,
        {
            let mut spec = sm_call::CallSpec::new(
                PACKAGE,
                "tool_registry",
                "assert_tool_owner_unchecked_generic",
            )
            .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::assert_tool_registered(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn assert_tool_registered(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "assert_tool_registered")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::claim_collateral_for_tool(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg2: 0x1::ascii::String, arg3: &0x2::clock::Clock): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn claim_collateral_for_tool(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
            arg3: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "claim_collateral_for_tool")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::deescalate<T0: drop>(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg2: 0x1::ascii::String, arg3: T0, arg4: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<T0>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn deescalate<T0>(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
            arg3: T0,
        ) -> sm_call::CallSpec
        where
            T0: sm::MoveType + sm::HasDrop,
        {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "deescalate")
                .expect("valid Move identifiers");
            spec.push_type_arg::<T0>();
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::did_unregister_period_pass(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String, arg2: &0x2::clock::Clock): bool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn did_unregister_period_pass(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
            arg2: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "did_unregister_period_pass")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::offchain_tool(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String): &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OffChainTool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn offchain_tool(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "offchain_tool")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::onchain_tool(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String): &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OnChainTool`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn onchain_tool(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "onchain_tool")
                .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::onchain_tool_module_name(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String): 0x1::ascii::String`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn onchain_tool_module_name(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "onchain_tool_module_name")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::onchain_tool_package_address(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String): address`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn onchain_tool_package_address(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "onchain_tool_package_address")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::onchain_tool_witness_id(arg0: &0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String): 0x2::object::ID`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn onchain_tool_witness_id(
            arg0: &impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "onchain_tool_witness_id")
                    .expect("valid Move identifiers");
            spec.push_arg(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::register_off_chain_tool(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: 0x1::ascii::String, arg2: vector<u8>, arg3: vector<u8>, arg4: vector<u8>, arg5: vector<u8>, arg6: &mut 0x2::coin::Coin<0x2::sui::SUI>, arg7: &0x2::clock::Clock, arg8: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_off_chain_tool(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::ascii::String,
            arg2: Vec<u8>,
            arg3: Vec<u8>,
            arg4: Vec<u8>,
            arg5: Vec<u8>,
            arg6: &mut impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
            arg7: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "register_off_chain_tool")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg_mut(arg6).expect("encode arg");
            spec.push_arg(arg7).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::register_on_chain_tool(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: address, arg2: 0x1::ascii::String, arg3: vector<u8>, arg4: vector<u8>, arg5: 0x1::ascii::String, arg6: vector<u8>, arg7: 0x2::object::ID, arg8: &mut 0x2::coin::Coin<0x2::sui::SUI>, arg9: &0x2::clock::Clock, arg10: &mut 0x2::tx_context::TxContext): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_on_chain_tool(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::prelude::Address,
            arg2: sm::ascii::String,
            arg3: Vec<u8>,
            arg4: Vec<u8>,
            arg5: sm::ascii::String,
            arg6: Vec<u8>,
            arg7: sm::types::ID,
            arg8: &mut impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
            arg9: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "register_on_chain_tool")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec.push_arg(&arg7).expect("encode arg");
            spec.push_arg_mut(arg8).expect("encode arg");
            spec.push_arg(arg9).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::register_on_chain_tool_for_self(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: address, arg2: 0x1::ascii::String, arg3: vector<u8>, arg4: vector<u8>, arg5: 0x1::ascii::String, arg6: vector<u8>, arg7: 0x2::object::ID, arg8: &mut 0x2::coin::Coin<0x2::sui::SUI>, arg9: &0x2::clock::Clock, arg10: &mut 0x2::tx_context::TxContext)`
        /// Note: `TxContext` is omitted; the runtime layer supplies it.
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn register_on_chain_tool_for_self(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: sm::prelude::Address,
            arg2: sm::ascii::String,
            arg3: Vec<u8>,
            arg4: Vec<u8>,
            arg5: sm::ascii::String,
            arg6: Vec<u8>,
            arg7: sm::types::ID,
            arg8: &mut impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
            arg9: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "register_on_chain_tool_for_self")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(&arg4).expect("encode arg");
            spec.push_arg(&arg5).expect("encode arg");
            spec.push_arg(&arg6).expect("encode arg");
            spec.push_arg(&arg7).expect("encode arg");
            spec.push_arg_mut(arg8).expect("encode arg");
            spec.push_arg(arg9).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::remove_allowed_owner(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverSlashing>, arg2: address)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn remove_allowed_owner(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::prelude::Address,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "remove_allowed_owner")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::set_lock_duration_ms(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverSlashing>, arg2: u64)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_lock_duration_ms(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "set_lock_duration_ms")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::set_mist_collateral_to_lock(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverSlashing>, arg2: u64)`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn set_mist_collateral_to_lock(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: u64,
        ) -> sm_call::CallSpec {
            let mut spec =
                sm_call::CallSpec::new(PACKAGE, "tool_registry", "set_mist_collateral_to_lock")
                    .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec
        }
        /// Move: `public fun tool_registry::slash_tool(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverSlashing>, arg2: 0x1::ascii::String, arg3: u64, arg4: &0x2::clock::Clock): 0x2::balance::Balance<0x2::sui::SUI>`
        /// Note: this function is not marked `entry`.
        #[must_use]
        pub fn slash_tool(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
            arg3: u64,
            arg4: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "slash_tool")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(&arg3).expect("encode arg");
            spec.push_arg(arg4).expect("encode arg");
            spec
        }
        /// Move: `public entry fun tool_registry::unregister_tool(arg0: &mut 0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::ToolRegistry, arg1: &0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::owner_cap::CloneableOwnerCap<0x64f81126f9e70d53754945dca42bb62d6a11cb26b2b61e32dd8a9ce0df202f06::tool_registry::OverTool>, arg2: 0x1::ascii::String, arg3: &0x2::clock::Clock)`
        #[must_use]
        pub fn unregister_tool(
            arg0: &mut impl sm_call::ObjectArg<ToolRegistry>,
            arg1: super::primitives::owner_cap::CloneableOwnerCap<super::leader_cap::OverNetwork>,
            arg2: sm::ascii::String,
            arg3: &impl sm_call::ObjectArg<sm::clock::Clock>,
        ) -> sm_call::CallSpec {
            let mut spec = sm_call::CallSpec::new(PACKAGE, "tool_registry", "unregister_tool")
                .expect("valid Move identifiers");
            spec.push_arg_mut(arg0).expect("encode arg");
            spec.push_arg(&arg1).expect("encode arg");
            spec.push_arg(&arg2).expect("encode arg");
            spec.push_arg(arg3).expect("encode arg");
            spec
        }
    }

    pub use {
        dag::{
            DAGCreatedEvent,
            DAGDefaultValueAddedEvent,
            DAGEdgeAddedEvent,
            DAGEntryVertexInputPortAddedEvent,
            DAGExecution,
            DAGOutputAddedEvent,
            DAGVertexAddedEvent,
            DAGWalk,
            DAGWalkStatus,
            DagExecutionConfig,
            Edge,
            EdgeKind,
            EndStateReachedEvent,
            EntryGroup,
            ExecutionFinishedEvent,
            InputPort,
            OutputPort,
            OutputVariant,
            OutputVariantPort,
            PortData,
            RequestWalkExecution,
            RequestWalkExecutionEvent,
            RuntimeVertex,
            Vertex,
            VertexEvaluations,
            VertexInfo,
            VertexInputPort,
            VertexKind,
            WalkAdvancedEvent,
            WalkFailedEvent,
            DAG,
        },
        default_tap::{BeginDagExecutionWitness, DefaultTAP, DefaultTAPV1Witness},
        gas::{
            ClaimedGas,
            ExecutionGas,
            GasBudgets,
            GasService,
            GasSettlementUpdateEvent,
            GasTicket,
            LeaderClaimedGasEvent,
            ModusOperandi,
            OverGas,
            Scope,
            ToolGas,
        },
        gas_extension::{
            ExpiryCostPerMinuteKey,
            ExpiryGasOwnerCapKey,
            LimitedInvocationsCostPerInvocationKey,
            LimitedInvocationsGasOwnerCapKey,
            LimitedInvocationsMaxInvocationsKey,
            LimitedInvocationsMinInvocationsKey,
        },
        leader_cap::{FoundingLeaderCapCreatedEvent, OverNetwork},
        pre_key_vault::{
            OverCrypto,
            PreKeyAssociatedEvent,
            PreKeyClaim,
            PreKeyFulfilledEvent,
            PreKeyRequestedEvent,
            PreKeyVault,
            PreKeyVaultCreatedEvent,
        },
        scheduler::{
            Constraints,
            Execution,
            Metadata,
            MissedOccurrenceEvent,
            Occurrence,
            OccurrenceConsumedEvent,
            OccurrenceScheduledEvent,
            PeriodicGeneratorState,
            PeriodicGeneratorWitness,
            PeriodicScheduleConfiguredEvent,
            QueueEntry,
            QueueGeneratorState,
            QueueGeneratorWitness,
            RequestScheduledExecution,
            State,
            Task,
            TaskCanceledEvent,
            TaskCreatedEvent,
            TaskPausedEvent,
            TaskResumedEvent,
        },
        tool_output::{OutputValue, ToolOutput},
        tool_registry::{
            AllowedOwnerAddedEvent,
            AllowedOwnerRemovedEvent,
            OffChainTool,
            OffChainToolRegisteredEvent,
            OnChainTool,
            OnChainToolRegisteredEvent,
            OverSlashing,
            OverTool,
            ToolRegistry,
            ToolRegistryCreatedEvent,
            ToolSlashedEvent,
            ToolUnregisteredEvent,
        },
    };
}
