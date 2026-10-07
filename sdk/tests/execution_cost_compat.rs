#![cfg(feature = "nexus")]

use nexus_sdk::{nexus::workflow::ExecutionCostResult, sui::types::Address};

#[test]
fn execution_cost_preserves_the_published_struct_shape() {
    let result = ExecutionCostResult {
        payment_id: Address::ZERO,
        max_budget_mist: 100,
        locked_budget_mist: 20,
        consumed: 30,
        outstanding_locks: 1,
        accomplished: false,
        refunded: false,
    };
    let ExecutionCostResult {
        payment_id,
        max_budget_mist,
        locked_budget_mist,
        consumed,
        outstanding_locks,
        accomplished,
        refunded,
    } = result;
    assert_eq!(payment_id, Address::ZERO);
    assert_eq!(
        (max_budget_mist, locked_budget_mist, consumed),
        (100, 20, 30)
    );
    assert_eq!(outstanding_locks, 1);
    assert!(!accomplished && !refunded);
}
