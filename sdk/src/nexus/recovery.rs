//! Discover current work through a retained interval of Nexus transactions.
//!
//! Every Task with outstanding work must have its shared anchor changed by a
//! Nexus transaction inside the requested interval, or have an Execution whose
//! shared anchor changes inside that interval.
//! This is an operating assumption, not a guarantee made by the contracts.
//! Discovery reads effects and current object metadata, never old object versions.

pub use sui_rpc::client::{ListConfig, ListEvent};
use {
    crate::{
        move_bindings::{
            interface::distributed_event::DistributedEventWrapper,
            primitives::{data::NexusData, event::EventWrapper},
            scheduler::task::Task,
            workflow::execution::DAGExecution,
        },
        sui::{
            self,
            observation::{Observation, Operation, Outcome},
        },
        types::NexusContext,
    },
    anyhow::{bail, ensure, Context as _},
    futures::{stream, StreamExt as _, TryStreamExt as _},
    std::{collections::BTreeSet, future::Future, num::NonZeroUsize, time::Duration},
    sui_rpc::{field::FieldMaskUtil as _, proto::sui::rpc::v2::filter::transaction},
};

// Matches the MAX_BATCH_REQUESTS bound enforced by Sui LedgerService::batch_get_objects.
const MAX_BATCH_OBJECT_REQUESTS: usize = 1_000;

/// Recovery reads sharing the Sui client's retry policy.
///
/// The caller supplies policy once. The Sui client owns transport timeouts and
/// resumable List requests. Recovery retains validated progress across incomplete
/// or invalid responses and retries other reads using the same delay settings.
/// Dropping a recovery future cancels its reads and waits.
pub struct RecoveryReader<'a> {
    rpc_url: &'a str,
    policy: ListConfig,
}

impl<'a> RecoveryReader<'a> {
    /// Create a reader with the endpoint and policy chosen by the caller.
    ///
    /// # Errors
    ///
    /// Rejects an invalid endpoint, a zero retry delay, or a maximum below it.
    pub fn new(rpc_url: &'a str, policy: ListConfig) -> anyhow::Result<Self> {
        ensure!(
            !policy.base_retry_delay.is_zero(),
            "Recovery retry delay must be positive"
        );
        ensure!(
            policy.max_retry_delay >= policy.base_retry_delay,
            "Recovery maximum retry delay is below its initial delay"
        );
        sui::grpc::client(rpc_url)?;
        Ok(Self { rpc_url, policy })
    }

    /// Wait for a validated read using the configured retry delays.
    ///
    /// The callback must only read state and validate its result. Bound individual
    /// RPCs through their transport; this method imposes no deadline on an operation
    /// containing several reads. Do not use it for mutations or transaction submission.
    pub async fn read<T, F, Fut>(&self, operation: &str, mut read: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        let observation = Observation::start(Operation::RecoveryRead);
        let mut delay = self.policy.base_retry_delay;
        loop {
            let attempt = Observation::start(Operation::RecoveryAttempt);
            let result = read().await;
            attempt.finish(if result.is_ok() {
                Outcome::Success
            } else {
                Outcome::Error(None)
            });
            match result {
                Ok(value) => {
                    observation.finish(Outcome::Success);
                    return value;
                }
                Err(error) => self.wait(operation, &error, &mut delay).await,
            }
        }
    }

    async fn wait(&self, operation: &str, error: &anyhow::Error, delay: &mut Duration) {
        let wait = delay.saturating_add(self.policy.retry_jitter.mul_f64(rand::random::<f64>()));
        tracing::warn!(operation, error = %format_args!("{error:#}"), delay = ?wait, "Recovery read will retry");
        let observation = Observation::start(Operation::RecoveryDelay);
        tokio::time::sleep(wait).await;
        observation.finish(Outcome::Success);
        *delay = delay.saturating_mul(2).min(self.policy.max_retry_delay);
    }

    /// Select a fixed interval using chain time and retained history.
    ///
    /// Endpoint failures and insufficient coverage keep recovery pending.
    ///
    /// # Errors
    ///
    /// Rejects a zero lookback or one that cannot be represented in milliseconds.
    pub async fn window(&self, lookback: Duration) -> anyhow::Result<RecoveryWindow> {
        ensure!(!lookback.is_zero(), "Recovery lookback must be positive");
        let lookback_ms = u64::try_from(lookback.as_millis())?;
        let mut window = self
            .read("checking recovery coverage", || async {
                let info = sui::grpc::client(self.rpc_url)?
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
                let timestamp_ms = self.checkpoint_time(tip).await;
                let cutoff = timestamp_ms.saturating_sub(lookback_ms);
                let floor_ms = self.checkpoint_time(floor).await;
                ensure!(
                    floor == 0 || floor_ms <= cutoff,
                    "Recovery needs history from timestamp {cutoff}, but retained history starts at \
                     checkpoint {floor} with timestamp {floor_ms}"
                );
                Ok(RecoveryWindow {
                    start: floor,
                    tip,
                    timestamp_ms,
                })
            })
            .await;
        window.start = self
            .checkpoint_at(window, window.timestamp_ms.saturating_sub(lookback_ms))
            .await?;
        Ok(window)
    }

    /// Find a conservative checkpoint boundary without restarting completed reads.
    ///
    /// Includes the checkpoint immediately before the first timestamp at or above
    /// the target so equal timestamps and boundary transactions cannot be lost.
    /// Unavailable coverage keeps recovery pending.
    ///
    /// # Errors
    ///
    /// Rejects a reversed window.
    pub async fn checkpoint_at(
        &self,
        window: RecoveryWindow,
        timestamp_ms: u64,
    ) -> anyhow::Result<u64> {
        ensure!(window.start <= window.tip, "Recovery interval is reversed");
        if window.start != 0 {
            self.read("checking active request coverage", || async {
                ensure!(
                    checkpoint_time(self.rpc_url, window.start).await? <= timestamp_ms,
                    "An active request predates the recovery interval"
                );
                Ok(())
            })
            .await;
        }
        let mut low = window.start;
        let mut high = window.tip;
        while low < high {
            let middle = low + (high - low) / 2;
            if self.checkpoint_time(middle).await < timestamp_ms {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        Ok(low.saturating_sub(1).max(window.start))
    }

    async fn checkpoint_time(&self, checkpoint: u64) -> u64 {
        self.read("reading recovery checkpoint", || {
            checkpoint_time(self.rpc_url, checkpoint)
        })
        .await
    }

    /// Discover shared work objects through a complete, validated transaction interval.
    ///
    /// Scans run concurrently and retain unique IDs from validated frames. Metadata
    /// is read once per unique ID in batches. Failed reads retain completed work.
    ///
    /// # Errors
    ///
    /// Rejects invalid checkpoint bounds. Endpoint failures keep recovery pending.
    pub async fn discover_work_objects(
        &self,
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
            .map(|start| self.scan(&filter, start, start.saturating_add(span).min(end)))
            .buffer_unordered(concurrency.get());
        let mut ids = BTreeSet::new();
        while let Some(discovered) = ranges.next().await {
            ids.extend(discovered);
        }
        let mut batches = stream::iter(ids)
            .chunks(MAX_BATCH_OBJECT_REQUESTS)
            .map(|ids| async move {
                self.read("reading recovery object metadata", || {
                    current_types(self.rpc_url, &ids)
                })
                .await
            })
            .buffer_unordered(concurrency.get());
        let task_type = crate::move_bindings::struct_tag::<Task>(context);
        let execution_type = crate::move_bindings::struct_tag::<DAGExecution>(context);
        let mut objects = WorkObjects::default();
        while let Some(batch) = batches.next().await {
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

    #[tracing::instrument(name = "recovery_scan", skip(self, filter))]
    async fn scan(
        &self,
        filter: &sui::grpc::TransactionFilter,
        start: u64,
        end: u64,
    ) -> BTreeSet<sui::types::Address> {
        use sui::grpc::QueryEndReason::{CheckpointBound, ItemLimit, LedgerTip, ScanLimit};

        let mut ids = BTreeSet::new();
        let mut after: Option<Vec<u8>> = None;
        let mut delay = self.policy.base_retry_delay;
        loop {
            let result = async {
                let mut options = sui::grpc::QueryOptions::default()
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
                let frames = sui::grpc::client(self.rpc_url)?
                    .list_transactions_with_config(request, self.policy.clone());
                futures::pin_mut!(frames);
                while let Some(frame) = frames.try_next().await? {
                    let watermark = frame
                        .watermark
                        .context("Recovery scan omitted its watermark")?;
                    let cursor = watermark
                        .cursor
                        .filter(|cursor| !cursor.is_empty())
                        .context("Recovery scan omitted its cursor")?;
                    let reason = frame.end.map(|end| end.reason());
                    if let Some(reason) = reason {
                        ensure!(
                            matches!(reason, CheckpointBound | ItemLimit | ScanLimit | LedgerTip),
                            "Recovery scan ended for an unsupported reason: {reason:?}"
                        );
                        if reason == CheckpointBound {
                            // Empty intervals may have no covered checkpoint. When present,
                            // the inclusive coverage must agree with the requested end.
                            ensure!(
                                watermark
                                    .checkpoint
                                    .is_none_or(|checkpoint| checkpoint == end - 1),
                                "Recovery scan did not cover its requested checkpoint bound"
                            );
                        }
                    }
                    let mut discovered = Vec::new();
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
                                discovered.push(
                                    object
                                        .object_id
                                        .context("Shared object omitted its ID")?
                                        .parse::<sui::types::Address>()?,
                                );
                            }
                        }
                    }
                    // Retain the application cursor only after validating the whole frame.
                    // Dropping a rejected List stream then resumes before that frame.
                    ids.extend(discovered);
                    if after.as_deref() != Some(cursor.as_ref()) {
                        delay = self.policy.base_retry_delay;
                    }
                    after = Some(cursor.to_vec());
                    match reason {
                        Some(CheckpointBound) => return Ok(()),
                        Some(LedgerTip) => bail!(
                            "Indexed ledger has not reached recovery checkpoint {}",
                            end - 1
                        ),
                        _ => {}
                    }
                }
                bail!("Recovery scan ended without its checkpoint bound")
            }
            .await;
            match result {
                Ok(()) => return ids,
                Err(error) => {
                    self.wait("scanning recovery transactions", &error, &mut delay)
                        .await
                }
            }
        }
    }
}

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
    /// Select a window using the Sui client's default policy.
    ///
    /// # Errors
    ///
    /// Rejects an invalid endpoint or lookback. See [`RecoveryReader::window`].
    pub async fn load(rpc_url: &str, lookback: Duration) -> anyhow::Result<Self> {
        RecoveryReader::new(rpc_url, ListConfig::default())?
            .window(lookback)
            .await
    }

    /// Find a boundary using the Sui client's default policy.
    ///
    /// # Errors
    ///
    /// Rejects an invalid endpoint or interval. See [`RecoveryReader::checkpoint_at`].
    pub async fn checkpoint_at(&self, rpc_url: &str, timestamp_ms: u64) -> anyhow::Result<u64> {
        RecoveryReader::new(rpc_url, ListConfig::default())?
            .checkpoint_at(*self, timestamp_ms)
            .await
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

/// Discover shared work objects using the Sui client's default policy.
///
/// # Errors
///
/// Rejects invalid endpoints or bounds. See [`RecoveryReader::discover_work_objects`].
pub async fn discover_work_objects(
    rpc_url: &str,
    context: &NexusContext,
    window: RecoveryWindow,
    concurrency: NonZeroUsize,
) -> anyhow::Result<WorkObjects> {
    RecoveryReader::new(rpc_url, ListConfig::default())?
        .discover_work_objects(context, window, concurrency)
        .await
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
