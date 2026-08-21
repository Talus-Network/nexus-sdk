use {
    crate::{
        move_boundary::NexusPtbBuilder, scheduler::SchedulerError,
        transactions::agent_input::AgentInput,
    },
    sui_move_call::{CallSpecError, CallTarget},
    sui_sdk_types::Argument,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResolvedAuthority {
    Address,
    Agent(AgentInput),
}

#[derive(Clone, Copy)]
enum AgentBorrow {
    Immutable,
    Mutable,
}

impl ResolvedAuthority {
    pub(crate) fn lower<T>(
        &self,
        tx: &mut NexusPtbBuilder,
        address: impl FnOnce(&mut NexusPtbBuilder) -> Result<T, SchedulerError>,
        agent: impl FnOnce(&mut NexusPtbBuilder, AgentInput) -> Result<T, SchedulerError>,
    ) -> Result<T, SchedulerError> {
        match self {
            Self::Address => address(tx),
            Self::Agent(input) => agent(tx, input.clone()),
        }
    }

    pub(crate) fn call(
        &self,
        tx: &mut NexusPtbBuilder,
        address_target: impl FnOnce() -> Result<CallTarget, CallSpecError>,
        agent_target: impl FnOnce() -> Result<CallTarget, CallSpecError>,
        task: Argument,
        tail: Vec<Argument>,
    ) -> Result<Argument, SchedulerError> {
        self.call_with_agent_borrow(
            tx,
            address_target,
            agent_target,
            task,
            tail,
            AgentBorrow::Immutable,
        )
    }

    pub(crate) fn call_mutably(
        &self,
        tx: &mut NexusPtbBuilder,
        address_target: impl FnOnce() -> Result<CallTarget, CallSpecError>,
        agent_target: impl FnOnce() -> Result<CallTarget, CallSpecError>,
        task: Argument,
        tail: Vec<Argument>,
    ) -> Result<Argument, SchedulerError> {
        self.call_with_agent_borrow(
            tx,
            address_target,
            agent_target,
            task,
            tail,
            AgentBorrow::Mutable,
        )
    }

    fn call_with_agent_borrow(
        &self,
        tx: &mut NexusPtbBuilder,
        address_target: impl FnOnce() -> Result<CallTarget, CallSpecError>,
        agent_target: impl FnOnce() -> Result<CallTarget, CallSpecError>,
        task: Argument,
        tail: Vec<Argument>,
        borrow: AgentBorrow,
    ) -> Result<Argument, SchedulerError> {
        let address_tail = tail.clone();
        self.lower(
            tx,
            |tx| {
                tx.call_target(
                    address_target,
                    std::iter::once(task).chain(address_tail).collect(),
                )
                .map_err(SchedulerError::transaction)
            },
            |tx, agent| {
                let agent = match borrow {
                    AgentBorrow::Immutable => agent.clone().immutable_ptb_argument(tx),
                    AgentBorrow::Mutable => agent.clone().mutable_ptb_argument(tx),
                }
                .map_err(SchedulerError::transaction)?;
                let arguments = [task, agent].into_iter().chain(tail).collect();
                tx.call_target(agent_target, arguments)
                    .map_err(SchedulerError::transaction)
            },
        )
    }
}
