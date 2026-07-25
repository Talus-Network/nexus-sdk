use {
    super::{
        authority::ResolvedAuthority,
        command::{
            append_schedule,
            create_unshared_task,
            share_task,
            PreparedSchedule,
            PreparedTask,
        },
    },
    crate::{move_boundary::NexusPtbBuilder, scheduler::SchedulerError},
    sui_sdk_types::Argument,
};

pub(crate) struct TaskDraftCompiler<'builder, 'objects> {
    transaction: &'builder mut NexusPtbBuilder<'objects>,
    task: Argument,
    authority: ResolvedAuthority,
}

impl<'builder, 'objects> TaskDraftCompiler<'builder, 'objects> {
    pub(crate) fn create(
        transaction: &'builder mut NexusPtbBuilder<'objects>,
        task: &PreparedTask,
    ) -> Result<Self, SchedulerError> {
        let (task, authority) = create_unshared_task(transaction, task)?;
        Ok(Self {
            transaction,
            task,
            authority,
        })
    }

    pub(crate) fn schedule(self, schedule: &PreparedSchedule) -> Result<Self, SchedulerError> {
        append_schedule(self.transaction, self.task, &self.authority, schedule)?;
        Ok(self)
    }

    pub(crate) fn share(self) -> Result<(), SchedulerError> {
        share_task(self.transaction, self.task)
    }
}
