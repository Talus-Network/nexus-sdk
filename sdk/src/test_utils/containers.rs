//! Module defining container setups via [`testcontainers`].
//!
//! Contains functions for
//! - Sui
//! - Redis

use {
    anyhow::Context as _,
    portpicker::pick_unused_port,
    std::time::Duration,
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

const SUI_TOOLS_TAG_AMD64: &str = "testnet-v1.76.0";
const SUI_TOOLS_TAG_ARM64: &str = "testnet-v1.76.0-arm64";

pub struct SuiInstance {
    pub container: SuiContainer,
    pub rpc_port: u16,
    pub faucet_port: u16,
}

/// Returns why Docker-backed tests cannot run in the current environment.
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

fn sui_image(epoch_duration: Option<Duration>) -> anyhow::Result<Sui> {
    let mut image = Sui::default().with_force_regenesis(true).with_faucet(true);

    if let Some(duration) = epoch_duration {
        let duration = duration
            .as_millis()
            .try_into()
            .context("convert Sui epoch duration to milliseconds")?;
        image = image.with_epoch_duration_ms(duration);
    }

    Ok(image)
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

    let sui_request = sui_image(epoch_duration)?
        .with_name("mysten/sui-tools")
        .with_tag(sui_tools_tag)
        .with_mapped_port(rpc_host_port, ContainerPort::Tcp(9000))
        .with_mapped_port(faucet_host_port, ContainerPort::Tcp(9123));

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
    use {
        super::*,
        std::{borrow::Cow, time::Duration},
        testcontainers_modules::testcontainers::Image,
    };

    #[test]
    fn sui_image_uses_configured_epoch_duration() {
        let command = sui_image(Some(Duration::from_secs(600)))
            .unwrap()
            .cmd()
            .into_iter()
            .map(|argument| {
                let argument: Cow<'_, str> = argument.into();
                argument.into_owned()
            })
            .collect::<Vec<_>>();

        assert!(command
            .windows(2)
            .any(|pair| pair == ["--epoch-duration-ms", "600000"]));
    }
}
