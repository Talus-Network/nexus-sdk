module onchain_tool::execution;

//! Defines only the active current-execution type identity needed by schema-generation tests.

/// This local key object keeps the publish fixture dependency-free while matching the active Nexus ABI identity.
public struct DAGExecution has key {
    id: UID,
}
