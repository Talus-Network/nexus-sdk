#![cfg(feature = "test_utils")]

use nexus_sdk::move_bindings::scheduler::scheduler::State as TaskState;

#[test]
fn task_state_roundtrips_through_bcs() {
    for state in [
        TaskState::Active,
        TaskState::Paused,
        TaskState::Canceled,
        TaskState::Completed,
        TaskState::Failed,
    ] {
        let bytes = bcs::to_bytes(&state).expect("bcs serialize TaskState");

        let decoded: TaskState = bcs::from_bytes(&bytes).expect("bcs deserialize TaskState");
        assert_eq!(decoded, state);
    }
}
