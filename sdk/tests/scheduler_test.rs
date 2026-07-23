#![cfg(feature = "test_utils")]

use nexus_sdk::move_bindings::scheduler::task::TaskStatus;

#[test]
fn task_state_roundtrips_through_bcs() {
    for state in [TaskStatus::Active, TaskStatus::Paused, TaskStatus::Canceled] {
        let bytes = bcs::to_bytes(&state).expect("bcs serialize TaskStatus");

        let decoded: TaskStatus = bcs::from_bytes(&bytes).expect("bcs deserialize TaskStatus");
        assert_eq!(decoded, state);
    }
}
