module onchain_tool::proof_of_uid;

/// Placeholder for the execution requirements supplied by the Nexus framework.
public struct UIDRequirements {
    id: UID,
}

public fun delete(requirements: UIDRequirements) {
    let UIDRequirements { id } = requirements;
    id.delete();
}
