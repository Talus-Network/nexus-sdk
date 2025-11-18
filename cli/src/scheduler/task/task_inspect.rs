use {
    crate::{
        command_title,
        display::json_output,
        item,
        loading,
        notify_success,
        prelude::*,
        sui::build_sui_client,
    },
    nexus_sdk::{
        object_crawler::{fetch_one, Structure},
        types::Task,
    },
    serde_json::json,
};

/// Inspect a scheduler task and display metadata plus raw JSON output.
pub(crate) async fn inspect_task(task_id: sui::ObjectID) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting scheduler task '{task_id}'", task_id = task_id);

    // Load CLI configuration.
    let conf_handle = loading!("Loading CLI configuration...");
    let conf = CliConf::load().await.unwrap_or_default();
    conf_handle.success();

    // Build Sui client.
    let sui = build_sui_client(&conf.sui).await?;
    let objects_handle = loading!("Fetching task object...");
    // Fetch the task object from chain.
    let task = fetch_one::<Structure<Task>>(&sui, task_id)
        .await
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;
    objects_handle.success();

    let task_ref = task.object_ref();
    let task_data = task.data.into_inner();

    notify_success!(
        "Task owner: {owner}",
        owner = task_data.owner.to_string().truecolor(100, 100, 100)
    );

    if let Some(metadata) = task_data.metadata.as_object() {
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
    } else {
        item!("Metadata entries: 0");
    }

    item!(
        "Constraints payload bytes: {bytes}",
        bytes = task_data.constraints.to_string().len()
    );
    item!(
        "Execution payload bytes: {bytes}",
        bytes = task_data.execution.data.to_string().len()
    );

    json_output(&json!({
        "task_ref": {
            "object_id": task_ref.object_id,
            "version": task_ref.version,
            "digest": task_ref.digest,
        },
        "task": task_data,
    }))?;

    Ok(())
}
