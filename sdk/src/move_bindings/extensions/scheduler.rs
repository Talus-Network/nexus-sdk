//! Scheduler conversions for generated Move bindings.

use crate::{
    move_bindings::scheduler::scheduler::OccurrenceAdvertisedEvent,
    scheduler::{DispatchOffer, OccurrenceRef, ScheduleError},
};

impl TryFrom<&OccurrenceAdvertisedEvent> for DispatchOffer {
    type Error = ScheduleError;

    fn try_from(event: &OccurrenceAdvertisedEvent) -> Result<Self, Self::Error> {
        Self::new(
            OccurrenceRef::new(event.task_id.bytes, event.occurrence_id),
            event.effective_start_time_ms,
            event.deadline_ms.copied_option(),
            event.priority_fee_percentage,
        )
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::move_bindings::{move_std::option::Option as MoveOption, sui_framework::object::ID},
    };

    fn event(
        start_time_ms: u64,
        deadline_ms: Option<u64>,
        priority_fee_percentage: u64,
    ) -> OccurrenceAdvertisedEvent {
        OccurrenceAdvertisedEvent::new(
            ID::new(crate::sui::types::Address::from_static("0xf")),
            9,
            start_time_ms,
            MoveOption::from_option(deadline_ms),
            priority_fee_percentage,
        )
    }

    #[test]
    fn advertised_occurrence_converts_to_validated_dispatch_offer() {
        let offer = DispatchOffer::try_from(&event(100, Some(120), 20)).unwrap();

        assert_eq!(
            offer.occurrence(),
            OccurrenceRef::new(crate::sui::types::Address::from_static("0xf"), 9)
        );
        assert_eq!(offer.effective_start_time_ms(), 100);
        assert_eq!(offer.deadline_ms(), Some(120));
        assert_eq!(offer.priority_fee_percentage(), 20);
        assert!(matches!(
            DispatchOffer::try_from(&event(100, Some(99), 20)),
            Err(ScheduleError::DeadlineBeforeStart { .. })
        ));
    }
}
