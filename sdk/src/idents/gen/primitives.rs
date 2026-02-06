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
        let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "borrow_mut_transition_config")
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "automaton", "delta").expect("valid Move identifiers");
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
    pub fn is_accepting<T0, T1>(arg0: DeterministicAutomaton<T0, T1>, arg1: T0) -> sm_call::CallSpec
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "automaton", "new").expect("valid Move identifiers");
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
        let mut spec = sm_call::CallSpec::new(PACKAGE, "automaton", "register_transition_config")
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "automaton", "run").expect("valid Move identifiers");
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
    pub fn state_at<T0, T1>(arg0: DeterministicAutomaton<T0, T1>, arg1: u64) -> sm_call::CallSpec
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "automaton", "states").expect("valid Move identifiers");
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "data", "inline_many").expect("valid Move identifiers");
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
        let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "inline_many_limited_persistent")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun data::inline_one(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn inline_one(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "data", "inline_one").expect("valid Move identifiers");
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "data", "split_many").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun data::walrus_many(arg0: vector<vector<u8>>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn walrus_many(arg0: Vec<Vec<u8>>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "data", "walrus_many").expect("valid Move identifiers");
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
        let mut spec = sm_call::CallSpec::new(PACKAGE, "data", "walrus_many_limited_persistent")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun data::walrus_one(arg0: vector<u8>): 0xd8f40e14a26960f53252f4022d091c8613d8be1137ebaf01c5b18c067652d79f::data::NexusData`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn walrus_one(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "data", "walrus_one").expect("valid Move identifiers");
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
    pub fn as_ref<T0>(arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "owner_cap", "as_ref").expect("valid Move identifiers");
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "owner_cap", "clone").expect("valid Move identifiers");
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
    pub fn destroy<T0>(arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>) -> sm_call::CallSpec
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "owner_cap", "is_for").expect("valid Move identifiers");
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
    pub fn what_for<T0>(arg0: &impl sm_call::ObjectArg<CloneableOwnerCap<T0>>) -> sm_call::CallSpec
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "policy", "add_state").expect("valid Move identifiers");
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
        let mut spec = sm_call::CallSpec::new(PACKAGE, "policy", "borrow_mut_state_config_for_uid")
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "policy", "data_mut").expect("valid Move identifiers");
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "policy", "register").expect("valid Move identifiers");
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "proof_of_uid", "new").expect("valid Move identifiers");
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
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "proven_value", "by").expect("valid Move identifiers");
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
    pub fn unwrap_as_recipient<T0>(arg0: ProvenValue<T0>, arg1: sm::types::UID) -> sm_call::CallSpec
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
    automaton::{ConfiguredAutomaton, DeterministicAutomaton, TransitionConfigKey, TransitionKey},
    data::NexusData,
    event::EventWrapper,
    owner_cap::{CloneableOwnerCap, OwnerCap},
    policy::{Policy, Symbol},
    proof_of_uid::ProofOfUID,
    proven_value::ProvenValue,
};
