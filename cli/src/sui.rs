use {
    crate::{loading, prelude::*},
    base64::{prelude::BASE64_STANDARD, Engine},
    nexus_sdk::{nexus::client::NexusClient, sui},
};

/// Build Sui client for the provided Sui net.
pub(crate) async fn build_sui_grpc_client(
    conf: &CliConf,
) -> AnyResult<Arc<Mutex<sui::grpc::Client>>, NexusCliError> {
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
            command = "$ nexus conf --sui.rpc-url <url>".to_string().bold(),
        )));
    };

    match sui::grpc::Client::new(url) {
        Ok(client) => {
            client_handle.success();

            Ok(Arc::new(Mutex::new(client)))
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
    let Some(pk_encoded) = std::env::var("SUI_PK").ok().or_else(|| conf.sui.pk.clone()) else {
        key_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "{message}\n\n{command}",
            message = "The Sui private key is not configured. Please set it via environment or the CLI configuration.",
            command = "$ nexus conf --sui.pk <base64_encoded_key>".to_string().bold(),
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

/// Fetch all coins owned by the provided address.
pub(crate) async fn fetch_coins_for_address(
    client: Arc<Mutex<sui::grpc::Client>>,
    owner: sui::types::Address,
) -> AnyResult<Vec<sui::types::ObjectReference>, NexusCliError> {
    let coins_handle = loading!("Fetching coins...");

    let request = sui::grpc::ListOwnedObjectsRequest::default()
        .with_owner(owner)
        .with_page_size(1000)
        .with_object_type(sui::types::StructTag::gas_coin())
        .with_read_mask(sui::grpc::FieldMask::from_paths([
            "object_id",
            "version",
            "digest",
        ]));

    let mut client = client.lock().await;

    let response = match client
        .state_client()
        .list_owned_objects(request)
        .await
        .map(|resp| resp.into_inner())
    {
        Ok(response) => response,
        Err(e) => {
            coins_handle.error();

            return Err(NexusCliError::Rpc(e.into()));
        }
    };

    drop(client);

    coins_handle.success();

    Ok(response
        .objects()
        .iter()
        .filter_map(|object| {
            Some(sui::types::ObjectReference::new(
                object.object_id_opt()?.parse().ok()?,
                object.version_opt()?,
                object.digest_opt()?.parse().ok()?,
            ))
        })
        .collect())
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

/// Fetch the gas coin from the Sui client. On Localnet, Devnet and Testnet, we
/// can use the faucet to get the coin. On Mainnet, this fails if the coin is
/// not present.
pub(crate) async fn fetch_coin(
    client: Arc<Mutex<sui::grpc::Client>>,
    owner: sui::types::Address,
    specific: Option<sui::types::Address>,
    nth: usize,
) -> AnyResult<sui::types::ObjectReference, NexusCliError> {
    let mut coins = fetch_coins_for_address(client, owner).await?;

    if coins.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "The wallet does not have enough coins to submit the transaction"
        )));
    }

    // If object gas coing object ID was specified, use it. If it was specified
    // and could not be found, return error.
    match specific {
        Some(id) => {
            let coin = coins
                .into_iter()
                .find(|coin| *coin.object_id() == id)
                .ok_or_else(|| NexusCliError::Any(anyhow!("Coin '{id}' not found in wallet")))?;

            Ok(coin)
        }
        None => {
            if nth >= coins.len() {
                return Err(NexusCliError::Any(anyhow!(
                    "The wallet does not have enough coins to select coin #{nth}"
                )));
            }

            Ok(coins.swap_remove(nth))
        }
    }
}

/// Create a Nexus client from CLI parameters.
pub(crate) async fn get_nexus_client(
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> Result<NexusClient, NexusCliError> {
    let mut conf = CliConf::load().await.unwrap_or_default();

    let client = build_sui_grpc_client(&conf).await?;
    let pk = get_signing_key(&conf).await?;
    let owner = pk.public_key().derive_address();
    let gas_coin = fetch_coin(client.clone(), owner, sui_gas_coin, 0).await?;
    let nexus_objects = get_nexus_objects(&mut conf).await?;
    let rpc_url = client.lock().await.uri().to_string();

    // Try to get the `SUI_GQL_URL` from the environment, otherwise use
    // the configuration.
    let Some(gql_url) = std::env::var("SUI_GQL_URL")
        .ok()
        .or_else(|| conf.sui.gql_url.as_ref().map(|u| u.to_string()))
    else {
        return Err(NexusCliError::Any(anyhow!(
            "{message}\n\n{command}",
            message =
                "The Sui GraphQL URL is not configured. Please set it via environment or the CLI configuration.",
            command = "$ nexus conf --sui.gql-url <url>".to_string().bold(),
        )));
    };

    // Create Nexus client.
    let nexus_client = NexusClient::builder()
        .with_private_key(pk)
        .with_nexus_objects(nexus_objects.clone())
        .with_gas(vec![gas_coin], sui_gas_budget)
        .with_rpc_url(&rpc_url)
        .with_gql_url(&gql_url)
        .build()
        .await
        .map_err(NexusCliError::Nexus)?;

    Ok(nexus_client)
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server, rstest::rstest};

    #[rstest]
    #[tokio::test]
    async fn test_fetch_devnet_objects() {
        let mut server = Server::new_async().await;

        let response_body = r#"
                primitives_pkg_id = "0x1"
                workflow_pkg_id = "0x2"
                interface_pkg_id = "0x3"
                network_id = "0x4"

                [tool_registry]
                object_id = "0x5"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [default_tap]
                object_id = "0x6"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [gas_service]
                object_id = "0x7"
                version = 1
                digest = "3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv"

                [pre_key_vault]
                object_id = "0x8"
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

        assert!(res.is_ok());

        let objects = res.unwrap();

        assert_eq!(objects.primitives_pkg_id, "0x1".parse().unwrap());
        assert_eq!(objects.workflow_pkg_id, "0x2".parse().unwrap());
        assert_eq!(objects.interface_pkg_id, "0x3".parse().unwrap());
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
            *objects.default_tap.object_id(),
            sui::types::Address::from_static("0x6")
        );
        assert_eq!(objects.default_tap.version(), 1);
        assert_eq!(
            *objects.default_tap.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );
        assert_eq!(
            *objects.gas_service.object_id(),
            sui::types::Address::from_static("0x7")
        );
        assert_eq!(objects.gas_service.version(), 1);
        assert_eq!(
            *objects.gas_service.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );
        assert_eq!(
            *objects.pre_key_vault.object_id(),
            sui::types::Address::from_static("0x8")
        );
        assert_eq!(objects.pre_key_vault.version(), 1);
        assert_eq!(
            *objects.pre_key_vault.digest(),
            sui::types::Digest::from_static("3LFAfxPb6Q81U8wXg6qc6UyV9Hoj1VdfFfMwvGTEq5Bv")
        );

        mock.assert_async().await;
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
