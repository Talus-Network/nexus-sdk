//! Generated Move package bindings.
//!
//! This is the SDK generated ABI boundary: Move types scoped by package, type tags, BCS and serde
//! implementations, and generated call targets come from here. Rust domain types may reexport
//! selected modules, but they should not duplicate Move ABI logic.

mod extensions;
#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
use self::registry::network_auth::IdentityKey;
pub use extensions::VersionedAnchor;
#[cfg(feature = "transactions")]
pub use sui_move_ptb::CLOCK_OBJECT_ID;
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
/// Storage IDs are used for Move call targets. Exact datatype origins are
/// used for type identity, including types introduced by later upgrades.
pub(crate) fn with_nexus_scope<R>(objects: &NexusObjects, f: impl FnOnce() -> R) -> R {
    move_std::with_packages(
        sui::types::Address::from_static("0x1"),
        sui::types::Address::from_static("0x1"),
        || {
            sui_framework::with_packages(
                sui::types::Address::from_static("0x2"),
                sui::types::Address::from_static("0x2"),
                || {
                    talus::with_packages(
                        objects.us_token.package_id,
                        objects.us_token.package_id,
                        || {
                            primitives::with_package_context(
                                objects.packages.primitives.storage_id,
                                objects.packages.primitives.initial_id,
                                &objects.packages.primitives.type_origins,
                                || {
                                    interface::with_package_context(
                                        objects.packages.interface.storage_id,
                                        objects.packages.interface.initial_id,
                                        &objects.packages.interface.type_origins,
                                        || {
                                            tool::with_package_context(
                                                objects.packages.tool.storage_id,
                                                objects.packages.tool.initial_id,
                                                &objects.packages.tool.type_origins,
                                                || {
                                                    registry::with_package_context(
                                                        objects.packages.registry.storage_id,
                                                        objects.packages.registry.initial_id,
                                                        &objects.packages.registry.type_origins,
                                                        || {
                                                            workflow::with_package_context(
                                                                objects
                                                                    .packages
                                                                    .workflow
                                                                    .storage_id,
                                                                objects
                                                                    .packages
                                                                    .workflow
                                                                    .initial_id,
                                                                &objects
                                                                    .packages
                                                                    .workflow
                                                                    .type_origins,
                                                                || {
                                                                    scheduler::with_package_context(
                                                                        objects
                                                                            .packages
                                                                            .scheduler
                                                                            .storage_id,
                                                                        objects
                                                                            .packages
                                                                            .scheduler
                                                                            .initial_id,
                                                                        &objects
                                                                            .packages
                                                                            .scheduler
                                                                            .type_origins,
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
fn registry_type_tag<T>(registry_type_origin_pkg_id: sui::types::Address) -> sui::types::TypeTag
where
    T: sui_move::MoveType,
{
    registry::with_packages(
        registry_type_origin_pkg_id,
        registry_type_origin_pkg_id,
        T::type_tag_static,
    )
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

/// Derive the onchain [`tool::tool_registry::Tool`] object ID for a Tool FQN.
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

/// Derive the onchain [`tool::tool_cashier::ToolCashier`] object ID for a Tool.
///
/// The key value must use the generated [`tool::tool_cashier::ToolCashierKey`]
/// layout. Move represents its empty declaration with a `false` dummy field,
/// and that byte is part of the derived object hash.
pub fn derive_tool_cashier_id(
    tool_cashier_type_origin: sui::types::Address,
    tool: sui::types::Address,
) -> anyhow::Result<sui::types::Address> {
    let key = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        tool_cashier_type_origin,
        sui::types::Identifier::from_static("tool_cashier"),
        sui::types::Identifier::from_static("ToolCashierKey"),
        vec![],
    )));
    derive_object_id(tool, &key, &tool::tool_cashier::ToolCashierKey::new(false))
}

#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
pub(crate) fn derive_network_auth_binding_id(
    registry_type_origin_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
    identity: &IdentityKey,
) -> anyhow::Result<sui::types::Address> {
    let key_type = registry_type_tag::<IdentityKey>(registry_type_origin_pkg_id);
    derive_object_id(network_auth_object_id, &key_type, identity)
}

/// Derive the task ID associated with a walk execution request event.
///
/// Pass [`NexusObjects::interface_type_origin_pkg_id`] so the derived ID
/// remains stable after an interface package upgrade.
pub fn derive_walk_execution_event_task_id(
    interface_type_origin_pkg_id: sui::types::Address,
    execution: sui::types::Address,
    vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Address> {
    use sui_move::MoveStruct;

    let (name, repetitive, iteration) = match vertex {
        RuntimeVertex::Plain { vertex } => (vertex, false, 0),
        RuntimeVertex::WithIterator {
            vertex, iteration, ..
        } => (vertex, true, *iteration),
    };
    let vertex_shape = interface::graph::Vertex::struct_tag_static();
    let vertex_tag = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        interface_type_origin_pkg_id,
        vertex_shape.module().clone(),
        vertex_shape.name().clone(),
        vec![],
    )));

    let vertex_task_id = derive_object_id(execution, &vertex_tag, name)?;
    let runtime_task_id = if repetitive {
        derive_object_id(vertex_task_id, &sui::types::TypeTag::Bool, &true)?
    } else {
        vertex_task_id
    };
    derive_object_id(runtime_task_id, &sui::types::TypeTag::U64, &iteration)
}

/// Derives the [`workflow::execution::DAGExecution`] ID for one [`scheduler::task::Task`] occurrence.
pub fn derive_task_execution_id(
    task: sui::types::Address,
    occurrence_id: u64,
) -> anyhow::Result<sui::types::Address> {
    derive_object_id(task, &sui::types::TypeTag::U64, &occurrence_id)
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

pub mod tool {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/tool_types.rs"));
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

pub mod talus {
    #![allow(
        clippy::all,
        dead_code,
        non_camel_case_types,
        private_interfaces,
        unused_imports
    )]
    include!(concat!(env!("OUT_DIR"), "/talus_types.rs"));
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
    use {
        super::{
            derive_task_execution_id,
            derive_tool_cashier_id,
            derive_walk_execution_event_task_id,
            interface::graph::RuntimeVertex,
            registry,
        },
        crate::sui,
        sui_move::MoveType,
    };
    #[test]
    fn generated_bindings_expose_calls() {
        let _ = registry::leader::claim_unstaked_for_self_target;
        let _ = super::talus::us::US::type_tag_static;
    }

    #[test]
    fn walk_task_id_distinguishes_plain_from_zero_based_repetitive_execution() {
        let interface = sui::types::Address::from_static("0x1");
        let execution = sui::types::Address::from_static("0x2");
        let plain = derive_walk_execution_event_task_id(
            interface,
            execution,
            &RuntimeVertex::plain("vertex"),
        )
        .unwrap();
        let repetitive_zero = derive_walk_execution_event_task_id(
            interface,
            execution,
            &RuntimeVertex::with_iterator("vertex", 0, 2),
        )
        .unwrap();
        let repetitive_one = derive_walk_execution_event_task_id(
            interface,
            execution,
            &RuntimeVertex::with_iterator("vertex", 1, 2),
        )
        .unwrap();

        assert_ne!(plain, repetitive_zero);
        assert_ne!(repetitive_zero, repetitive_one);
    }

    #[test]
    fn task_execution_id_uses_the_occurrence_identity() {
        let task = sui::types::Address::from_static("0x51");
        let expected = task.derive_object_id(&sui::types::TypeTag::U64, &7_u64.to_le_bytes());

        assert_eq!(derive_task_execution_id(task, 7).unwrap(), expected);
    }

    #[test]
    fn tool_cashier_id_matches_the_move_derived_object_key() {
        let package = sui::types::Address::from_static(
            "0xb9b0f588a28d41b7f40a8cc11e9ad5bb96f65f4da5f93d886db69519642fbebb",
        );
        let tool = sui::types::Address::from_static(
            "0x015002c6cb4dca09fecf1f52a09e5e92bc7b878ce6386a83cc021fb4df67b0ec",
        );
        let expected = sui::types::Address::from_static(
            "0x6d6d6f4789bef43ac69883d713e63fe34ed2aa9e012c14d3da9ede3cb3b57e78",
        );

        assert_eq!(derive_tool_cashier_id(package, tool).unwrap(), expected);
    }
}
