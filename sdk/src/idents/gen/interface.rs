/// Package address (the on-chain package object id).
pub const PACKAGE: sui_move::prelude::Address = sui_move::prelude::Address::from_static(
    "0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88",
);
pub mod v1 {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88::v1::AnnounceInterfacePackageEvent`.
    /// Abilities: `copy, drop`.
    #[sm::move_struct(
        address = "0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88",
        module = "v1",
        abilities = "copy, drop",
        phantoms = "T0"
    )]
    pub struct AnnounceInterfacePackageEvent<T0> {
        pub shared_objects: Vec<sm::types::ID>,
    }
    /// Move: `public fun v1::announce_interface_package<T0>(arg0: &T0, arg1: vector<0x2::object::ID>)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn announce_interface_package<T0>(arg0: T0, arg1: Vec<sm::types::ID>) -> sm_call::CallSpec
    where
        T0: sm::MoveType,
    {
        let mut spec = sm_call::CallSpec::new(PACKAGE, "v1", "announce_interface_package")
            .expect("valid Move identifiers");
        spec.push_type_arg::<T0>();
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
}

pub mod version {

    use super::PACKAGE;
    #[allow(unused_imports)]
    use sui_move as sm;
    #[allow(unused_imports)]
    use sui_move_call as sm_call;

    /// Move type: `0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88::version::InterfaceVersion`.
    /// Abilities: `store, copy, drop`.
    #[sm::move_struct(
        address = "0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88",
        module = "version",
        abilities = "store, copy, drop"
    )]
    pub struct InterfaceVersion {
        pub inner: u64,
    }
    /// Move: `public fun version::expect_v(arg0: &0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88::version::InterfaceVersion, arg1: u64)`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn expect_v(arg0: InterfaceVersion, arg1: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "version", "expect_v").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec.push_arg(&arg1).expect("encode arg");
        spec
    }
    /// Move: `public fun version::number(arg0: &0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88::version::InterfaceVersion): u64`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn number(arg0: InterfaceVersion) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "version", "number").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
    /// Move: `public fun version::v(arg0: u64): 0xd749533b00b57ed752a9ee4f530f4ae806fe0f48baf7faf36f07a4e6409b7a88::version::InterfaceVersion`
    /// Note: this function is not marked `entry`.
    #[must_use]
    pub fn v(arg0: u64) -> sm_call::CallSpec {
        let mut spec =
            sm_call::CallSpec::new(PACKAGE, "version", "v").expect("valid Move identifiers");
        spec.push_arg(&arg0).expect("encode arg");
        spec
    }
}

pub use {v1::AnnounceInterfacePackageEvent, version::InterfaceVersion};
