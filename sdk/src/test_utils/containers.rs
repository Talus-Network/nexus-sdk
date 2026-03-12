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
            core::{client, ports::ContainerPort},
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

const SUI_TOOLS_TAG_AMD64: &str = "mainnet-v1.67.3";
const SUI_TOOLS_TAG_ARM64: &str = "mainnet-v1.67.3-arm64";

pub struct SuiInstance {
    pub container: SuiContainer,
    pub rpc_port: u16,
    pub faucet_port: u16,
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

    let docker = client::docker_client_instance()
        .await
        .expect("Failed to get Docker client.");

    let sui_tools_tag = docker
        .version()
        .await
        .ok()
        .and_then(|version| version.arch)
        .map(|arch| arch.to_ascii_lowercase())
        .as_deref()
        .map(|arch| match arch {
            "arm64" | "aarch64" => SUI_TOOLS_TAG_ARM64,
            _ => SUI_TOOLS_TAG_AMD64,
        })
        .unwrap_or_else(|| {
            if cfg!(target_arch = "aarch64") {
                SUI_TOOLS_TAG_ARM64
            } else {
                SUI_TOOLS_TAG_AMD64
            }
        });

    let sui_request = Sui::default()
        .with_force_regenesis(true)
        .with_faucet(true)
        .with_name("mysten/sui-tools")
        .with_tag(sui_tools_tag)
        .with_mapped_port(rpc_host_port, ContainerPort::Tcp(9000))
        .with_mapped_port(faucet_host_port, ContainerPort::Tcp(9123));

    let container = sui_request
        .start()
        .await
        .expect("Failed to start Sui container.");

    SuiInstance {
        container,
        rpc_port: rpc_host_port,
        faucet_port: faucet_host_port,
    }
}

/// Spins up a Redis container and returns its handle and mapped Redis port.
pub async fn setup_redis_instance() -> (RedisContainer, u16) {
    const MAX_ATTEMPTS: usize = 10;

    for attempt in 1..=MAX_ATTEMPTS {
        let host_port = pick_unused_port().expect("No free port for Redis.");

        let redis_request = Redis::default()
            .with_tag("7.4-alpine")
            .with_env_var("REDIS_PASSWORD", "my_secret_password")
            .with_mapped_port(host_port, ContainerPort::Tcp(6379));

        match redis_request.start().await {
            Ok(container) => {
                let host_port = container
                    .get_host_port_ipv4(6379)
                    .await
                    .expect("Failed to get Redis port.");
                return (container, host_port);
            }
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("address already in use")
                    || msg.contains("failed to bind host port")
                    || msg.contains("bind host port")
                {
                    eprintln!(
                        "setup_redis_instance: port bind collision on attempt {attempt}/{MAX_ATTEMPTS}: {msg}"
                    );
                    continue;
                }

                panic!("Failed to start Redis container: {err}");
            }
        }
    }

    panic!("Failed to start Redis container after {MAX_ATTEMPTS} attempts");
}
