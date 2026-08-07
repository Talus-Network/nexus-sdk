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

pub mod grpc {
    use std::{
        collections::HashMap,
        sync::{LazyLock, Mutex},
    };
    pub use sui_rpc::{field::FieldMask, proto::sui::rpc::v2::*, Client};

    static TRANSPORT_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("nexus-sui-grpc")
            .enable_all()
            .build()
            .expect("Sui gRPC transport runtime should start")
    });

    #[derive(Default)]
    struct ClientPool {
        clients: Mutex<HashMap<String, Client>>,
    }

    impl ClientPool {
        fn client(&self, rpc_url: impl AsRef<str>) -> anyhow::Result<Client> {
            let rpc_url = rpc_url.as_ref();
            let mut clients = self
                .clients
                .lock()
                .map_err(|_| anyhow::anyhow!("Sui gRPC client pool lock was poisoned"))?;

            if let Some(client) = clients.get(rpc_url) {
                return Ok(client.clone());
            }

            let client = {
                let _runtime = TRANSPORT_RUNTIME.enter();
                Client::new(rpc_url).map_err(anyhow::Error::new)?
            };
            clients.insert(rpc_url.to_owned(), client.clone());
            Ok(client)
        }
    }

    static CLIENT_POOL: LazyLock<ClientPool> = LazyLock::new(ClientPool::default);

    /// Returns a [`Client`] backed by the process wide channel for `rpc_url`.
    ///
    /// # Errors
    ///
    /// Returns an error when the endpoint is invalid or the client pool is
    /// unavailable.
    pub fn client(rpc_url: impl AsRef<str>) -> anyhow::Result<Client> {
        CLIENT_POOL.client(rpc_url)
    }

    #[cfg(test)]
    mod tests {
        use super::ClientPool;

        #[test]
        fn client_pool_owns_its_transport_runtime() {
            let pool = ClientPool::default();

            let _first = pool.client("http://127.0.0.1:1").unwrap();
            let _second = pool.client("http://127.0.0.1:1").unwrap();

            assert_eq!(pool.clients.lock().unwrap().len(), 1);
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

/// Move build support for production package publishing and tests.
#[cfg(any(feature = "move_publish", feature = "test_utils"))]
pub mod build {
    pub use {
        move_package_alt::schema::Environment,
        sui_move_build::{BuildConfig, CompiledPackage},
    };
}
