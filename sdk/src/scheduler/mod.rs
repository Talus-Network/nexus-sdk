//! Canonical authoring and observation types for scheduled Tasks.
//!
//! Scheduling is the only public work request model:
//! [`TaskSpec`](crate::scheduler::TaskSpec) describes the work and its
//! funding, while [`Schedule`](crate::scheduler::Schedule) describes when
//! occurrences of that work may be dispatched.
//!
//! Empty schedules are valid composition values. Use
//! [`Schedule::validate_for_task_creation`](crate::scheduler::Schedule::validate_for_task_creation)
//! when invoking an atomic create and schedule shortcut that requires work:
//!
//! ```
//! use nexus_sdk::scheduler::{Occurrence, Schedule};
//!
//! let schedule = Schedule::new()
//!     .with_occurrence(Occurrence::now())
//!     .with_occurrence(Occurrence::at_ms(2_000));
//!
//! assert_eq!(schedule.occurrences().len(), 2);
//! assert!(schedule.validate_for_task_creation().is_ok());
//! ```

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

mod error;
mod model;
mod receipt;
mod snapshot;

pub use {
    error::{ErrorSource, ScheduleError, SchedulerError},
    model::{
        AuthorizationTemplate, DispatchOffer, FailurePolicy, Occurrence, OccurrenceRef, Recurrence,
        Schedule, TaskFunding, TaskInputs, TaskOperation, TaskSpec,
    },
    receipt::{
        AbortReceipt, ScheduleDelta, ScheduledOccurrence, TaskMutationReceipt,
        TransactionReference, WithdrawnOccurrence,
    },
    snapshot::{
        ExecutionSnapshot, OccurrenceCost, OccurrencePage, OccurrenceSnapshot, OccurrenceSource,
        OccurrenceStatus, TaskController, TaskPointer, TaskPointerPage, TaskSnapshot, TaskStatus,
        WatchOptions, WithdrawalReason,
    },
};
pub(crate) use {
    model::{Deadline, StartTime},
    snapshot::ExecutionObservation,
};
