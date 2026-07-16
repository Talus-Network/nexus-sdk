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
        collections::{BTreeMap, VecDeque},
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

    /// Counter: [`tonic::Code::ResourceExhausted`] responses while fetching transaction batches.
    static ref TX_BATCH_RESOURCE_EXHAUSTED: prometheus::Counter = prometheus::register_counter!(
        "poller_tx_batch_resource_exhausted",
        "Number of resource exhausted transaction batch responses"
    ).unwrap();

    /// Histogram: fallback batch size selected after [`tonic::Code::ResourceExhausted`].
    static ref TX_BATCH_FALLBACK_SIZE: prometheus::Histogram = prometheus::register_histogram!(
        "poller_tx_batch_fallback_size",
        "Transaction batch size selected after resource exhausted fallback",
        vec![1.0, 2.0, 5.0, 10.0, 15.0, 20.0]
    ).unwrap();

    /// Counter: single digest batches quarantined after [`tonic::Code::ResourceExhausted`].
    static ref TX_DIGESTS_QUARANTINED: prometheus::Counter = prometheus::register_counter!(
        "poller_tx_digests_quarantined",
        "Number of single transaction digests dropped after resource exhausted responses"
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

#[derive(Debug, Clone)]
struct PendingTransactionDigest {
    digest: String,
    checkpoint: u64,
}

const DEFAULT_TRANSACTIONS_MAX_DECODING_MESSAGE_SIZE: usize = 32 * 1024 * 1024;

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
    /// Maximum decompressed response size for [`sui::grpc::Client`] transaction fetches.
    transactions_max_decoding_message_size: usize,
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
            transactions_max_decoding_message_size: DEFAULT_TRANSACTIONS_MAX_DECODING_MESSAGE_SIZE,
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

    /// Set the maximum decompressed gRPC response size in bytes for transaction fetches.
    ///
    /// This is applied per [`EventPoller`] by forwarding to
    /// [`sui::grpc::Client::with_max_decoding_message_size`].
    pub fn with_transactions_max_decoding_message_size(mut self, limit_bytes: usize) -> Self {
        self.transactions_max_decoding_message_size = limit_bytes;
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

    fn transaction_client(&self, client: sui::grpc::Client) -> sui::grpc::Client {
        client.with_max_decoding_message_size(self.transactions_max_decoding_message_size)
    }

    /// Start polling Nexus events and tasks from the given checkpoint sequence
    /// number. If the `from_checkpoint` is in the future, it is ignored.
    pub fn start_polling(
        self,
        mut from_checkpoint: Option<u64>,
    ) -> Result<mpsc::Receiver<Result<EventPage, PollerError>>, PollerError> {
        let this = Arc::new(self);

        // Validate the URL eagerly. Reconnection attempts clone the endpoint's
        // shared channel; tonic reconnects that channel as needed.
        sui::grpc::client(&this.rpc_url).map_err(|_| {
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

                    // Clone the endpoint's shared channel for this stream. A
                    // clone has independent client state while tonic owns
                    // connection recovery for the underlying channel.
                    let mut client = match sui::grpc::client(&this.rpc_url) {
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
                            match sui::grpc::client(&this.rpc_url) {
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
                                    if send_digest
                                        .send(PendingTransactionDigest {
                                            digest,
                                            checkpoint: seq,
                                        })
                                        .await
                                        .is_err()
                                    {
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
                            if send_digest
                                .send(PendingTransactionDigest {
                                    digest: tx.digest().to_string(),
                                    checkpoint: checkpoint.sequence_number(),
                                })
                                .await
                                .is_err()
                            {
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
                                        .send(PendingTransactionDigest {
                                            digest: tx.digest().to_string(),
                                            checkpoint: checkpoint.sequence_number(),
                                        })
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
        mut next_digest: mpsc::Receiver<PendingTransactionDigest>,
        send_page: mpsc::Sender<Result<EventPage, PollerError>>,
    ) -> Result<(), PollerError> {
        let mut client = sui::grpc::client(&self.rpc_url)
            .map(|client| self.transaction_client(client))
            .map_err(|_| {
                PollerError::Configuration(format!("Invalid GRPC URL '{}'", self.rpc_url))
            })?;

        let mut consecutive_failures = 0;
        let mut batch = Vec::with_capacity(self.transactions_max_batch_size);
        let mut retry_batches = VecDeque::<Vec<PendingTransactionDigest>>::new();
        let mut last_fetched_at = Instant::now();

        loop {
            let (digests, flush_reason) = if let Some(digests) = retry_batches.pop_front() {
                (digests, "retry")
            } else {
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

                let digests = batch
                    .drain(..batch.len().min(self.transactions_max_batch_size))
                    .collect::<Vec<_>>();
                (digests, flush_reason)
            };

            TX_BATCH_FLUSH_REASON
                .with_label_values(&[flush_reason])
                .inc();
            TX_BATCH_SIZE.observe(digests.len() as f64);
            DIGEST_CHANNEL_LEN.set(next_digest.len() as f64);

            let request = sui::grpc::BatchGetTransactionsRequest::default()
                .with_digests(digests.iter().map(|tx| tx.digest.clone()).collect())
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

                    // Avoid trying to fetch the same batch too many times.
                    if e.code() == tonic::Code::ResourceExhausted {
                        TX_BATCH_RESOURCE_EXHAUSTED.inc();

                        if digests.len() > 1 {
                            let fallback_size = digests.len().div_ceil(2);
                            TX_BATCH_FALLBACK_SIZE.observe(fallback_size as f64);

                            let mut left = digests;
                            let right = left.split_off(fallback_size);
                            retry_batches.push_front(right);
                            retry_batches.push_front(left);
                        } else if let Some(tx) = digests.first() {
                            TX_DIGESTS_QUARANTINED.inc();
                            tracing::error!(
                                digest = %tx.digest,
                                checkpoint = tx.checkpoint,
                                "Quarantining transaction digest after resource exhausted response"
                            );
                        }

                        consecutive_failures = 0;
                    } else {
                        consecutive_failures += 1;

                        if consecutive_failures < self.transactions_batch_max_retries {
                            retry_batches.push_front(digests);
                        } else {
                            tracing::error!(
                                batch_size = digests.len(),
                                attempts = consecutive_failures,
                                "Dropping transaction batch after retry limit"
                            );
                            consecutive_failures = 0;
                        }
                    }

                    if let Ok(new_client) = sui::grpc::client(&self.rpc_url)
                        .map(|client| self.transaction_client(client))
                    {
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

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            events::NexusEventKind,
            move_bindings::{
                interface::graph::{OutputVariant, RuntimeVertex},
                move_std::ascii::String as MoveString,
                sui_framework::object::ID,
                workflow::execution_events::WalkAdvancedEvent,
            },
            test_utils::sui_mocks,
        },
        std::{
            sync::{
                atomic::{AtomicUsize, Ordering},
                Arc,
                Mutex,
            },
            time::Duration,
        },
        tokio::{sync::mpsc, time::timeout},
    };

    fn id(bytes: sui::types::Address) -> ID {
        ID { bytes }
    }

    fn empty_transaction_response(digests: Vec<String>) -> sui::grpc::BatchGetTransactionsResponse {
        let mut response = sui::grpc::BatchGetTransactionsResponse::default();
        let transactions = digests
            .into_iter()
            .enumerate()
            .map(|(index, digest)| {
                let mut result = sui::grpc::GetTransactionResult::default();
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(digest);
                transaction.set_checkpoint(index as u64 + 1);
                transaction.set_events(sui::grpc::TransactionEvents::default());
                result.set_transaction(transaction);
                result
            })
            .collect();

        response.set_transactions(transactions);
        response
    }

    #[tokio::test]
    async fn event_poller_receives_events() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        let walk_advanced_event = NexusEventKind::WalkAdvanced(WalkAdvancedEvent {
            dag: id(sui_mocks::mock_sui_address()),
            execution: id(sui_mocks::mock_sui_address()),
            walk_index: 0,
            vertex: RuntimeVertex::plain("v"),
            variant: OutputVariant {
                name: MoveString::from("ok"),
            },
            variant_ports_to_data: crate::move_bindings::sui_framework::vec_map::VecMap {
                contents: vec![],
            },
        });

        sui_mocks::grpc::mock_events_stream(&mut sub_service_mock, 2);
        sui_mocks::grpc::mock_events_get_checkpoint(
            &mut ledger_service_mock,
            (*nexus_objects).clone(),
            vec![walk_advanced_event],
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
            .with_transactions_batch_max_wait(Duration::from_millis(50));

        let mut receiver = poller.start_polling(Some(1)).expect("poller should start");
        let page = receiver
            .recv()
            .await
            .expect("should receive a page")
            .expect("no error");

        assert_eq!(page.checkpoint, 1);
        assert_eq!(page.events.len(), 1);
        assert!(matches!(
            &page.events[0].data,
            NexusEventKind::WalkAdvanced(_)
        ));
    }

    #[tokio::test]
    async fn event_poller_preserves_empty_event_pages() {
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
            .with_transactions_batch_max_wait(Duration::from_millis(50));

        let mut receiver = poller.start_polling(Some(1)).expect("poller should start");
        let page = receiver
            .recv()
            .await
            .expect("should receive a page")
            .expect("no error");

        assert_eq!(page.checkpoint, 1);
        assert!(page.events.is_empty());
    }

    #[tokio::test]
    async fn resource_exhausted_batches_are_retried_with_smaller_batches() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let attempts = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        ledger_service_mock
            .expect_batch_get_transactions()
            .returning({
                let attempts = Arc::clone(&attempts);
                move |request| {
                    let digests = request.into_inner().digests;
                    attempts.lock().unwrap().push(digests.clone());

                    if digests.len() > 1 {
                        Err(tonic::Status::resource_exhausted("batch too large"))
                    } else {
                        Ok(tonic::Response::new(empty_transaction_response(digests)))
                    }
                }
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let cancellation_token = CancellationToken::new();
        let poller = EventPoller::new(&rpc_url, nexus_objects)
            .with_transactions_max_batch_size(2)
            .with_transactions_batch_max_wait(Duration::from_millis(10))
            .with_cancellation_token(cancellation_token.clone());

        let (send_digest, next_digest) = mpsc::channel(4);
        let (send_page, mut next_page) = mpsc::channel(4);
        send_digest
            .send(PendingTransactionDigest {
                digest: "tx1".to_string(),
                checkpoint: 1,
            })
            .await
            .unwrap();
        send_digest
            .send(PendingTransactionDigest {
                digest: "tx2".to_string(),
                checkpoint: 1,
            })
            .await
            .unwrap();

        let handle = tokio::spawn(async move {
            poller
                .fetch_transactions_and_notify(next_digest, send_page)
                .await
        });

        timeout(Duration::from_secs(3), async {
            let mut received_pages = 0;
            while received_pages < 2 {
                if let Ok(page) = next_page.recv().await.expect("page channel closed") {
                    assert!(page.events.is_empty());
                    received_pages += 1;
                }
            }
        })
        .await
        .expect("timed out waiting for split batch");

        cancellation_token.cancel();
        handle.await.unwrap().unwrap();

        let attempts = attempts.lock().unwrap().clone();
        assert_eq!(
            attempts,
            vec![
                vec!["tx1".to_string(), "tx2".to_string()],
                vec!["tx1".to_string()],
                vec!["tx2".to_string()],
            ]
        );
    }

    #[tokio::test]
    async fn failed_batches_stop_retrying_after_max_retries() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        ledger_service_mock
            .expect_batch_get_transactions()
            .returning({
                let attempts = Arc::clone(&attempts);
                move |request| {
                    assert_eq!(request.into_inner().digests.len(), 2);
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(tonic::Status::unavailable("rpc down"))
                }
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let poller = EventPoller::new(&rpc_url, nexus_objects)
            .with_transactions_max_batch_size(2)
            .with_transactions_batch_max_wait(Duration::from_millis(10))
            .with_transactions_batch_max_retries(2);

        let (send_digest, next_digest) = mpsc::channel(4);
        let (send_page, mut next_page) = mpsc::channel(4);
        send_digest
            .send(PendingTransactionDigest {
                digest: "tx1".to_string(),
                checkpoint: 1,
            })
            .await
            .unwrap();
        send_digest
            .send(PendingTransactionDigest {
                digest: "tx2".to_string(),
                checkpoint: 1,
            })
            .await
            .unwrap();

        let handle = tokio::spawn(async move {
            poller
                .fetch_transactions_and_notify(next_digest, send_page)
                .await
        });

        for _ in 0..2 {
            assert!(timeout(Duration::from_secs(3), next_page.recv())
                .await
                .expect("timed out waiting for expected retry")
                .expect("page channel closed")
                .is_err());
        }

        assert!(
            timeout(Duration::from_millis(1200), next_page.recv())
                .await
                .is_err(),
            "batch was retried after the configured retry limit"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 2);

        handle.abort();
    }

    #[tokio::test]
    async fn single_resource_exhausted_digest_is_not_retried_forever() {
        let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();

        ledger_service_mock
            .expect_batch_get_transactions()
            .returning({
                let attempts = Arc::clone(&attempts);
                move |request| {
                    assert_eq!(request.into_inner().digests.len(), 1);
                    attempts.fetch_add(1, Ordering::SeqCst);
                    Err(tonic::Status::resource_exhausted(
                        "single transaction too large",
                    ))
                }
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });

        let poller = EventPoller::new(&rpc_url, nexus_objects)
            .with_transactions_max_batch_size(1)
            .with_transactions_batch_max_wait(Duration::from_millis(10));

        let (send_digest, next_digest) = mpsc::channel(2);
        let (send_page, mut next_page) = mpsc::channel(2);
        send_digest
            .send(PendingTransactionDigest {
                digest: "tx1".to_string(),
                checkpoint: 1,
            })
            .await
            .unwrap();

        let handle = tokio::spawn(async move {
            poller
                .fetch_transactions_and_notify(next_digest, send_page)
                .await
        });

        assert!(timeout(Duration::from_secs(3), next_page.recv())
            .await
            .expect("timed out waiting for expected resource exhausted error")
            .expect("page channel closed")
            .is_err());

        assert!(
            timeout(Duration::from_millis(1200), next_page.recv())
                .await
                .is_err(),
            "oversized single digest was retried after being quarantined"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 1);

        handle.abort();
    }
}
