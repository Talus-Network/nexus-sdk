use {
    crate::{
        command_title,
        display::json_output,
        item,
        loading,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::nexus::scheduler::{OccurrenceRef, WatchOccurrenceOptions},
};

/// Inspects one occurrence or follows it to a scheduler terminal state.
pub(crate) async fn inspect_occurrence(
    task_id: sui::types::Address,
    occurrence_id: u64,
    follow: bool,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting occurrence '{occurrence_id}' in Task '{task_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let occurrence = OccurrenceRef::new(task_id, occurrence_id);
    let inspection = loading!("Reconstructing occurrence lifecycle...");
    let snapshot = if follow {
        nexus_client
            .scheduler()
            .watch_occurrence(occurrence, WatchOccurrenceOptions::default())
            .await
    } else {
        nexus_client
            .scheduler()
            .inspect_occurrence(occurrence)
            .await
    }
    .map_err(NexusCliError::Nexus);
    let snapshot = match snapshot {
        Ok(snapshot) => {
            inspection.success();
            snapshot
        }
        Err(error) => {
            inspection.error();
            return Err(error);
        }
    };

    item!("Status: {:?}", snapshot.status);
    item!("Requested start: {} ms", snapshot.requested_start_time_ms);
    if let Some(execution) = &snapshot.execution {
        item!("Runtime object: {}", execution.execution_id);
    }

    json_output(&snapshot)
}
