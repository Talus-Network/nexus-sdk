//! This module attempts to make a little bit of sense when dealing with Sui
//! types.
//!
//! This way we can use, for example `sui::types::Address` in our code.

pub mod types {
    pub use sui_sdk_types::*;
}

pub mod crypto {
    pub use sui_crypto::{ed25519::Ed25519PrivateKey, *};
}

mod request_budget;

pub mod grpc {
    use std::{
        collections::HashMap,
        num::NonZeroU32,
        sync::{Arc, LazyLock, Mutex},
        time::Duration,
    };
    pub use sui_rpc::{field::FieldMask, proto::sui::rpc::v2::*, Client};

    /// Metadata used by nodes that can return execution only after the
    /// transaction checkpoint is locally visible.
    pub mod checkpoint_wait {
        const HEADER: &str = "x-sui-checkpoint-wait";

        /// Return whether a service response advertises checkpoint waiting.
        pub fn is_supported<T>(response: &tonic::Response<T>) -> bool {
            response
                .metadata()
                .get(HEADER)
                .is_some_and(|value| value == "true")
        }

        /// Build an execution request that waits for local checkpoint
        /// visibility on a node which advertised support.
        pub fn execution_request<T>(message: T) -> tonic::Request<T> {
            let mut request = tonic::Request::new(message);
            request
                .metadata_mut()
                .insert(HEADER, tonic::metadata::MetadataValue::from_static("true"));
            request
        }
    }

    /// Maximum independent HTTP/2 connections retained for one RPC endpoint.
    ///
    /// A Sui client and all of its clones multiplex responses through one
    /// connection. A burst of small responses can therefore exhaust h2's
    /// framing budget before callers consume them. Independent clients are
    /// spread across this bounded set so one busy connection cannot fail the
    /// entire process.
    const MAX_CHANNELS_PER_ENDPOINT: usize = 32;

    /// Maximum wait for an RPC response to begin.
    ///
    /// Streaming responses remain open after their headers arrive. This bound
    /// prevents a failed or saturated connection from parking an ordinary RPC
    /// forever before the server starts its response.
    const RESPONSE_HEADERS_TIMEOUT: Duration = Duration::from_secs(30);

    /// Request metadata for finalized causal simulation.
    #[cfg(feature = "events")]
    pub mod causal {
        use super::super::types::Digest;

        const PARENT_HEADER: &str = "x-sui-causal-parent";
        const RECORD_HEADER: &str = "x-sui-causal-record";
        const APPLIED_HEADER: &str = "x-sui-causal-applied";
        const STREAM_HEADER: &str = "x-sui-causal-stream";

        /// Build an execution request whose finalized state is retained.
        ///
        /// `parent` links the new state to a previously finalized transaction.
        /// An absent parent starts a new bounded causal view.
        pub fn execution_request<T>(
            message: T,
            parent: Option<Digest>,
        ) -> anyhow::Result<tonic::Request<T>> {
            let mut request = tonic::Request::new(message);
            request.metadata_mut().insert(
                RECORD_HEADER,
                tonic::metadata::MetadataValue::from_static("true"),
            );
            if let Some(parent) = parent {
                request.metadata_mut().insert(
                    PARENT_HEADER,
                    tonic::metadata::MetadataValue::try_from(parent.to_string())?,
                );
            }
            Ok(request)
        }

        /// Build a simulation request that must include `parent` state.
        pub fn simulation_request<T>(
            message: T,
            parent: Digest,
        ) -> anyhow::Result<tonic::Request<T>> {
            let mut request = tonic::Request::new(message);
            request.metadata_mut().insert(
                PARENT_HEADER,
                tonic::metadata::MetadataValue::try_from(parent.to_string())?,
            );
            Ok(request)
        }

        /// Return whether the server honored the requested causal view.
        pub fn was_applied<T>(response: &tonic::Response<T>) -> bool {
            response
                .metadata()
                .get(APPLIED_HEADER)
                .is_some_and(|value| value == "true")
        }

        /// Build an opt in stream request for quorum finalized receipts.
        ///
        /// The stream is opportunistic and has no replay cursor. Consumers
        /// must retain the canonical checkpoint stream as their recovery path.
        pub fn finality_stream_request<T>(message: T) -> tonic::Request<T> {
            let mut request = tonic::Request::new(message);
            request.metadata_mut().insert(
                STREAM_HEADER,
                tonic::metadata::MetadataValue::from_static("true"),
            );
            request
        }

        /// Return whether the server accepted the causal finality stream.
        pub fn finality_stream_was_applied<T>(response: &tonic::Response<T>) -> bool {
            response
                .metadata()
                .get(STREAM_HEADER)
                .is_some_and(|value| value == "true")
        }
    }

    static TRANSPORT_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("nexus-sui-grpc")
            .enable_all()
            .build()
            .expect("Sui gRPC transport runtime should start")
    });

    #[derive(Default)]
    struct EndpointPool {
        clients: Vec<Client>,
        next: usize,
        budget: Arc<Mutex<super::request_budget::RequestBudget>>,
    }

    #[derive(Default)]
    struct ClientPool {
        endpoints: Mutex<HashMap<String, EndpointPool>>,
    }

    impl ClientPool {
        fn client(&self, rpc_url: impl AsRef<str>) -> anyhow::Result<Client> {
            let rpc_url = rpc_url.as_ref();
            let mut endpoints = self
                .endpoints
                .lock()
                .map_err(|_| anyhow::anyhow!("Sui gRPC client pool lock was poisoned"))?;

            let endpoint = endpoints.entry(rpc_url.to_owned()).or_default();
            if endpoint.clients.len() < MAX_CHANNELS_PER_ENDPOINT {
                let budget = Arc::clone(&endpoint.budget);
                let client = {
                    let _runtime = TRANSPORT_RUNTIME.enter();
                    Client::new(rpc_url)
                        .map_err(anyhow::Error::new)?
                        .with_response_headers_timeout(RESPONSE_HEADERS_TIMEOUT)
                        .request_layer(tower::layer::layer_fn(move |service| {
                            super::request_budget::BudgetService::new(service, Arc::clone(&budget))
                        }))
                };
                endpoint.clients.push(client.clone());
                return Ok(client);
            }

            let index = endpoint.next;
            endpoint.next = (endpoint.next + 1) % endpoint.clients.len();
            Ok(endpoint.clients[index].clone())
        }
    }

    static CLIENT_POOL: LazyLock<ClientPool> = LazyLock::new(ClientPool::default);

    /// Returns a [`Client`] backed by a bounded process wide connection pool.
    ///
    /// # Errors
    ///
    /// Returns an error when the endpoint is invalid or the client pool is
    /// unavailable.
    pub fn client(rpc_url: impl AsRef<str>) -> anyhow::Result<Client> {
        CLIENT_POOL.client(rpc_url)
    }

    /// Bound recovery RPC attempts to this URL across pooled clients and their clones.
    ///
    /// Calls inside [`with_retry_budget`] include reads, submissions, and stream establishment. Existing
    /// streams keep receiving without consuming tokens. No request is retried by
    /// this layer, so callers retain control of transaction identity and deadlines.
    /// Repeating the same configuration does not replenish the shared budget.
    /// Ordinary calls and endpoints without a configured budget preserve unrestricted admission.
    pub fn set_retry_request_budget(
        rpc_url: &str,
        requests_per_second: NonZeroU32,
        burst: NonZeroU32,
    ) -> anyhow::Result<()> {
        client(rpc_url)?;
        let endpoints = CLIENT_POOL
            .endpoints
            .lock()
            .map_err(|_| anyhow::anyhow!("Sui gRPC client pool lock was poisoned"))?;
        endpoints[rpc_url]
            .budget
            .lock()
            .map_err(|_| anyhow::anyhow!("Sui RPC budget lock was poisoned"))?
            .configure(requests_per_second, burst);
        Ok(())
    }

    /// Apply the configured shared budget to every RPC made by a recovery attempt.
    ///
    /// This task scope leaves ordinary parallel execution unrestricted. It does
    /// not spawn work, repeat effects, or change the identity of a submission.
    pub async fn with_retry_budget<F: std::future::Future>(attempt: F) -> F::Output {
        super::request_budget::scope(attempt).await
    }

    /// Retain concurrent preparation until its observation deadline.
    ///
    /// Crawler reads retry only transient transport failures, retaining completed
    /// sibling reads. Dropping the scope cancels reads and waits. Use this only
    /// for preparation before an external effect can occur.
    pub async fn with_read_retry_until<F: std::future::Future>(
        deadline: tokio::time::Instant,
        preparation: F,
    ) -> F::Output {
        super::request_budget::read_scope(Some(deadline), preparation).await
    }

    /// End preparation retry semantics before performing or processing effects.
    pub async fn without_read_retry<F: std::future::Future>(operation: F) -> F::Output {
        super::request_budget::read_scope(None, operation).await
    }

    pub(crate) async fn retry_read<T, F, Fut>(operation: F) -> Result<T, tonic::Status>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Result<T, tonic::Status>>,
    {
        super::request_budget::read(operation).await
    }

    #[cfg(test)]
    mod tests {
        use super::{ClientPool, MAX_CHANNELS_PER_ENDPOINT};

        #[test]
        fn client_pool_bounds_channels_per_endpoint() {
            let pool = ClientPool::default();

            for _ in 0..MAX_CHANNELS_PER_ENDPOINT + 1 {
                let _client = pool.client("http://127.0.0.1:1").unwrap();
            }

            let endpoints = pool.endpoints.lock().unwrap();
            assert_eq!(endpoints.len(), 1);
            assert_eq!(
                endpoints["http://127.0.0.1:1"].clients.len(),
                MAX_CHANNELS_PER_ENDPOINT
            );
            assert_eq!(endpoints["http://127.0.0.1:1"].next, 1);
        }

        #[test]
        fn checkpoint_wait_metadata_is_explicit() {
            let request = super::checkpoint_wait::execution_request(());
            assert_eq!(
                request.metadata().get("x-sui-checkpoint-wait").unwrap(),
                "true"
            );

            let mut response = tonic::Response::new(());
            assert!(!super::checkpoint_wait::is_supported(&response));
            response.metadata_mut().insert(
                "x-sui-checkpoint-wait",
                tonic::metadata::MetadataValue::from_static("true"),
            );
            assert!(super::checkpoint_wait::is_supported(&response));
        }

        #[cfg(feature = "events")]
        #[test]
        fn causal_requests_carry_the_parent_digest() {
            let parent = super::super::types::Digest::ZERO;
            let request = super::causal::simulation_request((), parent).unwrap();

            assert_eq!(
                request.metadata().get("x-sui-causal-parent").unwrap(),
                parent.to_string().as_str(),
            );

            let stream = super::causal::finality_stream_request(());
            assert_eq!(
                stream.metadata().get("x-sui-causal-stream").unwrap(),
                "true",
            );
        }
    }
}

/// Generic Sui event queries and ingestion.
#[cfg(feature = "events")]
pub mod events;

/// Sui traits re-exported so that we can `use sui::traits::*` in our code.
pub mod traits {
    pub use {sui_crypto::SuiSigner, sui_rpc::field::FieldMaskUtil, sui_sdk_types::bcs::ToBcs};
}

pub const MIST_PER_SUI: u64 = 1_000_000_000;

/// Compiled Move package data accepted by package publishing APIs.
///
/// This value contains only transaction input data. Move project compilation
/// and file system access remain outside the registry compatible SDK.
/// Existing TAP publish artifacts describe skill metadata after deployment,
/// so they cannot carry the module bytes required by a Sui publish command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovePackageArtifact {
    /// Serialized Move modules in dependency order.
    pub modules: Vec<Vec<u8>>,
    /// Published storage package IDs for package dependencies.
    pub dependency_ids: Vec<types::Address>,
}
