module onchain_tool::onchain_tool_result;

/// Minimal local placeholder used only by SDK schema-generation tests.
public struct OnchainToolResult has key {
    id: UID,
}

public fun delete_for_testing(result: OnchainToolResult) {
    let OnchainToolResult { id } = result;
    object::delete(id);
}
