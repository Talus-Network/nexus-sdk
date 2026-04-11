//! This module defines logic for event polling from Sui GRPC endpoints. This is
//! achieved by streaming checkpoints and subsequently batch-fetching
//! transactions with their events.
//!
//! See also: <https://github.com/Talus-Network/nexus/issues/724>

use {
    crate::{
        events::{FromSuiGrpcEvent, NexusEvent},
        sui,
        types::NexusObjects,
    },
    futures::TryStreamExt,
    std::{
        collections::BTreeMap,
        sync::Arc,
        time::{Duration, Instant},
    },
    sui_rpc::field::FieldMaskUtil,
    thiserror::Error,
    tokio::sync::mpsc,
    tokio_util::sync::CancellationToken,
};

// -- Prometheus metrics for the event poller pipeline --

lazy_static::lazy_static! {
    // -- Checkpoint fetcher metrics --

    /// Histogram: how long each get_checkpoint RPC call takes [s].
    static ref CHECKPOINT_FETCH_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_checkpoint_fetch_duration",
        "Duration of individual get_checkpoint gRPC calls [s]"
    ).unwrap();

    /// Histogram: time between consecutive checkpoints from the subscription stream [s].
    static ref CHECKPOINT_STREAM_INTERVAL: prometheus::Histogram = prometheus::register_histogram!(
        "poller_checkpoint_stream_interval",
        "Interval between consecutive checkpoints from subscription [s]",
        vec![0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0]
    ).unwrap();

    /// Histogram: number of transaction digests in each checkpoint.
    static ref CHECKPOINT_DIGESTS_COUNT: prometheus::Histogram = prometheus::register_histogram!(
        "poller_checkpoint_digests_count",
        "Number of transaction digests per checkpoint",
        vec![0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0]
    ).unwrap();

    /// Counter: number of times the checkpoint stream reconnected.
    static ref STREAM_RECONNECTIONS: prometheus::Counter = prometheus::register_counter!(
        "poller_stream_reconnections",
        "Number of checkpoint stream reconnections"
    ).unwrap();

    /// Gauge: checkpoints remaining during catch-up.
    static ref CATCHUP_REMAINING: prometheus::Gauge = prometheus::register_gauge!(
        "poller_catchup_remaining",
        "Checkpoints remaining during catch-up (0 when live)"
    ).unwrap();

    // -- Transaction batch fetcher metrics --

    /// Histogram: how long each batch_get_transactions RPC call takes [s].
    static ref TX_BATCH_FETCH_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_tx_batch_fetch_duration",
        "Duration of batch_get_transactions gRPC calls [s]"
    ).unwrap();

    /// Histogram: number of digests in each flushed batch.
    static ref TX_BATCH_SIZE: prometheus::Histogram = prometheus::register_histogram!(
        "poller_tx_batch_size",
        "Number of transaction digests per flushed batch",
        vec![1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 50.0]
    ).unwrap();

    /// Counter: reason each batch was flushed (label: reason=full|timeout).
    static ref TX_BATCH_FLUSH_REASON: prometheus::CounterVec = prometheus::register_counter_vec!(
        "poller_tx_batch_flush_reason",
        "Reason each transaction batch was flushed",
        &["reason"]
    ).unwrap();

    // -- Channel backpressure metrics --

    /// Gauge: number of digests waiting in the channel between checkpoint
    /// fetcher and transaction fetcher.
    static ref DIGEST_CHANNEL_LEN: prometheus::Gauge = prometheus::register_gauge!(
        "poller_digest_channel_len",
        "Number of digests queued in the checkpoint-to-tx channel"
    ).unwrap();

    /// Gauge: number of event pages waiting in the output channel for the
    /// leader consumer.
    static ref EVENT_PAGE_CHANNEL_LEN: prometheus::Gauge = prometheus::register_gauge!(
        "poller_event_page_channel_len",
        "Number of event pages queued in the output channel"
    ).unwrap();

    /// Histogram: how long send_page.send() blocks when the consumer is slow [s].
    static ref SEND_PAGE_BACKPRESSURE_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_send_page_backpressure_duration",
        "Duration blocked on send_page.send() due to consumer backpressure [s]",
        vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]
    ).unwrap();

    /// Histogram: how long send_digest.send() blocks when the tx fetcher is slow [s].
    static ref SEND_DIGEST_BACKPRESSURE_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_send_digest_backpressure_duration",
        "Duration blocked on send_digest.send() due to tx fetcher backpressure [s]",
        vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
    ).unwrap();

    /// Histogram: how long event parsing takes per transaction [s].
    static ref EVENT_PARSE_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_event_parse_duration",
        "Duration of NexusEvent parsing per transaction [s]",
        vec![0.00001, 0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05]
    ).unwrap();

    /// Histogram: number of nexus events per EventPage at SDK level.
    static ref EVENTS_PER_PAGE: prometheus::Histogram = prometheus::register_histogram!(
        "poller_events_per_page",
        "Number of nexus events per EventPage",
        vec![0.0, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0]
    ).unwrap();

    /// Histogram: end-to-end duration of each catch-up parallel batch [s].
    static ref CATCHUP_BATCH_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_catchup_batch_duration",
        "Duration of each parallel catch-up batch (fetch + send) [s]",
        vec![0.01, 0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0]
    ).unwrap();

    /// Histogram: end-to-end checkpoint processing time (receive checkpoint to all pages sent) [s].
    static ref CHECKPOINT_PROCESS_DURATION: prometheus::Histogram = prometheus::register_histogram!(
        "poller_checkpoint_process_duration",
        "End-to-end time from receiving a live checkpoint to finishing page sends [s]",
        vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]
    ).unwrap();
}

#[derive(Debug, Error)]
pub enum PollerError {
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("GRPC error: {0}")]
    Rpc(anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct EventPage {
    pub events: Vec<NexusEvent>,
    pub checkpoint: u64,
}

#[derive(Clone)]
pub struct EventPoller {
    rpc_url: String,
    nexus_objects: Arc<NexusObjects>,
    channel_capacity: usize,
    /// How many transactions should be fetched in a single batch. This is just
    /// a max value.
    transactions_max_batch_size: usize,
    /// This timeout defines max wait between transaction batch fetches. If this
    /// timeout is reached, the batch will be fetched even if the batch size is
    /// not reached, provided there is at least one transaction to fetch.
    transactions_batch_max_wait: Duration,
    /// How many consecutive failures to tolerato when fetching transaction
    /// batches.
    transactions_batch_max_retries: usize,
    /// How many checkpoints to fetch in parallel during catch-up.
    catchup_parallel_fetches: usize,
    cancellation_token: CancellationToken,
}

impl EventPoller {
    pub fn new(rpc_url: &str, nexus_objects: Arc<NexusObjects>) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            nexus_objects,
            channel_capacity: 100,
            transactions_max_batch_size: 30,
            transactions_batch_max_wait: Duration::from_millis(200),
            transactions_batch_max_retries: 3,
            catchup_parallel_fetches: 10,
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    pub fn with_transactions_max_batch_size(mut self, batch_size: usize) -> Self {
        self.transactions_max_batch_size = batch_size;
        self
    }

    pub fn with_transactions_batch_max_wait(mut self, max_wait: Duration) -> Self {
        self.transactions_batch_max_wait = max_wait;
        self
    }

    pub fn with_transactions_batch_max_retries(mut self, max_retries: usize) -> Self {
        self.transactions_batch_max_retries = max_retries;
        self
    }

    pub fn with_catchup_parallel_fetches(mut self, n: usize) -> Self {
        self.catchup_parallel_fetches = n.max(1);
        self
    }

    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = cancellation_token;
        self
    }

    /// Start polling Nexus events and tasks from the given checkpoint sequence
    /// number. If the `from_checkpoint` is in the future, it is ignored.
    pub fn start_polling(
        self,
        mut from_checkpoint: Option<u64>,
    ) -> Result<mpsc::Receiver<Result<EventPage, PollerError>>, PollerError> {
        let this = Arc::new(self);

        // Validate the URL eagerly — actual clients are created per
        // reconnection attempt so DNS is always re-resolved.
        sui::grpc::Client::new(&this.rpc_url).map_err(|_| {
            PollerError::Configuration(format!("Invalid GRPC URL '{}'", this.rpc_url))
        })?;

        let (send_digest, next_digest) = mpsc::channel(this.transactions_max_batch_size * 2);
        let (send_page, next_page) = mpsc::channel(this.channel_capacity);

        // Spawn a task that accepts transaction digests via a channel and
        // fetches batches of transactions.
        tokio::spawn({
            let this = Arc::clone(&this);
            let send_page = send_page.clone();

            async move {
                this.fetch_transactions_and_notify(next_digest, send_page)
                    .await
            }
        });

        // Spawn a task that streams checkpoints and sends transaction digests
        // to the fetching task.
        tokio::spawn({
            let this = Arc::clone(&this);
            let send_page = send_page.clone();
            let send_digest = send_digest.clone();

            async move {
                let mut is_reconnection = false;

                'master: loop {
                    if is_reconnection {
                        STREAM_RECONNECTIONS.inc();

                        // Back off before reconnecting to avoid tight-looping
                        // when the RPC is down.
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                    is_reconnection = true;

                    tracing::info!("[EventPoller] Starting checkpoint stream from checkpoint {from_checkpoint:?} (is_reconnection={is_reconnection})");

                    // Create a fresh client on each reconnection attempt so
                    // DNS is re-resolved and stale connections are discarded.
                    let mut client = match sui::grpc::Client::new(&this.rpc_url) {
                        Ok(c) => c,
                        Err(e) => {
                            if send_page
                                .send(Err(PollerError::Rpc(anyhow::anyhow!(
                                    "Failed to create gRPC client: {e}"
                                ))))
                                .await
                                .is_err()
                            {
                                break;
                            }
                            continue;
                        }
                    };

                    // First, start streaming checkpoints. This way we know how many
                    // checkpoints we need to fetch in the past.
                    let request = sui::grpc::SubscribeCheckpointsRequest::default().with_read_mask(
                        sui::grpc::FieldMask::from_paths([
                            "transactions.digest",
                            "sequence_number",
                        ]),
                    );

                    let mut checkpoint_stream = match client
                        .subscription_client()
                        .subscribe_checkpoints(request)
                        .await
                    {
                        Ok(response) => response.into_inner(),
                        Err(e) => {
                            if send_page
                                .send(Err(PollerError::Rpc(anyhow::anyhow!(
                                    "Failed to subscribe to checkpoints stream: {e}"
                                ))))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            continue;
                        }
                    };

                    // If we need to catch up from the past.
                    if let Some(start_from) = from_checkpoint {
                        // Find the checkpoint we need to catch up to.
                        let checkpoint = match checkpoint_stream.try_next().await {
                            Ok(Some(response)) => response.checkpoint().clone(),
                            Ok(None) => {
                                if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Checkpoint stream ended unexpectedly while trying to find the starting checkpoint")))).await.is_err() {
                                    break;
                                }

                                continue;
                            }
                            Err(e) => {
                                if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Failed to receive checkpoint from stream while trying to find the starting checkpoint: {e}")))).await.is_err() {
                                    break;
                                }

                                continue;
                            }
                        };

                        tracing::info!(
                            "[EventPoller] Starting catch-up from checkpoint {start_from} to checkpoint {}",
                            checkpoint.sequence_number()
                        );

                        // Fetch all the checkpoints between the requested
                        // starting checkpoint and the current one, in parallel
                        // batches to maximize throughput during catch-up.
                        let catchup_end = checkpoint.sequence_number();
                        let parallel = this.catchup_parallel_fetches;
                        let mut cursor = start_from;

                        // Pre-create a pool of clients for parallel fetching.
                        // Wrapped in Arc<Mutex> so they can be moved into
                        // spawned tasks and reused across batches.
                        let mut client_pool: Vec<Arc<tokio::sync::Mutex<sui::grpc::Client>>> =
                            Vec::with_capacity(parallel);
                        for _ in 0..parallel {
                            match sui::grpc::Client::new(&this.rpc_url) {
                                Ok(c) => client_pool.push(Arc::new(tokio::sync::Mutex::new(c))),
                                Err(e) => {
                                    if send_page
                                        .send(Err(PollerError::Rpc(anyhow::anyhow!(
                                            "Failed to create gRPC client for parallel catch-up: {e}"
                                        ))))
                                        .await
                                        .is_err()
                                    {
                                        break 'master;
                                    }
                                    continue 'master;
                                }
                            }
                        }

                        while cursor < catchup_end {
                            if this.cancellation_token.is_cancelled() {
                                break 'master;
                            }

                            let catchup_batch_start = Instant::now();
                            CATCHUP_REMAINING.set((catchup_end - cursor) as f64);
                            let batch_end = (cursor + parallel as u64).min(catchup_end);
                            let mut tasks = tokio::task::JoinSet::new();

                            for (i, seq) in (cursor..batch_end).enumerate() {
                                let client = Arc::clone(&client_pool[i]);
                                tasks.spawn(async move {
                                    let start = Instant::now();
                                    let mut c = client.lock().await;
                                    let req = sui::grpc::GetCheckpointRequest::default()
                                        .with_sequence_number(seq)
                                        .with_read_mask(sui::grpc::FieldMask::from_paths([
                                            "transactions.digest",
                                        ]));
                                    let resp = c
                                        .ledger_client()
                                        .get_checkpoint(req)
                                        .await
                                        .map_err(|e| (seq, format!("{e}")))?;
                                    CHECKPOINT_FETCH_DURATION
                                        .observe(start.elapsed().as_secs_f64());
                                    let digests: Vec<String> = resp
                                        .into_inner()
                                        .checkpoint()
                                        .transactions()
                                        .iter()
                                        .map(|tx| tx.digest().to_string())
                                        .collect();
                                    Ok::<(u64, Vec<String>), (u64, String)>((seq, digests))
                                });
                            }

                            // Collect results and sort by checkpoint sequence
                            // number to preserve ordering.
                            let mut results: BTreeMap<u64, Vec<String>> = BTreeMap::new();
                            while let Some(join_result) = tasks.join_next().await {
                                match join_result {
                                    Ok(Ok((seq, digests))) => {
                                        results.insert(seq, digests);
                                    }
                                    Ok(Err((seq, err))) => {
                                        if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!(
                                            "Failed to fetch checkpoint {seq} during catch-up: {err}"
                                        )))).await.is_err() {
                                            break 'master;
                                        }
                                        continue 'master;
                                    }
                                    Err(e) => {
                                        if send_page
                                            .send(Err(PollerError::Rpc(anyhow::anyhow!(
                                                "Checkpoint fetch task panicked: {e}"
                                            ))))
                                            .await
                                            .is_err()
                                        {
                                            break 'master;
                                        }
                                        continue 'master;
                                    }
                                }
                            }

                            // Send digests in checkpoint order.
                            for (seq, digests) in results {
                                CHECKPOINT_DIGESTS_COUNT.observe(digests.len() as f64);
                                from_checkpoint = Some(seq);
                                for digest in digests {
                                    let send_start = Instant::now();
                                    if send_digest.send(digest).await.is_err() {
                                        break 'master;
                                    }
                                    SEND_DIGEST_BACKPRESSURE_DURATION
                                        .observe(send_start.elapsed().as_secs_f64());
                                }
                            }

                            CATCHUP_BATCH_DURATION
                                .observe(catchup_batch_start.elapsed().as_secs_f64());
                            cursor = batch_end;
                        }

                        // Only update if the stream is ahead. Avoids re-fetching
                        // everything if the stream starts from CP 0.
                        let next_checkpoint = checkpoint.sequence_number() + 1;

                        if from_checkpoint.is_none_or(|current| current < next_checkpoint) {
                            from_checkpoint = Some(next_checkpoint);
                        }

                        for tx in checkpoint.transactions() {
                            let send_start = Instant::now();
                            if send_digest.send(tx.digest().to_string()).await.is_err() {
                                break 'master;
                            }
                            SEND_DIGEST_BACKPRESSURE_DURATION
                                .observe(send_start.elapsed().as_secs_f64());
                        }
                    }

                    // Catch-up complete, now live.
                    CATCHUP_REMAINING.set(0.0);
                    let mut last_checkpoint_at = Instant::now();

                    tracing::info!(
                        "[EventPoller] Entering live streaming mode from checkpoint {from_checkpoint:?}"
                    );

                    // Finally we can just poll the stream.
                    loop {
                        tokio::select! {
                            _ = this.cancellation_token.cancelled() => {
                                break 'master;
                            }

                            response = checkpoint_stream.try_next() => {
                                let response = match response {
                                    Ok(Some(response)) => response,
                                    Ok(None) => {
                                        if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Checkpoint stream ended unexpectedly")))).await.is_err() {
                                            break 'master;
                                        }

                                        continue 'master;
                                    }
                                    Err(e) => {
                                        if send_page.send(Err(PollerError::Rpc(anyhow::anyhow!("Failed to receive checkpoint from stream: {e}")))).await.is_err() {
                                            break 'master;
                                        }

                                        continue 'master;
                                    }
                                };

                                CHECKPOINT_STREAM_INTERVAL.observe(last_checkpoint_at.elapsed().as_secs_f64());
                                last_checkpoint_at = Instant::now();
                                let checkpoint_process_start = Instant::now();

                                let checkpoint = response.checkpoint();

                                // Ignore stale checkpoints when reconnecting.
                                if from_checkpoint
                                    .is_some_and(|from| checkpoint.sequence_number() < from)
                                {
                                    continue;
                                }

                                CHECKPOINT_DIGESTS_COUNT.observe(checkpoint.transactions().len() as f64);
                                from_checkpoint = Some(checkpoint.sequence_number() + 1);

                                for tx in checkpoint.transactions() {
                                    let send_start = Instant::now();
                                    if send_digest
                                        .send(tx.digest().to_string())
                                        .await
                                        .is_err()
                                    {
                                        break 'master;
                                    }
                                    SEND_DIGEST_BACKPRESSURE_DURATION.observe(send_start.elapsed().as_secs_f64());
                                }

                                CHECKPOINT_PROCESS_DURATION.observe(checkpoint_process_start.elapsed().as_secs_f64());
                            }
                        }
                    }
                }
            }
        });

        Ok(next_page)
    }

    /// Accept transaction digests via a channel and fetch batches of
    /// transactions of their events if the batch size is reached or the max
    /// wait between fetches is reached. Then notify about the fetched events
    /// via another channel.
    async fn fetch_transactions_and_notify(
        &self,
        mut next_digest: mpsc::Receiver<String>,
        send_page: mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> Result<(), PollerError> {
        let mut client = sui::grpc::Client::new(&self.rpc_url).map_err(|_| {
            PollerError::Configuration(format!("Invalid GRPC URL '{}'", self.rpc_url))
        })?;

        let mut consecutive_failures = 0;
        let mut batch = Vec::with_capacity(self.transactions_max_batch_size);
        let mut last_fetched_at = Instant::now();

        loop {
            // Compute the remaining time before the batch should be flushed.
            let flush_deadline = self
                .transactions_batch_max_wait
                .saturating_sub(last_fetched_at.elapsed());

            let flush_reason;

            tokio::select! {
                _ = self.cancellation_token.cancelled() => {
                    break;
                }

                // Flush partial batch when the timeout fires, even if no
                // new digest has arrived.
                _ = tokio::time::sleep(flush_deadline), if !batch.is_empty() => {
                    flush_reason = "timeout";
                }

                Some(digest) = next_digest.recv() => {
                    batch.push(digest);

                    // Only fetch if the batch size is reached or if the max
                    // wait was exceeded.
                    if batch.len() < self.transactions_max_batch_size
                        && last_fetched_at.elapsed() < self.transactions_batch_max_wait
                    {
                        continue;
                    }

                    flush_reason = if batch.len() >= self.transactions_max_batch_size {
                        "full"
                    } else {
                        "timeout"
                    };
                }
            }

            if batch.is_empty() {
                continue;
            }

            // Record batch metrics.
            TX_BATCH_FLUSH_REASON
                .with_label_values(&[flush_reason])
                .inc();
            TX_BATCH_SIZE.observe(batch.len() as f64);
            DIGEST_CHANNEL_LEN.set(next_digest.len() as f64);

            // Drain the batch, preserving the checkpoint each digest
            // belongs to so that EventPages carry the correct value.
            // Only drain as many digests as the max batch size. There can be
            // more should the RPC calls fail.
            let digests = batch
                .drain(..batch.len().min(self.transactions_max_batch_size))
                .collect::<Vec<_>>();

            let request = sui::grpc::BatchGetTransactionsRequest::default()
                .with_digests(digests.clone())
                .with_read_mask(sui::grpc::FieldMask::from_paths([
                    "events.events",
                    "digest",
                    "checkpoint",
                ]));

            let fetch_start = Instant::now();
            let response = match client.ledger_client().batch_get_transactions(request).await {
                Ok(response) => {
                    TX_BATCH_FETCH_DURATION.observe(fetch_start.elapsed().as_secs_f64());
                    last_fetched_at = Instant::now();
                    consecutive_failures = 0;

                    response.into_inner()
                }
                Err(e) => {
                    if send_page
                        .send(Err(PollerError::Rpc(anyhow::anyhow!(
                            "Failed to fetch transactions for digests: (batch_size={}) (consecutive_failures={consecutive_failures}): {e}",
                            digests.len()
                        ))))
                        .await
                        .is_err()
                    {
                        break;
                    }

                    // Avoid trying to re-fetch a batch too many times.
                    consecutive_failures += 1;

                    if consecutive_failures < self.transactions_batch_max_retries {
                        // On fetch error, we return the digests back to
                        // the batch and recreate the client so DNS is
                        // re-resolved and stale connections are discarded.
                        //
                        // If this batch failed too many times, we drop it.
                        batch.splice(0..0, digests);
                        consecutive_failures = 0;
                    }

                    if let Ok(new_client) = sui::grpc::Client::new(&self.rpc_url) {
                        client = new_client;
                    }

                    tokio::time::sleep(Duration::from_millis(
                        500 * 2u64.pow(consecutive_failures as u32),
                    ))
                    .await;

                    continue;
                }
            };

            tracing::debug!(
                "[EventPoller] Fetched batch of {} transactions at checkpoint {:?} (flush_reason={flush_reason})",
                response.transactions.len(),
                response
                    .transactions
                    .first()
                    .and_then(|t| t.transaction().checkpoint_opt())
            );

            for transaction in response.transactions {
                let transaction = transaction.transaction();
                let checkpoint = transaction.checkpoint();

                let Ok(events) = sui::types::TransactionEvents::try_from(transaction.events())
                else {
                    continue;
                };

                let parse_start = Instant::now();
                let nexus_events: Vec<_> = events
                    .0
                    .iter()
                    .enumerate()
                    .filter_map(|(index, event)| {
                        NexusEvent::from_sui_grpc_event(
                            index as u64,
                            transaction.digest().parse().ok()?,
                            event,
                            &self.nexus_objects,
                        )
                        .ok()
                    })
                    .collect();
                EVENT_PARSE_DURATION.observe(parse_start.elapsed().as_secs_f64());
                EVENTS_PER_PAGE.observe(nexus_events.len() as f64);

                let send_start = Instant::now();
                if send_page
                    .send(Ok(EventPage {
                        events: nexus_events,
                        checkpoint,
                    }))
                    .await
                    .is_err()
                {
                    break;
                }
                SEND_PAGE_BACKPRESSURE_DURATION.observe(send_start.elapsed().as_secs_f64());
                EVENT_PAGE_CHANNEL_LEN
                    .set((send_page.max_capacity() - send_page.capacity()) as f64);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{events::NexusEventKind, test_utils::sui_mocks},
        std::sync::Arc,
    };

    #[tokio::test]
    async fn test_event_poller_receives_events() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        // Create a mock event
        let walk_advanced_event = NexusEventKind::WalkAdvanced(crate::events::WalkAdvancedEvent {
            dag: sui_mocks::mock_sui_address(),
            execution: sui_mocks::mock_sui_address(),
            walk_index: 0,
            vertex: crate::events::RuntimeVertex::Plain {
                vertex: crate::events::TypeName::new("v"),
            },
            variant: crate::events::TypeName::new("ok"),
            variant_ports_to_data: crate::events::PortsData::from_map(Default::default()),
        });

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);
        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            (*nexus_objects).clone(),
            vec![walk_advanced_event.clone()],
            1,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let poller = EventPoller::new(&rpc_url, nexus_objects)
            .with_channel_capacity(2)
            .with_transactions_max_batch_size(1)
            .with_transactions_batch_max_wait(std::time::Duration::from_millis(50));

        let mut receiver = poller.start_polling(Some(1)).expect("poller should start");
        let page = receiver
            .recv()
            .await
            .expect("should receive a page")
            .expect("no error");
        assert_eq!(page.checkpoint, 1);
        assert_eq!(page.events.len(), 1);
        match &page.events[0].data {
            NexusEventKind::WalkAdvanced(_) => {}
            _ => panic!("Expected WalkAdvanced event"),
        }
    }

    #[tokio::test]
    async fn test_event_poller_empty_events() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 1);
        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            (*nexus_objects).clone(),
            vec![],
            2,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            subscription_service_mock: Some(sub_service_mock),
            ..Default::default()
        });

        let poller = EventPoller::new(&rpc_url, nexus_objects)
            .with_channel_capacity(2)
            .with_transactions_max_batch_size(1)
            .with_transactions_batch_max_wait(std::time::Duration::from_millis(50));

        let mut receiver = poller.start_polling(Some(1)).expect("poller should start");
        let page = receiver
            .recv()
            .await
            .expect("should receive a page")
            .expect("no error");
        assert_eq!(page.checkpoint, 1);
        assert!(page.events.is_empty());
    }
}
