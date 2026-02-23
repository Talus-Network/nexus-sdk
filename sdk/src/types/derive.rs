//! Helper for deriving Sui object IDs.

use {
    crate::{
        idents::{move_std, sui_framework, workflow},
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

/// Derives the object ID for an InvokerGas object given the gas service and invoker address.
pub fn derive_invoker_gas_id(
    gas_service: sui::types::Address,
    invoker: sui::types::Address,
) -> anyhow::Result<sui::types::Address> {
    let tag = sui::types::TypeTag::Address;

    derive_object_id(gas_service, &tag, &invoker)
}

/// Derives the object ID for an ExecutionGas object given the gas service and execution ID.
pub fn derive_execution_gas_id(
    gas_service: sui::types::Address,
    execution: sui::types::Address,
) -> anyhow::Result<sui::types::Address> {
    let tag = sui_framework::into_type_tag(sui_framework::Object::ID);

    derive_object_id(gas_service, &tag, &execution)
}

/// Derive the object ID for a [`RequestWalkExecutionEvent`] task given the
/// execution ID and the [`RuntimeVertex`].
///
/// Note that this ID derived twice - first type based on the vertex name and
/// second time based on the iteration.
///
/// [`RequestWalkExecutionEvent`]: crate::events::RequestWalkExecutionEvent
/// [`RuntimeVertex`]: crate::types::RuntimeVertex
pub fn derive_walk_execution_event_task_id(
    workflow_pkg_id: sui::types::Address,
    execution: sui::types::Address,
    vertex: &RuntimeVertex,
) -> anyhow::Result<sui::types::Address> {
    let (name, iteration) = match &vertex {
        RuntimeVertex::Plain { vertex } => (vertex, &0),
        RuntimeVertex::WithIterator {
            vertex, iteration, ..
        } => (vertex, iteration),
    };

    let vertex_tag = workflow::into_type_tag(workflow_pkg_id, workflow::Dag::VERTEX);

    derive_object_id(
        derive_object_id(execution, &vertex_tag, &name)?,
        &sui::types::TypeTag::U64,
        &iteration,
    )
}

/// Derive the object ID for a [`OccurrenceScheduledEvent`] task given the
/// execution ID and the expected start time.
///
/// [`OccurrenceScheduledEvent`]: crate::events::OccurrenceScheduledEvent
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
    fn test_derive_invoker_gas_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );
        let address = sui::types::Address::from_static(
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        );
        let expected_id = sui::types::Address::from_static(
            "0x62000f053c3d54fa76229a93d255b5d40584c374b9c84aefee95bfd91a9d6bb1",
        );
        let derived_id = derive_invoker_gas_id(registry_id, address).unwrap();
        assert_eq!(derived_id, expected_id);
    }

    #[test]
    fn test_derive_execution_gas_id() {
        let registry_id = sui::types::Address::from_static(
            "0x940f0dd81d4e4ae2cd476ff61ca5699e0d9356e1874d6c4ba3a5bdf28e67b9e9",
        );
        let execution_id = sui::types::Address::from_static(
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        );
        let expected_id = sui::types::Address::from_static(
            "0x6cccb4844b7b37cfc085b976b9bb84f46763df213deea769072804164ffdb875",
        );
        let derived_id = derive_execution_gas_id(registry_id, execution_id).unwrap();
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
            "0xe6e6b5f315493c79d7cdfcc8fb4a40cb7264434a83df8e509e0fc159ca85798f",
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
