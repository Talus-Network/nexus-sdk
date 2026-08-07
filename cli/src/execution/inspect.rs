use {
    super::output,
    crate::{
        command_title,
        display::{human_output, json_output},
        prelude::*,
        sui,
    },
    nexus_sdk::{
        events::{NexusEvent, NexusEventKind},
        nexus::workflow::InspectExecutionOptions,
        scheduler::{OccurrenceRef, SchedulerError},
    },
    std::time::Duration,
};

pub(super) async fn run(
    execution_id: Option<nexus_sdk::sui::types::Address>,
    occurrence: Option<(nexus_sdk::sui::types::Address, u64)>,
    timeout_secs: u64,
    poll_ms: u64,
) -> AnyResult<(), NexusCliError> {
    let client = sui::get_read_only_nexus_client().await?;
    let (execution_id, mut occurrence) = match (execution_id, occurrence) {
        (Some(execution_id), None) => (execution_id, None),
        (None, Some((task_id, occurrence_id))) => {
            let reference = OccurrenceRef::new(task_id, occurrence_id);
            let snapshot = client
                .scheduler()
                .task(task_id)
                .occurrence(occurrence_id)
                .snapshot()
                .await?;
            let execution_id = snapshot.execution().map_or_else(
                || {
                    Err(SchedulerError::OccurrenceNotDispatched {
                        task_id,
                        occurrence_id,
                    })
                },
                |execution| Ok(execution.execution_id()),
            )?;
            (execution_id, Some(reference))
        }
        _ => {
            return Err(NexusCliError::Any(anyhow!(
                "execution identity arguments were not resolved"
            )))
        }
    };

    command_title!("Inspecting execution {execution_id}");
    let options = InspectExecutionOptions {
        timeout: Duration::from_secs(timeout_secs),
        poll_interval: Duration::from_millis(poll_ms),
    };
    let mut inspection = client
        .workflow()
        .inspect_execution(execution_id, options)
        .await
        .map_err(NexusCliError::Nexus)?;
    let mut events = Vec::<NexusEvent>::new();

    while let Some(event) = inspection.next_event.recv().await {
        if let NexusEventKind::OccurrenceDispatched(dispatched) = &event.data {
            occurrence = Some(OccurrenceRef::new(
                dispatched.task_id.bytes,
                dispatched.occurrence_id,
            ));
        }
        if let Some(summary) = output::render_event(&event.data) {
            human_output(&format!("{summary}\n"));
        }
        events.push(event);
    }

    if let Some(reference) = occurrence {
        human_output(&output::render_occurrence_command(&reference));
    }
    await_poller(inspection.poller).await?;
    let events = events
        .iter()
        .map(output::event_result_json)
        .collect::<anyhow::Result<Vec<_>>>()
        .map_err(NexusCliError::Any)?;
    json_output(&events)
}

async fn await_poller(
    poller: tokio::task::JoinHandle<Result<(), nexus_sdk::nexus::error::NexusError>>,
) -> Result<(), NexusCliError> {
    poller
        .await
        .map_err(|error| {
            NexusCliError::Any(anyhow!("execution inspection task failed to join: {error}"))
        })?
        .map_err(NexusCliError::Nexus)
}
