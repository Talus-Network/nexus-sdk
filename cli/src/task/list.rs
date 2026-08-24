use {
    super::output,
    crate::{
        command_title,
        display::{human_output, json_output},
        loading,
        prelude::*,
        sui::{get_owner_nexus_client, resolve_creator_package},
    },
    nexus_sdk::{scheduler::TaskPointer, types::PackageRole},
};

#[derive(Serialize)]
struct TaskListOutput<'a> {
    task_pointers: &'a [TaskPointer],
    next_cursor: Option<String>,
}

pub(crate) async fn run(
    scheduler_package: Option<sui::types::Address>,
    cursor: Option<String>,
    limit: usize,
) -> AnyResult<(), NexusCliError> {
    command_title!("Listing Tasks");
    let cursor = cursor
        .map(|cursor| {
            hex::decode(&cursor)
                .map_err(|error| anyhow!("Task pointer cursor must be hexadecimal: {error}"))
        })
        .transpose()
        .map_err(NexusCliError::Any)?;
    let client = get_owner_nexus_client().await?;
    let scheduler_package =
        resolve_creator_package(&client, scheduler_package, PackageRole::Scheduler).await?;
    let progress = loading!("Reading owned TaskPointer objects...");
    let page = client
        .scheduler()
        .task_pointers(scheduler_package, cursor, limit)
        .await?;
    progress.success();
    let next_cursor = page.next_cursor().map(hex::encode);
    human_output(&output::render_task_list(
        page.task_pointers(),
        next_cursor.as_deref(),
    ));
    json_output(&TaskListOutput {
        task_pointers: page.task_pointers(),
        next_cursor,
    })
}
