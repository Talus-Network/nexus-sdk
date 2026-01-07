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
    nexus_sdk::types::Task,
    serde_json::json,
};

/// Inspect a scheduler task and display metadata plus raw JSON output.
pub(crate) async fn inspect_task(task_id: sui::types::Address) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting scheduler task '{task_id}'", task_id = task_id);

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let crawler = nexus_client.crawler();

    let objects_handle = loading!("Fetching task object...");

    // Fetch the task object from chain.
    let task = crawler
        .get_object::<Task>(task_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    objects_handle.success();

    let task_ref = task.object_ref();
    let task_data = task.data;

    notify_success!(
        "Task owner: {owner}",
        owner = task_data.owner.to_string().truecolor(100, 100, 100)
    );

    let metadata = task_data.metadata.values.inner();
    item!("Metadata entries: {count}", count = metadata.len());
    for (key, value) in metadata.iter().take(10) {
        item!(
            "  {key}: {value}",
            key = key.truecolor(100, 100, 100),
            value = value
        );
    }
    if metadata.len() > 10 {
        item!(
            "  ... ({remain} more entries)",
            remain = metadata.len() - 10
        );
    }

    json_output(&json!({
        "task_ref": {
            "object_id": task_ref.object_id(),
            "version": task_ref.version(),
            "digest": task_ref.digest(),
        },
        "task": task_data,
    }))?;

    Ok(())
}
