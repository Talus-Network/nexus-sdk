use {
    super::query::EventQuery,
    crate::sui,
    sui_rpc::field::FieldMaskUtil as _,
    thiserror::Error,
    tokio::sync::mpsc,
    tokio_util::sync::CancellationToken,
};

const DEFAULT_CHANNEL_CAPACITY: usize = 100;
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

/// Events and progress observed at one checkpoint position.
#[derive(Clone, Debug)]
pub struct EventPage<T> {
    /// Events emitted for this page.
    pub events: Vec<T>,
    /// Checkpoint observed for this page.
    pub checkpoint: u64,
}

/// Receives pages and failures from an [`EventIngestor`].
pub type EventPageReceiver<T> = mpsc::Receiver<Result<EventPage<T>, EventIngestionError>>;

/// Streams the output of an [`EventQuery`] from Sui.
pub struct EventIngestor<Q: EventQuery> {
    pub(super) rpc_url: String,
    pub(super) query: Q,
    pub(super) filter: sui::grpc::EventFilter,
    pub(super) read_mask: sui::grpc::FieldMask,
    pub(super) channel_capacity: usize,
    pub(super) cancellation_token: CancellationToken,
}

impl<Q: EventQuery> EventIngestor<Q> {
    /// Creates an ingestor for `query`.
    pub fn new(rpc_url: impl Into<String>, query: Q) -> Self {
        let filter = query.filter();
        let read_mask = Self::effective_read_mask(query.read_mask());

        Self {
            rpc_url: rpc_url.into(),
            query,
            filter,
            read_mask,
            channel_capacity: DEFAULT_CHANNEL_CAPACITY,
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Sets the number of pages that may wait for the consumer.
    pub fn with_channel_capacity(mut self, capacity: usize) -> Self {
        self.channel_capacity = capacity;
        self
    }

    /// Sets the token used to stop ingestion.
    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = cancellation_token;
        self
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
        sui::grpc::client(&self.rpc_url).map_err(|error| {
            EventIngestionError::Configuration(format!(
                "invalid gRPC URL '{}': {error}",
                self.rpc_url
            ))
        })?;
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
