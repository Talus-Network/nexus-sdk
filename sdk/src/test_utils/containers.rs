//! Module defining container setups via [`testcontainers`].
//!
//! Contains functions for
//! - Sui
//! - Redis

use {
    portpicker::pick_unused_port,
    testcontainers_modules::{
        postgres::Postgres,
        redis::Redis,
        sui::Sui,
        testcontainers::{
            bollard::network::CreateNetworkOptions,
            core::{client, ports::ContainerPort, Mount},
            runners::AsyncRunner,
            ContainerAsync,
            ImageExt,
        },
    },
};

pub type SuiContainer = ContainerAsync<Sui>;
pub type PgContainer = ContainerAsync<Postgres>;
pub type RedisContainer = ContainerAsync<Redis>;
pub type ExecCommand = testcontainers_modules::testcontainers::core::ExecCommand;

pub struct SuiInstance {
    pub container: SuiContainer,
    pub pg: PgContainer,
    pub rpc_port: u16,
    pub faucet_port: u16,
    pub graphql_port: u16,
}

/// Spins up a Sui container and returns its handle and mapped RPC and faucet
/// ports.
pub async fn setup_sui_instance() -> SuiInstance {
    let rpc_host_port = pick_unused_port().expect("No free port for Sui RPC.");
    let faucet_host_port = loop {
        let port = pick_unused_port().expect("No free port for Sui faucet.");
        if port != rpc_host_port {
            break port;
        }
    };
    let graphql_host_port = loop {
        let port = pick_unused_port().expect("No free port for Sui GraphQL.");
        if port != rpc_host_port && port != faucet_host_port {
            break port;
        }
    };

    // Create a `sui-net` Docker network for Sui and Postgres to communicate.
    // If it already exists, it will be reused.
    let docker = client::docker_client_instance()
        .await
        .expect("Failed to get Docker client.");
    let mut request = CreateNetworkOptions::default();
    request.name = "sui-net";
    let _ = docker.create_network(request).await.ok();

    let container_name = format!("sui-postgres-{}", rpc_host_port);

    let pg_request = Postgres::default()
        .with_tag("latest")
        .with_mount(Mount::tmpfs_mount("/postgres_data"))
        .with_network("sui-net")
        .with_container_name(&container_name);

    let pg_container = pg_request
        .start()
        .await
        .expect("Failed to start Postgres container for Sui.");

    let sui_request = Sui::default()
        .with_force_regenesis(true)
        .with_faucet(true)
        .with_indexer(true)
        .with_indexer_pg_url(format!(
            "postgres://postgres:postgres@{}:5432/postgres",
            container_name
        ))
        .with_graphql(true)
        .with_name("mysten/sui-tools")
        .with_tag("mainnet-v1.61.2")
        .with_mapped_port(rpc_host_port, ContainerPort::Tcp(9000))
        .with_mapped_port(faucet_host_port, ContainerPort::Tcp(9123))
        .with_mapped_port(graphql_host_port, ContainerPort::Tcp(9125))
        .with_network("sui-net");

    let container = sui_request
        .start()
        .await
        .expect("Failed to start Sui container.");

    SuiInstance {
        container,
        pg: pg_container,
        rpc_port: rpc_host_port,
        faucet_port: faucet_host_port,
        graphql_port: graphql_host_port,
    }
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
