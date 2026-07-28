use {
    crate::{
        command_title,
        display::json_output,
        loading,
        prelude::*,
        sui::get_read_only_nexus_client,
    },
    nexus_sdk::scheduler::TaskPointer,
};

#[derive(Serialize)]
struct TaskListOutput<'a> {
    task_pointers: &'a [TaskPointer],
    next_cursor: Option<String>,
}

pub(crate) async fn run(cursor: Option<String>, limit: usize) -> AnyResult<(), NexusCliError> {
    command_title!("Listing Tasks");
    let cursor = cursor
        .map(|cursor| {
            hex::decode(&cursor)
                .map_err(|error| anyhow!("Task pointer cursor must be hexadecimal: {error}"))
        })
        .transpose()
        .map_err(NexusCliError::Any)?;
    let client = get_read_only_nexus_client().await?;
    let progress = loading!("Reading owned TaskPointer objects...");
    let page = client.scheduler().task_pointers(cursor, limit).await?;
    progress.success();
    json_output(&TaskListOutput {
        task_pointers: page.task_pointers(),
        next_cursor: page.next_cursor().map(hex::encode),
    })
}
