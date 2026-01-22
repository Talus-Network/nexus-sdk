module onchain_tool::onchain_tool;

use std::ascii::String as AsciiString;
use sui::bag::{Self, Bag};
use sui::transfer::share_object;

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

/// Placeholder for ProofOfUID.
public struct ProofOfUID {
    id: UID,
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
/// The tool must stamp the worksheet with the witness ID to prove it was executed.
/// This allows the Nexus framework to verify that the tool was actually invoked.
///
/// This execute function implements conditional logic:
/// - If increase_with > 0, the counter is increased and execution succeeds.
/// - If increase_with = 0, the execution should return an error output variant.
/// - If increase_with > 100, the counter is increased and returns the LargeIncrement variant.
///
/// We also need to return output data from the onchain tool execution.
/// We can do this in various ways, one being by simply returning the data directly
/// from the execute function call. When we do this, we need to consume this output
/// data for the next call, so we make it a hot potato that is being consumed in submit_on_chain_tool_eval_for_walk.
public fun execute(
    worksheet: &mut ProofOfUID,
    counter: &mut RandomCounter,
    increase_with: u64,
    _ctx: &mut TxContext,
) {
    let old_count = counter.count;
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

public fun witness_id(self: &RandomCounter): ID {
    self.witness().id.to_inner()
}

#[test_only]
public fun init_for_test(otw: ONCHAIN_TOOL, ctx: &mut TxContext) {
    init(otw, ctx);
}
