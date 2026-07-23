use {
    crate::sui,
    std::{convert::Infallible, error::Error},
};

/// Defines the Sui request and output conversion for an [`EventIngestor`].
pub trait EventQuery: Send + Sync + 'static {
    /// Event type emitted by the query.
    type Output: Send + 'static;
    /// Error returned while converting a Sui event.
    type Error: Error + Send + Sync + 'static;

    /// Returns the server filter for this query.
    fn filter(&self) -> sui::grpc::EventFilter;

    /// Returns the fields requested by this query.
    fn read_mask(&self) -> sui::grpc::FieldMask;

    /// Converts one Sui event into an optional query result.
    ///
    /// Returning [`None`] intentionally excludes an event accepted by the
    /// server filter.
    fn decode(&self, event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error>;
}

/// Query that returns unmodified Sui events.
#[derive(Clone)]
pub struct RawEventQuery {
    filter: sui::grpc::EventFilter,
    read_mask: sui::grpc::FieldMask,
}

impl RawEventQuery {
    /// Creates a query from a server filter and requested fields.
    pub fn new(filter: sui::grpc::EventFilter, read_mask: sui::grpc::FieldMask) -> Self {
        Self { filter, read_mask }
    }
}

impl EventQuery for RawEventQuery {
    type Error = Infallible;
    type Output = sui::grpc::Event;

    fn filter(&self) -> sui::grpc::EventFilter {
        self.filter.clone()
    }

    fn read_mask(&self) -> sui::grpc::FieldMask {
        self.read_mask.clone()
    }

    fn decode(&self, event: sui::grpc::Event) -> Result<Option<Self::Output>, Self::Error> {
        Ok(Some(event))
    }
}
