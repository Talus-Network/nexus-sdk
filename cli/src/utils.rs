use {
    crate::{loading, prelude::*},
    reqwest::{header, Client, StatusCode},
    std::path::PathBuf,
    sui_sdk::rpc_types::SuiTransactionBlockEffectsAPI,
};

/// Build Sui client for the provided Sui net.
pub(crate) async fn build_sui_client(net: SuiNet) -> AnyResult<sui::Client, NexusCliError> {
    let building_handle = loading!("Building Sui client...");

    let builder = sui::ClientBuilder::default();

    let client = match net {
        SuiNet::Localnet => builder.build_localnet().await,
        SuiNet::Testnet => builder.build_testnet().await,
        SuiNet::Mainnet => todo!("Mainnet not yet supported"),
    };

    match client {
        Ok(client) => {
            building_handle.success();

            Ok(client)
        }
        Err(e) => {
            building_handle.error();

            Err(NexusCliError::SuiError(e))
        }
    }
}

/// Create a wallet context from the provided path.
pub(crate) async fn create_wallet_context(
    path: &PathBuf,
    net: SuiNet,
) -> AnyResult<sui::WalletContext, NexusCliError> {
    let wallet_handle = loading!("Initiating SUI wallet...");

    let request_timeout = None;
    let max_concurrent_requests = None;

    let wallet = match sui::WalletContext::new(path, request_timeout, max_concurrent_requests) {
        Ok(wallet) => wallet,
        Err(e) => {
            wallet_handle.error();

            return Err(NexusCliError::AnyError(e));
        }
    };

    // Check that the Sui net matches.
    if wallet.config.active_env != Some(net.to_string()) {
        wallet_handle.error();

        if let Some(active_env) = wallet.config.active_env.as_ref() {
            return Err(NexusCliError::AnyError(anyhow!(
                "{message}\n\n{command}",
                message = "The Sui net of the wallet does not match the provided Sui net. Either use a different wallet or run:",
                command = format!("$ nexus conf --sui.net {active_env}").bold(),
            )));
        }

        return Err(NexusCliError::AnyError(anyhow!(
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

                return Err(NexusCliError::SuiError(e));
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

/// Request tokens from the Faucet for the given address.
///
/// Inspired by:
/// <https://github.com/MystenLabs/sui/blob/aa99382c9191cd592cd65d0e197c33c49e4d9c4f/crates/sui/src/client_commands.rs#L2541>
pub async fn request_tokens_from_faucet(
    sui_net: SuiNet,
    addr: sui::Address,
) -> AnyResult<(), NexusCliError> {
    let faucet_handle = loading!("Requesting tokens from faucet...");

    let url = match sui_net {
        SuiNet::Testnet => "https://faucet.testnet.sui.io/v1/gas",
        SuiNet::Localnet => "http://127.0.0.1:9123/gas",
        _ => {
            faucet_handle.error();

            return Err(NexusCliError::AnyError(anyhow!(
                "Mainnet faucet not supported"
            )));
        }
    };

    #[derive(Deserialize)]
    struct FaucetResponse {
        error: Option<String>,
    }

    let json_body = serde_json::json![{
        "FixedAmountRequest": {
            "recipient": &addr.to_string()
        }
    }];

    // Make the request to the faucet JSON RPC API for coin.
    let resp = match Client::new()
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, "nexus-leader")
        .json(&json_body)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            faucet_handle.error();

            return Err(NexusCliError::AnyError(anyhow!(e)));
        }
    };

    let result = match resp.status() {
        StatusCode::ACCEPTED | StatusCode::CREATED => {
            let faucet_resp = resp.json::<FaucetResponse>().await;

            if let Err(e) = faucet_resp {
                Err(anyhow!(e))
            } else {
                match faucet_resp.unwrap().error {
                    Some(e) => Err(anyhow!(e)),
                    None => Ok(()),
                }
            }
        }
        StatusCode::TOO_MANY_REQUESTS => {
            Err(anyhow!(
                "Faucet service received too many requests from this IP address. Please try again after 60 minutes."
            ))
        }
        StatusCode::SERVICE_UNAVAILABLE => {
            Err(anyhow!(
                "Faucet service is currently overloaded or unavailable. Please try again later."
            ))
        }
        status_code => {
            Err(anyhow!("Faucet request was unsuccessful: {status_code}"))
        }
    };

    match result {
        Ok(()) => {
            faucet_handle.success();

            Ok(())
        }
        Err(e) => {
            faucet_handle.error();

            Err(NexusCliError::AnyError(anyhow!(e)))
        }
    }
}

/// Fetch reference gas price from Sui.
pub(crate) async fn fetch_reference_gas_price(sui: &sui::Client) -> AnyResult<u64, NexusCliError> {
    let gas_price_handle = loading!("Fetching reference gas price...");

    let response = match sui.read_api().get_reference_gas_price().await {
        Ok(response) => response,
        Err(e) => {
            gas_price_handle.error();

            return Err(NexusCliError::SuiError(e));
        }
    };

    gas_price_handle.success();

    Ok(response)
}

/// Sign a transaction with the provided wallet.
pub(crate) async fn sign_transaction(
    sui: &sui::Client,
    wallet: &sui::WalletContext,
    tx_data: sui::TransactionData,
) -> AnyResult<(), NexusCliError> {
    let signing_handle = loading!("Signing transaction...");

    let envelope = wallet.sign_transaction(&tx_data);

    let resp_options = sui::TransactionBlockResponseOptions::new()
        .with_balance_changes()
        .with_effects();

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

            return Err(NexusCliError::SuiError(e));
        }
    };

    if response.errors.len() > 0 {
        signing_handle.error();

        return Err(NexusCliError::AnyError(anyhow!(
            "Transaction failed with errors: {errors:?}",
            errors = response.errors
        )));
    }

    // Check if any effects failed in the TX.
    if let Some(sui::TransactionBlockEffects::V1(effect)) = response.effects {
        if let sui::ExecutionStatus::Failure { error } = effect.into_status() {
            signing_handle.error();

            return Err(NexusCliError::AnyError(anyhow!(error)));
        }
    }

    signing_handle.success();

    println!(
        "[{check}] Transaction digest: {digest}",
        check = "✔".green().bold(),
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    Ok(())
}

/// Fetch a single object from Sui by its ID.
pub(crate) async fn fetch_object_by_id(
    sui: &sui::Client,
    object_id: sui::ObjectID,
) -> AnyResult<sui::ObjectRef, NexusCliError> {
    let object_handle = loading!("Fetching object {object_id}...");

    let options = sui::ObjectDataOptions::new().with_owner();

    let response = match sui
        .read_api()
        .get_object_with_options(object_id, options)
        .await
    {
        Ok(response) => response,
        Err(e) => {
            object_handle.error();

            return Err(NexusCliError::SuiError(e));
        }
    };

    if let Some(e) = response.error {
        object_handle.error();

        return Err(NexusCliError::AnyError(anyhow!(e)));
    }

    let object = match response.data {
        Some(object) => object,
        None => {
            object_handle.error();

            return Err(NexusCliError::AnyError(anyhow!(
                "The object with ID {object_id} was not found"
            )));
        }
    };

    // Find initial shared version for shared objects or fallback to the
    // object's version.
    let version = object
        .owner
        .and_then(|owner| match owner {
            sui::Owner::Shared {
                initial_shared_version,
            } => Some(initial_shared_version),
            _ => None,
        })
        .unwrap_or(object.version);

    object_handle.success();

    Ok((object.object_id, version, object.digest).into())
}
