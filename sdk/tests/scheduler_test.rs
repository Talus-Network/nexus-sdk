#![cfg(feature = "test_utils")]

use nexus_sdk::{
    move_bindings::{
        scheduler::{
            schedule::OccurrenceWithdrawalReason,
            task::{OccurrenceState, TaskStatus},
        },
        sui_framework::object::ID,
    },
    sui,
};

#[test]
fn task_state_roundtrips_through_bcs() {
    for state in [
        TaskStatus::Active,
        TaskStatus::Paused,
        TaskStatus::Canceled,
        TaskStatus::Finalized,
    ] {
        let bytes = bcs::to_bytes(&state).expect("bcs serialize TaskStatus");

        let decoded: TaskStatus = bcs::from_bytes(&bytes).expect("bcs deserialize TaskStatus");
        assert_eq!(decoded, state);
    }
}

#[test]
fn occurrence_state_roundtrips_through_bcs() {
    let execution_id = ID::new(sui::types::Address::from_static("0x1"));

    for state in [
        OccurrenceState::Scheduled,
        OccurrenceState::Dispatched {
            execution_id,
            dispatched_at_ms: 10,
        },
        OccurrenceState::Missed { missed_at_ms: 20 },
        OccurrenceState::Withdrawn {
            reason: OccurrenceWithdrawalReason::TaskCanceled,
        },
        OccurrenceState::Settled {
            execution_id,
            dispatched_at_ms: 10,
            settled_at_ms: 30,
            succeeded: true,
        },
    ] {
        let bytes = bcs::to_bytes(&state).expect("bcs serialize OccurrenceState");

        let decoded: OccurrenceState =
            bcs::from_bytes(&bytes).expect("bcs deserialize OccurrenceState");
        assert_eq!(decoded, state);
    }
}
