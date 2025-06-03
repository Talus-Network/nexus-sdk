use {
    crate::{loading, notify_success, prelude::*},
    nexus_sdk::{object_crawler::fetch_one, sui},
    reqwest::{header, Client, StatusCode},
};

/// Build Sui client for the provided Sui net.
pub(crate) async fn build_sui_client(conf: &SuiConf) -> AnyResult<sui::Client, NexusCliError> {
    let building_handle = loading!("Building Sui client...");
    let client;

    let mut builder = sui::ClientBuilder::default();

    if let (Some(user), Some(password)) = (conf.auth_user.as_ref(), conf.auth_password.as_ref()) {
        builder = builder.basic_auth(user, password);
    }

    if let Ok(sui_rpc_url) = std::env::var("SUI_RPC_URL") {
        client = builder.build(sui_rpc_url).await
    } else {
        client = match conf.net {
            SuiNet::Localnet => builder.build_localnet().await,
            SuiNet::Devnet => builder.build_devnet().await,
            SuiNet::Testnet => builder.build_testnet().await,
            SuiNet::Mainnet => todo!("Mainnet not yet supported"),
        };
    }

    match client {
        Ok(client) => {
            building_handle.success();

            Ok(client)
        }
        Err(e) => {
            building_handle.error();

            Err(NexusCliError::Sui(e))
        }
    }
}

/// Create a wallet context from the provided path.
pub(crate) async fn create_wallet_context(
    path: &Path,
    net: SuiNet,
) -> AnyResult<sui::WalletContext, NexusCliError> {
    let wallet_handle = loading!("Initiating SUI wallet...");

    let request_timeout = None;
    let max_concurrent_requests = None;

    let wallet = match sui::WalletContext::new(path, request_timeout, max_concurrent_requests) {
        Ok(wallet) => wallet,
        Err(e) => {
            wallet_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    // Check that the Sui net matches.
    if wallet.config.active_env != get_sui_env(net).map(|env| env.alias) {
        wallet_handle.error();

        if let Some(active_env) = wallet.config.active_env.as_ref() {
            return Err(NexusCliError::Any(anyhow!(
                "{message}\n\n{command}",
                message = "The Sui net of the wallet does not match the provided Sui net. Either use a different wallet or run:",
                command = format!("$ nexus conf --sui.net {active_env}").bold(),
            )));
        }

        return Err(NexusCliError::Any(anyhow!(
            "The Sui net of the wallet is not set. Please fix the Sui client configuration."
        )));
    }

    wallet_handle.success();

    Ok(wallet)
}

/// Fetch all coins owned by the provided address.
pub(crate) async fn fetch_all_coins_for_address(
    sui: &sui::Client,
    addr: sui::Address,
) -> AnyResult<Vec<sui::Coin>, NexusCliError> {
    let coins_handle = loading!("Fetching coins...");

    let limit = None;
    let mut cursor = None;
    let mut results = Vec::new();

    // Keep fetching gas coins until there are no more pages.
    loop {
        let default_to_sui_coin_type = None;

        let response = match sui
            .coin_read_api()
            .get_coins(addr, default_to_sui_coin_type, cursor, limit)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                coins_handle.error();

                return Err(NexusCliError::Sui(e));
            }
        };

        cursor = response.next_cursor;
        results.extend(response.data);

        if !response.has_next_page {
            break;
        }
    }

    coins_handle.success();

    Ok(results)
}

/// Fetch reference gas price from Sui.
pub(crate) async fn fetch_reference_gas_price(sui: &sui::Client) -> AnyResult<u64, NexusCliError> {
    let gas_price_handle = loading!("Fetching reference gas price...");

    let response = match sui.read_api().get_reference_gas_price().await {
        Ok(response) => response,
        Err(e) => {
            gas_price_handle.error();

            return Err(NexusCliError::Sui(e));
        }
    };

    gas_price_handle.success();

    Ok(response)
}

/// Sign a transaction with the provided wallet and execute it.
///
/// Returns `Ok` with the transaction block response if successful, or `Err` if
/// the signing or the execution fails, or if the response contains errors.
pub(crate) async fn sign_and_execute_transaction(
    sui: &sui::Client,
    wallet: &sui::WalletContext,
    tx_data: sui::TransactionData,
) -> AnyResult<sui::TransactionBlockResponse, NexusCliError> {
    let signing_handle = loading!("Signing transaction...");

    let envelope = wallet.sign_transaction(&tx_data);

    let resp_options = sui::TransactionBlockResponseOptions::new()
        .with_balance_changes()
        .with_effects()
        .with_object_changes()
        .with_events();

    // We want to confirm that the tx was executed (the name of this variant is
    // misleading).
    let resp_finality = sui::ExecuteTransactionRequestType::WaitForLocalExecution;

    let response = match sui
        .quorum_driver_api()
        .execute_transaction_block(envelope, resp_options, Some(resp_finality))
        .await
    {
        Ok(response) => response,
        Err(e) => {
            signing_handle.error();

            return Err(NexusCliError::Sui(e));
        }
    };

    if !response.errors.is_empty() {
        signing_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "Transaction failed with errors: {errors:?}",
            errors = response.errors
        )));
    }

    // Check if any effects failed in the TX.
    if let Some(sui::TransactionBlockEffects::V1(effect)) = &response.effects {
        if let sui::ExecutionStatus::Failure { error } = effect.clone().into_status() {
            signing_handle.error();

            return Err(NexusCliError::Any(anyhow!(error)));
        }
    };

    signing_handle.success();

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    Ok(response)
}

/// Fetch a single object from Sui by its ID.
pub(crate) async fn fetch_object_by_id(
    sui: &sui::Client,
    object_id: sui::ObjectID,
) -> AnyResult<sui::ObjectRef, NexusCliError> {
    let object_handle = loading!("Fetching object {object_id}...");

    match fetch_one::<serde_json::Value>(sui, object_id).await {
        Ok(response) => {
            object_handle.success();

            Ok(response.object_ref())
        }
        Err(e) => {
            object_handle.error();

            Err(NexusCliError::Any(e))
        }
    }
}

/// Wrapping some conf parsing functionality used around the CLI.
// TODO: <https://github.com/Talus-Network/nexus-sdk/issues/20>
pub(crate) fn get_nexus_objects(conf: &CliConf) -> AnyResult<&NexusObjects, NexusCliError> {
    let objects_handle = loading!("Loading Nexus object IDs configuration...");

    if let Some(objects) = &conf.nexus {
        objects_handle.success();

        return Ok(objects);
    }

    objects_handle.error();

    Err(NexusCliError::Any(anyhow!(
        "{message}\n\n{command}",
        message = "References to Nexus objects are missing in the CLI configuration. Use the following command to update it:",
        command = "$ nexus conf --nexus.objects <PATH_TO_OBJECTS_TOML>".bold(),
    )))
}

/// Fetch the gas coin from the Sui client. On Localnet, Devnet and Testnet, we
/// can use the faucet to get the coin. On Mainnet, this fails if the coin is
/// not present.
pub(crate) async fn fetch_gas_coin(
    sui: &sui::Client,
    addr: sui::Address,
    sui_gas_coin: Option<sui::ObjectID>,
) -> AnyResult<sui::Coin, NexusCliError> {
    let mut coins = fetch_all_coins_for_address(sui, addr).await?;

    if coins.is_empty() {
        return Err(NexusCliError::Any(anyhow!(
            "The wallet does not have enough coins to submit the transaction"
        )));
    }

    // If object gas coing object ID was specified, use it. If it was specified
    // and could not be found, return error.
    match sui_gas_coin {
        Some(id) => {
            let coin = coins
                .into_iter()
                .find(|coin| coin.coin_object_id == id)
                .ok_or_else(|| NexusCliError::Any(anyhow!("Coin '{id}' not found in wallet")))?;

            Ok(coin)
        }
        None => Ok(coins.remove(0)),
    }
}

pub fn resolve_wallet_path(
    cli_wallet_path: Option<PathBuf>,
    conf: &SuiConf,
) -> Result<PathBuf, NexusCliError> {
    if let Some(path) = cli_wallet_path {
        Ok(path)
    } else if let Ok(mnemonic) = std::env::var("SUI_SECRET_MNEMONIC") {
        retrieve_wallet_with_mnemonic(conf.net, &mnemonic).map_err(NexusCliError::Any)
    } else {
        Ok(conf.wallet_path.clone())
    }
}

fn retrieve_wallet_with_mnemonic(net: SuiNet, mnemonic: &str) -> Result<PathBuf, anyhow::Error> {
    // Determine configuration paths.
    let config_dir = sui::config_dir()?;
    let wallet_conf_path = config_dir.join(sui::CLIENT_CONFIG);
    let keystore_path = config_dir.join(sui::KEYSTORE_FILENAME);

    // Ensure the keystore exists.
    if !keystore_path.exists() {
        let keystore = sui::FileBasedKeystore::new(&keystore_path)?;
        keystore.save()?;
    }

    // If the wallet config file does not exist, create it.
    if !wallet_conf_path.exists() {
        let keystore = sui::FileBasedKeystore::new(&keystore_path)?;
        let mut client_config = sui::ClientConfig::new(keystore.into());
        if let Some(env) = get_sui_env(net) {
            client_config.add_env(env);
        }
        if client_config.active_env.is_none() {
            client_config.active_env = client_config.envs.first().map(|env| env.alias.clone());
        }

        client_config.save(&wallet_conf_path)?;
        println!("Client config file is stored in {:?}.", &wallet_conf_path);
    }

    // Import the mnemonic into the keystore.
    let mut keystore = sui::FileBasedKeystore::new(&keystore_path)?;
    let imported_address =
        keystore.import_from_mnemonic(mnemonic, sui::SignatureScheme::ED25519, None, None)?;

    // Read the existing client configuration.
    let mut client_config: sui::ClientConfig = sui::PersistedConfig::read(&wallet_conf_path)?;

    client_config.active_address = Some(imported_address);
    client_config.save(&wallet_conf_path)?;

    Ok(wallet_conf_path)
}

fn get_sui_env(net: SuiNet) -> Option<sui::Env> {
    if let Ok(sui_rpc_url) = std::env::var("SUI_RPC_URL") {
        Some(sui::Env {
            alias: "localnet".to_string(),
            rpc: sui_rpc_url,
            ws: None,
            basic_auth: None,
        })
    } else {
        let env = match net {
            SuiNet::Localnet => sui::Env::localnet(),
            SuiNet::Devnet => sui::Env::devnet(),
            SuiNet::Testnet => sui::Env::testnet(),
            SuiNet::Mainnet => todo!("Mainnet not yet supported"),
        };
        Some(env)
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::sui::Address,
        rstest::rstest,
        serial_test::serial,
        tempfile::tempdir,
    };

    #[rstest(
        cli_wallet_path,
        mnemonic_env,
        expected,
        case(
            Some(PathBuf::from("/tmp/sui/config/client.toml")),
            None,
            PathBuf::from("/tmp/sui/config/client.toml")
        ),
        case(None, None, PathBuf::from("/tmp/sui/config/client.toml")),
        case(
            None,
            Some("include zoo tiger rural ball demand senior asthma tunnel hero ritual domain"),
            PathBuf::from("/tmp/sui/config/client.yaml")
        )
    )]
    #[serial]
    fn test_resolve_wallet_path(
        cli_wallet_path: Option<PathBuf>,
        mnemonic_env: Option<&str>,
        expected: PathBuf,
    ) {
        let sui_default_config = "/tmp/sui/config";
        // Set the default sui config folder to /tmp
        std::env::set_var("SUI_CONFIG_DIR", sui_default_config);

        // Set or remove the mnemonic environment variable as needed.
        if let Some(mnemonic) = mnemonic_env {
            std::env::set_var("SUI_SECRET_MNEMONIC", mnemonic);
        } else {
            std::env::remove_var("SUI_SECRET_MNEMONIC");
        }

        // Prepare the SuiConf instance.
        let conf = SuiConf {
            net: SuiNet::Localnet,
            wallet_path: PathBuf::from(format!("{}/client.toml", &sui_default_config)),
            auth_user: None,
            auth_password: None,
        };

        // Call the function under test.
        let resolved = resolve_wallet_path(cli_wallet_path, &conf).unwrap();
        assert_eq!(resolved, expected);

        // Clean up the env variable.
        std::env::remove_var("SUI_SECRET_MNEMONIC");
        let _ = std::fs::remove_dir_all(sui_default_config);
    }

    #[rstest(
        mnemonic,
        expected_address_str,
        case(
            "include zoo tiger rural ball demand senior asthma tunnel hero ritual domain",
            "0x479c168e5ac1319a78b09eb922a26472fbad9fc9ac904b17453eb71f4d7eb831"
        ),
        case(
            "just place income emotion clutch column pledge same pool twist finish proof",
            "0xe58c2145af0546e7be946b214e908d7e08e99e907950b428dcfe1dc9d8d8c449"
        )
    )]
    #[serial]
    fn test_active_address_set_by_mnemonic(mnemonic: &str, expected_address_str: &str) {
        // Set up a clean temporary config directory.
        let temp_dir = tempdir().unwrap();
        let sui_default_config = temp_dir.path().to_str().unwrap();
        // Set the default sui config folder to /tmp
        std::env::set_var("SUI_CONFIG_DIR", sui_default_config);
        let config_dir = sui::config_dir().expect("Failed to get config dir");
        let wallet_conf_path = config_dir.join(sui::CLIENT_CONFIG);
        let keystore_path = config_dir.join(sui::KEYSTORE_FILENAME);

        // Clean up any existing files.
        let _ = std::fs::remove_file(&wallet_conf_path);
        let _ = std::fs::remove_file(&keystore_path);

        // Call the function under test.
        let _ = retrieve_wallet_with_mnemonic(SuiNet::Localnet, mnemonic)
            .expect("retrieve_wallet_with_mnemonic failed");

        let client_config: sui::ClientConfig =
            sui::PersistedConfig::read(&wallet_conf_path).expect("Failed to read client config");
        let expected_address: Address = expected_address_str.parse().expect("Invalid address");
        assert_eq!(client_config.active_address.unwrap(), expected_address);

        let _ = std::fs::remove_dir_all(&config_dir);
    }

    #[rstest(
        preexisting_mnemonic,
        new_mnemonic,
        expected_active_address_str,
        case(
            "include zoo tiger rural ball demand senior asthma tunnel hero ritual domain",
            "just place income emotion clutch column pledge same pool twist finish proof",
            "0xe58c2145af0546e7be946b214e908d7e08e99e907950b428dcfe1dc9d8d8c449"
        )
    )]
    #[serial]
    fn test_active_address_with_preexisting_keystore(
        preexisting_mnemonic: &str,
        new_mnemonic: &str,
        expected_active_address_str: &str,
    ) {
        // Create a temporary config directory.
        let temp_dir = tempdir().unwrap();
        let config_dir: PathBuf = temp_dir.path().to_path_buf();
        std::env::set_var("SUI_CONFIG_DIR", config_dir.to_str().unwrap());

        let wallet_conf_path = config_dir.join(sui::CLIENT_CONFIG);
        let keystore_path = config_dir.join(sui::KEYSTORE_FILENAME);

        // Create a preexisting keystore file with an active address derived from preexisting_mnemonic.
        {
            // FileBasedKeystore is assumed to be your real implementation.
            let mut preexisting_keystore =
                sui::FileBasedKeystore::new(&keystore_path).expect("Failed to create keystore");
            preexisting_keystore
                .import_from_mnemonic(
                    preexisting_mnemonic,
                    sui::SignatureScheme::ED25519,
                    None,
                    None,
                )
                .expect("Failed to import preexisting mnemonic");
            preexisting_keystore
                .save()
                .expect("Failed to save keystore");
        }

        // Create a default client configuration if it doesn't exist.
        if !wallet_conf_path.exists() {
            let keystore =
                sui::FileBasedKeystore::new(&keystore_path).expect("Failed to create keystore");
            let client_config = sui::ClientConfig::new(keystore.into());
            client_config
                .save(&wallet_conf_path)
                .expect("Failed to save client config");
        }

        // Call retrieve_wallet_with_mnemonic with the new mnemonic.
        // This should import the new mnemonic into the preexisting keystore so that its derived
        // address becomes the first (active) address.
        let _ = retrieve_wallet_with_mnemonic(SuiNet::Localnet, new_mnemonic)
            .expect("retrieve_wallet_with_mnemonic failed");

        // Read the updated client configuration.
        let updated_config: sui::ClientConfig =
            sui::PersistedConfig::read(&wallet_conf_path).expect("Failed to read client config");

        // Convert the expected address string into a SuiAddress.
        let expected_active_address: Address = expected_active_address_str
            .parse()
            .expect("Invalid SuiAddress string");

        // The active address in the config should match the one derived from the new mnemonic.
        assert_eq!(
            updated_config.active_address.unwrap(),
            expected_active_address
        );

        // Clean up temporary files.
        let _ = std::fs::remove_dir_all(&config_dir);
    }
}
