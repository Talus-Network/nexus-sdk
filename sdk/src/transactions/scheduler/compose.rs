use {
    super::{
        authority::ResolvedAuthority,
        command::{
            append_schedule, create_unshared_task, finish_task, PreparedSchedule, PreparedTask,
        },
    },
    crate::{move_boundary::NexusPtbBuilder, scheduler::SchedulerError},
    sui_sdk_types::Argument,
};

pub(crate) struct TaskDraftCompiler<'builder> {
    transaction: &'builder mut NexusPtbBuilder,
    task: Argument,
    pointer: Argument,
    authority: ResolvedAuthority,
}

impl<'builder> TaskDraftCompiler<'builder> {
    pub(crate) fn create(
        transaction: &'builder mut NexusPtbBuilder,
        task: &PreparedTask,
    ) -> Result<Self, SchedulerError> {
        let created = create_unshared_task(transaction, task)?;
        Ok(Self {
            transaction,
            task: created.task,
            pointer: created.pointer,
            authority: created.authority,
        })
    }

    pub(crate) fn schedule(self, schedule: &PreparedSchedule) -> Result<Self, SchedulerError> {
        append_schedule(self.transaction, self.task, &self.authority, schedule)?;
        Ok(self)
    }

    pub(crate) fn share(
        self,
        pointer_owner: crate::sui::types::Address,
    ) -> Result<(), SchedulerError> {
        finish_task(self.transaction, self.task, self.pointer, pointer_owner)
    }
}
