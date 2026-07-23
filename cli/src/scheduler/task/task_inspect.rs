use {
    crate::{
        command_title,
        display::json_output,
        item,
        loading,
        notify_success,
        prelude::*,
        sui::*,
    },
    serde_json::json,
};

/// Reads a Task and reports its execution and scheduling state.
pub(crate) async fn inspect_task(task_id: sui::types::Address) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting scheduled Task '{task_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let objects = loading!("Fetching Task object...");
    let task = nexus_client
        .scheduler()
        .fetch_task(task_id)
        .await
        .map_err(NexusCliError::Nexus)?;
    objects.success();

    let task_ref = task.object_ref();
    let task_data = task.data;
    notify_success!(
        "Task controller: {controller:?}",
        controller = task_data.controller
    );
    item!(
        "Pending occurrences: {count}",
        count = task_data.schedule.pending.len()
    );
    item!(
        "In flight occurrences: {count}",
        count = task_data.in_flight.size
    );

    json_output(&json!({
        "task_ref": {
            "object_id": task_ref.object_id(),
            "version": task_ref.version(),
            "digest": task_ref.digest(),
        },
        "task": task_data,
    }))
}
