use {
    super::query::EventQuery,
    crate::sui,
    futures::TryStreamExt as _,
    std::num::NonZeroUsize,
    sui_rpc::field::FieldMaskUtil as _,
    thiserror::Error,
    tokio::sync::mpsc,
    tokio_util::sync::CancellationToken,
};

const DEFAULT_CHANNEL_CAPACITY: usize = 100;
const DEFAULT_REPLAY_CONCURRENCY: NonZeroUsize = NonZeroUsize::MIN;
const ENGINE_EVENT_FIELDS: [&str; 3] = ["checkpoint", "transaction_digest", "event_index"];

/// Failure produced while ingesting events for an [`EventQuery`].
#[derive(Debug, Error)]
pub enum EventIngestionError {
    /// The endpoint or requested fields are invalid.
    #[error("Event ingestion configuration is invalid: {0}")]
    Configuration(String),
    /// A Sui RPC request or response failed.
    #[error("Sui event RPC failed while {operation}: {status}")]
    Rpc {
        /// Operation that failed.
        operation: &'static str,
        /// Status returned by Sui.
        #[source]
        status: tonic::Status,
    },
    /// Sui no longer retains the requested replay checkpoint.
    #[error(
        "Sui cannot replay events from checkpoint {start_checkpoint} while {operation}: {status}"
    )]
    ReplayGap {
        /// Inclusive checkpoint requested from Sui.
        start_checkpoint: u64,
        /// Replay operation that failed.
        operation: &'static str,
        /// Status returned by Sui.
        #[source]
        status: tonic::Status,
    },
    /// A Sui response violated the event stream contract.
    #[error("Sui event stream protocol failed: {0}")]
    Protocol(String),
    /// An event accepted by the query could not be decoded.
    #[error(
        "Event at checkpoint {checkpoint}, transaction {transaction_digest}, \
         index {event_index} could not be decoded: {source}"
    )]
    Decode {
        /// Checkpoint containing the event.
        checkpoint: u64,
        /// Transaction that emitted the event.
        transaction_digest: String,
        /// Position of the event in the transaction.
        event_index: u32,
        /// Query conversion failure.
        #[source]
        source: anyhow::Error,
    },
}

impl EventIngestionError {
    pub(super) fn rpc(operation: &'static str, status: tonic::Status) -> Self {
        Self::Rpc { operation, status }
    }

    pub(super) fn replay_rpc(
        start_checkpoint: u64,
        operation: &'static str,
        status: tonic::Status,
    ) -> Self {
        if status.code() == tonic::Code::OutOfRange {
            Self::ReplayGap {
                start_checkpoint,
                operation,
                status,
            }
        } else {
            Self::rpc(operation, status)
        }
    }

    pub(super) fn is_replay_gap(&self) -> bool {
        matches!(self, Self::ReplayGap { .. })
    }

    pub(super) fn is_retryable(&self) -> bool {
        let Self::Rpc { status, .. } = self else {
            return false;
        };

        matches!(
            status.code(),
            tonic::Code::Cancelled
                | tonic::Code::Unknown
                | tonic::Code::DeadlineExceeded
                | tonic::Code::ResourceExhausted
                | tonic::Code::Aborted
                | tonic::Code::Internal
                | tonic::Code::Unavailable
        )
    }
}

/// Source phase for one event page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventPageSource {
    /// Historical events requested from the Ledger service.
    Replay,
    /// Events received from the live subscription.
    Live,
}

/// Events and progress observed at one checkpoint position.
#[derive(Clone, Debug)]
pub struct EventPage<T> {
    /// Events emitted for this page.
    pub events: Vec<T>,
    /// Checkpoint observed for this page.
    pub checkpoint: u64,
    /// Whether this progress came from replay or the live subscription.
    pub source: EventPageSource,
}

/// Receives pages and failures from an [`EventIngestor`].
pub type EventPageReceiver<T> = mpsc::Receiver<Result<EventPage<T>, EventIngestionError>>;

/// Streams the output of an [`EventQuery`] from Sui.
pub struct EventIngestor<Q: EventQuery> {
    pub(super) subscription_rpc_url: String,
    pub(super) replay_rpc_url: Option<String>,
    pub(super) query: Q,
    pub(super) filter: sui::grpc::EventFilter,
    pub(super) read_mask: sui::grpc::FieldMask,
    pub(super) channel_capacity: usize,
    pub(super) replay_concurrency: NonZeroUsize,
    pub(super) cancellation_token: CancellationToken,
    pub(super) recover_replay_gap: bool,
}

impl<Q: EventQuery> EventIngestor<Q> {
    /// Creates an ingestor for `query`.
    pub fn new(subscription_rpc_url: impl Into<String>, query: Q) -> Self {
        let filter = query.filter();
        let read_mask = Self::effective_read_mask(query.read_mask());

        Self {
            subscription_rpc_url: subscription_rpc_url.into(),
            replay_rpc_url: None,
            query,
            filter,
            read_mask,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            replay_concurrency: DEFAULT_REPLAY_CONCURRENCY,
            cancellation_token: CancellationToken::new(),
            recover_replay_gap: false,
        }
    }

    /// Uses a distinct Sui service for the live event subscription.
    ///
    /// Historical replay continues to use the endpoint selected by
    /// [`Self::with_replay_rpc_url`]. This permits the subscription to bypass
    /// index publication without routing indexed state queries through an
    /// unindexed fullnode.
    pub fn with_subscription_rpc_url(mut self, rpc_url: impl Into<String>) -> Self {
        self.subscription_rpc_url = rpc_url.into();
        self
    }

    /// Uses a distinct Ledger service for historical replay.
    ///
    /// The endpoint passed to [`EventIngestor::new`] still owns the live
    /// subscription. This permits callers to read pruned history from an
    /// archival service without routing current object reads or transaction
    /// submission through that service.
    pub fn with_replay_rpc_url(mut self, rpc_url: impl Into<String>) -> Self {
        self.replay_rpc_url = Some(rpc_url.into());
        self
    }

    /// Sets the number of pages that may wait for the consumer.
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Sets the maximum number of disjoint checkpoint ranges replayed at once.
    ///
    /// Range results are delivered in ascending checkpoint order. This changes
    /// only recovery throughput; it does not change the event stream order.
    pub fn with_replay_concurrency(mut self, concurrency: NonZeroUsize) -> Self {
        self.replay_concurrency = concurrency;
        self
    }

    /// Sets the token used to stop ingestion.
    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = cancellation_token;
        self
    }

    /// Resumes at the live cursor when Sui no longer retains replay history.
    ///
    /// The ingestor reports [`EventIngestionError::ReplayGap`] before
    /// it resumes. Leave this disabled when the consumer requires every event
    /// from the requested checkpoint.
    pub fn with_replay_gap_recovery(mut self) -> Self {
        self.recover_replay_gap = true;
        self
    }

    /// Returns the earliest checkpoint accepted by the configured
    /// [`EventQuery::filter`].
    ///
    /// The query uses the replay endpoint when one was configured with
    /// [`Self::with_replay_rpc_url`]. It requests one indexed event in explicit
    /// ascending order and does not decode its payload.
    ///
    /// # Errors
    ///
    /// Returns [`EventIngestionError::ReplayGap`] when the endpoint no longer
    /// retains genesis history. Other endpoint and stream failures retain their
    /// normal [`EventIngestionError`] classification.
    pub async fn first_matching_checkpoint(&self) -> Result<Option<u64>, EventIngestionError> {
        let rpc_url = self
            .replay_rpc_url
            .as_deref()
            .unwrap_or(&self.subscription_rpc_url);
        let mut client = sui::grpc::client(rpc_url).map_err(|error| {
            EventIngestionError::Configuration(format!(
                "invalid replay gRPC URL '{rpc_url}': {error}"
            ))
        })?;
        let request = sui::grpc::ListEventsRequest::default()
            .with_read_mask(sui::grpc::FieldMask::from_paths(["checkpoint"]))
            .with_filter(self.filter.clone())
            .with_options(
                sui::grpc::QueryOptions::default()
                    .with_limit(1)
                    .with_ordering(sui::grpc::Ordering::Ascending),
            );
        let mut stream = client
            .ledger_client()
            .list_events(request)
            .await
            .map_err(|status| {
                EventIngestionError::replay_rpc(0, "locating the first matching event", status)
            })?
            .into_inner();

        while let Some(frame) = stream.try_next().await.map_err(|status| {
            EventIngestionError::replay_rpc(0, "reading the first matching event", status)
        })? {
            if let Some(event) = frame.event {
                return event.checkpoint.map(Some).ok_or_else(|| {
                    EventIngestionError::Protocol(
                        "the first matching event omitted its checkpoint".to_owned(),
                    )
                });
            }
        }

        Ok(None)
    }

    /// Starts ingestion from an inclusive checkpoint.
    ///
    /// Passing [`None`] starts at the current stream position.
    ///
    /// # Errors
    ///
    /// Returns [`EventIngestionError::Configuration`] when the endpoint or
    /// requested fields are invalid.
    pub fn start(
        self,
        from_checkpoint: Option<u64>,
    ) -> Result<EventPageReceiver<Q::Output>, EventIngestionError> {
        if self.channel_capacity == 0 {
            return Err(EventIngestionError::Configuration(
                "channel capacity must be greater than zero".to_owned(),
            ));
        }
        sui::grpc::client(&self.subscription_rpc_url).map_err(|error| {
            EventIngestionError::Configuration(format!(
                "invalid subscription gRPC URL '{}': {error}",
                self.subscription_rpc_url
            ))
        })?;
        if let Some(replay_rpc_url) = &self.replay_rpc_url {
            sui::grpc::client(replay_rpc_url).map_err(|error| {
                EventIngestionError::Configuration(format!(
                    "invalid replay gRPC URL '{replay_rpc_url}': {error}"
                ))
            })?;
        }
        self.read_mask
            .validate::<sui::grpc::Event>()
            .map_err(|path| {
                EventIngestionError::Configuration(format!("invalid event field '{path}'"))
            })?;

        let (send_page, next_page) = mpsc::channel(self.channel_capacity);
        tokio::spawn(self.run(from_checkpoint, send_page));

        Ok(next_page)
    }

    fn effective_read_mask(mut read_mask: sui::grpc::FieldMask) -> sui::grpc::FieldMask {
        read_mask
            .paths
            .extend(ENGINE_EVENT_FIELDS.map(str::to_owned));
        read_mask.normalize()
    }
}
