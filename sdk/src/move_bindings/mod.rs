//! Generated Move package bindings.
//!
//! This is the SDK generated ABI boundary: Move types scoped by package, type tags, BCS and serde
//! implementations, and generated call targets come from here. Rust domain types may reexport
//! selected modules, but they should not duplicate Move ABI logic.

mod extensions;

#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
use self::registry::network_auth::IdentityKey;
use {
    self::interface::graph::RuntimeVertex,
    crate::{sui, types::NexusObjects},
};

fn derive_object_id<T: sui::traits::ToBcs>(
    parent: sui::types::Address,
    tag: &sui::types::TypeTag,
    key: &T,
) -> anyhow::Result<sui::types::Address> {
    Ok(parent.derive_object_id(tag, &key.to_bcs()?))
}

/// Run `f` with all Nexus generated bindings scoped to the package IDs in `objects`.
///
/// Current package IDs are used for Move call targets. Original package IDs are used for type
/// identity where Sui package upgrades keep type tags pinned to the defining package.
pub(crate) fn with_nexus_scope<R>(objects: &NexusObjects, f: impl FnOnce() -> R) -> R {
    move_std::with_packages(
        sui::types::Address::from_static("0x1"),
        sui::types::Address::from_static("0x1"),
        || {
            sui_framework::with_packages(
                sui::types::Address::from_static("0x2"),
                sui::types::Address::from_static("0x2"),
                || {
                    primitives::with_packages(
                        objects.primitives_pkg_id,
                        objects.primitives_pkg_id,
                        || {
                            interface::with_packages(
                                objects.interface_pkg_id,
                                objects.interface_pkg_id,
                                || {
                                    registry::with_packages(
                                        objects.registry_pkg_id,
                                        objects.registry_pkg_id,
                                        || {
                                            workflow::with_packages(
                                                objects.workflow_pkg_id,
                                                objects.workflow_type_origin_pkg_id(),
                                                || {
                                                    scheduler::with_packages(
                                                        objects.scheduler_pkg_id,
                                                        objects.scheduler_type_origin_pkg_id(),
                                                        f,
                                                    )
                                                },
                                            )
                                        },
                                    )
                                },
                            )
                        },
                    )
                },
            )
        },
    )
}

/// Build a generated Move type tag scoped to this Nexus deployment.
pub fn type_tag<T>(objects: &NexusObjects) -> sui::types::TypeTag
where
    T: sui_move::MoveType,
{
    with_nexus_scope(objects, T::type_tag_static)
}

#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
fn registry_type_tag<T>(registry_pkg_id: sui::types::Address) -> sui::types::TypeTag
where
    T: sui_move::MoveType,
{
    registry::with_packages(registry_pkg_id, registry_pkg_id, T::type_tag_static)
}

/// Build a generated Move struct tag scoped to this Nexus deployment.
pub fn struct_tag<T>(objects: &NexusObjects) -> sui::types::StructTag
where
    T: sui_move::MoveStruct,
{
    with_nexus_scope(objects, T::struct_tag_static)
}

/// Return whether `tag` matches the generated struct `T` identity scoped to this deployment.
pub fn struct_tag_matches<T>(objects: &NexusObjects, tag: &sui::types::StructTag) -> bool
where
    T: sui_move::MoveStruct,
{
    let expected = struct_tag::<T>(objects);
    tag.address() == expected.address()
        && tag.module() == expected.module()
        && tag.name() == expected.name()
}

/// Return whether `tag` has the same generated module/name shape as struct `T`.
///
/// This intentionally ignores the package address. Use it for arbitrary Move signatures where the
/// caller package is the value being inspected rather than the configured Nexus deployment.
pub fn struct_shape_matches<T>(tag: &sui::types::StructTag) -> bool
where
    T: sui_move::MoveStruct,
{
    let expected = T::struct_tag_static();
    tag.module() == expected.module() && tag.name() == expected.name()
}

/// Build a generated Move struct tag with a specific package address.
pub(crate) fn struct_tag_with_package<T>(
    objects: &NexusObjects,
    package: sui::types::Address,
) -> sui::types::StructTag
where
    T: sui_move::MoveStruct,
{
    let tag = struct_tag::<T>(objects);
    sui::types::StructTag::new(
        package,
        tag.module().clone(),
        tag.name().clone(),
        tag.type_params().to_vec(),
    )
}

/// Qualified generated Move struct name scoped to this Nexus deployment.
pub(crate) fn struct_type_name<T>(objects: &NexusObjects) -> String
where
    T: sui_move::MoveStruct,
{
    qualified_struct_name(&struct_tag::<T>(objects))
}

/// Qualified generated Move struct name with a specific package address.
pub(crate) fn struct_type_name_with_package<T>(
    objects: &NexusObjects,
    package: sui::types::Address,
) -> String
where
    T: sui_move::MoveStruct,
{
    qualified_struct_name(&struct_tag_with_package::<T>(objects, package))
}

fn qualified_struct_name(tag: &sui::types::StructTag) -> String {
    format!("{}::{}::{}", tag.address(), tag.module(), tag.name())
}

/// Derive the on chain [`registry::tool_registry::Tool`] object ID for a tool FQN.
pub fn derive_tool_id(
    tool_registry: sui::types::Address,
    tool_fqn: &crate::ToolFqn,
) -> anyhow::Result<sui::types::Address> {
    use sui_move::MoveType as _;

    derive_object_id(
        tool_registry,
        &move_std::ascii::String::type_tag_static(),
        tool_fqn,
    )
}

/// Derive the on chain [`workflow::gas::ToolGas`] object ID for a tool FQN.
pub fn derive_tool_gas_id(
    gas_service: sui::types::Address,
    tool_fqn: &crate::ToolFqn,
) -> anyhow::Result<sui::types::Address> {
    use sui_move::MoveType as _;

    derive_object_id(
        gas_service,
        &move_std::ascii::String::type_tag_static(),
        tool_fqn,
    )
}

#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
pub(crate) fn derive_network_auth_binding_id(
    registry_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
    identity: &IdentityKey,
) -> anyhow::Result<sui::types::Address> {
    let key_type = registry_type_tag::<IdentityKey>(registry_pkg_id);
    derive_object_id(network_auth_object_id, &key_type, identity)
}

/// Derive the task ID associated with a walk execution request event.
pub fn derive_walk_execution_event_task_id(
    interface_pkg_id: sui::types::Address,
    execution: sui::types::Address,
    vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Address> {
    use sui_move::MoveStruct;

    let (name, iteration) = match vertex {
        RuntimeVertex::Plain { vertex } => (vertex, &0),
        RuntimeVertex::WithIterator {
            vertex, iteration, ..
        } => (vertex, iteration),
    };
    let vertex_shape = interface::graph::Vertex::struct_tag_static();
    let vertex_tag = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        interface_pkg_id,
        vertex_shape.module().clone(),
        vertex_shape.name().clone(),
        vec![],
    )));

    derive_object_id(
        derive_object_id(execution, &vertex_tag, name)?,
        &sui::types::TypeTag::U64,
        iteration,
    )
}

/// Derive the task ID associated with a scheduled occurrence event.
pub fn derive_occurrence_scheduled_event_task_id(
    task: sui::types::Address,
    start_time_ms: &u64,
) -> anyhow::Result<sui::types::Address> {
    derive_object_id(task, &sui::types::TypeTag::U64, start_time_ms)
}

pub mod interface {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/interface_types.rs"));
}

pub mod move_std {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/move_std_types.rs"));
}

pub mod primitives {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/primitives_types.rs"));
}

pub mod registry {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/registry_types.rs"));
}

pub mod scheduler {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/scheduler_types.rs"));
}

pub mod sui_framework {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/sui_framework_types.rs"));
}

pub mod workflow {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/workflow_types.rs"));
}

#[cfg(test)]
mod tests {
    use super::registry;

    #[test]
    fn generated_bindings_expose_calls() {
        let _ = registry::leader::claim_unstaked_for_self_target;
    }
}
