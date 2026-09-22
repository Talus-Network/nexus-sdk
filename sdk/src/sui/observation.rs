//! Optional observations at SDK boundaries. Applications own metric names,
//! registries, export, and policy. No observer is installed by default.

use std::{sync::OnceLock, time::Duration};

/// Bounded RPC names. Unrecognized methods share one value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcMethod {
    GetObject,
    BatchGetObjects,
    GetTransaction,
    BatchGetTransactions,
    GetCheckpoint,
    GetServiceInfo,
    GetEpoch,
    ListDynamicFields,
    ListOwnedObjects,
    ListTransactions,
    ExecuteTransaction,
    SimulateTransaction,
    SubscribeCheckpoints,
    SubscribeExecutedTransactions,
    GetBalance,
    Other,
}

impl RpcMethod {
    /// All possible values, in discriminant order.
    pub const ALL: [Self; 16] = [
        Self::GetObject,
        Self::BatchGetObjects,
        Self::GetTransaction,
        Self::BatchGetTransactions,
        Self::GetCheckpoint,
        Self::GetServiceInfo,
        Self::GetEpoch,
        Self::ListDynamicFields,
        Self::ListOwnedObjects,
        Self::ListTransactions,
        Self::ExecuteTransaction,
        Self::SimulateTransaction,
        Self::SubscribeCheckpoints,
        Self::SubscribeExecutedTransactions,
        Self::GetBalance,
        Self::Other,
    ];

    /// Stable method label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::GetObject => "GetObject",
            Self::BatchGetObjects => "BatchGetObjects",
            Self::GetTransaction => "GetTransaction",
            Self::BatchGetTransactions => "BatchGetTransactions",
            Self::GetCheckpoint => "GetCheckpoint",
            Self::GetServiceInfo => "GetServiceInfo",
            Self::GetEpoch => "GetEpoch",
            Self::ListDynamicFields => "ListDynamicFields",
            Self::ListOwnedObjects => "ListOwnedObjects",
            Self::ListTransactions => "ListTransactions",
            Self::ExecuteTransaction => "ExecuteTransaction",
            Self::SimulateTransaction => "SimulateTransaction",
            Self::SubscribeCheckpoints => "SubscribeCheckpoints",
            Self::SubscribeExecutedTransactions => "SubscribeExecutedTransactions",
            Self::GetBalance => "GetBalance",
            Self::Other => "other",
        }
    }

    pub(super) fn from_path(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|method| method.label() == name)
            .unwrap_or(Self::Other)
    }
}

/// Admission class, independent of whether this is a repeated logical read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Traffic {
    Normal,
    Recovery,
}

/// SDK boundaries with fixed semantics and no resource identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    /// One physical request after admission, through trailers or cancellation.
    Rpc { method: RpcMethod, traffic: Traffic },
    /// One crawler read, including its repeated attempts and waits.
    Read,
    /// One invocation of a crawler read callback, before transport admission.
    ReadAttempt,
    /// Delay between crawler read attempts.
    RetryDelay,
    /// Time obtaining permission for one recovery request.
    RequestAdmission,
    /// One startup read and its validation, including recovery attempts.
    RecoveryRead,
    /// One startup read callback, which can contain multiple RPC requests.
    RecoveryAttempt,
    /// Delay between startup read attempts.
    RecoveryDelay,
}

/// Terminal operation outcome. A protocol failure can still be a successful RPC.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Success,
    Error(Option<tonic::Code>),
    Deadline,
    Cancelled,
    Skipped,
}

/// Applications implement these bounded, synchronous callbacks. Implementations
/// must not block, perform I/O, or panic. Export belongs outside request execution.
pub trait Observer: Send + Sync + 'static {
    /// An operation has begun.
    fn started(&self, operation: Operation);
    /// Exactly one outcome for every started operation, including cancellation.
    fn finished(&self, operation: Operation, elapsed: Duration, outcome: Outcome);
}

static OBSERVER: OnceLock<Box<dyn Observer>> = OnceLock::new();

/// Install the application observer once, before starting SDK work. The original
/// observer is returned if one is already installed. This follows the lifetime
/// of the shared process RPC pool and does not change transport policy.
pub fn install(observer: Box<dyn Observer>) -> Result<(), Box<dyn Observer>> {
    OBSERVER.set(observer)
}

struct ActiveObservation {
    observer: &'static dyn Observer,
    operation: Operation,
    started: tokio::time::Instant,
    outcome: Outcome,
}

pub(crate) struct Observation(Option<ActiveObservation>);

impl Observation {
    #[cfg(test)]
    pub(super) fn for_test(observer: &'static dyn Observer, operation: Operation) -> Self {
        observer.started(operation);
        Self(Some(ActiveObservation {
            observer,
            operation,
            started: tokio::time::Instant::now(),
            outcome: Outcome::Cancelled,
        }))
    }

    pub(crate) fn start(operation: Operation) -> Self {
        Self(OBSERVER.get().map(|observer| {
            observer.started(operation);
            ActiveObservation {
                observer: observer.as_ref(),
                operation,
                started: tokio::time::Instant::now(),
                outcome: Outcome::Cancelled,
            }
        }))
    }

    pub(crate) fn finish(mut self, outcome: Outcome) {
        if let Some(active) = self.0.as_mut() {
            active.outcome = outcome;
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.0.is_some()
    }
}

impl Drop for Observation {
    fn drop(&mut self) {
        if let Some(active) = &self.0 {
            active
                .observer
                .finished(active.operation, active.started.elapsed(), active.outcome);
        }
    }
}
