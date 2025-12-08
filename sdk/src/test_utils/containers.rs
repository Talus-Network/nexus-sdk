//! Module defining container setups via [`testcontainers`].
//!
//! Contains functions for
//! - Sui
//! - Redis

use {
    portpicker::pick_unused_port,
    testcontainers_modules::{
        redis::Redis,
        sui::Sui,
        testcontainers::{
            core::ports::ContainerPort,
            runners::AsyncRunner,
            ContainerAsync,
            ImageExt,
        },
    },
};

pub type SuiContainer = ContainerAsync<Sui>;
pub type RedisContainer = ContainerAsync<Redis>;
pub type ExecCommand = testcontainers_modules::testcontainers::core::ExecCommand;

/// Spins up a Sui container and returns its handle and mapped RPC and faucet
/// ports.
pub async fn setup_sui_instance() -> (SuiContainer, u16, u16) {
    let rpc_host_port = pick_unused_port().expect("No free port for Sui RPC.");
    let faucet_host_port = loop {
        let port = pick_unused_port().expect("No free port for Sui faucet.");
        if port != rpc_host_port {
            break port;
        }
    };

    let sui_request = Sui::default()
        .with_force_regenesis(true)
        .with_faucet(true)
        // TODO: add indexer and gql.
        // .with_cmd(cmd)
        // TODO: adjust
        .with_name("mysten/sui-tools")
        .with_tag("mainnet-v1.61.2")
        .with_mapped_port(rpc_host_port, ContainerPort::Tcp(9000))
        .with_mapped_port(faucet_host_port, ContainerPort::Tcp(9123));

    let container = sui_request
        .start()
        .await
        .expect("Failed to start Sui container.");

    (container, rpc_host_port, faucet_host_port)
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
