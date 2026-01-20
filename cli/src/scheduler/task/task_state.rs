use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::nexus::scheduler::TaskStateAction,
    serde_json::json,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum TaskStateRequest {
    Pause,
    Resume,
    Cancel,
}

impl std::fmt::Display for TaskStateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let verb = match self {
            TaskStateRequest::Pause => "Pausing",
            TaskStateRequest::Resume => "Resuming",
            TaskStateRequest::Cancel => "Canceling",
        };
        write!(f, "{verb}")
    }
}

/// Toggle scheduler task state between paused, resumed, or canceled.
pub(crate) async fn set_task_state(
    task_id: sui::types::Address,
    gas: GasArgs,
    request: TaskStateRequest,
) -> AnyResult<(), NexusCliError> {
    command_title!("{request} scheduler task '{task_id}'");

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;

    let action = match request {
        TaskStateRequest::Pause => TaskStateAction::Pause,
        TaskStateRequest::Resume => TaskStateAction::Resume,
        TaskStateRequest::Cancel => TaskStateAction::Cancel,
    };

    let result = nexus_client
        .scheduler()
        .set_task_state(task_id, action)
        .await
        .map_err(NexusCliError::Nexus)?;

    match request {
        TaskStateRequest::Pause => notify_success!("Task paused"),
        TaskStateRequest::Resume => notify_success!("Task resumed"),
        TaskStateRequest::Cancel => notify_success!("Task canceled"),
    }

    json_output(&json!({
        "digest": result.tx_digest,
        "task_id": task_id,
        "state": match request {
            TaskStateRequest::Pause => "paused",
            TaskStateRequest::Resume => "resumed",
            TaskStateRequest::Cancel => "canceled",
        },
    }))?;

    Ok(())
}
