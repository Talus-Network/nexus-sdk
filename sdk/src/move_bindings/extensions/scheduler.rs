//! Scheduler conversions for generated Move bindings.

use crate::{
    move_bindings::{
        interface::agent::SkillSchedulePolicy,
        scheduler::{
            schedule::Schedule,
            scheduler::OccurrenceAdvertisedEvent,
            task::{TaskInnerV1, TaskStatus},
        },
    },
    scheduler::{DispatchOffer, OccurrenceRef, ScheduleError},
};

impl TaskInnerV1 {
    /// Returns the current dispatch proposal without reading past advertisements.
    ///
    /// # Errors
    ///
    /// Returns an error when the current proposal violates scheduling bounds.
    pub fn advertised_offer(
        &self,
        task_id: crate::sui::types::Address,
    ) -> Result<Option<DispatchOffer>, ScheduleError> {
        if self.status != TaskStatus::Active {
            return Ok(None);
        }
        advertised_offer(&self.schedule, &self.schedule_policy, task_id)
    }
}

fn advertised_offer(
    schedule: &Schedule,
    policy: &SkillSchedulePolicy,
    task_id: crate::sui::types::Address,
) -> Result<Option<DispatchOffer>, ScheduleError> {
    let Some(id) = schedule.advertised_occurrence_id.copied_option() else {
        return Ok(None);
    };
    let occurrence = schedule
        .pending
        .iter()
        .find(|occurrence| occurrence.id == id)
        .or_else(|| {
            schedule
                .recurrence
                .as_option()
                .map(|recurrence| &recurrence.next)
                .filter(|occurrence| occurrence.id == id)
        });
    let Some(occurrence) = occurrence else {
        return Ok(None);
    };
    let minimum_interval_ms = match policy {
        SkillSchedulePolicy::Once => 0,
        SkillSchedulePolicy::Recurring {
            min_interval_ms, ..
        } => *min_interval_ms,
    };
    let effective_start = schedule
        .last_dispatch_ms
        .copied_option()
        .map_or(0, |last| last.saturating_add(minimum_interval_ms))
        .max(occurrence.start_time_ms);
    DispatchOffer::new(
        OccurrenceRef::new(task_id, occurrence.id),
        effective_start,
        occurrence.deadline_ms.copied_option(),
        occurrence.priority_fee_percentage,
    )
    .map(Some)
}

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
        crate::move_bindings::{
            move_std::option::Option as MoveOption,
            scheduler::schedule::{
                Occurrence as MoveOccurrence,
                OccurrenceSource,
                Recurrence,
                Schedule as MoveSchedule,
            },
            sui_framework::object::ID,
        },
    };

    fn advertised_occurrence(
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
        let offer = DispatchOffer::try_from(&advertised_occurrence(100, Some(120), 20)).unwrap();

        assert_eq!(
            offer.occurrence(),
            OccurrenceRef::new(crate::sui::types::Address::from_static("0xf"), 9)
        );
        assert_eq!(offer.effective_start_time_ms(), 100);
        assert_eq!(offer.deadline_ms(), Some(120));
        assert_eq!(offer.priority_fee_percentage(), 20);
        assert!(matches!(
            DispatchOffer::try_from(&advertised_occurrence(100, Some(99), 20)),
            Err(ScheduleError::DeadlineBeforeStart { .. })
        ));
    }
    fn address(value: &'static str) -> crate::sui::types::Address {
        crate::sui::types::Address::from_static(value)
    }

    fn offer(task: crate::sui::types::Address) -> DispatchOffer {
        DispatchOffer::new(OccurrenceRef::new(task, 7), 100, Some(120), 20).unwrap()
    }

    fn move_occurrence(
        id: u64,
        start: u64,
        deadline: Option<u64>,
        priority: u64,
    ) -> MoveOccurrence {
        MoveOccurrence::new(
            id,
            start,
            MoveOption::from_option(deadline),
            priority,
            OccurrenceSource::Standalone,
        )
    }

    fn move_schedule(occurrence: MoveOccurrence, last_dispatch_ms: Option<u64>) -> MoveSchedule {
        MoveSchedule::new(
            vec![occurrence],
            MoveOption::from_option(None::<Recurrence>),
            MoveOption::from_option(Some(7)),
            8,
            0,
            MoveOption::from_option(last_dispatch_ms),
        )
    }

    #[test]
    fn current_offer_is_derived_from_the_exact_schedule_state() {
        let task = address("0x50");
        let schedule = move_schedule(move_occurrence(7, 90, Some(120), 20), Some(80));
        let policy = SkillSchedulePolicy::Recurring {
            min_interval_ms: 20,
            max_occurrences: MoveOption::from_option(None),
        };

        assert_eq!(
            advertised_offer(&schedule, &policy, task).unwrap(),
            Some(offer(task))
        );
    }

    #[test]
    fn effective_start_uses_the_protocol_saturating_interval() {
        let task = address("0x50");
        let schedule = move_schedule(move_occurrence(7, 1, None, 20), Some(u64::MAX - 5));
        let policy = SkillSchedulePolicy::Recurring {
            min_interval_ms: 10,
            max_occurrences: MoveOption::from_option(None),
        };
        let offer = DispatchOffer::new(OccurrenceRef::new(task, 7), u64::MAX, None, 20)
            .expect("valid saturated offer");

        assert_eq!(
            advertised_offer(&schedule, &policy, task).unwrap(),
            Some(offer)
        );
    }

    #[test]
    fn advertised_recurrence_is_derived_when_pending_is_empty() {
        let task = address("0x50");
        let next = MoveOccurrence::new(
            7,
            100,
            MoveOption::from_option(Some(120)),
            20,
            OccurrenceSource::Recurring { iteration: 4 },
        );
        let schedule = MoveSchedule::new(
            Vec::new(),
            MoveOption::from_option(Some(Recurrence::new(
                next,
                50,
                MoveOption::from_option(Some(2)),
                4,
            ))),
            MoveOption::from_option(Some(7)),
            8,
            3,
            MoveOption::from_option(Some(80)),
        );
        let policy = SkillSchedulePolicy::Recurring {
            min_interval_ms: 20,
            max_occurrences: MoveOption::from_option(Some(6)),
        };

        assert_eq!(
            advertised_offer(&schedule, &policy, task).unwrap(),
            Some(offer(task))
        );
    }
}
