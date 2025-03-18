//! Module definining container setups via [`testcontainers`].
//!
//! Contains functions for
//! - Sui
//! - Redis

use testcontainers_modules::{
    redis::Redis,
    sui::Sui,
    testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt},
};

pub type SuiContainer = ContainerAsync<Sui>;
pub type RedisContainer = ContainerAsync<Redis>;

/// Spins up a Sui container and returns its handle and mapped RPC and faucet
/// ports.
pub async fn setup_sui_instance() -> (SuiContainer, u16, u16) {
    let tag = if cfg!(target_arch = "aarch64") {
        "testnet-v1.38.2-arm64"
    } else {
        "testnet-v1.38.2"
    };

    let sui_request = Sui::default()
        .with_force_regenesis(true)
        .with_faucet(true)
        .with_tag(tag);

    let container = sui_request
        .start()
        .await
        .expect("Failed to start Sui container");

    let rpc_port = container
        .get_host_port_ipv4(9000)
        .await
        .expect("Failed to get mapped host port for Sui");

    let faucet_port = container
        .get_host_port_ipv4(9123)
        .await
        .expect("Failed to get mapped host port for Sui faucet");

    (container, rpc_port, faucet_port)
}

/// Spins up a Redis container and returns its handle and mapped Redis port.
pub async fn setup_redis_instance() -> (RedisContainer, u16) {
    let redis_request = Redis::default()
        .with_tag("6.2-alpine")
        .with_env_var("REDIS_PASSWORD", "my_secret_password");

    let container = redis_request
        .start()
        .await
        .expect("Failed to start Redis container");

    let host_port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("Failed to get mapped host port for Redis");

    (container, host_port)
}
