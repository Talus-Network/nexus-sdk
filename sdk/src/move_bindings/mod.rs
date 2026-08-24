//! Generated Move package bindings.
//!
//! This is the SDK generated ABI boundary: Move types scoped by package, type tags, BCS and serde
//! implementations, and generated call targets come from here. Rust domain types may reexport
//! selected modules, but they should not duplicate Move ABI logic.

mod extensions;
pub(crate) mod protocol_limits {
    include!(concat!(env!("OUT_DIR"), "/protocol_limits.rs"));
}
#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
use self::registry::network_auth::IdentityKey;
#[cfg(feature = "walrus")]
pub(crate) use extensions::canonical_walrus_blob_id;
#[cfg(feature = "transactions")]
pub use sui_move_ptb::CLOCK_OBJECT_ID;
use {
    self::interface::graph::RuntimeVertex,
    crate::{
        sui,
        types::{NexusContext, PackageVersion, TypeOrigins},
    },
};

/// Canonical finite credit account for one Tool and payment beneficiary.
pub type FiniteCredits =
    tool::tool_cashier::PolicyAccount<tool::finite_credits::Policy, tool::finite_credits::State>;

/// Canonical time pass account for one Tool and payment beneficiary.
pub type TimePass =
    tool::tool_cashier::PolicyAccount<tool::time_pass::Policy, tool::time_pass::State>;

fn package_scope(
    package: Option<&PackageVersion>,
) -> (sui::types::Address, sui::types::Address, &TypeOrigins) {
    static EMPTY_ORIGINS: std::sync::LazyLock<TypeOrigins> =
        std::sync::LazyLock::new(TypeOrigins::new);

    package.map_or(
        (
            sui::types::Address::ZERO,
            sui::types::Address::ZERO,
            &EMPTY_ORIGINS,
        ),
        |package| {
            (
                package.storage_id,
                package.initial_id,
                &package.type_origins,
            )
        },
    )
}

fn derive_object_id<T: sui::traits::ToBcs>(
    parent: sui::types::Address,
    tag: &sui::types::TypeTag,
    key: &T,
) -> anyhow::Result<sui::types::Address> {
    Ok(parent.derive_object_id(tag, &key.to_bcs()?))
}

/// Runs `f` with generated bindings scoped to the packages in `context`.
///
/// Storage IDs select Move call targets and exact datatype origins select type
/// identity. An absent role receives an inert zero scope; transaction builders
/// must reject calls for roles absent from [`NexusContext::packages`].
pub(crate) fn with_nexus_scope<R>(context: &NexusContext, f: impl FnOnce() -> R) -> R {
    let (primitives_storage, primitives_initial, primitives_origins) =
        package_scope(context.packages().primitives.as_ref());
    let (interface_storage, interface_initial, interface_origins) =
        package_scope(context.packages().interface.as_ref());
    let (tool_storage, tool_initial, tool_origins) =
        package_scope(context.packages().tool.as_ref());
    let (registry_storage, registry_initial, registry_origins) =
        package_scope(context.packages().registry.as_ref());
    let (workflow_storage, workflow_initial, workflow_origins) =
        package_scope(context.packages().workflow.as_ref());
    let (scheduler_storage, scheduler_initial, scheduler_origins) =
        package_scope(context.packages().scheduler.as_ref());

    move_std::with_packages(
        sui::types::Address::from_static("0x1"),
        sui::types::Address::from_static("0x1"),
        || {
            sui_framework::with_packages(
                sui::types::Address::from_static("0x2"),
                sui::types::Address::from_static("0x2"),
                || {
                    talus::with_packages(
                        context.us_token.package_id,
                        context.us_token.package_id,
                        || {
                            primitives::with_package_context(
                                primitives_storage,
                                primitives_initial,
                                primitives_origins,
                                || {
                                    interface::with_package_context(
                                        interface_storage,
                                        interface_initial,
                                        interface_origins,
                                        || {
                                            tool::with_package_context(
                                                tool_storage,
                                                tool_initial,
                                                tool_origins,
                                                || {
                                                    registry::with_package_context(
                                                        registry_storage,
                                                        registry_initial,
                                                        registry_origins,
                                                        || {
                                                            workflow::with_package_context(
                                                                workflow_storage,
                                                                workflow_initial,
                                                                workflow_origins,
                                                                || {
                                                                    scheduler::with_package_context(
                                                                        scheduler_storage,
                                                                        scheduler_initial,
                                                                        scheduler_origins,
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

/// Builds the canonical Move type tag for `T` in a [`NexusContext`].
///
/// The generated binding scope resolves datatype origins across package
/// upgrades before constructing the tag.
pub fn type_tag<T>(context: &NexusContext) -> sui::types::TypeTag
where
    T: sui_move::MoveType,
{
    with_nexus_scope(context, T::type_tag_static)
}

#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
fn registry_type_tag_from_origin<T>(
    registry_type_origin_pkg_id: sui::types::Address,
) -> sui::types::TypeTag
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
pub fn struct_tag<T>(context: &NexusContext) -> sui::types::StructTag
where
    T: sui_move::MoveStruct,
{
    with_nexus_scope(context, T::struct_tag_static)
}

/// Return whether `tag` matches the generated struct `T` identity scoped to this deployment.
pub fn struct_tag_matches<T>(context: &NexusContext, tag: &sui::types::StructTag) -> bool
where
    T: sui_move::MoveStruct,
{
    let expected = struct_tag::<T>(context);
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

/// Derive the canonical finite credit account for [beneficiary].
pub fn derive_finite_credits_id(
    context: &NexusContext,
    cashier: sui::types::Address,
    beneficiary: interface::payment::PaymentSourceKind,
) -> anyhow::Result<sui::types::Address> {
    type Key = tool::tool_cashier::PolicyAccountKey<tool::finite_credits::Policy>;
    derive_object_id(cashier, &type_tag::<Key>(context), &Key::new(beneficiary))
}

/// Derive the canonical time pass account for [beneficiary].
pub fn derive_time_pass_id(
    context: &NexusContext,
    cashier: sui::types::Address,
    beneficiary: interface::payment::PaymentSourceKind,
) -> anyhow::Result<sui::types::Address> {
    type Key = tool::tool_cashier::PolicyAccountKey<tool::time_pass::Policy>;
    derive_object_id(cashier, &type_tag::<Key>(context), &Key::new(beneficiary))
}

#[cfg(any(feature = "nexus", all(test, feature = "transactions")))]
pub(crate) fn derive_network_auth_binding_id(
    registry_type_origin_pkg_id: sui::types::Address,
    network_auth_object_id: sui::types::Address,
    identity: &IdentityKey,
) -> anyhow::Result<sui::types::Address> {
    let key_type = registry_type_tag_from_origin::<IdentityKey>(registry_type_origin_pkg_id);
    derive_object_id(network_auth_object_id, &key_type, identity)
}

/// Derive the task ID associated with a walk execution request event.
///
/// Pass the defining Interface package so the derived ID remains stable after
/// an Interface package upgrade.
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
            derive_finite_credits_id,
            derive_task_execution_id,
            derive_time_pass_id,
            derive_tool_cashier_id,
            derive_walk_execution_event_task_id,
            interface::graph::RuntimeVertex,
            registry,
        },
        crate::{
            move_bindings::interface::payment::PaymentSourceKind,
            sui,
            test_utils::sui_mocks::mock_nexus_context,
        },
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

    #[test]
    fn policy_account_ids_are_deterministic_and_policy_scoped() {
        let context = mock_nexus_context();
        let cashier = sui::types::Address::from_static("0x71");
        let beneficiary = PaymentSourceKind::user_funded(sui::types::Address::from_static("0x72"));

        let credits = derive_finite_credits_id(&context, cashier, beneficiary.clone()).unwrap();
        let repeated = derive_finite_credits_id(&context, cashier, beneficiary.clone()).unwrap();
        let pass = derive_time_pass_id(&context, cashier, beneficiary).unwrap();

        assert_eq!(credits, repeated);
        assert_ne!(credits, pass);
    }
}
