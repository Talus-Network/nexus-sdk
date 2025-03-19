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
    let sui_request = Sui::default()
        .with_force_regenesis(true)
        .with_faucet(true)
        .with_name("taluslabs/sui-tools")
        .with_tag("testnet-v1.38.1");

    let container = sui_request
        .start()
        .await
        .expect("Failed to start Sui container.");

    let rpc_port = container
        .get_host_port_ipv4(9000)
        .await
        .expect("Failed to get RPC port.");

    let faucet_port = container
        .get_host_port_ipv4(9123)
        .await
        .expect("Failed to get faucet port.");

    (container, rpc_port, faucet_port)
}

/// Spins up a Redis container and returns its handle and mapped Redis port.
pub async fn setup_redis_instance() -> (RedisContainer, u16) {
    let redis_request = Redis::default()
        .with_tag("7.4-alpine")
        .with_env_var("REDIS_PASSWORD", "my_secret_password");

    let container = redis_request
        .start()
        .await
        .expect("Failed to start Redis container.");

    let host_port = container
        .get_host_port_ipv4(6379)
        .await
        .expect("Failed to get Redis port.");

    (container, host_port)
}
