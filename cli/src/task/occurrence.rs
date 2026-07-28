use {
    super::{args::OccurrenceArgs, OccurrenceCommand},
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::{get_nexus_client, get_read_only_nexus_client},
    },
    nexus_sdk::scheduler::WatchOptions,
};

pub(crate) async fn handle(command: OccurrenceCommand) -> AnyResult<(), NexusCliError> {
    match command {
        OccurrenceCommand::List {
            task_id,
            cursor,
            limit,
        } => list(task_id, cursor, limit).await,
        OccurrenceCommand::Add {
            task_id,
            occurrence,
            gas,
        } => add(task_id, occurrence, gas).await,
        OccurrenceCommand::Inspect {
            task_id,
            occurrence_id,
            follow,
        } => inspect(task_id, occurrence_id, follow).await,
        OccurrenceCommand::Expire {
            task_id,
            occurrence_id,
            gas,
        } => expire(task_id, occurrence_id, gas).await,
        OccurrenceCommand::Cost {
            task_id,
            occurrence_id,
        } => cost(task_id, occurrence_id).await,
        OccurrenceCommand::AbortExpired {
            task_id,
            occurrence_id,
            tool_gas_id,
            gas,
        } => abort_expired(task_id, occurrence_id, tool_gas_id, gas).await,
    }
}

#[derive(Serialize)]
struct OccurrenceListOutput<'a> {
    occurrences: &'a [nexus_sdk::scheduler::OccurrenceSnapshot],
    next_cursor: Option<String>,
}

async fn list(
    task_id: sui::types::Address,
    cursor: Option<String>,
    limit: usize,
) -> AnyResult<(), NexusCliError> {
    command_title!("Listing Task occurrences");
    let cursor = cursor
        .map(|cursor| {
            hex::decode(&cursor)
                .map_err(|error| anyhow!("occurrence cursor must be hexadecimal: {error}"))
        })
        .transpose()
        .map_err(NexusCliError::Any)?;
    let client = get_read_only_nexus_client().await?;
    let progress = loading!("Reading permanent occurrence records...");
    let page = client
        .scheduler()
        .task(task_id)
        .occurrences(cursor, limit)
        .await?;
    progress.success();
    json_output(&OccurrenceListOutput {
        occurrences: page.occurrences(),
        next_cursor: page.next_cursor().map(hex::encode),
    })
}

async fn add(
    task_id: sui::types::Address,
    occurrence: OccurrenceArgs,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Adding occurrence");
    let occurrence = occurrence.into_occurrence()?;
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting occurrence transaction...");
    let receipt = client
        .scheduler()
        .task(task_id)
        .add_occurrence(occurrence)
        .await?;
    progress.success();
    notify_success!(
        "Occurrence added to Task: {task_id}",
        task_id = receipt.task_id().to_string().truecolor(100, 100, 100)
    );
    json_output(&receipt)
}

async fn inspect(
    task_id: sui::types::Address,
    occurrence_id: u64,
    follow: bool,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting occurrence");
    let client = get_read_only_nexus_client().await?;
    let occurrence = client.scheduler().task(task_id).occurrence(occurrence_id);
    let progress = loading!("Reading permanent occurrence record...");
    let snapshot = if follow {
        occurrence.watch(WatchOptions::default()).await?
    } else {
        occurrence.snapshot().await?
    };
    progress.success();
    json_output(&snapshot)
}

async fn expire(
    task_id: sui::types::Address,
    occurrence_id: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Expiring occurrence");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting occurrence expiration transaction...");
    let receipt = client
        .scheduler()
        .task(task_id)
        .occurrence(occurrence_id)
        .expire()
        .await?;
    progress.success();
    notify_success!("Occurrence recorded as missed");
    json_output(&receipt)
}

async fn cost(task_id: sui::types::Address, occurrence_id: u64) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting occurrence cost");
    let client = get_read_only_nexus_client().await?;
    let progress = loading!("Reading occurrence payment accounting...");
    let cost = client
        .scheduler()
        .task(task_id)
        .occurrence(occurrence_id)
        .cost()
        .await?;
    progress.success();
    json_output(&cost)
}

async fn abort_expired(
    task_id: sui::types::Address,
    occurrence_id: u64,
    tool_gas_id: Option<sui::types::Address>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Aborting expired occurrence");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting runtime abort transaction...");
    let receipt = client
        .scheduler()
        .task(task_id)
        .occurrence(occurrence_id)
        .abort_expired(tool_gas_id)
        .await?;
    progress.success();
    notify_success!(
        "Runtime execution aborted: {execution_id}",
        execution_id = receipt.execution_id().to_string().truecolor(100, 100, 100)
    );
    json_output(&receipt)
}
