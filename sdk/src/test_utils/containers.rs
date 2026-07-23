//! Container setup helpers built with [`testcontainers`].
//!
//! The module supports Sui and Redis.

use {
    anyhow::Context as _,
    http::header::{HeaderValue, CONTENT_TYPE},
    portpicker::pick_unused_port,
    std::time::Duration,
    testcontainers::{
        core::{client, ports::ContainerPort, wait::HttpWaitStrategy, ContainerRequest, WaitFor},
        runners::AsyncRunner,
        ContainerAsync,
        GenericImage,
        ImageExt,
    },
    testcontainers_modules::{postgres::Postgres, redis::Redis},
};

/// A running Sui container built from [`GenericImage`].
pub type SuiContainer = ContainerAsync<GenericImage>;
pub type PgContainer = ContainerAsync<Postgres>;
pub type RedisContainer = ContainerAsync<Redis>;
pub type ExecCommand = testcontainers::core::ExecCommand;

const SUI_TOOLS_IMAGE: &str = "mysten/sui-tools";
const SUI_TOOLS_TAG_AMD64: &str = "testnet-v1.76.0";
const SUI_TOOLS_TAG_ARM64: &str = "testnet-v1.76.0-arm64";
const SUI_RPC_PORT: ContainerPort = ContainerPort::Tcp(9000);
const SUI_FAUCET_PORT: ContainerPort = ContainerPort::Tcp(9123);
const SUI_GRAPHQL_PORT: ContainerPort = ContainerPort::Tcp(9125);
const SUI_READY_REQUEST: &str = r#"{"jsonrpc":"2.0","method":"sui_getChainIdentifier","id":1}"#;

/// A running Sui test network and its mapped service ports.
pub struct SuiInstance {
    /// The container handle.
    pub container: SuiContainer,
    /// The mapped Sui gRPC port.
    pub rpc_port: u16,
    /// The mapped faucet port.
    pub faucet_port: u16,
}

/// Returns why Docker-based tests cannot run in the current environment.
pub async fn docker_unavailable_reason() -> Option<String> {
    let docker = match client::docker_client_instance().await {
        Ok(docker) => docker,
        Err(error) => return Some(format!("failed to create Docker client: {error}")),
    };

    docker
        .version()
        .await
        .err()
        .map(|error| format!("failed to query Docker daemon: {error}"))
}

fn sui_request(
    tag: &str,
    epoch_duration: Option<Duration>,
) -> anyhow::Result<ContainerRequest<GenericImage>> {
    let mut command = vec![
        "start".to_owned(),
        "--force-regenesis".to_owned(),
        "--with-faucet".to_owned(),
    ];

    if let Some(duration) = epoch_duration {
        let duration_ms: u64 = duration
            .as_millis()
            .try_into()
            .context("convert Sui epoch duration to milliseconds")?;
        command.push("--epoch-duration-ms".to_owned());
        command.push(duration_ms.to_string());
    }

    let ready = HttpWaitStrategy::new("/")
        .with_port(SUI_RPC_PORT)
        .with_method("POST".parse().expect("POST is a valid HTTP method"))
        .with_body(SUI_READY_REQUEST)
        .with_header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .with_expected_status_code(200_u16);

    Ok(GenericImage::new(SUI_TOOLS_IMAGE, tag)
        .with_entrypoint("sui")
        .with_exposed_port(SUI_RPC_PORT)
        .with_exposed_port(SUI_FAUCET_PORT)
        .with_exposed_port(SUI_GRAPHQL_PORT)
        .with_wait_for(WaitFor::http(ready))
        .with_env_var("RUST_LOG", "warning,sui_node=info")
        .with_cmd(command))
}

/// Starts a Sui container with the default epoch duration.
///
/// Use [`try_setup_sui_instance`] when startup errors must be handled or the
/// epoch duration must be configured.
///
/// # Panics
///
/// Panics when the container cannot be started.
pub async fn setup_sui_instance() -> SuiInstance {
    try_setup_sui_instance(None)
        .await
        .expect("Failed to start Sui container.")
}

/// Tries to start a Sui container with the requested epoch duration.
///
/// An explicit duration keeps suites with epoch scoped transactions within one
/// epoch. Passing [`None`] retains the Sui local network default.
pub async fn try_setup_sui_instance(
    epoch_duration: Option<Duration>,
) -> anyhow::Result<SuiInstance> {
    let rpc_host_port = pick_unused_port().context("find Sui RPC port")?;
    let faucet_host_port = loop {
        let port = pick_unused_port().context("find Sui faucet port")?;
        if port != rpc_host_port {
            break port;
        }
    };

    let docker = client::docker_client_instance()
        .await
        .context("connect to Docker")?;

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

    let sui_request = sui_request(sui_tools_tag, epoch_duration)?
        .with_mapped_port(rpc_host_port, SUI_RPC_PORT)
        .with_mapped_port(faucet_host_port, SUI_FAUCET_PORT);

    let container = sui_request.start().await.context("start Sui container")?;

    Ok(SuiInstance {
        container,
        rpc_port: rpc_host_port,
        faucet_port: faucet_host_port,
    })
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

#[cfg(test)]
mod tests {
    use {super::*, std::time::Duration, testcontainers::Image};

    #[test]
    fn sui_request_uses_configured_epoch_duration() {
        let command = sui_request(SUI_TOOLS_TAG_AMD64, Some(Duration::from_secs(600)))
            .unwrap()
            .cmd()
            .map(|argument| argument.into_owned())
            .collect::<Vec<_>>();

        assert!(command
            .windows(2)
            .any(|pair| pair == ["--epoch-duration-ms", "600000"]));
    }

    #[test]
    fn sui_request_preserves_the_container_contract() {
        let request = sui_request(SUI_TOOLS_TAG_AMD64, None).unwrap();
        let command = request
            .cmd()
            .map(|argument| argument.into_owned())
            .collect::<Vec<_>>();
        let environment = request
            .env_vars()
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();

        assert_eq!(command, vec!["start", "--force-regenesis", "--with-faucet"]);
        assert_eq!(request.descriptor(), "mysten/sui-tools:testnet-v1.76.0");
        assert_eq!(request.entrypoint(), Some("sui"));
        assert_eq!(
            request.image().expose_ports(),
            &[SUI_RPC_PORT, SUI_FAUCET_PORT, SUI_GRAPHQL_PORT]
        );
        assert_eq!(
            environment,
            vec![("RUST_LOG".to_owned(), "warning,sui_node=info".to_owned())]
        );
        assert!(matches!(
            request.image().ready_conditions().as_slice(),
            [WaitFor::Http(_)]
        ));
    }
}
