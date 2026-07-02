//! Helper for deriving Sui object IDs.

use {
    crate::{
        idents::{interface, move_std},
        sui::{self, traits::ToBcs},
        types::RuntimeVertex,
        ToolFqn,
    },
    serde::Serialize,
};

/// Generic function for deriving an object ID given a parent, type tag, and key.
pub fn derive_object_id<T: Serialize>(
    parent: sui::types::Address,
    tag: &sui::types::TypeTag,
    key: &T,
) -> anyhow::Result<sui::types::Address> {
    Ok(parent.derive_object_id(tag, &key.to_bcs()?))
}

/// Derives the object ID for a Tool given the tool registry and tool FQN.
pub fn derive_tool_id(
    tool_registry: sui::types::Address,
    tool_fqn: &ToolFqn,
) -> anyhow::Result<sui::types::Address> {
    let tag = move_std::into_type_tag(move_std::Ascii::STRING_TYPE);

    derive_object_id(tool_registry, &tag, &tool_fqn)
}

/// Derives the object ID for a ToolGas object given the gas service and tool FQN.
pub fn derive_tool_gas_id(
    gas_service: sui::types::Address,
    tool_fqn: &ToolFqn,
) -> anyhow::Result<sui::types::Address> {
    let tag = move_std::into_type_tag(move_std::Ascii::STRING_TYPE);

    derive_object_id(gas_service, &tag, &tool_fqn)
}

/// Derive the object ID for a [`RequestWalkExecutionEvent`] task given the
/// execution ID and the [`RuntimeVertex`].
///
/// Note that this ID derived twice - first type based on the vertex name and
/// second time based on the iteration.
///
/// [`RequestWalkExecutionEvent`]: crate::types::workflow::execution_events::RequestWalkExecutionEvent
/// [`RuntimeVertex`]: crate::types::RuntimeVertex
pub fn derive_walk_execution_event_task_id(
    interface_pkg_id: sui::types::Address,
    execution: sui::types::Address,
    vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Address> {
    let (name, iteration) = match &vertex {
        RuntimeVertex::Plain { vertex } => (vertex, &0),
        RuntimeVertex::WithIterator {
            vertex, iteration, ..
        } => (vertex, iteration),
    };

    let vertex_tag = interface::into_type_tag(interface_pkg_id, interface::Graph::VERTEX);

    derive_object_id(
        derive_object_id(execution, &vertex_tag, &name)?,
        &sui::types::TypeTag::U64,
        &iteration,
    )
}

/// Derive the object ID for a [`OccurrenceScheduledEvent`] task given the
/// execution ID and the expected start time.
///
/// [`OccurrenceScheduledEvent`]: crate::types::scheduler::scheduler::OccurrenceScheduledEvent
pub fn derive_occurrence_scheduled_event_task_id(
    task: sui::types::Address,
    start_time_ms: &u64,
) -> anyhow::Result<sui::types::Address> {
    derive_object_id(task, &sui::types::TypeTag::U64, start_time_ms)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::fqn};

    #[test]
    fn test_derive_tool_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        // 1
        let fqn = fqn!("xyz.taluslabs.math.i64.add@1");
        let expected_id = sui::types::Address::from_static(
            "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35",
        );
        let derived_id = derive_tool_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);

        // 2
        let fqn = fqn!("xyz.taluslabs.math.i64.mul@1");
        let expected_id = sui::types::Address::from_static(
            "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da",
        );
        let derived_id = derive_tool_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_tool_gas_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );

        // 1
        let fqn = fqn!("xyz.taluslabs.math.i64.add@1");
        let expected_id = sui::types::Address::from_static(
            "0x63152163bf12d54f38742656cba5d37a05e89d3ef5df7e9d22062e7bff0aed35",
        );
        let derived_id = derive_tool_gas_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);

        // 2
        let fqn = fqn!("xyz.taluslabs.math.i64.mul@1");
        let expected_id = sui::types::Address::from_static(
            "0xc841b225a7e79c76942f3df05f1fcf17c2b259626ed51cb84e562cb3403604da",
        );
        let derived_id = derive_tool_gas_id(registry_id, &fqn).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_walk_execution_event_task_id() {
        let workflow_pkg_id = sui::types::Address::from_static(
            "0x2b1edf076c7ca1d6db65f1b2afd671ef35f64483d12ef4c78e0c63b2822596d8",
        );
        let execution_id = sui::types::Address::from_static(
            "0x322e905ae56c5c89c54ebc77e21c78d2bf6341ba4e39e83b04fcdb7f3f5c0037",
        );
        let vertex = RuntimeVertex::plain("dummy");
        let expected_id = sui::types::Address::from_static(
            "0x01f8994c6f49b74a625da5fad442f350555752aefbdc9f4ce23d7c91b92f6b29",
        );

        let derived_id =
            derive_walk_execution_event_task_id(workflow_pkg_id, execution_id, &vertex).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_occurrence_scheduled_event_task_id() {
        let task_id = sui::types::Address::from_static(
            "0x322e905ae56c5c89c54ebc77e21c78d2bf6341ba4e39e83b04fcdb7f3f5c0037",
        );
        let start_time_ms = 1_000_000;
        let expected_id = sui::types::Address::from_static(
            "0x6e3c05b00082994ce88cedb4ed5c2c6b0113de4945d726830bad158cc4e70772",
        );

        let derived_id =
            derive_occurrence_scheduled_event_task_id(task_id, &start_time_ms).unwrap();

        assert_eq!(derived_id, expected_id);
    }
}
