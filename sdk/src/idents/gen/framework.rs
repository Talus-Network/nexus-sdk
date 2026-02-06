/// Package address (the on-chain package object id).
pub const PACKAGE: sui_move::prelude::Address = sui_move::prelude::Address::from_static("0x2");
pub mod accumulator {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::accumulator::AccumulatorRoot`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "accumulator", abilities = "key")]
    pub struct AccumulatorRoot {
        pub id: sm::types::UID,
    }
    /// Move type: `0x2::accumulator::Key`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "accumulator",
        abilities = "store, copy, drop",
        phantoms = "T0"
    )]
    pub struct Key<T0> {
        pub address: sm::prelude::Address,
    }
    /// Move type: `0x2::accumulator::U128`.
    /// Abilities: `store`.
    #[sm::move_struct(address = "0x2", module = "accumulator", abilities = "store")]
    pub struct U128 {
        pub value: u128,
    }
}

pub mod accumulator_metadata {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::accumulator_metadata::Metadata`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "accumulator_metadata",
        abilities = "store",
        phantoms = "T0"
    )]
    pub struct Metadata<T0> {
        pub fields: sm::bag::Bag,
    }
    /// Move type: `0x2::accumulator_metadata::MetadataKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "accumulator_metadata",
        abilities = "store, copy, drop",
        phantoms = "T0"
    )]
    pub struct MetadataKey<T0> {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::accumulator_metadata::Owner`.
    /// Abilities: `store`.
    #[sm::move_struct(address = "0x2", module = "accumulator_metadata", abilities = "store")]
    pub struct Owner {
        pub balances: sm::bag::Bag,
        pub owner: sm::prelude::Address,
    }
    /// Move type: `0x2::accumulator_metadata::OwnerKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "accumulator_metadata",
        abilities = "store, copy, drop"
    )]
    pub struct OwnerKey {
        pub owner: sm::prelude::Address,
    }
}

pub mod accumulator_settlement {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::accumulator_settlement::EventStreamHead`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "accumulator_settlement",
        abilities = "store"
    )]
    pub struct EventStreamHead {
        pub mmr: Vec<u64>,
        pub checkpoint_seq: u64,
        pub num_events: u64,
    }
}

pub mod address {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun address::from_ascii_bytes(arg0: &vector<u8>): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_ascii_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "address", "from_ascii_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun address::from_bytes(arg0: vector<u8>): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "address", "from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun address::length(): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "address", "length").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun address::max(): u256`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn max() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "address", "max").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun address::to_ascii_string(arg0: address): 0x1::ascii::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_ascii_string(arg0: sm::prelude::Address) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "address", "to_ascii_string")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun address::to_bytes(arg0: address): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_bytes(arg0: sm::prelude::Address) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "address", "to_bytes").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun address::to_string(arg0: address): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_string(arg0: sm::prelude::Address) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "address", "to_string")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun address::to_u256(arg0: address): u256`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_u256(arg0: sm::prelude::Address) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "address", "to_u256").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod authenticator_state {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::authenticator_state::ActiveJwk`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "authenticator_state",
        abilities = "store, copy, drop"
    )]
    pub struct ActiveJwk {
        pub jwk_id: JwkId,
        pub jwk: JWK,
        pub epoch: u64,
    }
    /// Move type: `0x2::authenticator_state::AuthenticatorState`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "authenticator_state", abilities = "key")]
    pub struct AuthenticatorState {
        pub id: sm::types::UID,
        pub version: u64,
    }
    /// Move type: `0x2::authenticator_state::AuthenticatorStateInner`.
    /// Abilities: `store`.
    #[sm::move_struct(address = "0x2", module = "authenticator_state", abilities = "store")]
    pub struct AuthenticatorStateInner {
        pub version: u64,
        pub active_jwks: Vec<ActiveJwk>,
    }
    /// Move type: `0x2::authenticator_state::JWK`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "authenticator_state",
        abilities = "store, copy, drop"
    )]
    pub struct JWK {
        pub kty: sm::string::String,
        pub e: sm::string::String,
        pub n: sm::string::String,
        pub alg: sm::string::String,
    }
    /// Move type: `0x2::authenticator_state::JwkId`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "authenticator_state",
        abilities = "store, copy, drop"
    )]
    pub struct JwkId {
        pub iss: sm::string::String,
        pub kid: sm::string::String,
    }
}

pub mod bag {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::bag::Bag`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "bag", abilities = "key, store")]
    pub struct Bag {
        pub id: sm::types::UID,
        pub size: u64,
    }
    /// Move: `public fun bag::add<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::bag::Bag, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<sm::bag::Bag>,
        arg1: T0,
        arg2: T1,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "add").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::borrow<T0: store + copy + drop, T1: store>(arg0: &0x2::bag::Bag, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(
        arg0: &impl sm_call::ObjectArg<sm::bag::Bag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "borrow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::borrow_mut<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::bag::Bag, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<sm::bag::Bag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "borrow_mut").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::contains<T0: store + copy + drop>(arg0: &0x2::bag::Bag, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains<T0>(arg0: &impl sm_call::ObjectArg<sm::bag::Bag>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "contains").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::contains_with_type<T0: store + copy + drop, T1: store>(arg0: &0x2::bag::Bag, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains_with_type<T0, T1>(
        arg0: &impl sm_call::ObjectArg<sm::bag::Bag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bag", "contains_with_type")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::destroy_empty(arg0: 0x2::bag::Bag)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_empty(arg0: &impl sm_call::ObjectArg<sm::bag::Bag>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bag", "destroy_empty")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::is_empty(arg0: &0x2::bag::Bag): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty(arg0: &impl sm_call::ObjectArg<sm::bag::Bag>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "is_empty").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::length(arg0: &0x2::bag::Bag): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length(arg0: &impl sm_call::ObjectArg<sm::bag::Bag>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "length").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bag::new(arg0: &mut 0x2::tx_context::TxContext): 0x2::bag::Bag`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "new").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bag::remove<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::bag::Bag, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<sm::bag::Bag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bag", "remove").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod balance {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::balance::Balance`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "balance",
        abilities = "store",
        phantoms = "T0"
    )]
    pub struct Balance<T0> {
        pub value: u64,
    }
    /// Move type: `0x2::balance::Supply`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "balance",
        abilities = "store",
        phantoms = "T0"
    )]
    pub struct Supply<T0> {
        pub value: u64,
    }
    /// Move: `public fun balance::create_supply<T0: drop>(arg0: T0): 0x2::balance::Supply<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn create_supply<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "create_supply")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::decrease_supply<T0>(arg0: &mut 0x2::balance::Supply<T0>, arg1: 0x2::balance::Balance<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn decrease_supply<T0>(
        arg0: Supply<T0>,
        arg1: sm::balance::Balance<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "decrease_supply")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::destroy_zero<T0>(arg0: 0x2::balance::Balance<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_zero<T0>(arg0: sm::balance::Balance<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "destroy_zero")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::increase_supply<T0>(arg0: &mut 0x2::balance::Supply<T0>, arg1: u64): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn increase_supply<T0>(arg0: Supply<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "increase_supply")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::join<T0>(arg0: &mut 0x2::balance::Balance<T0>, arg1: 0x2::balance::Balance<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn join<T0>(
        arg0: sm::balance::Balance<T0>,
        arg1: sm::balance::Balance<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "balance", "join").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::redeem_funds<T0>(arg0: 0x2::funds_accumulator::Withdrawal<0x2::balance::Balance<T0>>): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn redeem_funds<T0>(
        arg0: super::funds_accumulator::Withdrawal<sm::balance::Balance<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "redeem_funds")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::send_funds<T0>(arg0: 0x2::balance::Balance<T0>, arg1: address)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn send_funds<T0>(
        arg0: sm::balance::Balance<T0>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "send_funds")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::split<T0>(arg0: &mut 0x2::balance::Balance<T0>, arg1: u64): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn split<T0>(arg0: sm::balance::Balance<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "balance", "split").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::supply_value<T0>(arg0: &0x2::balance::Supply<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn supply_value<T0>(arg0: Supply<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "supply_value")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::value<T0>(arg0: &0x2::balance::Balance<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn value<T0>(arg0: sm::balance::Balance<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "balance", "value").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::withdraw_all<T0>(arg0: &mut 0x2::balance::Balance<T0>): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdraw_all<T0>(arg0: sm::balance::Balance<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "balance", "withdraw_all")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun balance::zero<T0>(): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn zero<T0>() -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "balance", "zero").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec
    }
}

pub mod bcs {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::bcs::BCS`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "bcs", abilities = "store, copy, drop")]
    pub struct BCS {
        pub bytes: Vec<u8>,
    }
    /// Move: `public fun bcs::into_remainder_bytes(arg0: 0x2::bcs::BCS): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn into_remainder_bytes(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "into_remainder_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::new(arg0: vector<u8>): 0x2::bcs::BCS`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "new").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_address(arg0: &mut 0x2::bcs::BCS): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_address(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_address").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_bool(arg0: &mut 0x2::bcs::BCS): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_bool(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_bool").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_enum_tag(arg0: &mut 0x2::bcs::BCS): u32`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_enum_tag(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_enum_tag")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_address(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<address>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_address(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_address")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_bool(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<bool>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_bool(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_bool")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_u128(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<u128>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_u128(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_u128")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_u16(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<u16>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_u16(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_u16")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_u256(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<u256>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_u256(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_u256")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_u32(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<u32>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_u32(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_u32")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_u64(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<u64>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_u64(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_u64")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_option_u8(arg0: &mut 0x2::bcs::BCS): 0x1::option::Option<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_option_u8(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_option_u8")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_u128(arg0: &mut 0x2::bcs::BCS): u128`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_u128(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_u128").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_u16(arg0: &mut 0x2::bcs::BCS): u16`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_u16(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_u16").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_u256(arg0: &mut 0x2::bcs::BCS): u256`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_u256(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_u256").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_u32(arg0: &mut 0x2::bcs::BCS): u32`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_u32(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_u32").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_u64(arg0: &mut 0x2::bcs::BCS): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_u64(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_u64").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_u8(arg0: &mut 0x2::bcs::BCS): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_u8(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_u8").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_address(arg0: &mut 0x2::bcs::BCS): vector<address>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_address(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_address")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_bool(arg0: &mut 0x2::bcs::BCS): vector<bool>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_bool(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_bool")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_length(arg0: &mut 0x2::bcs::BCS): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_length(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_length")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_u128(arg0: &mut 0x2::bcs::BCS): vector<u128>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_u128(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_u128")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_u16(arg0: &mut 0x2::bcs::BCS): vector<u16>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_u16(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_u16").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_u256(arg0: &mut 0x2::bcs::BCS): vector<u256>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_u256(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_u256")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_u32(arg0: &mut 0x2::bcs::BCS): vector<u32>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_u32(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_u32").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_u64(arg0: &mut 0x2::bcs::BCS): vector<u64>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_u64(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_u64").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_u8(arg0: &mut 0x2::bcs::BCS): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_u8(arg0: BCS) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_u8").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::peel_vec_vec_u8(arg0: &mut 0x2::bcs::BCS): vector<vector<u8>>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn peel_vec_vec_u8(arg0: BCS) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bcs", "peel_vec_vec_u8")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bcs::to_bytes<T0>(arg0: &T0): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_bytes<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bcs", "to_bytes").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod bls12381 {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::bls12381::G1`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "bls12381")]
    pub struct G1 {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::bls12381::G2`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "bls12381")]
    pub struct G2 {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::bls12381::GT`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "bls12381")]
    pub struct GT {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::bls12381::Scalar`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "bls12381")]
    pub struct Scalar {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::bls12381::UncompressedG1`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "bls12381")]
    pub struct UncompressedG1 {
        pub dummy_field: bool,
    }
    /// Move: `public fun bls12381::bls12381_min_pk_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn bls12381_min_pk_verify(
        arg0: Vec<u8>,
        arg1: Vec<u8>,
        arg2: Vec<u8>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "bls12381_min_pk_verify")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::bls12381_min_sig_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn bls12381_min_sig_verify(
        arg0: Vec<u8>,
        arg1: Vec<u8>,
        arg2: Vec<u8>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "bls12381_min_sig_verify")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_add(arg0: &0x2::group_ops::Element<0x2::bls12381::G1>, arg1: &0x2::group_ops::Element<0x2::bls12381::G1>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_add(
        arg0: super::group_ops::Element<G1>,
        arg1: super::group_ops::Element<G1>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_add").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_div(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::G1>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_div(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<G1>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_div").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_from_bytes(arg0: &vector<u8>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_generator(): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_generator() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_generator")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::g1_identity(): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_identity() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_identity")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::g1_mul(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::G1>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_mul(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<G1>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_mul").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_multi_scalar_multiplication(arg0: &vector<0x2::group_ops::Element<0x2::bls12381::Scalar>>, arg1: &vector<0x2::group_ops::Element<0x2::bls12381::G1>>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_multi_scalar_multiplication(
        arg0: Vec<super::group_ops::Element<Scalar>>,
        arg1: Vec<super::group_ops::Element<G1>>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_multi_scalar_multiplication")
                .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_neg(arg0: &0x2::group_ops::Element<0x2::bls12381::G1>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_neg(arg0: super::group_ops::Element<G1>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_neg").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_sub(arg0: &0x2::group_ops::Element<0x2::bls12381::G1>, arg1: &0x2::group_ops::Element<0x2::bls12381::G1>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_sub(
        arg0: super::group_ops::Element<G1>,
        arg1: super::group_ops::Element<G1>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_sub").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g1_to_uncompressed_g1(arg0: &0x2::group_ops::Element<0x2::bls12381::G1>): 0x2::group_ops::Element<0x2::bls12381::UncompressedG1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g1_to_uncompressed_g1(arg0: super::group_ops::Element<G1>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g1_to_uncompressed_g1")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_add(arg0: &0x2::group_ops::Element<0x2::bls12381::G2>, arg1: &0x2::group_ops::Element<0x2::bls12381::G2>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_add(
        arg0: super::group_ops::Element<G2>,
        arg1: super::group_ops::Element<G2>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_add").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_div(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::G2>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_div(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<G2>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_div").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_from_bytes(arg0: &vector<u8>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_generator(): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_generator() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_generator")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::g2_identity(): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_identity() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_identity")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::g2_mul(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::G2>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_mul(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<G2>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_mul").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_multi_scalar_multiplication(arg0: &vector<0x2::group_ops::Element<0x2::bls12381::Scalar>>, arg1: &vector<0x2::group_ops::Element<0x2::bls12381::G2>>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_multi_scalar_multiplication(
        arg0: Vec<super::group_ops::Element<Scalar>>,
        arg1: Vec<super::group_ops::Element<G2>>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_multi_scalar_multiplication")
                .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_neg(arg0: &0x2::group_ops::Element<0x2::bls12381::G2>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_neg(arg0: super::group_ops::Element<G2>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_neg").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::g2_sub(arg0: &0x2::group_ops::Element<0x2::bls12381::G2>, arg1: &0x2::group_ops::Element<0x2::bls12381::G2>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn g2_sub(
        arg0: super::group_ops::Element<G2>,
        arg1: super::group_ops::Element<G2>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "g2_sub").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::gt_add(arg0: &0x2::group_ops::Element<0x2::bls12381::GT>, arg1: &0x2::group_ops::Element<0x2::bls12381::GT>): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_add(
        arg0: super::group_ops::Element<GT>,
        arg1: super::group_ops::Element<GT>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_add").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::gt_div(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::GT>): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_div(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<GT>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_div").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::gt_generator(): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_generator() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_generator")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::gt_identity(): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_identity() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_identity")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::gt_mul(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::GT>): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_mul(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<GT>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_mul").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::gt_neg(arg0: &0x2::group_ops::Element<0x2::bls12381::GT>): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_neg(arg0: super::group_ops::Element<GT>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_neg").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::gt_sub(arg0: &0x2::group_ops::Element<0x2::bls12381::GT>, arg1: &0x2::group_ops::Element<0x2::bls12381::GT>): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gt_sub(
        arg0: super::group_ops::Element<GT>,
        arg1: super::group_ops::Element<GT>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "gt_sub").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::hash_to_g1(arg0: &vector<u8>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn hash_to_g1(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "hash_to_g1")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::hash_to_g2(arg0: &vector<u8>): 0x2::group_ops::Element<0x2::bls12381::G2>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn hash_to_g2(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "hash_to_g2")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::pairing(arg0: &0x2::group_ops::Element<0x2::bls12381::G1>, arg1: &0x2::group_ops::Element<0x2::bls12381::G2>): 0x2::group_ops::Element<0x2::bls12381::GT>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pairing(
        arg0: super::group_ops::Element<G1>,
        arg1: super::group_ops::Element<G2>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "bls12381", "pairing").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_add(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::Scalar>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_add(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<Scalar>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_add")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_div(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::Scalar>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_div(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<Scalar>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_div")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_from_bytes(arg0: &vector<u8>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_from_u64(arg0: u64): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_from_u64(arg0: u64) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_from_u64")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_inv(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_inv(arg0: super::group_ops::Element<Scalar>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_inv")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_mul(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::Scalar>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_mul(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<Scalar>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_mul")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_neg(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_neg(arg0: super::group_ops::Element<Scalar>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_neg")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_one(): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_one() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_one")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::scalar_sub(arg0: &0x2::group_ops::Element<0x2::bls12381::Scalar>, arg1: &0x2::group_ops::Element<0x2::bls12381::Scalar>): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_sub(
        arg0: super::group_ops::Element<Scalar>,
        arg1: super::group_ops::Element<Scalar>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_sub")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::scalar_zero(): 0x2::group_ops::Element<0x2::bls12381::Scalar>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn scalar_zero() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "scalar_zero")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun bls12381::uncompressed_g1_sum(arg0: &vector<0x2::group_ops::Element<0x2::bls12381::UncompressedG1>>): 0x2::group_ops::Element<0x2::bls12381::UncompressedG1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uncompressed_g1_sum(
        arg0: Vec<super::group_ops::Element<UncompressedG1>>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "uncompressed_g1_sum")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun bls12381::uncompressed_g1_to_g1(arg0: &0x2::group_ops::Element<0x2::bls12381::UncompressedG1>): 0x2::group_ops::Element<0x2::bls12381::G1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uncompressed_g1_to_g1(
        arg0: super::group_ops::Element<UncompressedG1>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "bls12381", "uncompressed_g1_to_g1")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod borrow {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::borrow::Borrow`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "borrow")]
    pub struct Borrow {
        pub r#ref: sm::prelude::Address,
        pub obj: sm::types::ID,
    }
    /// Move type: `0x2::borrow::Referent`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "borrow",
        abilities = "store",
        type_abilities = "T0: key, store"
    )]
    pub struct Referent<T0> {
        pub id: sm::prelude::Address,
        pub value: sm::containers::MoveOption<T0>,
    }
    /// Move: `public fun borrow::borrow<T0: key + store>(arg0: &mut 0x2::borrow::Referent<T0>): (T0, 0x2::borrow::Borrow)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0>(arg0: Referent<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "borrow", "borrow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun borrow::destroy<T0: key + store>(arg0: 0x2::borrow::Referent<T0>): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy<T0>(arg0: Referent<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "borrow", "destroy").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun borrow::new<T0: key + store>(arg0: T0, arg1: &mut 0x2::tx_context::TxContext): 0x2::borrow::Referent<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "borrow", "new").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun borrow::put_back<T0: key + store>(arg0: &mut 0x2::borrow::Referent<T0>, arg1: T0, arg2: 0x2::borrow::Borrow)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn put_back<T0>(
        arg0: Referent<T0>,
        arg1: &impl sm_call::ObjectArg<T0>,
        arg2: Borrow,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "borrow", "put_back").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
}

pub mod clock {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::clock::Clock`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "clock", abilities = "key")]
    pub struct Clock {
        pub id: sm::types::UID,
        pub timestamp_ms: u64,
    }
    /// Move: `public fun clock::timestamp_ms(arg0: &0x2::clock::Clock): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn timestamp_ms(arg0: &impl sm_call::ObjectArg<sm::clock::Clock>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "clock", "timestamp_ms")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
}

pub mod coin {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::coin::Coin`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "coin",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct Coin<T0> {
        pub id: sm::types::UID,
        pub balance: sm::balance::Balance<T0>,
    }
    /// Move type: `0x2::coin::CoinMetadata`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "coin",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct CoinMetadata<T0> {
        pub id: sm::types::UID,
        pub decimals: u8,
        pub name: sm::string::String,
        pub symbol: sm::ascii::String,
        pub description: sm::string::String,
        pub icon_url: sm::containers::MoveOption<super::url::Url>,
    }
    /// Move type: `0x2::coin::CurrencyCreated`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "coin",
        abilities = "copy, drop",
        phantoms = "T0"
    )]
    pub struct CurrencyCreated<T0> {
        pub decimals: u8,
    }
    /// Move type: `0x2::coin::DenyCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "coin",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct DenyCap<T0> {
        pub id: sm::types::UID,
    }
    /// Move type: `0x2::coin::DenyCapV2`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "coin",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct DenyCapV2<T0> {
        pub id: sm::types::UID,
        pub allow_global_pause: bool,
    }
    /// Move type: `0x2::coin::RegulatedCoinMetadata`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "coin", abilities = "key", phantoms = "T0")]
    pub struct RegulatedCoinMetadata<T0> {
        pub id: sm::types::UID,
        pub coin_metadata_object: sm::types::ID,
        pub deny_cap_object: sm::types::ID,
    }
    /// Move type: `0x2::coin::TreasuryCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "coin",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct TreasuryCap<T0> {
        pub id: sm::types::UID,
        pub total_supply: super::balance::Supply<T0>,
    }
    /// Move: `public fun coin::balance<T0>(arg0: &0x2::coin::Coin<T0>): &0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn balance<T0>(arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "balance").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::balance_mut<T0>(arg0: &mut 0x2::coin::Coin<T0>): &mut 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn balance_mut<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "balance_mut").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::burn<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: 0x2::coin::Coin<T0>): u64`
    #[must_use]
    pub fn burn<T0>(
        arg0: &mut impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "burn").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::create_currency<T0: drop>(arg0: T0, arg1: u8, arg2: vector<u8>, arg3: vector<u8>, arg4: vector<u8>, arg5: 0x1::option::Option<0x2::url::Url>, arg6: &mut 0x2::tx_context::TxContext): (0x2::coin::TreasuryCap<T0>, 0x2::coin::CoinMetadata<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn create_currency<T0>(
        arg0: T0,
        arg1: u8,
        arg2: Vec<u8>,
        arg3: Vec<u8>,
        arg4: Vec<u8>,
        arg5: sm::containers::MoveOption<super::url::Url>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "create_currency")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec.push_arg(&arg4).expect("encode arg");
        spec.push_arg(&arg5).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::create_regulated_currency<T0: drop>(arg0: T0, arg1: u8, arg2: vector<u8>, arg3: vector<u8>, arg4: vector<u8>, arg5: 0x1::option::Option<0x2::url::Url>, arg6: &mut 0x2::tx_context::TxContext): (0x2::coin::TreasuryCap<T0>, 0x2::coin::DenyCap<T0>, 0x2::coin::CoinMetadata<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn create_regulated_currency<T0>(
        arg0: T0,
        arg1: u8,
        arg2: Vec<u8>,
        arg3: Vec<u8>,
        arg4: Vec<u8>,
        arg5: sm::containers::MoveOption<super::url::Url>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "create_regulated_currency")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec.push_arg(&arg4).expect("encode arg");
        spec.push_arg(&arg5).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::create_regulated_currency_v2<T0: drop>(arg0: T0, arg1: u8, arg2: vector<u8>, arg3: vector<u8>, arg4: vector<u8>, arg5: 0x1::option::Option<0x2::url::Url>, arg6: bool, arg7: &mut 0x2::tx_context::TxContext): (0x2::coin::TreasuryCap<T0>, 0x2::coin::DenyCapV2<T0>, 0x2::coin::CoinMetadata<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn create_regulated_currency_v2<T0>(
        arg0: T0,
        arg1: u8,
        arg2: Vec<u8>,
        arg3: Vec<u8>,
        arg4: Vec<u8>,
        arg5: sm::containers::MoveOption<super::url::Url>,
        arg6: bool,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "create_regulated_currency_v2")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec.push_arg(&arg4).expect("encode arg");
        spec.push_arg(&arg5).expect("encode arg");
        spec.push_arg(&arg6).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_add<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: &mut 0x2::coin::DenyCap<T0>, arg2: address, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_add<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &mut impl sm_call::ObjectArg<DenyCap<T0>>,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_add")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_contains<T0>(arg0: &0x2::deny_list::DenyList, arg1: address): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_contains<T0>(
        arg0: &impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_contains")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_remove<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: &mut 0x2::coin::DenyCap<T0>, arg2: address, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_remove<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &mut impl sm_call::ObjectArg<DenyCap<T0>>,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_add<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: &mut 0x2::coin::DenyCapV2<T0>, arg2: address, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_add<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &mut impl sm_call::ObjectArg<DenyCapV2<T0>>,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_v2_add")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_contains_current_epoch<T0>(arg0: &0x2::deny_list::DenyList, arg1: address, arg2: &0x2::tx_context::TxContext): bool`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_contains_current_epoch<T0>(
        arg0: &impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_v2_contains_current_epoch")
                .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_contains_next_epoch<T0>(arg0: &0x2::deny_list::DenyList, arg1: address): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_contains_next_epoch<T0>(
        arg0: &impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_v2_contains_next_epoch")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_disable_global_pause<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: &mut 0x2::coin::DenyCapV2<T0>, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_disable_global_pause<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &mut impl sm_call::ObjectArg<DenyCapV2<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_v2_disable_global_pause")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_enable_global_pause<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: &mut 0x2::coin::DenyCapV2<T0>, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_enable_global_pause<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &mut impl sm_call::ObjectArg<DenyCapV2<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_v2_enable_global_pause")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_is_global_pause_enabled_current_epoch<T0>(arg0: &0x2::deny_list::DenyList, arg1: &0x2::tx_context::TxContext): bool`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_is_global_pause_enabled_current_epoch<T0>(
        arg0: &impl sm_call::ObjectArg<super::deny_list::DenyList>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(
            PACKAGE,
            "coin",
            "deny_list_v2_is_global_pause_enabled_current_epoch",
        )
        .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_is_global_pause_enabled_next_epoch<T0>(arg0: &0x2::deny_list::DenyList): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_is_global_pause_enabled_next_epoch<T0>(
        arg0: &impl sm_call::ObjectArg<super::deny_list::DenyList>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(
            PACKAGE,
            "coin",
            "deny_list_v2_is_global_pause_enabled_next_epoch",
        )
        .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::deny_list_v2_remove<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: &mut 0x2::coin::DenyCapV2<T0>, arg2: address, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn deny_list_v2_remove<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &mut impl sm_call::ObjectArg<DenyCapV2<T0>>,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "deny_list_v2_remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::destroy_zero<T0>(arg0: 0x2::coin::Coin<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_zero<T0>(arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "destroy_zero")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::divide_into_n<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): vector<0x2::coin::Coin<T0>>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn divide_into_n<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "divide_into_n")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::from_balance<T0>(arg0: 0x2::balance::Balance<T0>, arg1: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_balance<T0>(arg0: sm::balance::Balance<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "from_balance")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::get_decimals<T0>(arg0: &0x2::coin::CoinMetadata<T0>): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn get_decimals<T0>(arg0: &impl sm_call::ObjectArg<CoinMetadata<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "get_decimals")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::get_description<T0>(arg0: &0x2::coin::CoinMetadata<T0>): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn get_description<T0>(
        arg0: &impl sm_call::ObjectArg<CoinMetadata<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "get_description")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::get_icon_url<T0>(arg0: &0x2::coin::CoinMetadata<T0>): 0x1::option::Option<0x2::url::Url>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn get_icon_url<T0>(arg0: &impl sm_call::ObjectArg<CoinMetadata<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "get_icon_url")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::get_name<T0>(arg0: &0x2::coin::CoinMetadata<T0>): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn get_name<T0>(arg0: &impl sm_call::ObjectArg<CoinMetadata<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "get_name").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::get_symbol<T0>(arg0: &0x2::coin::CoinMetadata<T0>): 0x1::ascii::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn get_symbol<T0>(arg0: &impl sm_call::ObjectArg<CoinMetadata<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "get_symbol").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::into_balance<T0>(arg0: 0x2::coin::Coin<T0>): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn into_balance<T0>(arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "into_balance")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::join<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: 0x2::coin::Coin<T0>)`
    #[must_use]
    pub fn join<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "join").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::migrate_regulated_currency_to_v2<T0>(arg0: &mut 0x2::deny_list::DenyList, arg1: 0x2::coin::DenyCap<T0>, arg2: bool, arg3: &mut 0x2::tx_context::TxContext): 0x2::coin::DenyCapV2<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn migrate_regulated_currency_to_v2<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::deny_list::DenyList>,
        arg1: &impl sm_call::ObjectArg<DenyCap<T0>>,
        arg2: bool,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "migrate_regulated_currency_to_v2")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::mint<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn mint<T0>(
        arg0: &mut impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "mint").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::mint_and_transfer<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: u64, arg2: address, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    #[must_use]
    pub fn mint_and_transfer<T0>(
        arg0: &mut impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: u64,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "mint_and_transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::mint_balance<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: u64): 0x2::balance::Balance<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn mint_balance<T0>(
        arg0: &mut impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "mint_balance")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::put<T0>(arg0: &mut 0x2::balance::Balance<T0>, arg1: 0x2::coin::Coin<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn put<T0>(
        arg0: sm::balance::Balance<T0>,
        arg1: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "put").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::redeem_funds<T0>(arg0: 0x2::funds_accumulator::Withdrawal<0x2::balance::Balance<T0>>, arg1: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn redeem_funds<T0>(
        arg0: super::funds_accumulator::Withdrawal<sm::balance::Balance<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "redeem_funds")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::send_funds<T0>(arg0: 0x2::coin::Coin<T0>, arg1: address)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn send_funds<T0>(
        arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "send_funds").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::split<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn split<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "split").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::supply<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>): &0x2::balance::Supply<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn supply<T0>(arg0: &mut impl sm_call::ObjectArg<TreasuryCap<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "supply").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::supply_immut<T0>(arg0: &0x2::coin::TreasuryCap<T0>): &0x2::balance::Supply<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn supply_immut<T0>(arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "supply_immut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::supply_mut<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>): &mut 0x2::balance::Supply<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn supply_mut<T0>(arg0: &mut impl sm_call::ObjectArg<TreasuryCap<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "supply_mut").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::take<T0>(arg0: &mut 0x2::balance::Balance<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn take<T0>(arg0: sm::balance::Balance<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "take").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::total_supply<T0>(arg0: &0x2::coin::TreasuryCap<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn total_supply<T0>(arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "total_supply")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::treasury_into_supply<T0>(arg0: 0x2::coin::TreasuryCap<T0>): 0x2::balance::Supply<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn treasury_into_supply<T0>(
        arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "treasury_into_supply")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::update_description<T0>(arg0: &0x2::coin::TreasuryCap<T0>, arg1: &mut 0x2::coin::CoinMetadata<T0>, arg2: 0x1::string::String)`
    #[must_use]
    pub fn update_description<T0>(
        arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: &mut impl sm_call::ObjectArg<CoinMetadata<T0>>,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "update_description")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::update_icon_url<T0>(arg0: &0x2::coin::TreasuryCap<T0>, arg1: &mut 0x2::coin::CoinMetadata<T0>, arg2: 0x1::ascii::String)`
    #[must_use]
    pub fn update_icon_url<T0>(
        arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: &mut impl sm_call::ObjectArg<CoinMetadata<T0>>,
        arg2: sm::ascii::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "update_icon_url")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::update_name<T0>(arg0: &0x2::coin::TreasuryCap<T0>, arg1: &mut 0x2::coin::CoinMetadata<T0>, arg2: 0x1::string::String)`
    #[must_use]
    pub fn update_name<T0>(
        arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: &mut impl sm_call::ObjectArg<CoinMetadata<T0>>,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "update_name").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun coin::update_symbol<T0>(arg0: &0x2::coin::TreasuryCap<T0>, arg1: &mut 0x2::coin::CoinMetadata<T0>, arg2: 0x1::ascii::String)`
    #[must_use]
    pub fn update_symbol<T0>(
        arg0: &impl sm_call::ObjectArg<TreasuryCap<T0>>,
        arg1: &mut impl sm_call::ObjectArg<CoinMetadata<T0>>,
        arg2: sm::ascii::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "coin", "update_symbol")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::value<T0>(arg0: &0x2::coin::Coin<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn value<T0>(arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "value").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun coin::zero<T0>(arg0: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn zero<T0>() -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "coin", "zero").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec
    }
}

pub mod config {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::config::Config`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "config", abilities = "key", phantoms = "T0")]
    pub struct Config<T0> {
        pub id: sm::types::UID,
    }
    /// Move type: `0x2::config::Setting`.
    /// Abilities: `store, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "config",
        abilities = "store, drop",
        type_abilities = "T0: store, copy, drop"
    )]
    pub struct Setting<T0> {
        pub data: sm::containers::MoveOption<SettingData<T0>>,
    }
    /// Move type: `0x2::config::SettingData`.
    /// Abilities: `store, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "config",
        abilities = "store, drop",
        type_abilities = "T0: store, copy, drop"
    )]
    pub struct SettingData<T0> {
        pub newer_value_epoch: u64,
        pub newer_value: sm::containers::MoveOption<T0>,
        pub older_value_opt: sm::containers::MoveOption<T0>,
    }
}

pub mod deny_list {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::deny_list::AddressKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "store, copy, drop")]
    pub struct AddressKey {
        pub pos0: sm::prelude::Address,
    }
    /// Move type: `0x2::deny_list::ConfigKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "store, copy, drop")]
    pub struct ConfigKey {
        pub per_type_index: u64,
        pub per_type_key: Vec<u8>,
    }
    /// Move type: `0x2::deny_list::ConfigWriteCap`.
    /// Abilities: `drop`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "drop")]
    pub struct ConfigWriteCap {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::deny_list::DenyList`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "key")]
    pub struct DenyList {
        pub id: sm::types::UID,
        pub lists: sm::bag::Bag,
    }
    /// Move type: `0x2::deny_list::GlobalPauseKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "store, copy, drop")]
    pub struct GlobalPauseKey {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::deny_list::PerTypeConfigCreated`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "store, copy, drop")]
    pub struct PerTypeConfigCreated {
        pub key: ConfigKey,
        pub config_id: sm::types::ID,
    }
    /// Move type: `0x2::deny_list::PerTypeList`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "deny_list", abilities = "key, store")]
    pub struct PerTypeList {
        pub id: sm::types::UID,
        pub denied_count: sm::containers::Table<sm::prelude::Address, u64>,
        pub denied_addresses:
            sm::containers::Table<Vec<u8>, sm::vec_set::VecSet<sm::prelude::Address>>,
    }
}

pub mod derived_object {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::derived_object::Claimed`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "derived_object",
        abilities = "store, copy, drop"
    )]
    pub struct Claimed {
        pub pos0: sm::types::ID,
    }
    /// Move type: `0x2::derived_object::ClaimedStatus`.
    /// Abilities: `store`.
    #[derive(
        ::core::fmt::Debug,
        ::core::cmp::PartialEq,
        ::core::cmp::Eq,
        sm::__private::serde::Serialize,
        sm::__private::serde::Deserialize,
    )]
    #[serde(crate = "sui_move::__private::serde")]
    pub enum ClaimedStatus {
        Reserved,
    }

    impl sm::MoveType for ClaimedStatus {
        fn type_tag_static() -> sm::__private::sui_sdk_types::TypeTag {
            sm::__private::sui_sdk_types::TypeTag::Struct(Box::new(
                <Self as sm::MoveStruct>::struct_tag_static(),
            ))
        }
    }

    impl sm::MoveStruct for ClaimedStatus {
        fn struct_tag_static() -> sm::__private::sui_sdk_types::StructTag {
            sm::__private::sui_sdk_types::StructTag::new(
                sm::parse_address("0x2").expect("invalid address literal"),
                sm::parse_identifier("derived_object").expect("invalid module"),
                sm::parse_identifier("ClaimedStatus").expect("invalid struct name"),
                vec![],
            )
        }
    }

    impl sm::HasStore for ClaimedStatus {}
    /// Move type: `0x2::derived_object::DerivedObjectKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "derived_object",
        abilities = "store, copy, drop",
        type_abilities = "T0: store, copy, drop"
    )]
    pub struct DerivedObjectKey<T0> {
        pub pos0: T0,
    }
    /// Move: `public fun derived_object::claim<T0: store + copy + drop>(arg0: &mut 0x2::object::UID, arg1: T0): 0x2::object::UID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn claim<T0>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "derived_object", "claim")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun derived_object::derive_address<T0: store + copy + drop>(arg0: 0x2::object::ID, arg1: T0): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn derive_address<T0>(arg0: sm::types::ID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "derived_object", "derive_address")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun derived_object::exists<T0: store + copy + drop>(arg0: &0x2::object::UID, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn exists<T0>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "derived_object", "exists")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod display {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::display::Display`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "display",
        abilities = "key, store",
        phantoms = "T0",
        type_abilities = "T0: key"
    )]
    pub struct Display<T0> {
        pub id: sm::types::UID,
        pub fields: sm::vec_map::VecMap<sm::string::String, sm::string::String>,
        pub version: u16,
    }
    /// Move type: `0x2::display::DisplayCreated`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "display",
        abilities = "copy, drop",
        phantoms = "T0",
        type_abilities = "T0: key"
    )]
    pub struct DisplayCreated<T0> {
        pub id: sm::types::ID,
    }
    /// Move type: `0x2::display::VersionUpdated`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "display",
        abilities = "copy, drop",
        phantoms = "T0",
        type_abilities = "T0: key"
    )]
    pub struct VersionUpdated<T0> {
        pub id: sm::types::ID,
        pub version: u16,
        pub fields: sm::vec_map::VecMap<sm::string::String, sm::string::String>,
    }
    /// Move: `public entry fun display::add<T0: key>(arg0: &mut 0x2::display::Display<T0>, arg1: 0x1::string::String, arg2: 0x1::string::String)`
    #[must_use]
    pub fn add<T0>(
        arg0: &mut impl sm_call::ObjectArg<Display<T0>>,
        arg1: sm::string::String,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "display", "add").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun display::add_multiple<T0: key>(arg0: &mut 0x2::display::Display<T0>, arg1: vector<0x1::string::String>, arg2: vector<0x1::string::String>)`
    #[must_use]
    pub fn add_multiple<T0>(
        arg0: &mut impl sm_call::ObjectArg<Display<T0>>,
        arg1: Vec<sm::string::String>,
        arg2: Vec<sm::string::String>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "display", "add_multiple")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun display::create_and_keep<T0: key>(arg0: &0x2::package::Publisher, arg1: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    #[must_use]
    pub fn create_and_keep<T0>(
        arg0: &impl sm_call::ObjectArg<super::package::Publisher>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "display", "create_and_keep")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun display::edit<T0: key>(arg0: &mut 0x2::display::Display<T0>, arg1: 0x1::string::String, arg2: 0x1::string::String)`
    #[must_use]
    pub fn edit<T0>(
        arg0: &mut impl sm_call::ObjectArg<Display<T0>>,
        arg1: sm::string::String,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "display", "edit").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun display::fields<T0: key>(arg0: &0x2::display::Display<T0>): &0x2::vec_map::VecMap<0x1::string::String, 0x1::string::String>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn fields<T0>(arg0: &impl sm_call::ObjectArg<Display<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "display", "fields").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun display::is_authorized<T0: key>(arg0: &0x2::package::Publisher): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_authorized<T0>(
        arg0: &impl sm_call::ObjectArg<super::package::Publisher>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "display", "is_authorized")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun display::new<T0: key>(arg0: &0x2::package::Publisher, arg1: &mut 0x2::tx_context::TxContext): 0x2::display::Display<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0>(arg0: &impl sm_call::ObjectArg<super::package::Publisher>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "display", "new").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun display::new_with_fields<T0: key>(arg0: &0x2::package::Publisher, arg1: vector<0x1::string::String>, arg2: vector<0x1::string::String>, arg3: &mut 0x2::tx_context::TxContext): 0x2::display::Display<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_with_fields<T0>(
        arg0: &impl sm_call::ObjectArg<super::package::Publisher>,
        arg1: Vec<sm::string::String>,
        arg2: Vec<sm::string::String>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "display", "new_with_fields")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun display::remove<T0: key>(arg0: &mut 0x2::display::Display<T0>, arg1: 0x1::string::String)`
    #[must_use]
    pub fn remove<T0>(
        arg0: &mut impl sm_call::ObjectArg<Display<T0>>,
        arg1: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "display", "remove").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public entry fun display::update_version<T0: key>(arg0: &mut 0x2::display::Display<T0>)`
    #[must_use]
    pub fn update_version<T0>(arg0: &mut impl sm_call::ObjectArg<Display<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "display", "update_version")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun display::version<T0: key>(arg0: &0x2::display::Display<T0>): u16`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn version<T0>(arg0: &impl sm_call::ObjectArg<Display<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "display", "version").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
}

pub mod dynamic_field {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::dynamic_field::Field`.
    /// Abilities: `key`.
    #[sm::move_struct(
        address = "0x2",
        module = "dynamic_field",
        abilities = "key",
        type_abilities = "T0: store, copy, drop; T1: store"
    )]
    pub struct Field<T0, T1> {
        pub id: sm::types::UID,
        pub name: T0,
        pub value: T1,
    }
    /// Move: `public fun dynamic_field::add<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::object::UID, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0, T1>(arg0: sm::types::UID, arg1: T0, arg2: T1) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "add")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_field::borrow<T0: store + copy + drop, T1: store>(arg0: &0x2::object::UID, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "borrow")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_field::borrow_mut<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::object::UID, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "borrow_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_field::exists_<T0: store + copy + drop>(arg0: &0x2::object::UID, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn exists_<T0>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "exists_")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_field::exists_with_type<T0: store + copy + drop, T1: store>(arg0: &0x2::object::UID, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn exists_with_type<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "exists_with_type")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_field::remove<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::object::UID, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_field::remove_if_exists<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::object::UID, arg1: T0): 0x1::option::Option<T1>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove_if_exists<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_field", "remove_if_exists")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod dynamic_object_field {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::dynamic_object_field::Wrapper`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "dynamic_object_field",
        abilities = "store, copy, drop"
    )]
    pub struct Wrapper<T0> {
        pub name: T0,
    }
    /// Move: `public fun dynamic_object_field::add<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object::UID, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0, T1>(
        arg0: sm::types::UID,
        arg1: T0,
        arg2: &impl sm_call::ObjectArg<T1>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "add")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_object_field::borrow<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object::UID, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "borrow")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_object_field::borrow_mut<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object::UID, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "borrow_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_object_field::exists_<T0: store + copy + drop>(arg0: &0x2::object::UID, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn exists_<T0>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "exists_")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_object_field::exists_with_type<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object::UID, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn exists_with_type<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "exists_with_type")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_object_field::id<T0: store + copy + drop>(arg0: &0x2::object::UID, arg1: T0): 0x1::option::Option<0x2::object::ID>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id<T0>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "id")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun dynamic_object_field::remove<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object::UID, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(arg0: sm::types::UID, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "dynamic_object_field", "remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod ecdsa_k1 {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun ecdsa_k1::decompress_pubkey(arg0: &vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn decompress_pubkey(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ecdsa_k1", "decompress_pubkey")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun ecdsa_k1::secp256k1_ecrecover(arg0: &vector<u8>, arg1: &vector<u8>, arg2: u8): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn secp256k1_ecrecover(arg0: Vec<u8>, arg1: Vec<u8>, arg2: u8) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ecdsa_k1", "secp256k1_ecrecover")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun ecdsa_k1::secp256k1_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>, arg3: u8): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn secp256k1_verify(
        arg0: Vec<u8>,
        arg1: Vec<u8>,
        arg2: Vec<u8>,
        arg3: u8,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ecdsa_k1", "secp256k1_verify")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
}

pub mod ecdsa_r1 {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun ecdsa_r1::secp256r1_ecrecover(arg0: &vector<u8>, arg1: &vector<u8>, arg2: u8): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn secp256r1_ecrecover(arg0: Vec<u8>, arg1: Vec<u8>, arg2: u8) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ecdsa_r1", "secp256r1_ecrecover")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun ecdsa_r1::secp256r1_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>, arg3: u8): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn secp256r1_verify(
        arg0: Vec<u8>,
        arg1: Vec<u8>,
        arg2: Vec<u8>,
        arg3: u8,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ecdsa_r1", "secp256r1_verify")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
}

pub mod ecvrf {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun ecvrf::ecvrf_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>, arg3: &vector<u8>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn ecvrf_verify(
        arg0: Vec<u8>,
        arg1: Vec<u8>,
        arg2: Vec<u8>,
        arg3: Vec<u8>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ecvrf", "ecvrf_verify")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
}

pub mod ed25519 {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun ed25519::ed25519_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn ed25519_verify(arg0: Vec<u8>, arg1: Vec<u8>, arg2: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "ed25519", "ed25519_verify")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
}

pub mod event {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

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
    /// Move: `public fun event::emit_authenticated<T0: copy + drop>(arg0: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn emit_authenticated<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "event", "emit_authenticated")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod funds_accumulator {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::funds_accumulator::Withdrawal`.
    /// Abilities: `drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "funds_accumulator",
        abilities = "drop",
        phantoms = "T0",
        type_abilities = "T0: store"
    )]
    pub struct Withdrawal<T0> {
        pub owner: sm::prelude::Address,
        pub limit: u64,
    }
    /// Move: `public fun funds_accumulator::withdrawal_join<T0: store>(arg0: &mut 0x2::funds_accumulator::Withdrawal<T0>, arg1: 0x2::funds_accumulator::Withdrawal<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdrawal_join<T0>(arg0: Withdrawal<T0>, arg1: Withdrawal<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "funds_accumulator", "withdrawal_join")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun funds_accumulator::withdrawal_limit<T0: store>(arg0: &0x2::funds_accumulator::Withdrawal<T0>): u256`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdrawal_limit<T0>(arg0: Withdrawal<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "funds_accumulator", "withdrawal_limit")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun funds_accumulator::withdrawal_owner<T0: store>(arg0: &0x2::funds_accumulator::Withdrawal<T0>): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdrawal_owner<T0>(arg0: Withdrawal<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "funds_accumulator", "withdrawal_owner")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun funds_accumulator::withdrawal_split<T0: store>(arg0: &mut 0x2::funds_accumulator::Withdrawal<T0>, arg1: u256): 0x2::funds_accumulator::Withdrawal<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdrawal_split<T0>(arg0: Withdrawal<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "funds_accumulator", "withdrawal_split")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod groth16 {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::groth16::Curve`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "groth16", abilities = "store, copy, drop")]
    pub struct Curve {
        pub id: u8,
    }
    /// Move type: `0x2::groth16::PreparedVerifyingKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "groth16", abilities = "store, copy, drop")]
    pub struct PreparedVerifyingKey {
        pub vk_gamma_abc_g1_bytes: Vec<u8>,
        pub alpha_g1_beta_g2_bytes: Vec<u8>,
        pub gamma_g2_neg_pc_bytes: Vec<u8>,
        pub delta_g2_neg_pc_bytes: Vec<u8>,
    }
    /// Move type: `0x2::groth16::ProofPoints`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "groth16", abilities = "store, copy, drop")]
    pub struct ProofPoints {
        pub bytes: Vec<u8>,
    }
    /// Move type: `0x2::groth16::PublicProofInputs`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "groth16", abilities = "store, copy, drop")]
    pub struct PublicProofInputs {
        pub bytes: Vec<u8>,
    }
    /// Move: `public fun groth16::bls12381(): 0x2::groth16::Curve`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn bls12381() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "groth16", "bls12381").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun groth16::bn254(): 0x2::groth16::Curve`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn bn254() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "groth16", "bn254").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun groth16::prepare_verifying_key(arg0: &0x2::groth16::Curve, arg1: &vector<u8>): 0x2::groth16::PreparedVerifyingKey`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn prepare_verifying_key(arg0: Curve, arg1: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "groth16", "prepare_verifying_key")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun groth16::proof_points_from_bytes(arg0: vector<u8>): 0x2::groth16::ProofPoints`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn proof_points_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "groth16", "proof_points_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun groth16::public_proof_inputs_from_bytes(arg0: vector<u8>): 0x2::groth16::PublicProofInputs`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_proof_inputs_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "groth16", "public_proof_inputs_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun groth16::pvk_from_bytes(arg0: vector<u8>, arg1: vector<u8>, arg2: vector<u8>, arg3: vector<u8>): 0x2::groth16::PreparedVerifyingKey`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pvk_from_bytes(
        arg0: Vec<u8>,
        arg1: Vec<u8>,
        arg2: Vec<u8>,
        arg3: Vec<u8>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "groth16", "pvk_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun groth16::pvk_to_bytes(arg0: 0x2::groth16::PreparedVerifyingKey): vector<vector<u8>>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pvk_to_bytes(arg0: PreparedVerifyingKey) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "groth16", "pvk_to_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun groth16::verify_groth16_proof(arg0: &0x2::groth16::Curve, arg1: &0x2::groth16::PreparedVerifyingKey, arg2: &0x2::groth16::PublicProofInputs, arg3: &0x2::groth16::ProofPoints): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn verify_groth16_proof(
        arg0: Curve,
        arg1: PreparedVerifyingKey,
        arg2: PublicProofInputs,
        arg3: ProofPoints,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "groth16", "verify_groth16_proof")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
}

pub mod group_ops {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::group_ops::Element`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "group_ops",
        abilities = "store, copy, drop",
        phantoms = "T0"
    )]
    pub struct Element<T0> {
        pub bytes: Vec<u8>,
    }
    /// Move: `public fun group_ops::bytes<T0>(arg0: &0x2::group_ops::Element<T0>): &vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn bytes<T0>(arg0: Element<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "group_ops", "bytes").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun group_ops::equal<T0>(arg0: &0x2::group_ops::Element<T0>, arg1: &0x2::group_ops::Element<T0>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn equal<T0>(arg0: Element<T0>, arg1: Element<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "group_ops", "equal").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod hash {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun hash::blake2b256(arg0: &vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn blake2b256(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "hash", "blake2b256").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun hash::keccak256(arg0: &vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn keccak256(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "hash", "keccak256").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod hex {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun hex::decode(arg0: vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn decode(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "hex", "decode").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun hex::encode(arg0: vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn encode(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "hex", "encode").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod hmac {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun hmac::hmac_sha3_256(arg0: &vector<u8>, arg1: &vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn hmac_sha3_256(arg0: Vec<u8>, arg1: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "hmac", "hmac_sha3_256")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod kiosk {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::kiosk::Borrow`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "kiosk")]
    pub struct Borrow {
        pub kiosk_id: sm::types::ID,
        pub item_id: sm::types::ID,
    }
    /// Move type: `0x2::kiosk::Item`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "kiosk", abilities = "store, copy, drop")]
    pub struct Item {
        pub id: sm::types::ID,
    }
    /// Move type: `0x2::kiosk::ItemDelisted`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "kiosk",
        abilities = "copy, drop",
        phantoms = "T0",
        type_abilities = "T0: key, store"
    )]
    pub struct ItemDelisted<T0> {
        pub kiosk: sm::types::ID,
        pub id: sm::types::ID,
    }
    /// Move type: `0x2::kiosk::ItemListed`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "kiosk",
        abilities = "copy, drop",
        phantoms = "T0",
        type_abilities = "T0: key, store"
    )]
    pub struct ItemListed<T0> {
        pub kiosk: sm::types::ID,
        pub id: sm::types::ID,
        pub price: u64,
    }
    /// Move type: `0x2::kiosk::ItemPurchased`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "kiosk",
        abilities = "copy, drop",
        phantoms = "T0",
        type_abilities = "T0: key, store"
    )]
    pub struct ItemPurchased<T0> {
        pub kiosk: sm::types::ID,
        pub id: sm::types::ID,
        pub price: u64,
    }
    /// Move type: `0x2::kiosk::Kiosk`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "kiosk", abilities = "key, store")]
    pub struct Kiosk {
        pub id: sm::types::UID,
        pub profits: sm::balance::Balance<sm::sui::SUI>,
        pub owner: sm::prelude::Address,
        pub item_count: u32,
        pub allow_extensions: bool,
    }
    /// Move type: `0x2::kiosk::KioskOwnerCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "kiosk", abilities = "key, store")]
    pub struct KioskOwnerCap {
        pub id: sm::types::UID,
        pub r#for: sm::types::ID,
    }
    /// Move type: `0x2::kiosk::Listing`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "kiosk", abilities = "store, copy, drop")]
    pub struct Listing {
        pub id: sm::types::ID,
        pub is_exclusive: bool,
    }
    /// Move type: `0x2::kiosk::Lock`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "kiosk", abilities = "store, copy, drop")]
    pub struct Lock {
        pub id: sm::types::ID,
    }
    /// Move type: `0x2::kiosk::PurchaseCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "kiosk",
        abilities = "key, store",
        phantoms = "T0",
        type_abilities = "T0: key, store"
    )]
    pub struct PurchaseCap<T0> {
        pub id: sm::types::UID,
        pub kiosk_id: sm::types::ID,
        pub item_id: sm::types::ID,
        pub min_price: u64,
    }
    /// Move: `public fun kiosk::borrow<T0: key + store>(arg0: &0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID): &T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0>(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "borrow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::borrow_mut<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID): &mut T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "borrow_mut").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::borrow_val<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID): (T0, 0x2::kiosk::Borrow)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_val<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "borrow_val").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::close_and_withdraw(arg0: 0x2::kiosk::Kiosk, arg1: 0x2::kiosk::KioskOwnerCap, arg2: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<0x2::sui::SUI>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn close_and_withdraw(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "close_and_withdraw")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::delist<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn delist<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "delist").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::has_access(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn has_access(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "has_access").expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::has_item(arg0: &0x2::kiosk::Kiosk, arg1: 0x2::object::ID): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn has_item(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: sm::types::ID,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "has_item").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::has_item_with_type<T0: key + store>(arg0: &0x2::kiosk::Kiosk, arg1: 0x2::object::ID): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn has_item_with_type<T0>(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: sm::types::ID,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "has_item_with_type")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::is_listed(arg0: &0x2::kiosk::Kiosk, arg1: 0x2::object::ID): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_listed(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: sm::types::ID,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "is_listed").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::is_listed_exclusively(arg0: &0x2::kiosk::Kiosk, arg1: 0x2::object::ID): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_listed_exclusively(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: sm::types::ID,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "is_listed_exclusively")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::is_locked(arg0: &0x2::kiosk::Kiosk, arg1: 0x2::object::ID): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_locked(
        arg0: &impl sm_call::ObjectArg<Kiosk>,
        arg1: sm::types::ID,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "is_locked").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::item_count(arg0: &0x2::kiosk::Kiosk): u32`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn item_count(arg0: &impl sm_call::ObjectArg<Kiosk>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "item_count").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::kiosk_owner_cap_for(arg0: &0x2::kiosk::KioskOwnerCap): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn kiosk_owner_cap_for(arg0: &impl sm_call::ObjectArg<KioskOwnerCap>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "kiosk_owner_cap_for")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::list<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID, arg3: u64)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn list<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
        arg3: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "list").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::list_with_purchase_cap<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID, arg3: u64, arg4: &mut 0x2::tx_context::TxContext): 0x2::kiosk::PurchaseCap<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn list_with_purchase_cap<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
        arg3: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "list_with_purchase_cap")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::lock<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: &0x2::transfer_policy::TransferPolicy<T0>, arg3: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn lock<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: &impl sm_call::ObjectArg<super::transfer_policy::TransferPolicy<T0>>,
        arg3: &impl sm_call::ObjectArg<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "lock").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::new(arg0: &mut 0x2::tx_context::TxContext): (0x2::kiosk::Kiosk, 0x2::kiosk::KioskOwnerCap)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "new").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun kiosk::owner(arg0: &0x2::kiosk::Kiosk): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn owner(arg0: &impl sm_call::ObjectArg<Kiosk>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "owner").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::place<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn place<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: &impl sm_call::ObjectArg<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "place").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::place_and_list<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: T0, arg3: u64)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn place_and_list<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: &impl sm_call::ObjectArg<T0>,
        arg3: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "place_and_list")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::profits_amount(arg0: &0x2::kiosk::Kiosk): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn profits_amount(arg0: &impl sm_call::ObjectArg<Kiosk>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "profits_amount")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::profits_mut(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap): &mut 0x2::balance::Balance<0x2::sui::SUI>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn profits_mut(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "profits_mut")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::purchase<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: 0x2::object::ID, arg2: 0x2::coin::Coin<0x2::sui::SUI>): (T0, 0x2::transfer_policy::TransferRequest<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn purchase<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: sm::types::ID,
        arg2: &impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "purchase").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::purchase_cap_item<T0: key + store>(arg0: &0x2::kiosk::PurchaseCap<T0>): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn purchase_cap_item<T0>(
        arg0: &impl sm_call::ObjectArg<PurchaseCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "purchase_cap_item")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::purchase_cap_kiosk<T0: key + store>(arg0: &0x2::kiosk::PurchaseCap<T0>): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn purchase_cap_kiosk<T0>(
        arg0: &impl sm_call::ObjectArg<PurchaseCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "purchase_cap_kiosk")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::purchase_cap_min_price<T0: key + store>(arg0: &0x2::kiosk::PurchaseCap<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn purchase_cap_min_price<T0>(
        arg0: &impl sm_call::ObjectArg<PurchaseCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "purchase_cap_min_price")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::purchase_with_cap<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: 0x2::kiosk::PurchaseCap<T0>, arg2: 0x2::coin::Coin<0x2::sui::SUI>): (T0, 0x2::transfer_policy::TransferRequest<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn purchase_with_cap<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<PurchaseCap<T0>>,
        arg2: &impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "purchase_with_cap")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::return_purchase_cap<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: 0x2::kiosk::PurchaseCap<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn return_purchase_cap<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<PurchaseCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "return_purchase_cap")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::return_val<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: T0, arg2: 0x2::kiosk::Borrow)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn return_val<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<T0>,
        arg2: Borrow,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "return_val").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::set_allow_extensions(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: bool)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn set_allow_extensions(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: bool,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "set_allow_extensions")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::set_owner(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: &0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn set_owner(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "set_owner").expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::set_owner_custom(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: address)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn set_owner_custom(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "set_owner_custom")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::take<T0: key + store>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x2::object::ID): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn take<T0>(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::types::ID,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "take").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::uid(arg0: &0x2::kiosk::Kiosk): &0x2::object::UID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid(arg0: &impl sm_call::ObjectArg<Kiosk>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "uid").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::uid_mut(arg0: &mut 0x2::kiosk::Kiosk): &mut 0x2::object::UID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_mut(arg0: &mut impl sm_call::ObjectArg<Kiosk>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "uid_mut").expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::uid_mut_as_owner(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap): &mut 0x2::object::UID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_mut_as_owner(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk", "uid_mut_as_owner")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk::withdraw(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap, arg2: 0x1::option::Option<u64>, arg3: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<0x2::sui::SUI>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdraw(
        arg0: &mut impl sm_call::ObjectArg<Kiosk>,
        arg1: &impl sm_call::ObjectArg<KioskOwnerCap>,
        arg2: sm::containers::MoveOption<u64>,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "kiosk", "withdraw").expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
}

pub mod kiosk_extension {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::kiosk_extension::Extension`.
    /// Abilities: `store`.
    #[sm::move_struct(address = "0x2", module = "kiosk_extension", abilities = "store")]
    pub struct Extension {
        pub storage: sm::bag::Bag,
        pub permissions: u128,
        pub is_enabled: bool,
    }
    /// Move type: `0x2::kiosk_extension::ExtensionKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "kiosk_extension",
        abilities = "store, copy, drop",
        phantoms = "T0"
    )]
    pub struct ExtensionKey<T0> {
        pub dummy_field: bool,
    }
    /// Move: `public fun kiosk_extension::add<T0: drop>(arg0: T0, arg1: &mut 0x2::kiosk::Kiosk, arg2: &0x2::kiosk::KioskOwnerCap, arg3: u128, arg4: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0>(
        arg0: T0,
        arg1: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
        arg2: &impl sm_call::ObjectArg<super::kiosk::KioskOwnerCap>,
        arg3: u128,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "add")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::can_lock<T0: drop>(arg0: &0x2::kiosk::Kiosk): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn can_lock<T0>(arg0: &impl sm_call::ObjectArg<super::kiosk::Kiosk>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "can_lock")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::can_place<T0: drop>(arg0: &0x2::kiosk::Kiosk): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn can_place<T0>(arg0: &impl sm_call::ObjectArg<super::kiosk::Kiosk>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "can_place")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::disable<T0: drop>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn disable<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
        arg1: &impl sm_call::ObjectArg<super::kiosk::KioskOwnerCap>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "disable")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::enable<T0: drop>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn enable<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
        arg1: &impl sm_call::ObjectArg<super::kiosk::KioskOwnerCap>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "enable")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::is_enabled<T0: drop>(arg0: &0x2::kiosk::Kiosk): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_enabled<T0>(arg0: &impl sm_call::ObjectArg<super::kiosk::Kiosk>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "is_enabled")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::is_installed<T0: drop>(arg0: &0x2::kiosk::Kiosk): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_installed<T0>(
        arg0: &impl sm_call::ObjectArg<super::kiosk::Kiosk>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "is_installed")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::lock<T0: drop, T1: key + store>(arg0: T0, arg1: &mut 0x2::kiosk::Kiosk, arg2: T1, arg3: &0x2::transfer_policy::TransferPolicy<T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn lock<T0, T1>(
        arg0: T0,
        arg1: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
        arg2: &impl sm_call::ObjectArg<T1>,
        arg3: &impl sm_call::ObjectArg<super::transfer_policy::TransferPolicy<T1>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "lock")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::place<T0: drop, T1: key + store>(arg0: T0, arg1: &mut 0x2::kiosk::Kiosk, arg2: T1, arg3: &0x2::transfer_policy::TransferPolicy<T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn place<T0, T1>(
        arg0: T0,
        arg1: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
        arg2: &impl sm_call::ObjectArg<T1>,
        arg3: &impl sm_call::ObjectArg<super::transfer_policy::TransferPolicy<T1>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "place")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::remove<T0: drop>(arg0: &mut 0x2::kiosk::Kiosk, arg1: &0x2::kiosk::KioskOwnerCap)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
        arg1: &impl sm_call::ObjectArg<super::kiosk::KioskOwnerCap>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::storage<T0: drop>(arg0: T0, arg1: &0x2::kiosk::Kiosk): &0x2::bag::Bag`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn storage<T0>(
        arg0: T0,
        arg1: &impl sm_call::ObjectArg<super::kiosk::Kiosk>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "storage")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun kiosk_extension::storage_mut<T0: drop>(arg0: T0, arg1: &mut 0x2::kiosk::Kiosk): &mut 0x2::bag::Bag`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn storage_mut<T0>(
        arg0: T0,
        arg1: &mut impl sm_call::ObjectArg<super::kiosk::Kiosk>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "kiosk_extension", "storage_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec
    }
}

pub mod linked_table {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::linked_table::LinkedTable`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "linked_table",
        abilities = "key, store",
        phantoms = "T1",
        type_abilities = "T0: store, copy, drop; T1: store"
    )]
    pub struct LinkedTable<T0, T1> {
        pub id: sm::types::UID,
        pub size: u64,
        pub head: sm::containers::MoveOption<T0>,
        pub tail: sm::containers::MoveOption<T0>,
    }
    /// Move type: `0x2::linked_table::Node`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "linked_table",
        abilities = "store",
        type_abilities = "T0: store, copy, drop; T1: store"
    )]
    pub struct Node<T0, T1> {
        pub prev: sm::containers::MoveOption<T0>,
        pub next: sm::containers::MoveOption<T0>,
        pub value: T1,
    }
    /// Move: `public fun linked_table::back<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>): &0x1::option::Option<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn back<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "back")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::borrow<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(
        arg0: sm::linked_table::LinkedTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "borrow")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::borrow_mut<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::linked_table::LinkedTable<T0, T1>, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(
        arg0: sm::linked_table::LinkedTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "borrow_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::contains<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains<T0, T1>(
        arg0: sm::linked_table::LinkedTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "contains")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::destroy_empty<T0: store + copy + drop, T1: store>(arg0: 0x2::linked_table::LinkedTable<T0, T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_empty<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "destroy_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::drop<T0: store + copy + drop, T1: store + drop>(arg0: 0x2::linked_table::LinkedTable<T0, T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn drop<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "drop")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::front<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>): &0x1::option::Option<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn front<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "front")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::is_empty<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "is_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::length<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "length")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::new<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::tx_context::TxContext): 0x2::linked_table::LinkedTable<T0, T1>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0, T1>() -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "linked_table", "new").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec
    }
    /// Move: `public fun linked_table::next<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>, arg1: T0): &0x1::option::Option<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn next<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "next")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::pop_back<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::linked_table::LinkedTable<T0, T1>): (T0, T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pop_back<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "pop_back")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::pop_front<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::linked_table::LinkedTable<T0, T1>): (T0, T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pop_front<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "pop_front")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::prev<T0: store + copy + drop, T1: store>(arg0: &0x2::linked_table::LinkedTable<T0, T1>, arg1: T0): &0x1::option::Option<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn prev<T0, T1>(arg0: sm::linked_table::LinkedTable<T0, T1>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "prev")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::push_back<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::linked_table::LinkedTable<T0, T1>, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn push_back<T0, T1>(
        arg0: sm::linked_table::LinkedTable<T0, T1>,
        arg1: T0,
        arg2: T1,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "push_back")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::push_front<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::linked_table::LinkedTable<T0, T1>, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn push_front<T0, T1>(
        arg0: sm::linked_table::LinkedTable<T0, T1>,
        arg1: T0,
        arg2: T1,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "push_front")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun linked_table::remove<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::linked_table::LinkedTable<T0, T1>, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(
        arg0: sm::linked_table::LinkedTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "linked_table", "remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod math {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun math::diff(arg0: u64, arg1: u64): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn diff(arg0: u64, arg1: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "math", "diff").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun math::divide_and_round_up(arg0: u64, arg1: u64): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn divide_and_round_up(arg0: u64, arg1: u64) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "math", "divide_and_round_up")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun math::max(arg0: u64, arg1: u64): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn max(arg0: u64, arg1: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "math", "max").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun math::min(arg0: u64, arg1: u64): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn min(arg0: u64, arg1: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "math", "min").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun math::pow(arg0: u64, arg1: u8): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pow(arg0: u64, arg1: u8) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "math", "pow").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun math::sqrt(arg0: u64): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn sqrt(arg0: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "math", "sqrt").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun math::sqrt_u128(arg0: u128): u128`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn sqrt_u128(arg0: u128) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "math", "sqrt_u128").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod nitro_attestation {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::nitro_attestation::NitroAttestationDocument`.
    /// Abilities: `drop`.
    #[sm::move_struct(address = "0x2", module = "nitro_attestation", abilities = "drop")]
    pub struct NitroAttestationDocument {
        pub module_id: Vec<u8>,
        pub timestamp: u64,
        pub digest: Vec<u8>,
        pub pcrs: Vec<PCREntry>,
        pub public_key: sm::containers::MoveOption<Vec<u8>>,
        pub user_data: sm::containers::MoveOption<Vec<u8>>,
        pub nonce: sm::containers::MoveOption<Vec<u8>>,
    }
    /// Move type: `0x2::nitro_attestation::PCREntry`.
    /// Abilities: `drop`.
    #[sm::move_struct(address = "0x2", module = "nitro_attestation", abilities = "drop")]
    pub struct PCREntry {
        pub index: u8,
        pub value: Vec<u8>,
    }
    /// Move: `public fun nitro_attestation::digest(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn digest(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "digest")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::index(arg0: &0x2::nitro_attestation::PCREntry): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn index(arg0: PCREntry) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "index")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::module_id(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn module_id(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "module_id")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::nonce(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &0x1::option::Option<vector<u8>>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn nonce(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "nonce")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::pcrs(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &vector<0x2::nitro_attestation::PCREntry>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pcrs(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "pcrs")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::public_key(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &0x1::option::Option<vector<u8>>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_key(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "public_key")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::timestamp(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn timestamp(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "timestamp")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::user_data(arg0: &0x2::nitro_attestation::NitroAttestationDocument): &0x1::option::Option<vector<u8>>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn user_data(arg0: NitroAttestationDocument) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "user_data")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun nitro_attestation::value(arg0: &0x2::nitro_attestation::PCREntry): &vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn value(arg0: PCREntry) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "nitro_attestation", "value")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod object {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::object::ID`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "object", abilities = "store, copy, drop")]
    pub struct ID {
        pub bytes: sm::prelude::Address,
    }
    /// Move type: `0x2::object::UID`.
    /// Abilities: `store`.
    #[sm::move_struct(address = "0x2", module = "object", abilities = "store")]
    pub struct UID {
        pub id: sm::types::ID,
    }
    /// Move: `public fun object::borrow_id<T0: key>(arg0: &T0): &0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_id<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object", "borrow_id").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::delete(arg0: 0x2::object::UID)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn delete(arg0: sm::types::UID) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object", "delete").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id<T0: key>(arg0: &T0): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object", "id").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id_address<T0: key>(arg0: &T0): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id_address<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "id_address")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id_bytes<T0: key>(arg0: &T0): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id_bytes<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object", "id_bytes").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id_from_address(arg0: address): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id_from_address(arg0: sm::prelude::Address) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "id_from_address")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id_from_bytes(arg0: vector<u8>): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "id_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id_to_address(arg0: &0x2::object::ID): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id_to_address(arg0: sm::types::ID) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "id_to_address")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::id_to_bytes(arg0: &0x2::object::ID): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn id_to_bytes(arg0: sm::types::ID) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "id_to_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::new(arg0: &mut 0x2::tx_context::TxContext): 0x2::object::UID`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object", "new").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun object::uid_as_inner(arg0: &0x2::object::UID): &0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_as_inner(arg0: sm::types::UID) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "uid_as_inner")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::uid_to_address(arg0: &0x2::object::UID): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_to_address(arg0: sm::types::UID) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "uid_to_address")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::uid_to_bytes(arg0: &0x2::object::UID): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_to_bytes(arg0: sm::types::UID) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "uid_to_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object::uid_to_inner(arg0: &0x2::object::UID): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_to_inner(arg0: sm::types::UID) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object", "uid_to_inner")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod object_bag {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::object_bag::ObjectBag`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "object_bag", abilities = "key, store")]
    pub struct ObjectBag {
        pub id: sm::types::UID,
        pub size: u64,
    }
    /// Move: `public fun object_bag::add<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object_bag::ObjectBag, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
        arg2: &impl sm_call::ObjectArg<T1>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object_bag", "add").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::borrow<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_bag::ObjectBag, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(
        arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "borrow")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::borrow_mut<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object_bag::ObjectBag, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "borrow_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::contains<T0: store + copy + drop>(arg0: &0x2::object_bag::ObjectBag, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains<T0>(
        arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "contains")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::contains_with_type<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_bag::ObjectBag, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains_with_type<T0, T1>(
        arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "contains_with_type")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::destroy_empty(arg0: 0x2::object_bag::ObjectBag)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_empty(
        arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "destroy_empty")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::is_empty(arg0: &0x2::object_bag::ObjectBag): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty(
        arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "is_empty")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::length(arg0: &0x2::object_bag::ObjectBag): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length(arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "length")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::new(arg0: &mut 0x2::tx_context::TxContext): 0x2::object_bag::ObjectBag`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object_bag", "new").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun object_bag::remove<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object_bag::ObjectBag, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_bag::value_id<T0: store + copy + drop>(arg0: &0x2::object_bag::ObjectBag, arg1: T0): 0x1::option::Option<0x2::object::ID>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn value_id<T0>(
        arg0: &impl sm_call::ObjectArg<sm::object_bag::ObjectBag>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_bag", "value_id")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod object_table {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::object_table::ObjectTable`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "object_table",
        abilities = "key, store",
        phantoms = "T0, T1",
        type_abilities = "T0: store, copy, drop; T1: key, store"
    )]
    pub struct ObjectTable<T0, T1> {
        pub id: sm::types::UID,
        pub size: u64,
    }
    /// Move: `public fun object_table::add<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object_table::ObjectTable<T0, T1>, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0, T1>(
        arg0: sm::object_table::ObjectTable<T0, T1>,
        arg1: T0,
        arg2: &impl sm_call::ObjectArg<T1>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object_table", "add").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::borrow<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_table::ObjectTable<T0, T1>, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(
        arg0: sm::object_table::ObjectTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "borrow")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::borrow_mut<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object_table::ObjectTable<T0, T1>, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(
        arg0: sm::object_table::ObjectTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "borrow_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::contains<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_table::ObjectTable<T0, T1>, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains<T0, T1>(
        arg0: sm::object_table::ObjectTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "contains")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::destroy_empty<T0: store + copy + drop, T1: key + store>(arg0: 0x2::object_table::ObjectTable<T0, T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_empty<T0, T1>(arg0: sm::object_table::ObjectTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "destroy_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::is_empty<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_table::ObjectTable<T0, T1>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty<T0, T1>(arg0: sm::object_table::ObjectTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "is_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::length<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_table::ObjectTable<T0, T1>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length<T0, T1>(arg0: sm::object_table::ObjectTable<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "length")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::new<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::tx_context::TxContext): 0x2::object_table::ObjectTable<T0, T1>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0, T1>() -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "object_table", "new").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec
    }
    /// Move: `public fun object_table::remove<T0: store + copy + drop, T1: key + store>(arg0: &mut 0x2::object_table::ObjectTable<T0, T1>, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(
        arg0: sm::object_table::ObjectTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun object_table::value_id<T0: store + copy + drop, T1: key + store>(arg0: &0x2::object_table::ObjectTable<T0, T1>, arg1: T0): 0x1::option::Option<0x2::object::ID>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn value_id<T0, T1>(
        arg0: sm::object_table::ObjectTable<T0, T1>,
        arg1: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "object_table", "value_id")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod package {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::package::Publisher`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "package", abilities = "key, store")]
    pub struct Publisher {
        pub id: sm::types::UID,
        pub package: sm::ascii::String,
        pub module_name: sm::ascii::String,
    }
    /// Move type: `0x2::package::UpgradeCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "package", abilities = "key, store")]
    pub struct UpgradeCap {
        pub id: sm::types::UID,
        pub package: sm::types::ID,
        pub version: u64,
        pub policy: u8,
    }
    /// Move type: `0x2::package::UpgradeReceipt`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "package")]
    pub struct UpgradeReceipt {
        pub cap: sm::types::ID,
        pub package: sm::types::ID,
    }
    /// Move type: `0x2::package::UpgradeTicket`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "package")]
    pub struct UpgradeTicket {
        pub cap: sm::types::ID,
        pub package: sm::types::ID,
        pub policy: u8,
        pub digest: Vec<u8>,
    }
    /// Move: `public fun package::additive_policy(): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn additive_policy() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "additive_policy")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun package::authorize_upgrade(arg0: &mut 0x2::package::UpgradeCap, arg1: u8, arg2: vector<u8>): 0x2::package::UpgradeTicket`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn authorize_upgrade(
        arg0: &mut impl sm_call::ObjectArg<UpgradeCap>,
        arg1: u8,
        arg2: Vec<u8>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "authorize_upgrade")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun package::burn_publisher(arg0: 0x2::package::Publisher)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn burn_publisher(arg0: &impl sm_call::ObjectArg<Publisher>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "burn_publisher")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::claim<T0: drop>(arg0: T0, arg1: &mut 0x2::tx_context::TxContext): 0x2::package::Publisher`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn claim<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "package", "claim").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::claim_and_keep<T0: drop>(arg0: T0, arg1: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn claim_and_keep<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "claim_and_keep")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::commit_upgrade(arg0: &mut 0x2::package::UpgradeCap, arg1: 0x2::package::UpgradeReceipt)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn commit_upgrade(
        arg0: &mut impl sm_call::ObjectArg<UpgradeCap>,
        arg1: UpgradeReceipt,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "commit_upgrade")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun package::compatible_policy(): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn compatible_policy() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "compatible_policy")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun package::dep_only_policy(): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn dep_only_policy() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "dep_only_policy")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun package::from_module<T0>(arg0: &0x2::package::Publisher): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_module<T0>(arg0: &impl sm_call::ObjectArg<Publisher>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "from_module")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::from_package<T0>(arg0: &0x2::package::Publisher): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_package<T0>(arg0: &impl sm_call::ObjectArg<Publisher>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "from_package")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun package::make_immutable(arg0: 0x2::package::UpgradeCap)`
    #[must_use]
    pub fn make_immutable(arg0: &impl sm_call::ObjectArg<UpgradeCap>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "make_immutable")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun package::only_additive_upgrades(arg0: &mut 0x2::package::UpgradeCap)`
    #[must_use]
    pub fn only_additive_upgrades(
        arg0: &mut impl sm_call::ObjectArg<UpgradeCap>,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "only_additive_upgrades")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun package::only_dep_upgrades(arg0: &mut 0x2::package::UpgradeCap)`
    #[must_use]
    pub fn only_dep_upgrades(arg0: &mut impl sm_call::ObjectArg<UpgradeCap>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "only_dep_upgrades")
            .expect("valid Move identifiers");
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::published_module(arg0: &0x2::package::Publisher): &0x1::ascii::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn published_module(arg0: &impl sm_call::ObjectArg<Publisher>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "published_module")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::published_package(arg0: &0x2::package::Publisher): &0x1::ascii::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn published_package(arg0: &impl sm_call::ObjectArg<Publisher>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "published_package")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::receipt_cap(arg0: &0x2::package::UpgradeReceipt): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn receipt_cap(arg0: UpgradeReceipt) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "receipt_cap")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::receipt_package(arg0: &0x2::package::UpgradeReceipt): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn receipt_package(arg0: UpgradeReceipt) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "receipt_package")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::ticket_digest(arg0: &0x2::package::UpgradeTicket): &vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn ticket_digest(arg0: UpgradeTicket) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "ticket_digest")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::ticket_package(arg0: &0x2::package::UpgradeTicket): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn ticket_package(arg0: UpgradeTicket) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "ticket_package")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::ticket_policy(arg0: &0x2::package::UpgradeTicket): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn ticket_policy(arg0: UpgradeTicket) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "ticket_policy")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::upgrade_package(arg0: &0x2::package::UpgradeCap): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn upgrade_package(arg0: &impl sm_call::ObjectArg<UpgradeCap>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "upgrade_package")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::upgrade_policy(arg0: &0x2::package::UpgradeCap): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn upgrade_policy(arg0: &impl sm_call::ObjectArg<UpgradeCap>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "package", "upgrade_policy")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun package::version(arg0: &0x2::package::UpgradeCap): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn version(arg0: &impl sm_call::ObjectArg<UpgradeCap>) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "package", "version").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
}

pub mod party {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::party::Party`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(address = "0x2", module = "party", abilities = "copy, drop")]
    pub struct Party {
        pub default: Permissions,
        pub members: sm::vec_map::VecMap<sm::prelude::Address, Permissions>,
    }
    /// Move type: `0x2::party::Permissions`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(address = "0x2", module = "party", abilities = "copy, drop")]
    pub struct Permissions {
        pub pos0: u64,
    }
    /// Move: `public fun party::single_owner(arg0: address): 0x2::party::Party`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn single_owner(arg0: sm::prelude::Address) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "party", "single_owner")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod pay {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public entry fun pay::divide_and_keep<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    #[must_use]
    pub fn divide_and_keep<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "pay", "divide_and_keep")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public entry fun pay::join<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: 0x2::coin::Coin<T0>)`
    #[must_use]
    pub fn join<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "pay", "join").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public entry fun pay::join_vec<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: vector<0x2::coin::Coin<T0>>)`
    #[must_use]
    pub fn join_vec<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: Vec<sm::coin::Coin<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "pay", "join_vec").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public entry fun pay::join_vec_and_transfer<T0>(arg0: vector<0x2::coin::Coin<T0>>, arg1: address)`
    #[must_use]
    pub fn join_vec_and_transfer<T0>(
        arg0: Vec<sm::coin::Coin<T0>>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "pay", "join_vec_and_transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun pay::keep<T0>(arg0: 0x2::coin::Coin<T0>, arg1: &0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn keep<T0>(arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "pay", "keep").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public entry fun pay::split<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    #[must_use]
    pub fn split<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "pay", "split").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public entry fun pay::split_and_transfer<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: u64, arg2: address, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    #[must_use]
    pub fn split_and_transfer<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: u64,
        arg2: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "pay", "split_and_transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public entry fun pay::split_vec<T0>(arg0: &mut 0x2::coin::Coin<T0>, arg1: vector<u64>, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    #[must_use]
    pub fn split_vec<T0>(
        arg0: &mut impl sm_call::ObjectArg<sm::coin::Coin<T0>>,
        arg1: Vec<u64>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "pay", "split_vec").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod poseidon {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun poseidon::poseidon_bn254(arg0: &vector<u256>): u256`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn poseidon_bn254(arg0: Vec<u64>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "poseidon", "poseidon_bn254")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod priority_queue {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::priority_queue::Entry`.
    /// Abilities: `store, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "priority_queue",
        abilities = "store, drop",
        type_abilities = "T0: drop"
    )]
    pub struct Entry<T0> {
        pub priority: u64,
        pub value: T0,
    }
    /// Move type: `0x2::priority_queue::PriorityQueue`.
    /// Abilities: `store, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "priority_queue",
        abilities = "store, drop",
        type_abilities = "T0: drop"
    )]
    pub struct PriorityQueue<T0> {
        pub entries: Vec<sm::priority_queue::Entry<T0>>,
    }
    /// Move: `public fun priority_queue::create_entries<T0: drop>(arg0: vector<u64>, arg1: vector<T0>): vector<0x2::priority_queue::Entry<T0>>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn create_entries<T0>(arg0: Vec<u64>, arg1: Vec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "priority_queue", "create_entries")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun priority_queue::insert<T0: drop>(arg0: &mut 0x2::priority_queue::PriorityQueue<T0>, arg1: u64, arg2: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn insert<T0>(
        arg0: sm::priority_queue::PriorityQueue<T0>,
        arg1: u64,
        arg2: T0,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "priority_queue", "insert")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun priority_queue::new<T0: drop>(arg0: vector<0x2::priority_queue::Entry<T0>>): 0x2::priority_queue::PriorityQueue<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0>(arg0: Vec<sm::priority_queue::Entry<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "priority_queue", "new")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun priority_queue::new_entry<T0: drop>(arg0: u64, arg1: T0): 0x2::priority_queue::Entry<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_entry<T0>(arg0: u64, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "priority_queue", "new_entry")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun priority_queue::pop_max<T0: drop>(arg0: &mut 0x2::priority_queue::PriorityQueue<T0>): (u64, T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pop_max<T0>(arg0: sm::priority_queue::PriorityQueue<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "priority_queue", "pop_max")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun priority_queue::priorities<T0: drop>(arg0: &0x2::priority_queue::PriorityQueue<T0>): vector<u64>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn priorities<T0>(arg0: sm::priority_queue::PriorityQueue<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "priority_queue", "priorities")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod prover {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;
}

pub mod random {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::random::Random`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "random", abilities = "key")]
    pub struct Random {
        pub id: sm::types::UID,
        pub inner: super::versioned::Versioned,
    }
    /// Move type: `0x2::random::RandomGenerator`.
    /// Abilities: `drop`.
    #[sm::move_struct(address = "0x2", module = "random", abilities = "drop")]
    pub struct RandomGenerator {
        pub seed: Vec<u8>,
        pub counter: u16,
        pub buffer: Vec<u8>,
    }
    /// Move type: `0x2::random::RandomInner`.
    /// Abilities: `store`.
    #[sm::move_struct(address = "0x2", module = "random", abilities = "store")]
    pub struct RandomInner {
        pub version: u64,
        pub epoch: u64,
        pub randomness_round: u64,
        pub random_bytes: Vec<u8>,
    }
    /// Move: `public fun random::generate_bool(arg0: &mut 0x2::random::RandomGenerator): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_bool(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_bool")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_bytes(arg0: &mut 0x2::random::RandomGenerator, arg1: u16): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_bytes(arg0: RandomGenerator, arg1: u16) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u128(arg0: &mut 0x2::random::RandomGenerator): u128`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u128(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u128")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u128_in_range(arg0: &mut 0x2::random::RandomGenerator, arg1: u128, arg2: u128): u128`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u128_in_range(
        arg0: RandomGenerator,
        arg1: u128,
        arg2: u128,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u128_in_range")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u16(arg0: &mut 0x2::random::RandomGenerator): u16`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u16(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u16")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u16_in_range(arg0: &mut 0x2::random::RandomGenerator, arg1: u16, arg2: u16): u16`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u16_in_range(arg0: RandomGenerator, arg1: u16, arg2: u16) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u16_in_range")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u256(arg0: &mut 0x2::random::RandomGenerator): u256`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u256(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u256")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u32(arg0: &mut 0x2::random::RandomGenerator): u32`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u32(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u32")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u32_in_range(arg0: &mut 0x2::random::RandomGenerator, arg1: u32, arg2: u32): u32`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u32_in_range(arg0: RandomGenerator, arg1: u32, arg2: u32) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u32_in_range")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u64(arg0: &mut 0x2::random::RandomGenerator): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u64(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u64")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u64_in_range(arg0: &mut 0x2::random::RandomGenerator, arg1: u64, arg2: u64): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u64_in_range(arg0: RandomGenerator, arg1: u64, arg2: u64) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u64_in_range")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u8(arg0: &mut 0x2::random::RandomGenerator): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u8(arg0: RandomGenerator) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u8")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::generate_u8_in_range(arg0: &mut 0x2::random::RandomGenerator, arg1: u8, arg2: u8): u8`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn generate_u8_in_range(arg0: RandomGenerator, arg1: u8, arg2: u8) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "generate_u8_in_range")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun random::new_generator(arg0: &0x2::random::Random, arg1: &mut 0x2::tx_context::TxContext): 0x2::random::RandomGenerator`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_generator(arg0: &impl sm_call::ObjectArg<Random>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "random", "new_generator")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun random::shuffle<T0>(arg0: &mut 0x2::random::RandomGenerator, arg1: &mut vector<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn shuffle<T0>(arg0: RandomGenerator, arg1: Vec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "random", "shuffle").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod sui {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::sui::SUI`.
    /// Abilities: `drop`.
    #[sm::move_struct(address = "0x2", module = "sui", abilities = "drop")]
    pub struct SUI {
        pub dummy_field: bool,
    }
    /// Move: `public entry fun sui::transfer(arg0: 0x2::coin::Coin<0x2::sui::SUI>, arg1: address)`
    #[must_use]
    pub fn transfer(
        arg0: &impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "sui", "transfer").expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod table {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::table::Table`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "table",
        abilities = "key, store",
        phantoms = "T0, T1",
        type_abilities = "T0: store, copy, drop; T1: store"
    )]
    pub struct Table<T0, T1> {
        pub id: sm::types::UID,
        pub size: u64,
    }
    /// Move: `public fun table::add<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::table::Table<T0, T1>, arg1: T0, arg2: T1)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add<T0, T1>(arg0: sm::containers::Table<T0, T1>, arg1: T0, arg2: T1) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "add").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun table::borrow<T0: store + copy + drop, T1: store>(arg0: &0x2::table::Table<T0, T1>, arg1: T0): &T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0, T1>(arg0: sm::containers::Table<T0, T1>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "borrow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun table::borrow_mut<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::table::Table<T0, T1>, arg1: T0): &mut T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0, T1>(arg0: sm::containers::Table<T0, T1>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "borrow_mut").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun table::contains<T0: store + copy + drop, T1: store>(arg0: &0x2::table::Table<T0, T1>, arg1: T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains<T0, T1>(arg0: sm::containers::Table<T0, T1>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "contains").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun table::destroy_empty<T0: store + copy + drop, T1: store>(arg0: 0x2::table::Table<T0, T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_empty<T0, T1>(arg0: sm::containers::Table<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table", "destroy_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table::drop<T0: store + copy + drop, T1: store + drop>(arg0: 0x2::table::Table<T0, T1>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn drop<T0, T1>(arg0: sm::containers::Table<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasDrop + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "drop").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table::is_empty<T0: store + copy + drop, T1: store>(arg0: &0x2::table::Table<T0, T1>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty<T0, T1>(arg0: sm::containers::Table<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "is_empty").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table::length<T0: store + copy + drop, T1: store>(arg0: &0x2::table::Table<T0, T1>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length<T0, T1>(arg0: sm::containers::Table<T0, T1>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "length").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table::new<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::tx_context::TxContext): 0x2::table::Table<T0, T1>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0, T1>() -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "new").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec
    }
    /// Move: `public fun table::remove<T0: store + copy + drop, T1: store>(arg0: &mut 0x2::table::Table<T0, T1>, arg1: T0): T1`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0, T1>(arg0: sm::containers::Table<T0, T1>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop + sm::HasStore,
        T1: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table", "remove").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod table_vec {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::table_vec::TableVec`.
    /// Abilities: `store`.
    #[sm::move_struct(
        address = "0x2",
        module = "table_vec",
        abilities = "store",
        phantoms = "T0",
        type_abilities = "T0: store"
    )]
    pub struct TableVec<T0> {
        pub contents: sm::containers::Table<u64, T0>,
    }
    /// Move: `public fun table_vec::borrow<T0: store>(arg0: &0x2::table_vec::TableVec<T0>, arg1: u64): &T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow<T0>(arg0: sm::containers::TableVec<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table_vec", "borrow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::borrow_mut<T0: store>(arg0: &mut 0x2::table_vec::TableVec<T0>, arg1: u64): &mut T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn borrow_mut<T0>(arg0: sm::containers::TableVec<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "borrow_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::destroy_empty<T0: store>(arg0: 0x2::table_vec::TableVec<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_empty<T0>(arg0: sm::containers::TableVec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "destroy_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::drop<T0: store + drop>(arg0: 0x2::table_vec::TableVec<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn drop<T0>(arg0: sm::containers::TableVec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table_vec", "drop").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::empty<T0: store>(arg0: &mut 0x2::tx_context::TxContext): 0x2::table_vec::TableVec<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn empty<T0>() -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table_vec", "empty").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec
    }
    /// Move: `public fun table_vec::is_empty<T0: store>(arg0: &0x2::table_vec::TableVec<T0>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty<T0>(arg0: sm::containers::TableVec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "is_empty")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::length<T0: store>(arg0: &0x2::table_vec::TableVec<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length<T0>(arg0: sm::containers::TableVec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table_vec", "length").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::pop_back<T0: store>(arg0: &mut 0x2::table_vec::TableVec<T0>): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn pop_back<T0>(arg0: sm::containers::TableVec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "pop_back")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::push_back<T0: store>(arg0: &mut 0x2::table_vec::TableVec<T0>, arg1: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn push_back<T0>(arg0: sm::containers::TableVec<T0>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "push_back")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::singleton<T0: store>(arg0: T0, arg1: &mut 0x2::tx_context::TxContext): 0x2::table_vec::TableVec<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn singleton<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "singleton")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::swap<T0: store>(arg0: &mut 0x2::table_vec::TableVec<T0>, arg1: u64, arg2: u64)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn swap<T0>(arg0: sm::containers::TableVec<T0>, arg1: u64, arg2: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "table_vec", "swap").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun table_vec::swap_remove<T0: store>(arg0: &mut 0x2::table_vec::TableVec<T0>, arg1: u64): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn swap_remove<T0>(arg0: sm::containers::TableVec<T0>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "table_vec", "swap_remove")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod token {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::token::ActionRequest`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "token", phantoms = "T0")]
    pub struct ActionRequest<T0> {
        pub name: sm::string::String,
        pub amount: u64,
        pub sender: sm::prelude::Address,
        pub recipient: sm::containers::MoveOption<sm::prelude::Address>,
        pub spent_balance: sm::containers::MoveOption<sm::balance::Balance<T0>>,
        pub approvals: sm::vec_set::VecSet<sm::type_name::TypeName>,
    }
    /// Move type: `0x2::token::RuleKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "token",
        abilities = "store, copy, drop",
        phantoms = "T0"
    )]
    pub struct RuleKey<T0> {
        pub is_protected: bool,
    }
    /// Move type: `0x2::token::Token`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "token", abilities = "key", phantoms = "T0")]
    pub struct Token<T0> {
        pub id: sm::types::UID,
        pub balance: sm::balance::Balance<T0>,
    }
    /// Move type: `0x2::token::TokenPolicy`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "token", abilities = "key", phantoms = "T0")]
    pub struct TokenPolicy<T0> {
        pub id: sm::types::UID,
        pub spent_balance: sm::balance::Balance<T0>,
        pub rules:
            sm::vec_map::VecMap<sm::string::String, sm::vec_set::VecSet<sm::type_name::TypeName>>,
    }
    /// Move type: `0x2::token::TokenPolicyCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "token",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct TokenPolicyCap<T0> {
        pub id: sm::types::UID,
        pub r#for: sm::types::ID,
    }
    /// Move type: `0x2::token::TokenPolicyCreated`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "token",
        abilities = "copy, drop",
        phantoms = "T0"
    )]
    pub struct TokenPolicyCreated<T0> {
        pub id: sm::types::ID,
        pub is_mutable: bool,
    }
    /// Move: `public fun token::action<T0>(arg0: &0x2::token::ActionRequest<T0>): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn action<T0>(arg0: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "action").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::add_approval<T0, T1: drop>(arg0: T1, arg1: &mut 0x2::token::ActionRequest<T0>, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add_approval<T0, T1>(arg0: T1, arg1: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "add_approval")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::add_rule_config<T0, T1: drop, T2: store>(arg0: T1, arg1: &mut 0x2::token::TokenPolicy<T0>, arg2: &0x2::token::TokenPolicyCap<T0>, arg3: T2, arg4: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add_rule_config<T0, T1, T2>(
        arg0: T1,
        arg1: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg2: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
        arg3: T2,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
        T2: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "add_rule_config")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun token::add_rule_for_action<T0, T1: drop>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: &0x2::token::TokenPolicyCap<T0>, arg2: 0x1::string::String, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add_rule_for_action<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "add_rule_for_action")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun token::allow<T0>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: &0x2::token::TokenPolicyCap<T0>, arg2: 0x1::string::String, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn allow<T0>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "allow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun token::amount<T0>(arg0: &0x2::token::ActionRequest<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn amount<T0>(arg0: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "amount").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::approvals<T0>(arg0: &0x2::token::ActionRequest<T0>): 0x2::vec_set::VecSet<0x1::type_name::TypeName>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn approvals<T0>(arg0: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "approvals").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::burn<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: 0x2::token::Token<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn burn<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::coin::TreasuryCap<T0>>,
        arg1: &impl sm_call::ObjectArg<Token<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "burn").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::confirm_request<T0>(arg0: &0x2::token::TokenPolicy<T0>, arg1: 0x2::token::ActionRequest<T0>, arg2: &mut 0x2::tx_context::TxContext): (0x1::string::String, u64, address, 0x1::option::Option<address>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn confirm_request<T0>(
        arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: ActionRequest<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "confirm_request")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::confirm_request_mut<T0>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: 0x2::token::ActionRequest<T0>, arg2: &mut 0x2::tx_context::TxContext): (0x1::string::String, u64, address, 0x1::option::Option<address>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn confirm_request_mut<T0>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: ActionRequest<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "confirm_request_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::confirm_with_policy_cap<T0>(arg0: &0x2::token::TokenPolicyCap<T0>, arg1: 0x2::token::ActionRequest<T0>, arg2: &mut 0x2::tx_context::TxContext): (0x1::string::String, u64, address, 0x1::option::Option<address>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn confirm_with_policy_cap<T0>(
        arg0: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
        arg1: ActionRequest<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "confirm_with_policy_cap")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::confirm_with_treasury_cap<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: 0x2::token::ActionRequest<T0>, arg2: &mut 0x2::tx_context::TxContext): (0x1::string::String, u64, address, 0x1::option::Option<address>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn confirm_with_treasury_cap<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::coin::TreasuryCap<T0>>,
        arg1: ActionRequest<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "confirm_with_treasury_cap")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::destroy_zero<T0>(arg0: 0x2::token::Token<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_zero<T0>(arg0: &impl sm_call::ObjectArg<Token<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "destroy_zero")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::disallow<T0>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: &0x2::token::TokenPolicyCap<T0>, arg2: 0x1::string::String, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn disallow<T0>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "disallow").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun token::flush<T0>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: &mut 0x2::coin::TreasuryCap<T0>, arg2: &mut 0x2::tx_context::TxContext): u64`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn flush<T0>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: &mut impl sm_call::ObjectArg<super::coin::TreasuryCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "flush").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::from_coin<T0>(arg0: 0x2::coin::Coin<T0>, arg1: &mut 0x2::tx_context::TxContext): (0x2::token::Token<T0>, 0x2::token::ActionRequest<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_coin<T0>(arg0: &impl sm_call::ObjectArg<sm::coin::Coin<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "from_coin").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::from_coin_action(): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_coin_action() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "from_coin_action")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun token::has_rule_config<T0, T1>(arg0: &0x2::token::TokenPolicy<T0>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn has_rule_config<T0, T1>(
        arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "has_rule_config")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::has_rule_config_with_type<T0, T1, T2: store>(arg0: &0x2::token::TokenPolicy<T0>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn has_rule_config_with_type<T0, T1, T2>(
        arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType,
        T2: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "has_rule_config_with_type")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::is_allowed<T0>(arg0: &0x2::token::TokenPolicy<T0>, arg1: &0x1::string::String): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_allowed<T0>(
        arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "is_allowed").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::join<T0>(arg0: &mut 0x2::token::Token<T0>, arg1: 0x2::token::Token<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn join<T0>(
        arg0: &mut impl sm_call::ObjectArg<Token<T0>>,
        arg1: &impl sm_call::ObjectArg<Token<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "join").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::keep<T0>(arg0: 0x2::token::Token<T0>, arg1: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn keep<T0>(arg0: &impl sm_call::ObjectArg<Token<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "keep").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::mint<T0>(arg0: &mut 0x2::coin::TreasuryCap<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): 0x2::token::Token<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn mint<T0>(
        arg0: &mut impl sm_call::ObjectArg<super::coin::TreasuryCap<T0>>,
        arg1: u64,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "mint").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::new_policy<T0>(arg0: &0x2::coin::TreasuryCap<T0>, arg1: &mut 0x2::tx_context::TxContext): (0x2::token::TokenPolicy<T0>, 0x2::token::TokenPolicyCap<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_policy<T0>(
        arg0: &impl sm_call::ObjectArg<super::coin::TreasuryCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "new_policy").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::recipient<T0>(arg0: &0x2::token::ActionRequest<T0>): 0x1::option::Option<address>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn recipient<T0>(arg0: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "recipient").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::remove_rule_config<T0, T1, T2: store>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: &0x2::token::TokenPolicyCap<T0>, arg2: &mut 0x2::tx_context::TxContext): T2`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove_rule_config<T0, T1, T2>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType,
        T2: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "remove_rule_config")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::remove_rule_for_action<T0, T1: drop>(arg0: &mut 0x2::token::TokenPolicy<T0>, arg1: &0x2::token::TokenPolicyCap<T0>, arg2: 0x1::string::String, arg3: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove_rule_for_action<T0, T1>(
        arg0: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "remove_rule_for_action")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun token::rule_config<T0, T1: drop, T2: store>(arg0: T1, arg1: &0x2::token::TokenPolicy<T0>): &T2`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn rule_config<T0, T1, T2>(
        arg0: T1,
        arg1: &impl sm_call::ObjectArg<TokenPolicy<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
        T2: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "rule_config")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::rule_config_mut<T0, T1: drop, T2: store>(arg0: T1, arg1: &mut 0x2::token::TokenPolicy<T0>, arg2: &0x2::token::TokenPolicyCap<T0>): &mut T2`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn rule_config_mut<T0, T1, T2>(
        arg0: T1,
        arg1: &mut impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg2: &impl sm_call::ObjectArg<TokenPolicyCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
        T2: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "rule_config_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun token::rules<T0>(arg0: &0x2::token::TokenPolicy<T0>, arg1: &0x1::string::String): 0x2::vec_set::VecSet<0x1::type_name::TypeName>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn rules<T0>(
        arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>,
        arg1: sm::string::String,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "rules").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::sender<T0>(arg0: &0x2::token::ActionRequest<T0>): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn sender<T0>(arg0: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "sender").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::share_policy<T0>(arg0: 0x2::token::TokenPolicy<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn share_policy<T0>(arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "share_policy")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::spend<T0>(arg0: 0x2::token::Token<T0>, arg1: &mut 0x2::tx_context::TxContext): 0x2::token::ActionRequest<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn spend<T0>(arg0: &impl sm_call::ObjectArg<Token<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "spend").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::spend_action(): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn spend_action() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "spend_action")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun token::spent<T0>(arg0: &0x2::token::ActionRequest<T0>): 0x1::option::Option<u64>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn spent<T0>(arg0: ActionRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "spent").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::spent_balance<T0>(arg0: &0x2::token::TokenPolicy<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn spent_balance<T0>(arg0: &impl sm_call::ObjectArg<TokenPolicy<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "spent_balance")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::split<T0>(arg0: &mut 0x2::token::Token<T0>, arg1: u64, arg2: &mut 0x2::tx_context::TxContext): 0x2::token::Token<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn split<T0>(arg0: &mut impl sm_call::ObjectArg<Token<T0>>, arg1: u64) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "split").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::to_coin<T0>(arg0: 0x2::token::Token<T0>, arg1: &mut 0x2::tx_context::TxContext): (0x2::coin::Coin<T0>, 0x2::token::ActionRequest<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_coin<T0>(arg0: &impl sm_call::ObjectArg<Token<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "to_coin").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::to_coin_action(): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn to_coin_action() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "to_coin_action")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun token::transfer<T0>(arg0: 0x2::token::Token<T0>, arg1: address, arg2: &mut 0x2::tx_context::TxContext): 0x2::token::ActionRequest<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn transfer<T0>(
        arg0: &impl sm_call::ObjectArg<Token<T0>>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "transfer").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun token::transfer_action(): 0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn transfer_action() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "token", "transfer_action")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun token::value<T0>(arg0: &0x2::token::Token<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn value<T0>(arg0: &impl sm_call::ObjectArg<Token<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "value").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun token::zero<T0>(arg0: &mut 0x2::tx_context::TxContext): 0x2::token::Token<T0>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn zero<T0>() -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "token", "zero").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec
    }
}

pub mod transfer {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::transfer::Receiving`.
    /// Abilities: `drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "transfer",
        abilities = "drop",
        phantoms = "T0",
        type_abilities = "T0: key"
    )]
    pub struct Receiving<T0> {
        pub id: sm::types::ID,
        pub version: u64,
    }
    /// Move: `public fun transfer::freeze_object<T0: key>(arg0: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn freeze_object<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "freeze_object")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::party_transfer<T0: key>(arg0: T0, arg1: 0x2::party::Party)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn party_transfer<T0>(
        arg0: &impl sm_call::ObjectArg<T0>,
        arg1: super::party::Party,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "party_transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::public_freeze_object<T0: key + store>(arg0: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_freeze_object<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "public_freeze_object")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::public_party_transfer<T0: key + store>(arg0: T0, arg1: 0x2::party::Party)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_party_transfer<T0>(
        arg0: &impl sm_call::ObjectArg<T0>,
        arg1: super::party::Party,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "public_party_transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::public_receive<T0: key + store>(arg0: &mut 0x2::object::UID, arg1: 0x2::transfer::Receiving<T0>): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_receive<T0>(arg0: sm::types::UID, arg1: Receiving<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "public_receive")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::public_share_object<T0: key + store>(arg0: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_share_object<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "public_share_object")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::public_transfer<T0: key + store>(arg0: T0, arg1: address)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn public_transfer<T0>(
        arg0: &impl sm_call::ObjectArg<T0>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasStore + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "public_transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::receive<T0: key>(arg0: &mut 0x2::object::UID, arg1: 0x2::transfer::Receiving<T0>): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn receive<T0>(arg0: sm::types::UID, arg1: Receiving<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "transfer", "receive").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::receiving_object_id<T0: key>(arg0: &0x2::transfer::Receiving<T0>): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn receiving_object_id<T0>(arg0: Receiving<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "receiving_object_id")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::share_object<T0: key>(arg0: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn share_object<T0>(arg0: &impl sm_call::ObjectArg<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "share_object")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer::transfer<T0: key>(arg0: T0, arg1: address)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn transfer<T0>(
        arg0: &impl sm_call::ObjectArg<T0>,
        arg1: sm::prelude::Address,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveStruct + sm::HasKey,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer", "transfer")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod transfer_policy {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::transfer_policy::RuleKey`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "transfer_policy",
        abilities = "store, copy, drop",
        phantoms = "T0",
        type_abilities = "T0: drop"
    )]
    pub struct RuleKey<T0> {
        pub dummy_field: bool,
    }
    /// Move type: `0x2::transfer_policy::TransferPolicy`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "transfer_policy",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct TransferPolicy<T0> {
        pub id: sm::types::UID,
        pub balance: sm::balance::Balance<sm::sui::SUI>,
        pub rules: sm::vec_set::VecSet<sm::type_name::TypeName>,
    }
    /// Move type: `0x2::transfer_policy::TransferPolicyCap`.
    /// Abilities: `key, store`.
    #[sm::move_struct(
        address = "0x2",
        module = "transfer_policy",
        abilities = "key, store",
        phantoms = "T0"
    )]
    pub struct TransferPolicyCap<T0> {
        pub id: sm::types::UID,
        pub policy_id: sm::types::ID,
    }
    /// Move type: `0x2::transfer_policy::TransferPolicyCreated`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "transfer_policy",
        abilities = "copy, drop",
        phantoms = "T0"
    )]
    pub struct TransferPolicyCreated<T0> {
        pub id: sm::types::ID,
    }
    /// Move type: `0x2::transfer_policy::TransferPolicyDestroyed`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "transfer_policy",
        abilities = "copy, drop",
        phantoms = "T0"
    )]
    pub struct TransferPolicyDestroyed<T0> {
        pub id: sm::types::ID,
    }
    /// Move type: `0x2::transfer_policy::TransferRequest`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "transfer_policy", phantoms = "T0")]
    pub struct TransferRequest<T0> {
        pub item: sm::types::ID,
        pub paid: u64,
        pub from: sm::types::ID,
        pub receipts: sm::vec_set::VecSet<sm::type_name::TypeName>,
    }
    /// Move: `public fun transfer_policy::add_receipt<T0, T1: drop>(arg0: T1, arg1: &mut 0x2::transfer_policy::TransferRequest<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add_receipt<T0, T1>(arg0: T1, arg1: TransferRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "add_receipt")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::add_rule<T0, T1: drop, T2: store + drop>(arg0: T1, arg1: &mut 0x2::transfer_policy::TransferPolicy<T0>, arg2: &0x2::transfer_policy::TransferPolicyCap<T0>, arg3: T2)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add_rule<T0, T1, T2>(
        arg0: T1,
        arg1: &mut impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg2: &impl sm_call::ObjectArg<TransferPolicyCap<T0>>,
        arg3: T2,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
        T2: sm::MoveType + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "add_rule")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::add_to_balance<T0, T1: drop>(arg0: T1, arg1: &mut 0x2::transfer_policy::TransferPolicy<T0>, arg2: 0x2::coin::Coin<0x2::sui::SUI>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn add_to_balance<T0, T1>(
        arg0: T1,
        arg1: &mut impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg2: &impl sm_call::ObjectArg<sm::coin::Coin<sm::sui::SUI>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "add_to_balance")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg_mut(arg1).expect("encode arg");
        spec.push_arg(arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::confirm_request<T0>(arg0: &0x2::transfer_policy::TransferPolicy<T0>, arg1: 0x2::transfer_policy::TransferRequest<T0>): (0x2::object::ID, u64, 0x2::object::ID)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn confirm_request<T0>(
        arg0: &impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg1: TransferRequest<T0>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "confirm_request")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::destroy_and_withdraw<T0>(arg0: 0x2::transfer_policy::TransferPolicy<T0>, arg1: 0x2::transfer_policy::TransferPolicyCap<T0>, arg2: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<0x2::sui::SUI>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy_and_withdraw<T0>(
        arg0: &impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TransferPolicyCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "destroy_and_withdraw")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::from<T0>(arg0: &0x2::transfer_policy::TransferRequest<T0>): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from<T0>(arg0: TransferRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "from")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::get_rule<T0, T1: drop, T2: store + drop>(arg0: T1, arg1: &0x2::transfer_policy::TransferPolicy<T0>): &T2`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn get_rule<T0, T1, T2>(
        arg0: T1,
        arg1: &impl sm_call::ObjectArg<TransferPolicy<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
        T2: sm::MoveType + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "get_rule")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::has_rule<T0, T1: drop>(arg0: &0x2::transfer_policy::TransferPolicy<T0>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn has_rule<T0, T1>(arg0: &impl sm_call::ObjectArg<TransferPolicy<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "has_rule")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::item<T0>(arg0: &0x2::transfer_policy::TransferRequest<T0>): 0x2::object::ID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn item<T0>(arg0: TransferRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "item")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::new<T0>(arg0: &0x2::package::Publisher, arg1: &mut 0x2::tx_context::TxContext): (0x2::transfer_policy::TransferPolicy<T0>, 0x2::transfer_policy::TransferPolicyCap<T0>)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new<T0>(arg0: &impl sm_call::ObjectArg<super::package::Publisher>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "new")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::new_request<T0>(arg0: 0x2::object::ID, arg1: u64, arg2: 0x2::object::ID): 0x2::transfer_policy::TransferRequest<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_request<T0>(arg0: sm::types::ID, arg1: u64, arg2: sm::types::ID) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "new_request")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::paid<T0>(arg0: &0x2::transfer_policy::TransferRequest<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn paid<T0>(arg0: TransferRequest<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "paid")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::remove_rule<T0, T1: drop, T2: store + drop>(arg0: &mut 0x2::transfer_policy::TransferPolicy<T0>, arg1: &0x2::transfer_policy::TransferPolicyCap<T0>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove_rule<T0, T1, T2>(
        arg0: &mut impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TransferPolicyCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
        T1: sm::MoveType + sm::HasDrop,
        T2: sm::MoveType + sm::HasDrop + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "remove_rule")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_type_arg::<T1>();
        spec.push_type_arg::<T2>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::rules<T0>(arg0: &0x2::transfer_policy::TransferPolicy<T0>): &0x2::vec_set::VecSet<0x1::type_name::TypeName>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn rules<T0>(arg0: &impl sm_call::ObjectArg<TransferPolicy<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "rules")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::uid<T0>(arg0: &0x2::transfer_policy::TransferPolicy<T0>): &0x2::object::UID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid<T0>(arg0: &impl sm_call::ObjectArg<TransferPolicy<T0>>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "uid")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::uid_mut_as_owner<T0>(arg0: &mut 0x2::transfer_policy::TransferPolicy<T0>, arg1: &0x2::transfer_policy::TransferPolicyCap<T0>): &mut 0x2::object::UID`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn uid_mut_as_owner<T0>(
        arg0: &mut impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TransferPolicyCap<T0>>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "uid_mut_as_owner")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun transfer_policy::withdraw<T0>(arg0: &mut 0x2::transfer_policy::TransferPolicy<T0>, arg1: &0x2::transfer_policy::TransferPolicyCap<T0>, arg2: 0x1::option::Option<u64>, arg3: &mut 0x2::tx_context::TxContext): 0x2::coin::Coin<0x2::sui::SUI>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn withdraw<T0>(
        arg0: &mut impl sm_call::ObjectArg<TransferPolicy<T0>>,
        arg1: &impl sm_call::ObjectArg<TransferPolicyCap<T0>>,
        arg2: sm::containers::MoveOption<u64>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "transfer_policy", "withdraw")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
}

pub mod tx_context {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::tx_context::TxContext`.
    /// Abilities: `drop`.
    #[sm::move_struct(address = "0x2", module = "tx_context", abilities = "drop")]
    pub struct TxContext {
        pub sender: sm::prelude::Address,
        pub tx_hash: Vec<u8>,
        pub epoch: u64,
        pub epoch_timestamp_ms: u64,
        pub ids_created: u64,
    }
    /// Move: `public fun tx_context::digest(arg0: &0x2::tx_context::TxContext): &vector<u8>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn digest() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "digest")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::epoch(arg0: &0x2::tx_context::TxContext): u64`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn epoch() -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "tx_context", "epoch").expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::epoch_timestamp_ms(arg0: &0x2::tx_context::TxContext): u64`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn epoch_timestamp_ms() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "epoch_timestamp_ms")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::fresh_object_address(arg0: &mut 0x2::tx_context::TxContext): address`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn fresh_object_address() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "fresh_object_address")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::gas_price(arg0: &0x2::tx_context::TxContext): u64`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn gas_price() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "gas_price")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::reference_gas_price(arg0: &0x2::tx_context::TxContext): u64`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn reference_gas_price() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "reference_gas_price")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::sender(arg0: &0x2::tx_context::TxContext): address`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn sender() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "sender")
            .expect("valid Move identifiers");
        spec
    }
    /// Move: `public fun tx_context::sponsor(arg0: &0x2::tx_context::TxContext): 0x1::option::Option<address>`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn sponsor() -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "tx_context", "sponsor")
            .expect("valid Move identifiers");
        spec
    }
}

pub mod types {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun types::is_one_time_witness<T0: drop>(arg0: &T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_one_time_witness<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "types", "is_one_time_witness")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod url {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::url::Url`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(address = "0x2", module = "url", abilities = "store, copy, drop")]
    pub struct Url {
        pub url: sm::ascii::String,
    }
    /// Move: `public fun url::inner_url(arg0: &0x2::url::Url): 0x1::ascii::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn inner_url(arg0: Url) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "url", "inner_url").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun url::new_unsafe(arg0: 0x1::ascii::String): 0x2::url::Url`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_unsafe(arg0: sm::ascii::String) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "url", "new_unsafe").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun url::new_unsafe_from_bytes(arg0: vector<u8>): 0x2::url::Url`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn new_unsafe_from_bytes(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "url", "new_unsafe_from_bytes")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun url::update(arg0: &mut 0x2::url::Url, arg1: 0x1::ascii::String)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn update(arg0: Url, arg1: sm::ascii::String) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "url", "update").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod vdf {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move: `public fun vdf::hash_to_input(arg0: &vector<u8>): vector<u8>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn hash_to_input(arg0: Vec<u8>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "vdf", "hash_to_input")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vdf::vdf_verify(arg0: &vector<u8>, arg1: &vector<u8>, arg2: &vector<u8>, arg3: u64): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn vdf_verify(arg0: Vec<u8>, arg1: Vec<u8>, arg2: Vec<u8>, arg3: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vdf", "vdf_verify").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
}

pub mod vec_set {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::vec_set::VecSet`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0x2",
        module = "vec_set",
        abilities = "store, copy, drop",
        type_abilities = "T0: copy, drop"
    )]
    pub struct VecSet<T0> {
        pub contents: Vec<T0>,
    }
    /// Move: `public fun vec_set::contains<T0: copy + drop>(arg0: &0x2::vec_set::VecSet<T0>, arg1: &T0): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn contains<T0>(arg0: sm::vec_set::VecSet<T0>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "contains").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::empty<T0: copy + drop>(): 0x2::vec_set::VecSet<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn empty<T0>() -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "empty").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec
    }
    /// Move: `public fun vec_set::from_keys<T0: copy + drop>(arg0: vector<T0>): 0x2::vec_set::VecSet<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn from_keys<T0>(arg0: Vec<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "vec_set", "from_keys")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::insert<T0: copy + drop>(arg0: &mut 0x2::vec_set::VecSet<T0>, arg1: T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn insert<T0>(arg0: sm::vec_set::VecSet<T0>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "insert").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::into_keys<T0: copy + drop>(arg0: 0x2::vec_set::VecSet<T0>): vector<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn into_keys<T0>(arg0: sm::vec_set::VecSet<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "vec_set", "into_keys")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::is_empty<T0: copy + drop>(arg0: &0x2::vec_set::VecSet<T0>): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn is_empty<T0>(arg0: sm::vec_set::VecSet<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "is_empty").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::keys<T0: copy + drop>(arg0: &0x2::vec_set::VecSet<T0>): &vector<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn keys<T0>(arg0: sm::vec_set::VecSet<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "keys").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::length<T0: copy + drop>(arg0: &0x2::vec_set::VecSet<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn length<T0>(arg0: sm::vec_set::VecSet<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "length").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::remove<T0: copy + drop>(arg0: &mut 0x2::vec_set::VecSet<T0>, arg1: &T0)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove<T0>(arg0: sm::vec_set::VecSet<T0>, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "remove").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::singleton<T0: copy + drop>(arg0: T0): 0x2::vec_set::VecSet<T0>`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn singleton<T0>(arg0: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "vec_set", "singleton")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun vec_set::size<T0: copy + drop>(arg0: &0x2::vec_set::VecSet<T0>): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn size<T0>(arg0: sm::vec_set::VecSet<T0>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasCopy + sm::HasDrop,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "vec_set", "size").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub mod versioned {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::versioned::VersionChangeCap`.
    /// Abilities: *(none)*.
    #[sm::move_struct(address = "0x2", module = "versioned")]
    pub struct VersionChangeCap {
        pub versioned_id: sm::types::ID,
        pub old_version: u64,
    }
    /// Move type: `0x2::versioned::Versioned`.
    /// Abilities: `key, store`.
    #[sm::move_struct(address = "0x2", module = "versioned", abilities = "key, store")]
    pub struct Versioned {
        pub id: sm::types::UID,
        pub version: u64,
    }
    /// Move: `public fun versioned::create<T0: store>(arg0: u64, arg1: T0, arg2: &mut 0x2::tx_context::TxContext): 0x2::versioned::Versioned`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn create<T0>(arg0: u64, arg1: T0) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "versioned", "create").expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun versioned::destroy<T0: store>(arg0: 0x2::versioned::Versioned): T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn destroy<T0>(arg0: &impl sm_call::ObjectArg<Versioned>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "versioned", "destroy")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun versioned::load_value<T0: store>(arg0: &0x2::versioned::Versioned): &T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn load_value<T0>(arg0: &impl sm_call::ObjectArg<Versioned>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "versioned", "load_value")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun versioned::load_value_mut<T0: store>(arg0: &mut 0x2::versioned::Versioned): &mut T0`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn load_value_mut<T0>(arg0: &mut impl sm_call::ObjectArg<Versioned>) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "versioned", "load_value_mut")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun versioned::remove_value_for_upgrade<T0: store>(arg0: &mut 0x2::versioned::Versioned): (T0, 0x2::versioned::VersionChangeCap)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn remove_value_for_upgrade<T0>(
        arg0: &mut impl sm_call::ObjectArg<Versioned>,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "versioned", "remove_value_for_upgrade")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun versioned::upgrade<T0: store>(arg0: &mut 0x2::versioned::Versioned, arg1: u64, arg2: T0, arg3: 0x2::versioned::VersionChangeCap)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn upgrade<T0>(
        arg0: &mut impl sm_call::ObjectArg<Versioned>,
        arg1: u64,
        arg2: T0,
        arg3: VersionChangeCap,
    ) -> sm_call::CallSpec
    where
        T0: sm::MoveType + sm::HasStore,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "versioned", "upgrade")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg_mut(arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec
    }
    /// Move: `public fun versioned::version(arg0: &0x2::versioned::Versioned): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn version(arg0: &impl sm_call::ObjectArg<Versioned>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "versioned", "version")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
}

pub mod zklogin_verified_id {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::zklogin_verified_id::VerifiedID`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "zklogin_verified_id", abilities = "key")]
    pub struct VerifiedID {
        pub id: sm::types::UID,
        pub owner: sm::prelude::Address,
        pub key_claim_name: sm::string::String,
        pub key_claim_value: sm::string::String,
        pub issuer: sm::string::String,
        pub audience: sm::string::String,
    }
    /// Move: `public fun zklogin_verified_id::audience(arg0: &0x2::zklogin_verified_id::VerifiedID): &0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn audience(arg0: &impl sm_call::ObjectArg<VerifiedID>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "audience")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::check_zklogin_id(arg0: address, arg1: &0x1::string::String, arg2: &0x1::string::String, arg3: &0x1::string::String, arg4: &0x1::string::String, arg5: u256): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn check_zklogin_id(
        arg0: sm::prelude::Address,
        arg1: sm::string::String,
        arg2: sm::string::String,
        arg3: sm::string::String,
        arg4: sm::string::String,
        arg5: u64,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "check_zklogin_id")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec.push_arg(&arg4).expect("encode arg");
        spec.push_arg(&arg5).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::delete(arg0: 0x2::zklogin_verified_id::VerifiedID)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn delete(arg0: &impl sm_call::ObjectArg<VerifiedID>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "delete")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::issuer(arg0: &0x2::zklogin_verified_id::VerifiedID): &0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn issuer(arg0: &impl sm_call::ObjectArg<VerifiedID>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "issuer")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::key_claim_name(arg0: &0x2::zklogin_verified_id::VerifiedID): &0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn key_claim_name(arg0: &impl sm_call::ObjectArg<VerifiedID>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "key_claim_name")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::key_claim_value(arg0: &0x2::zklogin_verified_id::VerifiedID): &0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn key_claim_value(arg0: &impl sm_call::ObjectArg<VerifiedID>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "key_claim_value")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::owner(arg0: &0x2::zklogin_verified_id::VerifiedID): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn owner(arg0: &impl sm_call::ObjectArg<VerifiedID>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "owner")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_id::verify_zklogin_id(arg0: 0x1::string::String, arg1: 0x1::string::String, arg2: 0x1::string::String, arg3: 0x1::string::String, arg4: u256, arg5: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn verify_zklogin_id(
        arg0: sm::string::String,
        arg1: sm::string::String,
        arg2: sm::string::String,
        arg3: sm::string::String,
        arg4: u64,
    ) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_id", "verify_zklogin_id")
            .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec.push_arg(&arg3).expect("encode arg");
        spec.push_arg(&arg4).expect("encode arg");
        spec
    }
}

pub mod zklogin_verified_issuer {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0x2::zklogin_verified_issuer::VerifiedIssuer`.
    /// Abilities: `key`.
    #[sm::move_struct(address = "0x2", module = "zklogin_verified_issuer", abilities = "key")]
    pub struct VerifiedIssuer {
        pub id: sm::types::UID,
        pub owner: sm::prelude::Address,
        pub issuer: sm::string::String,
    }
    /// Move: `public fun zklogin_verified_issuer::check_zklogin_issuer(arg0: address, arg1: u256, arg2: &0x1::string::String): bool`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn check_zklogin_issuer(
        arg0: sm::prelude::Address,
        arg1: u64,
        arg2: sm::string::String,
    ) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "zklogin_verified_issuer", "check_zklogin_issuer")
                .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec.push_arg(&arg2).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_issuer::delete(arg0: 0x2::zklogin_verified_issuer::VerifiedIssuer)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn delete(arg0: &impl sm_call::ObjectArg<VerifiedIssuer>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_issuer", "delete")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_issuer::issuer(arg0: &0x2::zklogin_verified_issuer::VerifiedIssuer): &0x1::string::String`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn issuer(arg0: &impl sm_call::ObjectArg<VerifiedIssuer>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_issuer", "issuer")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_issuer::owner(arg0: &0x2::zklogin_verified_issuer::VerifiedIssuer): address`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn owner(arg0: &impl sm_call::ObjectArg<VerifiedIssuer>) -> sm_call::CallSpec {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "zklogin_verified_issuer", "owner")
            .expect("valid Move identifiers");
        spec.push_arg(arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun zklogin_verified_issuer::verify_zklogin_issuer(arg0: u256, arg1: 0x1::string::String, arg2: &mut 0x2::tx_context::TxContext)`
    /// Note: `TxContext` is omitted; the runtime layer supplies it.
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn verify_zklogin_issuer(arg0: u64, arg1: sm::string::String) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "zklogin_verified_issuer", "verify_zklogin_issuer")
                .expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub use {
    accumulator::{AccumulatorRoot, Key, U128},
    accumulator_metadata::{Metadata, MetadataKey, Owner, OwnerKey},
    accumulator_settlement::EventStreamHead,
    authenticator_state::{ActiveJwk, AuthenticatorState, AuthenticatorStateInner, JwkId, JWK},
    bag::Bag,
    balance::{Balance, Supply},
    bcs::BCS,
    bls12381::{Scalar, UncompressedG1, G1, G2, GT},
    borrow::{Borrow, Referent},
    clock::Clock,
    coin::{
        Coin,
        CoinMetadata,
        CurrencyCreated,
        DenyCap,
        DenyCapV2,
        RegulatedCoinMetadata,
        TreasuryCap,
    },
    config::{Config, Setting, SettingData},
    deny_list::{
        AddressKey,
        ConfigKey,
        ConfigWriteCap,
        DenyList,
        GlobalPauseKey,
        PerTypeConfigCreated,
        PerTypeList,
    },
    derived_object::{Claimed, ClaimedStatus, DerivedObjectKey},
    display::{Display, DisplayCreated, VersionUpdated},
    dynamic_field::Field,
    dynamic_object_field::Wrapper,
    funds_accumulator::Withdrawal,
    groth16::{Curve, PreparedVerifyingKey, ProofPoints, PublicProofInputs},
    group_ops::Element,
    kiosk::{
        Item,
        ItemDelisted,
        ItemListed,
        ItemPurchased,
        Kiosk,
        KioskOwnerCap,
        Listing,
        Lock,
        PurchaseCap,
    },
    kiosk_extension::{Extension, ExtensionKey},
    linked_table::{LinkedTable, Node},
    nitro_attestation::{NitroAttestationDocument, PCREntry},
    object::{ID, UID},
    object_bag::ObjectBag,
    object_table::ObjectTable,
    package::{Publisher, UpgradeCap, UpgradeReceipt, UpgradeTicket},
    party::{Party, Permissions},
    priority_queue::{Entry, PriorityQueue},
    random::{Random, RandomGenerator, RandomInner},
    sui::SUI,
    table::Table,
    table_vec::TableVec,
    token::{ActionRequest, RuleKey, Token, TokenPolicy, TokenPolicyCap, TokenPolicyCreated},
    transfer::Receiving,
    transfer_policy::{
        TransferPolicy,
        TransferPolicyCap,
        TransferPolicyCreated,
        TransferPolicyDestroyed,
        TransferRequest,
    },
    tx_context::TxContext,
    url::Url,
    vec_set::VecSet,
    versioned::{VersionChangeCap, Versioned},
    zklogin_verified_id::VerifiedID,
    zklogin_verified_issuer::VerifiedIssuer,
};
