use {
    super::{args::OccurrenceArgs, output, OccurrenceCommand},
    crate::{
        command_title,
        display::{human_output, json_output},
        loading, notify_success,
        prelude::*,
        sui::{get_nexus_client, get_read_only_nexus_client},
    },
    nexus_sdk::scheduler::{OccurrenceStatus, WatchOptions},
    std::time::Duration,
};

const fn stop_following(status: OccurrenceStatus) -> bool {
    matches!(status, OccurrenceStatus::Finished) || status.is_terminal()
}

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
            timeout_secs,
            poll_ms,
        } => inspect(task_id, occurrence_id, follow, timeout_secs, poll_ms).await,
        OccurrenceCommand::Settle {
            task_id,
            occurrence_id,
            gas,
        } => settle(task_id, occurrence_id, gas).await,
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
            invocation_id,
            gas,
        } => abort_expired(task_id, occurrence_id, invocation_id, gas).await,
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
    let next_cursor = page.next_cursor().map(hex::encode);
    human_output(&output::render_occurrence_list(
        task_id,
        page.occurrences(),
        next_cursor.as_deref(),
    ));
    json_output(&OccurrenceListOutput {
        occurrences: page.occurrences(),
        next_cursor,
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
    human_output(&output::render_task_receipt(&receipt, None));
    json_output(&receipt)
}

async fn inspect(
    task_id: sui::types::Address,
    occurrence_id: u64,
    follow: bool,
    timeout_secs: Option<u64>,
    poll_ms: Option<u64>,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting occurrence");
    let client = get_read_only_nexus_client().await?;
    let occurrence = client.scheduler().task(task_id).occurrence(occurrence_id);
    let progress = loading!("Reading permanent occurrence record...");
    let snapshot = if follow {
        let defaults = WatchOptions::default();
        let options = WatchOptions::new(
            timeout_secs.map_or(defaults.timeout(), Duration::from_secs),
            poll_ms.map_or(defaults.poll_interval(), Duration::from_millis),
        );
        occurrence
            .watch_until(options, |snapshot| stop_following(snapshot.status()))
            .await?
    } else {
        occurrence.snapshot().await?
    };
    progress.success();
    human_output(&output::render_occurrence(&snapshot));
    json_output(&snapshot)
}

async fn settle(
    task_id: sui::types::Address,
    occurrence_id: u64,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Settling occurrence");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting occurrence settlement transaction...");
    let receipt = client
        .scheduler()
        .task(task_id)
        .occurrence(occurrence_id)
        .settle()
        .await
        .map_err(|source| NexusCliError::OccurrenceSettlement {
            task_id,
            occurrence_id,
            source: Box::new(source),
        })?;
    progress.success();
    notify_success!("Occurrence settled into Task");
    human_output(&output::render_task_receipt(&receipt, Some(occurrence_id)));
    json_output(&receipt)
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
    human_output(&output::render_task_receipt(&receipt, Some(occurrence_id)));
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
    human_output(&output::render_occurrence_cost(
        task_id,
        occurrence_id,
        &cost,
    ));
    json_output(&cost)
}

async fn abort_expired(
    task_id: sui::types::Address,
    occurrence_id: u64,
    invocation_id: Option<sui::types::Address>,
    gas: GasArgs,
) -> AnyResult<(), NexusCliError> {
    command_title!("Aborting expired occurrence");
    let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let progress = loading!("Submitting runtime abort transaction...");
    let receipt = client
        .scheduler()
        .task(task_id)
        .occurrence(occurrence_id)
        .abort_expired(invocation_id)
        .await?;
    progress.success();
    notify_success!(
        "Runtime execution aborted: {execution_id}",
        execution_id = receipt.execution_id().to_string().truecolor(100, 100, 100)
    );
    human_output(&output::render_abort_receipt(&receipt));
    json_output(&receipt)
}

#[cfg(test)]
mod tests {
    use {super::stop_following, nexus_sdk::scheduler::OccurrenceStatus};

    #[test]
    fn occurrence_follow_stops_when_action_is_required() {
        assert!(stop_following(OccurrenceStatus::Finished));
        assert!(stop_following(OccurrenceStatus::Settled {
            succeeded: false
        }));
        assert!(!stop_following(OccurrenceStatus::Executing));
    }
}
