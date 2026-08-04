//! Constructs CLI Sui and Nexus clients and attaches transaction gas.

use {
    crate::{loading, prelude::*},
    base64::{prelude::BASE64_STANDARD, Engine},
    nexus_sdk::{
        nexus::{
            client::{AddressBalanceGas, GasSource, NexusClient},
            error::NexusError,
            protocol::ProtocolExtras,
        },
        sui,
    },
};

/// Build Sui client for the provided Sui net.
pub(crate) async fn build_sui_grpc_client(
    conf: &CliConf,
) -> AnyResult<Arc<sui::grpc::Client>, NexusCliError> {
    let client_handle = loading!("Building Sui client...");

    // Try to get the `SUI_RPC_URL` from the environment, otherwise use
    // the configuration.
    let Some(url) = std::env::var("SUI_RPC_URL")
        .ok()
        .or_else(|| conf.sui.rpc_url.as_ref().map(|u| u.to_string()))
    else {
        client_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "{message}\n\n{command}",
            message = "The Sui RPC URL is not configured. Please set it via the environment variable or the CLI configuration.",
            command = "$ nexus conf set --sui.rpc-url <url>".to_string().bold(),
        )));
    };

    match sui::grpc::Client::new(url) {
        Ok(client) => {
            client_handle.success();

            Ok(Arc::new(client))
        }
        Err(e) => {
            client_handle.error();

            Err(NexusCliError::Rpc(e.into()))
        }
    }
}

/// Parses an Ed25519 private key from base64.
///
/// Tries formats in order (like Sui's keytool import):
/// 1. Base64 33 bytes (flag + key) - Sui format, flag must be 0x00 (ed25519)
/// 2. Base64 32 bytes (raw key) - assumes Ed25519
fn parse_ed25519_private_key(
    pk_encoded: &str,
) -> AnyResult<sui::crypto::Ed25519PrivateKey, String> {
    let pk_bytes = BASE64_STANDARD
        .decode(pk_encoded)
        .map_err(|e| format!("Failed to decode Sui private key from base64: {e}"))?;

    // Try Sui format: 33 bytes (flag + key)
    if let Ok(bytes) = <[u8; 33]>::try_from(pk_bytes.as_slice()) {
        const ED25519_FLAG: u8 = 0x00;
        return match bytes[0] {
            ED25519_FLAG => Ok(sui::crypto::Ed25519PrivateKey::new(
                bytes[1..].try_into().unwrap(),
            )),
            flag => Err(format!(
                "unsupported key scheme flag 0x{flag:02x}, only ed25519 (0x00) is supported"
            )),
        };
    }

    // Try raw Ed25519: 32 bytes
    if let Ok(bytes) = <[u8; 32]>::try_from(pk_bytes.as_slice()) {
        return Ok(sui::crypto::Ed25519PrivateKey::new(bytes));
    }

    Err(format!(
        "invalid private key length {}, expected 32 (raw ed25519) or 33 (sui format with flag)",
        pk_bytes.len()
    ))
}

/// Create a wallet context from the provided path.
pub(crate) async fn get_signing_key(
    conf: &CliConf,
) -> AnyResult<sui::crypto::Ed25519PrivateKey, NexusCliError> {
    let key_handle = loading!("Retrieving Sui signing key...");

    // Try to get the `SUI_PK` from the environment, otherwise use the
    // configuration. This value is a base64 encoded string of the private key
    // bytes.
    let Some(pk_encoded) = std::env::var("SUI_PK")
        .ok()
        .or_else(|| conf.sui.pk.clone().map(|pk| pk.peek().to_string()))
    else {
        key_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "{message}\n\n{command}",
            message = "The Sui private key is not configured. Please set it via environment or the CLI configuration.",
            command = "$ nexus conf set --sui.pk <base64_encoded_key>".to_string().bold(),
        )));
    };

    match parse_ed25519_private_key(&pk_encoded) {
        Ok(key) => {
            key_handle.success();
            Ok(key)
        }
        Err(e) => {
            key_handle.error();
            Err(NexusCliError::Any(anyhow!("{e}")))
        }
    }
}

/// Wrapping some conf parsing functionality used around the CLI.
pub(crate) async fn get_nexus_objects(
    conf: &mut CliConf,
) -> AnyResult<NexusObjects, NexusCliError> {
    let objects_handle = loading!("Loading Nexus object IDs configuration...");

    // If objects are configured locally, return them.
    if let Some(objects) = conf.nexus.clone() {
        objects_handle.success();

        return Ok(objects);
    }

    // For some networks, we attempt to load the objects from public endpoints.
    let response = match conf.sui.rpc_url.as_ref() {
        Some(url) if url.as_str() == DEVNET_NEXUS_RPC_URL => {
            fetch_objects_from_url(DEVNET_OBJECTS_TOML).await
        }
        Some(url) if url.as_str() == TESTNET_NEXUS_RPC_URL => {
            fetch_objects_from_url(TESTNET_OBJECTS_TOML).await
        }
        Some(url) if url.as_str() == MAINNET_NEXUS_RPC_URL => {
            fetch_objects_from_url(MAINNET_OBJECTS_TOML).await
        }
        _ => Err(anyhow!(
            "Nexus objects are not configured for this network."
        )),
    };

    if let Ok(objects) = response {
        objects_handle.success();

        conf.nexus = Some(objects.clone());
        conf.save().await.map_err(NexusCliError::Any)?;

        return Ok(objects);
    }

    objects_handle.error();

    Err(NexusCliError::Any(anyhow!(
        "{message}\n\n{command}",
        message = "References to Nexus objects are missing in the CLI configuration. Use the following command to update it:",
        command = "$ nexus conf set --nexus.objects <PATH_TO_OBJECTS_TOML>".bold(),
    )))
}

async fn fetch_objects_from_url(url: &str) -> AnyResult<NexusObjects> {
    let response = reqwest::Client::new().get(url).send().await?;

    if !response.status().is_success() {
        bail!(
            "Failed to fetch Nexus objects from {url}: {}",
            response.status()
        );
    }

    let text = response.text().await?;
    let objects: NexusObjects = toml::from_str(&text)?;

    Ok(objects)
}

async fn configure_nexus_client_gas(
    nexus_client: &NexusClient,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> Result<(), NexusCliError> {
    let config = match sui_gas_coin {
        Some(gas_coin_id) => {
            let gas_coin = nexus_client
                .fetch_coin(gas_coin_id)
                .await
                .map_err(NexusCliError::Nexus)?;
            GasSource::coin(vec![gas_coin], sui_gas_budget)
        }
        None => GasSource::AddressBalance(AddressBalanceGas::new(sui_gas_budget)),
    };

    nexus_client
        .set_gas_source(config)
        .await
        .map_err(NexusCliError::Nexus)
}

#[derive(Clone, Copy)]
enum ClientAccess {
    ReadOnly,
    Signing,
}

async fn build_nexus_client_context(access: ClientAccess) -> Result<NexusClient, NexusCliError> {
    let mut conf = CliConf::load().await.unwrap_or_default();

    let client = build_sui_grpc_client(&conf).await?;
    let nexus_objects = get_nexus_objects(&mut conf).await?;
    let rpc_url = client.uri().to_string();

    if *nexus_objects.protocol.object_id() == sui::types::Address::ZERO {
        return Err(NexusCliError::Nexus(NexusError::Configuration(
            "a protocol root is required".into(),
        )));
    }

    let builder = NexusClient::builder().with_rpc_url(&rpc_url);
    let builder = match access {
        ClientAccess::ReadOnly => builder,
        ClientAccess::Signing => builder.with_private_key(get_signing_key(&conf).await?),
    };
    let builder = builder
        .with_protocol(nexus_objects.protocol.clone())
        .with_protocol_extras(ProtocolExtras::from(&nexus_objects));
    builder.build().await.map_err(NexusCliError::Nexus)
}

/// Creates a Nexus client without attaching a gas source.
pub(crate) async fn get_read_only_nexus_client() -> Result<NexusClient, NexusCliError> {
    build_nexus_client_context(ClientAccess::ReadOnly).await
}

/// Creates a Nexus client with owner identity but without a gas source.
pub(crate) async fn get_owner_nexus_client() -> Result<NexusClient, NexusCliError> {
    build_nexus_client_context(ClientAccess::Signing).await
}

/// Creates a Nexus client and attaches the selected transaction gas source.
pub(crate) async fn get_nexus_client(
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> Result<NexusClient, NexusCliError> {
    let nexus_client = build_nexus_client_context(ClientAccess::Signing).await?;
    configure_nexus_client_gas(&nexus_client, sui_gas_coin, sui_gas_budget).await?;

    Ok(nexus_client)
}

#[cfg(test)]
mod tests {
    use {super::*, rstest::rstest};

    struct CoinFreeClientCallSite {
        command: &'static str,
        source: &'static str,
        function_signature: &'static str,
        boundary_test_source: &'static str,
        boundary_test_signature: &'static str,
        boundary_test_marker: &'static str,
    }

    const OWNER_SCOPED_COIN_FREE_NEXUS_COMMANDS: &[CoinFreeClientCallSite] = &[
        CoinFreeClientCallSite {
            command: "nexus task list",
            source: include_str!("task/list.rs"),
            function_signature: "pub(crate) async fn run(",
            boundary_test_source: include_str!("../../sdk/src/nexus/scheduler/mod.rs"),
            boundary_test_signature:
                "async fn task_pointer_discovery_reaches_grpc_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus gas balance",
            source: include_str!("gas/balance.rs"),
            function_signature: "pub(super) async fn show(",
            boundary_test_source: include_str!("sui.rs"),
            boundary_test_signature:
                "async fn cli_client_without_explicit_gas_coin_supports_reads(",
            boundary_test_marker: "assert_coin_free_client_supports_read",
        },
        CoinFreeClientCallSite {
            command: "nexus gas deposit",
            source: include_str!("gas/balance.rs"),
            function_signature: "pub(super) async fn deposit(",
            boundary_test_source: include_str!("sui.rs"),
            boundary_test_signature: "async fn cli_transaction_setup_attaches_explicit_coin_gas(",
            boundary_test_marker: "configure_nexus_client_gas",
        },
    ];

    const READ_ONLY_NEXUS_COMMANDS: &[CoinFreeClientCallSite] = &[
        CoinFreeClientCallSite {
            command: "nexus dag inspect",
            source: include_str!("dag/dag_inspect.rs"),
            function_signature: "pub(crate) async fn inspect_dag(",
            boundary_test_source: include_str!("../../sdk/src/nexus/workflow.rs"),
            boundary_test_signature: "async fn inspect_dag_succeeds_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus task inspect",
            source: include_str!("task/state.rs"),
            function_signature: "pub(crate) async fn inspect(",
            boundary_test_source: include_str!("../../sdk/src/nexus/scheduler/task.rs"),
            boundary_test_signature: "async fn scheduler_reads_reach_rpc_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus task occurrence list",
            source: include_str!("task/occurrence.rs"),
            function_signature: "async fn list(",
            boundary_test_source: include_str!("../../sdk/src/nexus/scheduler/task.rs"),
            boundary_test_signature: "async fn scheduler_reads_reach_rpc_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus task occurrence inspect",
            source: include_str!("task/occurrence.rs"),
            function_signature: "async fn inspect(",
            boundary_test_source: include_str!("../../sdk/src/nexus/scheduler/task.rs"),
            boundary_test_signature: "async fn scheduler_reads_reach_rpc_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus task occurrence cost",
            source: include_str!("task/occurrence.rs"),
            function_signature: "async fn cost(",
            boundary_test_source: include_str!("../../sdk/src/nexus/workflow.rs"),
            boundary_test_signature: "async fn test_workflow_actions_execution_cost(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus execution inspect",
            source: include_str!("execution/inspect.rs"),
            function_signature: "pub(super) async fn run(",
            boundary_test_source: include_str!("../../sdk/src/nexus/workflow.rs"),
            boundary_test_signature: "async fn inspect_execution_replays_update_chain_in_order(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tap create-skill-artifact",
            source: include_str!("tap/tap_create_skill_artifact.rs"),
            function_signature: "async fn fetch_input_commitment(",
            boundary_test_source: include_str!("tap/tap_create_skill_artifact.rs"),
            boundary_test_signature:
                "async fn fetch_input_commitment_succeeds_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tap default-agent show",
            source: include_str!("tap/tap_default_agent.rs"),
            function_signature: "pub(crate) async fn show_default_agent(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tap.rs"),
            boundary_test_signature:
                "async fn fetch_configured_default_tap_dag_executor_succeeds_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tap payments show",
            source: include_str!("tap/tap_payments.rs"),
            function_signature: "async fn show_payment(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tap.rs"),
            boundary_test_signature:
                "async fn fetch_execution_payment_succeeds_without_owned_coins(",
            boundary_test_marker: "coin_free_payment_client",
        },
        CoinFreeClientCallSite {
            command: "nexus tap payments wait",
            source: include_str!("tap/tap_payments.rs"),
            function_signature: "async fn wait_payment(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tap.rs"),
            boundary_test_signature:
                "async fn wait_for_payment_settled_succeeds_without_owned_coins(",
            boundary_test_marker: "coin_free_payment_client",
        },
        CoinFreeClientCallSite {
            command: "nexus tap registry show",
            source: include_str!("tap/tap_registry.rs"),
            function_signature: "pub(crate) async fn show_registry(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tap.rs"),
            boundary_test_signature:
                "async fn fetch_agent_registry_still_decodes_default_executor(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tap requirements",
            source: include_str!("tap/tap_requirements.rs"),
            function_signature: "pub(crate) async fn fetch_requirements(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tap.rs"),
            boundary_test_signature:
                "async fn tap_actions_get_skill_requirements_resolves_active_skill_revision(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tap vault balance",
            source: include_str!("tap/tap_vault.rs"),
            function_signature: "pub(crate) async fn handle_vault_command(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tap.rs"),
            boundary_test_signature:
                "async fn fetch_agent_payment_vault_for_agent_succeeds_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tool auth list-keys",
            source: include_str!("tool/tool_auth.rs"),
            function_signature: "async fn list_keys(",
            boundary_test_source: include_str!("../../sdk/src/nexus/network_auth.rs"),
            boundary_test_signature: "async fn list_tool_keys_returns_sorted_entries(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tool auth export-allowed-leaders",
            source: include_str!("tool/tool_auth.rs"),
            function_signature: "async fn export_allowed_leaders(",
            boundary_test_source: include_str!("../../sdk/src/nexus/network_auth.rs"),
            boundary_test_signature: "async fn actions_export_allowlists(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tool auth sync-allowed-leaders",
            source: include_str!("tool/tool_auth.rs"),
            function_signature: "async fn sync_allowed_leaders(",
            boundary_test_source: include_str!("tool/tool_auth.rs"),
            boundary_test_signature:
                "async fn sync_once_writes_allowlist_without_wallet_or_owned_coins(",
            boundary_test_marker: "mock_network_auth_client_without_wallet",
        },
        CoinFreeClientCallSite {
            command: "nexus tool inspect",
            source: include_str!("tool/tool_inspect.rs"),
            function_signature: "pub(crate) async fn inspect_tool(",
            boundary_test_source: include_str!("../../sdk/src/nexus/tool.rs"),
            boundary_test_signature:
                "async fn inspect_tool_reports_missing_when_neither_object_exists(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
        CoinFreeClientCallSite {
            command: "nexus tool list",
            source: include_str!("tool/tool_list.rs"),
            function_signature: "pub(crate) async fn list_tools(",
            boundary_test_source: include_str!("tool/tool_list.rs"),
            boundary_test_signature: "async fn fetch_tools_succeeds_without_owned_coins(",
            boundary_test_marker: "mock_nexus_client_without_coins",
        },
    ];

    fn function_source<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("missing function signature '{signature}'"));
        let tail = &source[start + signature.len()..];
        let end = ["\nasync fn ", "\npub(crate) async fn ", "\n#[cfg(test)]"]
            .into_iter()
            .filter_map(|boundary| tail.find(boundary))
            .min()
            .unwrap_or(tail.len());

        &tail[..end]
    }

    fn count_coin_free_read_calls(path: &std::path::Path, call: &str) -> usize {
        std::fs::read_dir(path)
            .unwrap_or_else(|error| panic!("failed to read '{}': {error}", path.display()))
            .map(|entry| {
                entry
                    .expect("CLI source directory entry should be readable")
                    .path()
            })
            .map(|path| {
                if path.is_dir() {
                    count_coin_free_read_calls(&path, call)
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    std::fs::read_to_string(&path)
                        .unwrap_or_else(|error| {
                            panic!("failed to read '{}': {error}", path.display())
                        })
                        .matches(call)
                        .count()
                } else {
                    0
                }
            })
            .sum()
    }

    async fn assert_coin_free_client_supports_read(command: &str) {
        let object = nexus_sdk::test_utils::sui_mocks::mock_sui_object_ref();
        let mut ledger_service_mock =
            nexus_sdk::test_utils::sui_mocks::grpc::MockLedgerService::new();
        nexus_sdk::test_utils::sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            object.clone(),
            sui::types::Owner::Immutable,
            None,
        );
        let rpc_url = nexus_sdk::test_utils::sui_mocks::grpc::mock_server(
            nexus_sdk::test_utils::sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service_mock),
                ..Default::default()
            },
        );
        let client = NexusClient::builder()
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_sdk::test_utils::sui_mocks::mock_nexus_objects())
            .build()
            .await
            .unwrap_or_else(|error| panic!("{command} should build without owned coins: {error}"));
        let response = client
            .crawler()
            .get_object_metadata(*object.object_id())
            .await
            .unwrap_or_else(|error| panic!("{command} read should succeed: {error}"));

        assert!(client.gas_config().is_none(), "{command}");
        assert_eq!(client.get_reference_gas_price(), None, "{command}");
        assert_eq!(response.object_ref(), object, "{command}");
    }

    #[test]
    fn every_coin_free_nexus_client_call_has_boundary_proof() {
        let assert_call_sites = |call_sites: &[CoinFreeClientCallSite], client_call: &str| {
            let coin_free_call = [client_call, ".await?"].concat();
            for call_site in call_sites {
                assert!(
                    function_source(call_site.source, call_site.function_signature)
                        .contains(&coin_free_call),
                    "{} does not use its coin free Nexus client path",
                    call_site.command
                );
                assert!(
                    function_source(
                        call_site.boundary_test_source,
                        call_site.boundary_test_signature
                    )
                    .contains(call_site.boundary_test_marker),
                    "{} does not map to its named coin free execution boundary",
                    call_site.command
                );
            }
        };
        let owner_client = ["get_owner_nexus_client", "()"].concat();
        let anonymous_client = ["get_read_only_nexus_client", "()"].concat();
        assert_call_sites(OWNER_SCOPED_COIN_FREE_NEXUS_COMMANDS, &owner_client);
        assert_call_sites(READ_ONLY_NEXUS_COMMANDS, &anonymous_client);

        let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for (client_call, expected) in [
            (
                [owner_client.as_str(), ".await?"].concat(),
                OWNER_SCOPED_COIN_FREE_NEXUS_COMMANDS.len(),
            ),
            (
                [anonymous_client.as_str(), ".await?"].concat(),
                READ_ONLY_NEXUS_COMMANDS.len(),
            ),
        ] {
            assert_eq!(
                count_coin_free_read_calls(&source_root, &client_call),
                expected,
                "the coin free command inventory must include every coin free Nexus client call"
            );
        }
    }

    #[test]
    fn network_auth_sync_command_has_no_wallet_dependency() {
        let source = include_str!("tool/tool_auth.rs");
        let body = function_source(source, "async fn sync_allowed_leaders(");

        for wallet_dependency in ["get_nexus_client(", "get_signing_key(", "fetch_coin("] {
            assert!(
                !body.contains(wallet_dependency),
                "nexus tool auth sync-allowed-leaders unexpectedly uses {wallet_dependency}"
            );
        }
        assert!(
            function_source(
                source,
                "async fn sync_once_writes_allowlist_without_wallet_or_owned_coins("
            )
            .contains("mock_network_auth_client_without_wallet"),
            "sync allowed leaders must retain execution level read only proof"
        );
    }

    #[rstest]
    #[tokio::test]
    async fn test_fetch_devnet_objects() {
        use mockito::Server;
        let mut server = Server::new_async().await;

        let response_body = r#"
                protocol_version = 1
                config_hash = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
                network_id = "0x4"

                [protocol]
                object_id = "0x16"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [packages.primitives]
                initial_id = "0x1"
                storage_id = "0x1"
                version = 1

                [packages.gas]
                initial_id = "0x15"
                storage_id = "0x15"
                version = 1

                [packages.workflow]
                initial_id = "0x2"
                storage_id = "0x2"
                version = 1

                [packages.interface]
                initial_id = "0x3"
                storage_id = "0x3"
                version = 1

                [packages.scheduler]
                initial_id = "0x13"
                storage_id = "0x13"
                version = 1

                [packages.registry]
                initial_id = "0x11"
                storage_id = "0x11"
                version = 1

                [tool_registry]
                object_id = "0x5"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [network_auth]
                object_id = "0x6"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [agent_registry]
                object_id = "0x70"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [default_dag_executor]
                agent_id = "0xa1"
                skill_id = 177

                [gas_service]
                object_id = "0x8"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [leader_registry]
                object_id = "0x10"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [priority_fee_vault]
                object_id = "0x14"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [verifier_registry]
                object_id = "0x12"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"
            "#
        .to_string();

        // Create a mock for the devnet objects endpoint.
        let mock = server
            .mock("GET", "/production-talus-sui-packages/objects.devnet.toml")
            .with_status(200)
            .with_body(&response_body)
            .create_async()
            .await;

        let res = fetch_objects_from_url(
            format!(
                "http://{}/production-talus-sui-packages/objects.devnet.toml",
                server.host_with_port()
            )
            .as_str(),
        )
        .await;

        let objects = res.expect("mock object document should match NexusObjects");

        assert_eq!(objects.primitives_pkg_id(), "0x1".parse().unwrap());
        assert_eq!(objects.gas_pkg_id(), "0x15".parse().unwrap());
        assert_eq!(objects.workflow_pkg_id(), "0x2".parse().unwrap());
        assert_eq!(objects.interface_pkg_id(), "0x3".parse().unwrap());
        assert_eq!(objects.scheduler_pkg_id(), "0x13".parse().unwrap());
        assert_eq!(objects.registry_pkg_id(), "0x11".parse().unwrap());
        assert_eq!(
            objects.primitives_type_origin_pkg_id(),
            objects.primitives_pkg_id()
        );
        assert_eq!(
            objects.interface_type_origin_pkg_id(),
            objects.interface_pkg_id()
        );
        assert_eq!(
            objects.registry_type_origin_pkg_id(),
            objects.registry_pkg_id()
        );
        assert_eq!(objects.gas_type_origin_pkg_id(), objects.gas_pkg_id());
        assert_eq!(
            objects.workflow_type_origin_pkg_id(),
            objects.workflow_pkg_id()
        );
        assert_eq!(
            objects.scheduler_type_origin_pkg_id(),
            objects.scheduler_pkg_id()
        );
        assert_eq!(objects.network_id, "0x4".parse().unwrap());
        assert_eq!(
            *objects.tool_registry.object_id(),
            sui::types::Address::from_static("0x5")
        );
        assert_eq!(objects.tool_registry.version(), 1);
        assert_eq!(
            *objects.tool_registry.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );
        assert_eq!(
            *objects.network_auth.object_id(),
            sui::types::Address::from_static("0x6")
        );
        assert_eq!(objects.network_auth.version(), 1);
        assert_eq!(
            *objects.network_auth.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );
        assert_eq!(
            *objects.agent_registry.object_id(),
            sui::types::Address::from_static("0x70")
        );
        assert_eq!(
            objects.default_dag_executor.agent_id,
            sui::types::Address::from_static("0xa1")
        );
        assert_eq!(objects.default_dag_executor.skill_id, 177);
        assert_eq!(
            *objects.gas_service.object_id(),
            sui::types::Address::from_static("0x8")
        );
        assert_eq!(objects.gas_service.version(), 1);
        assert_eq!(
            *objects.gas_service.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );
        assert_eq!(
            *objects.leader_registry.object_id(),
            sui::types::Address::from_static("0x10")
        );
        assert_eq!(objects.leader_registry.version(), 1);
        assert_eq!(
            *objects.leader_registry.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );
        assert_eq!(
            *objects.priority_fee_vault.object_id(),
            sui::types::Address::from_static("0x14")
        );
        assert_eq!(objects.priority_fee_vault.version(), 1);
        assert_eq!(
            *objects.verifier_registry.object_id(),
            sui::types::Address::from_static("0x12")
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn cli_client_without_explicit_gas_coin_supports_reads() {
        assert_coin_free_client_supports_read("shared CLI client").await;
    }

    #[tokio::test]
    async fn cli_transaction_setup_attaches_address_balance_gas() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let rpc_url = nexus_sdk::test_utils::sui_mocks::grpc::mock_server(Default::default());
        let client = NexusClient::builder()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_sdk::test_utils::sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("coin-free client should build");

        configure_nexus_client_gas(&client, None, 4_321)
            .await
            .expect("address balance gas should attach without owned coins");

        assert_eq!(
            client
                .gas_config()
                .expect("transaction setup should attach gas")
                .get_budget(),
            4_321
        );
    }

    #[tokio::test]
    async fn cli_transaction_setup_attaches_explicit_coin_gas() {
        let pk = sui::crypto::Ed25519PrivateKey::generate(rand::thread_rng());
        let owner = pk.public_key().derive_address();
        let coin = nexus_sdk::test_utils::sui_mocks::mock_sui_object_ref();
        let mut coin_object = sui::grpc::Object::default();
        coin_object.set_object_id(*coin.object_id());
        coin_object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Address(owner)));
        coin_object.set_version(coin.version());
        coin_object.set_digest(*coin.digest());
        coin_object.set_balance(50_000);
        coin_object.set_object_type(sui::types::StructTag::gas_coin().to_string());
        let mut state_service_mock =
            nexus_sdk::test_utils::sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once(move |_| {
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(vec![coin_object]);
                Ok(response.into())
            });
        let mut ledger_service_mock =
            nexus_sdk::test_utils::sui_mocks::grpc::MockLedgerService::new();
        nexus_sdk::test_utils::sui_mocks::grpc::mock_reference_gas_price(
            &mut ledger_service_mock,
            789,
        );
        let rpc_url = nexus_sdk::test_utils::sui_mocks::grpc::mock_server(
            nexus_sdk::test_utils::sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger_service_mock),
                state_service_mock: Some(state_service_mock),
                ..Default::default()
            },
        );
        let client = NexusClient::builder()
            .with_private_key(pk)
            .with_rpc_url(&rpc_url)
            .with_nexus_objects(nexus_sdk::test_utils::sui_mocks::mock_nexus_objects())
            .build()
            .await
            .expect("coin-free client should build");

        configure_nexus_client_gas(&client, Some(*coin.object_id()), 5_678)
            .await
            .expect("explicit coin gas should attach");

        assert_eq!(
            client
                .gas_config()
                .expect("transaction setup should attach coin gas")
                .get_budget(),
            5_678
        );
        assert_eq!(client.get_reference_gas_price(), Some(789));
    }

    mod parse_ed25519_private_key_tests {
        use super::*;

        // Test key generated with: sui keytool generate ed25519
        // mnemonic: "nut garden prefer climb giggle armed snap sibling layer extra obvious fade"
        const TEST_KEY_BASE64_WITH_FLAG: &str = "ADvFIUMRieVEkqG05MLT8h8QVd1xZuS6xF9KA2EumjLd";
        const TEST_KEY_BASE64_WITHOUT_FLAG: &str = "O8UhQxGJ5USSobTkwtPyHxBV3XFm5LrEX0oDYS6aMt0=";
        const TEST_KEY_ADDRESS: &str =
            "0x79d85606d67f3d046098d93d51b5de4c4606743267713fa0338846ec1729dce1";

        #[test]
        fn test_33_bytes_sui_format_with_ed25519_flag() {
            // Sui format: 0x00 (ed25519 flag) + 32 byte key
            let result = parse_ed25519_private_key(TEST_KEY_BASE64_WITH_FLAG);
            assert!(result.is_ok(), "Expected Ok, got: {result:?}");

            let pk = result.unwrap();
            assert_eq!(
                pk.public_key().derive_address().to_string(),
                TEST_KEY_ADDRESS
            );
        }

        #[test]
        fn test_32_bytes_raw_ed25519_key() {
            // Raw 32-byte key without flag (leader format)
            let result = parse_ed25519_private_key(TEST_KEY_BASE64_WITHOUT_FLAG);
            assert!(result.is_ok(), "Expected Ok, got: {result:?}");

            let pk = result.unwrap();
            // Same address as above - same key, just different encoding
            assert_eq!(
                pk.public_key().derive_address().to_string(),
                TEST_KEY_ADDRESS
            );
        }

        #[test]
        fn test_33_bytes_with_unsupported_flag_fails() {
            // 0x01 is secp256k1 flag - not supported
            let mut bytes = vec![0x01]; // secp256k1 flag
            bytes.extend_from_slice(&[0u8; 32]); // dummy key
            let input = BASE64_STANDARD.encode(&bytes);

            let result = parse_ed25519_private_key(&input);
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .contains("unsupported key scheme flag 0x01"),
                "Expected unsupported flag error"
            );
        }

        #[test]
        fn test_invalid_length_fails() {
            // 31 bytes - neither 32 nor 33
            let bytes = [0u8; 31];
            let input = BASE64_STANDARD.encode(bytes);

            let result = parse_ed25519_private_key(&input);
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                err.contains("invalid private key length 31"),
                "Expected length error, got: {err}"
            );
        }
    }
}
