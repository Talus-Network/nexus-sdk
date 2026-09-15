//! Discover current work through a retained interval of Nexus transactions.
//!
//! Every Task with outstanding work must have its shared anchor changed by a
//! Nexus transaction inside the requested interval, or have an Execution whose
//! shared anchor changes inside that interval.
//! This is an operating assumption, not a guarantee made by the contracts.
//! Discovery reads effects and current object metadata, never old object versions.

use {
    crate::{
        move_bindings::{
            interface::distributed_event::DistributedEventWrapper,
            primitives::{data::NexusData, event::EventWrapper},
            scheduler::task::Task,
            workflow::execution::DAGExecution,
        },
        sui,
        types::NexusContext,
    },
    anyhow::{ensure, Context as _},
    futures::{stream, StreamExt as _, TryStreamExt as _},
    std::{collections::BTreeSet, num::NonZeroUsize, time::Duration},
    sui_rpc::{field::FieldMaskUtil as _, proto::sui::rpc::v2::filter::transaction},
};

/// A fixed interval whose full transaction history is retained by the endpoint.
#[derive(Clone, Copy, Debug)]
pub struct RecoveryWindow {
    /// Inclusive lower checkpoint.
    pub start: u64,
    /// Inclusive upper checkpoint captured before discovery.
    pub tip: u64,
    /// Chain timestamp at `tip`, in milliseconds since the Unix epoch.
    pub timestamp_ms: u64,
}

impl RecoveryWindow {
    /// Locates the requested time interval using chain time and retained history.
    ///
    /// # Errors
    ///
    /// Fails when the endpoint cannot prove coverage of the whole interval.
    pub async fn load(rpc_url: &str, lookback: Duration) -> anyhow::Result<Self> {
        ensure!(!lookback.is_zero(), "Recovery lookback must be positive");
        let info = sui::grpc::client(rpc_url)?
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await?
            .into_inner();
        let tip = info
            .checkpoint_height
            .context("Service omitted its checkpoint height")?;
        let floor = info
            .lowest_available_checkpoint
            .context("Service omitted its retained checkpoint boundary")?;
        ensure!(floor <= tip, "Retained checkpoint boundary exceeds the tip");
        let timestamp_ms = checkpoint_time(rpc_url, tip).await?;
        let cutoff = timestamp_ms.saturating_sub(u64::try_from(lookback.as_millis())?);
        let floor_ms = checkpoint_time(rpc_url, floor).await?;
        ensure!(
            floor == 0 || floor_ms <= cutoff,
            "Recovery needs history from timestamp {cutoff}, but retained history starts at \
             checkpoint {floor} with timestamp {floor_ms}"
        );
        let window = Self {
            start: floor,
            tip,
            timestamp_ms,
        };
        Ok(Self {
            start: window.checkpoint_at(rpc_url, cutoff).await?,
            ..window
        })
    }

    /// Finds a conservative checkpoint boundary for a timestamp in this window.
    ///
    /// The checkpoint immediately before the first timestamp at or above the
    /// target is included. Equal timestamps and transactions at the boundary
    /// therefore cannot be lost.
    ///
    /// # Errors
    ///
    /// Returns an error if a required checkpoint is unavailable or malformed.
    pub async fn checkpoint_at(&self, rpc_url: &str, timestamp_ms: u64) -> anyhow::Result<u64> {
        ensure!(self.start <= self.tip, "Recovery interval is reversed");
        ensure!(
            self.start == 0 || checkpoint_time(rpc_url, self.start).await? <= timestamp_ms,
            "An active request predates the recovery interval"
        );
        let mut low = self.start;
        let mut high = self.tip;
        while low < high {
            let middle = low + (high - low) / 2;
            if checkpoint_time(rpc_url, middle).await? < timestamp_ms {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low.saturating_sub(1).max(self.start))
    }
}

/// Stable object identities discovered from recent Nexus activity.
#[derive(Debug, Default)]
pub struct WorkObjects {
    /// Current Task anchors. Their bounded in flight tables can name older executions.
    pub tasks: BTreeSet<sui::types::Address>,
    /// Current Execution anchors. Their state identifies the owning Task.
    pub executions: BTreeSet<sui::types::Address>,
}

/// Discovers shared work objects without downloading event payloads or old objects.
///
/// Indexed transaction scans run concurrently and reduce each frame immediately
/// to unique shared IDs. Current metadata is then read once per unique ID in
/// batches. Memory depends on unique objects and concurrency, not event payloads.
///
/// # Errors
///
/// Returns RPC, incomplete history, or malformed response errors. No partial
/// inventory is returned as successful recovery.
pub async fn discover_work_objects(
    rpc_url: &str,
    context: &NexusContext,
    window: RecoveryWindow,
    concurrency: NonZeroUsize,
) -> anyhow::Result<WorkObjects> {
    ensure!(window.start <= window.tip, "Recovery interval is reversed");
    let end = window
        .tip
        .checked_add(1)
        .context("Recovery checkpoint bound overflow")?;
    let span = (end - window.start)
        .div_ceil(concurrency.get() as u64)
        .max(1);
    let filter = transaction_filter(context);
    let mut ranges = stream::iter((window.start..end).step_by(usize::try_from(span)?))
        .map(|start| scan(rpc_url, &filter, start, start.saturating_add(span).min(end)))
        .buffer_unordered(concurrency.get());
    let mut ids = BTreeSet::new();
    while let Some(discovered) = ranges.try_next().await? {
        ids.extend(discovered);
    }
    let ids = ids.into_iter().collect::<Vec<_>>();
    let batches = ids.chunks(100).map(<[_]>::to_vec).collect::<Vec<_>>();
    let mut batches = stream::iter(batches)
        .map(|ids| async move { current_types(rpc_url, &ids).await })
        .buffer_unordered(concurrency.get());
    let task_type = crate::move_bindings::struct_tag::<Task>(context);
    let execution_type = crate::move_bindings::struct_tag::<DAGExecution>(context);
    let mut objects = WorkObjects::default();
    while let Some(batch) = batches.try_next().await? {
        for (id, object_type) in batch {
            if object_type == task_type {
                objects.tasks.insert(id);
            } else if object_type == execution_type {
                objects.executions.insert(id);
            }
        }
    }
    Ok(objects)
}

fn transaction_filter(context: &NexusContext) -> sui::grpc::TransactionFilter {
    let wrappers = [
        crate::move_bindings::struct_tag::<EventWrapper<NexusData>>(context),
        crate::move_bindings::struct_tag::<DistributedEventWrapper<NexusData>>(context),
    ];
    sui::grpc::TransactionFilter::any(wrappers.map(|tag| {
        sui::grpc::TransactionTerm::all([transaction::event_type(format!(
            "{}::{}::{}",
            tag.address(),
            tag.module(),
            tag.name()
        ))])
    }))
}

async fn checkpoint_time(rpc_url: &str, checkpoint: u64) -> anyhow::Result<u64> {
    let response = sui::grpc::client(rpc_url)?
        .ledger_client()
        .get_checkpoint(
            sui::grpc::GetCheckpointRequest::default()
                .with_sequence_number(checkpoint)
                .with_read_mask(sui::grpc::FieldMask::from_paths(["summary.timestamp"])),
        )
        .await?
        .into_inner();
    let timestamp = response
        .checkpoint
        .and_then(|checkpoint| checkpoint.summary)
        .and_then(|summary| summary.timestamp)
        .context("Checkpoint omitted its timestamp")?;
    ensure!(
        (0..1_000_000_000).contains(&timestamp.nanos),
        "Invalid checkpoint timestamp"
    );
    u64::try_from(timestamp.seconds)?
        .checked_mul(1_000)
        .and_then(|ms| ms.checked_add(timestamp.nanos as u64 / 1_000_000))
        .context("Checkpoint timestamp overflow")
}

async fn scan(
    rpc_url: &str,
    filter: &sui::grpc::TransactionFilter,
    start: u64,
    end: u64,
) -> anyhow::Result<BTreeSet<sui::types::Address>> {
    let mut client = sui::grpc::client(rpc_url)?;
    let mut ids = BTreeSet::new();
    let mut after: Option<Vec<u8>> = None;
    loop {
        let mut options = sui::grpc::QueryOptions::default()
            .with_limit(1_000)
            .with_ordering(sui::grpc::Ordering::Ascending);
        if let Some(cursor) = &after {
            options.set_after(cursor.clone());
        }
        let request = sui::grpc::ListTransactionsRequest::default()
            .with_start_checkpoint(start)
            .with_end_checkpoint(end)
            .with_filter(filter.clone())
            .with_options(options)
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "checkpoint",
                "effects.changed_objects.object_id",
                "effects.changed_objects.output_owner.kind",
            ]));
        let mut frames = client
            .ledger_client()
            .list_transactions(request)
            .await?
            .into_inner();
        let reason =
            loop {
                let frame = frames
                    .try_next()
                    .await?
                    .context("Recovery scan ended without a terminal frame")?;
                after = Some(
                    frame
                        .watermark
                        .and_then(|watermark| watermark.cursor)
                        .filter(|cursor| !cursor.is_empty())
                        .context("Recovery scan omitted its cursor")?
                        .to_vec(),
                );
                if let Some(transaction) = frame.transaction {
                    let checkpoint = transaction
                        .checkpoint
                        .context("Recovery transaction omitted its checkpoint")?;
                    ensure!(
                        (start..end).contains(&checkpoint),
                        "Recovery transaction is outside its checkpoint range"
                    );
                    let effects = transaction
                        .effects
                        .context("Recovery transaction omitted its effects")?;
                    for object in effects.changed_objects {
                        if object.output_owner.as_ref().is_some_and(|owner| {
                            owner.kind() == sui::grpc::owner::OwnerKind::Shared
                        }) {
                            ids.insert(
                                object
                                    .object_id
                                    .context("Shared object omitted its ID")?
                                    .parse()?,
                            );
                        }
                    }
                }
                if let Some(reason) = frame.end {
                    break reason.reason();
                }
            };
        match reason {
            sui::grpc::QueryEndReason::CheckpointBound => return Ok(ids),
            sui::grpc::QueryEndReason::ItemLimit | sui::grpc::QueryEndReason::ScanLimit => {}
            sui::grpc::QueryEndReason::LedgerTip => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            reason => anyhow::bail!("Recovery scan ended for an unsupported reason: {reason:?}"),
        }
    }
}

async fn current_types(
    rpc_url: &str,
    ids: &[sui::types::Address],
) -> anyhow::Result<Vec<(sui::types::Address, sui::types::StructTag)>> {
    let response = sui::grpc::client(rpc_url)?
        .ledger_client()
        .batch_get_objects(
            sui::grpc::BatchGetObjectsRequest::default()
                .with_requests(
                    ids.iter()
                        .map(|id| {
                            sui::grpc::GetObjectRequest::default().with_object_id(id.to_string())
                        })
                        .collect(),
                )
                .with_read_mask(sui::grpc::FieldMask::from_paths([
                    "object_id",
                    "object_type",
                ])),
        )
        .await?
        .into_inner();
    ensure!(
        response.objects.len() == ids.len(),
        "Recovery metadata batch is incomplete"
    );
    let mut types = Vec::new();
    for (id, result) in ids.iter().zip(response.objects) {
        match result.result.context("Recovery metadata result is empty")? {
            sui::grpc::get_object_result::Result::Object(object) => {
                let observed: sui::types::Address = object
                    .object_id
                    .context("Current object omitted its ID")?
                    .parse()?;
                ensure!(*id == observed, "Recovery metadata returned another object");
                types.push((
                    observed,
                    object
                        .object_type
                        .context("Current object omitted its type")?
                        .parse()?,
                ));
            }
            // Other shared objects can be deleted. Task and Execution anchors persist.
            sui::grpc::get_object_result::Result::Error(error)
                if error.code == tonic::Code::NotFound as i32 => {}
            sui::grpc::get_object_result::Result::Error(error) => {
                anyhow::bail!("Recovery object {id}: {}", error.message)
            }
            _ => anyhow::bail!("Recovery metadata returned an unsupported result"),
        }
    }
    Ok(types)
}
