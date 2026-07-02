module onchain_tool::onchain_tool;

use onchain_tool::onchain_tool_result::{Self as onchain_tool_result, OnchainToolResult};
use std::ascii::String as AsciiString;
use sui::bag::{Self, Bag};
use sui::transfer::share_object;
use onchain_tool::proof_of_uid::ProofOfUID;

/// One-time witness for package initialization.
public struct ONCHAIN_TOOL has drop {}

/// Witness object used to identify this tool.
public struct OnchainToolWitness has key, store {
    id: UID,
}

/// Random counter used for demonstration purposes. This is up to the tool dev.
public struct RandomCounter has key {
    id: UID,
    count: u64,
    /// Store the witness object that identifies this tool.
    witness: Bag,
}

/// Tool execution output variants.
/// update: We only use this for when we register the tool. The SDK automatically fetches
/// the output schema from this enum. It is not used in the tool execution. Instead, we use
/// the new `TaggedOutput` object.
public enum Output {
    Ok {
        old_count: u64,
        new_count: u64,
        increment: u64,
    },
    Err {
        reason: AsciiString,
    },
    LargeIncrement {
        old_count: u64,
        new_count: u64,
        increment: u64,
        warning: AsciiString,
    },
}

/// Initialize the counter for testing purposes.
fun init(_otw: ONCHAIN_TOOL, ctx: &mut TxContext) {
    let counter = RandomCounter {
        id: object::new(ctx),
        count: 0,
        witness: {
            let mut bag = bag::new(ctx);
            bag.add(b"witness", OnchainToolWitness { id: object::new(ctx) });
            bag
        },
    };
    share_object(counter);
}

/// Execute function that takes a ProofOfUID worksheet as its first argument.
/// The tool must stamp the worksheet with the tool witness ID to prove it was executed.
/// This allows the Nexus framework to verify that the tool was actually invoked.
///
/// This execute function implements conditional logic:
/// - If increase_with > 0, the counter is increased and execution succeeds.
/// - If increase_with = 0, the execution should return an error output variant.
/// - If increase_with > 100, the counter is increased and returns the LargeIncrement variant.
///
/// The production Nexus ABI finalizes output through an owned OnchainToolResult
/// argument and does not return values from execute.
entry fun execute(
    worksheet: ProofOfUID,
    result: OnchainToolResult,
    counter: &mut RandomCounter,
    increase_with: u64,
    _ctx: &mut TxContext,
) {
    let old_count = counter.count;
    let ProofOfUID { id } = worksheet;
    object::delete(id);
    onchain_tool_result::delete_for_testing(result);
    let _ = old_count;
    let _ = increase_with;
}

// === Getters ===

/// Get the count value.
public fun count(self: &RandomCounter): u64 {
    self.count
}

/// Helper function to get the witness object.
fun witness(self: &RandomCounter): &OnchainToolWitness {
    self.witness.borrow(b"witness")
}

public fun tool_witness_id(self: &RandomCounter): ID {
    object::uid_to_inner(&self.witness().id)
}

#[test_only]
public fun init_for_test(otw: ONCHAIN_TOOL, ctx: &mut TxContext) {
    init(otw, ctx);
}
