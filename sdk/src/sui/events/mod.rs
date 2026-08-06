//! Generic queries and ingestion for Sui events.

mod driver;
mod ingestor;
mod metrics;
mod query;

pub use {
    ingestor::{EventIngestionError, EventIngestor, EventPage, EventPageReceiver},
    query::{EventQuery, RawEventQuery},
};

#[cfg(all(test, feature = "test_utils"))]
mod tests;
