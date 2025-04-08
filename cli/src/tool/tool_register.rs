use {
    crate::{
        command_title,
        display::json_output,
        loading,
        prelude::*,
        sui::*,
        tool::{tool_validate::*, ToolIdent},
    },
    nexus_sdk::transactions::tool,
};

/// Validate and then register a new Tool.
pub(crate) async fn register_tool(
    ident: ToolIdent,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_collateral_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let ident_check = ident.clone();

    let meta = validate_tool(ident).await?;

    command_title!(
        "Registering Tool '{fqn}' at '{url}'",
        fqn = meta.fqn,
        url = meta.url
    );

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        workflow_pkg_id,
        tool_registry_object_id,
        ..
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::Any(e));
        }
    };

    // Fetch gas and collateral coin objects.
    let (gas_coin, collateral_coin) = fetch_gas_and_collateral_coins(
        &sui,
        conf.sui.net,
        address,
        sui_gas_coin,
        sui_collateral_coin,
    )
    .await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Fetch the tool registry object.
    let tool_registry = fetch_object_by_id(&sui, tool_registry_object_id).await?;

    // Craft a TX to register the tool.
    let tx_handle = loading!("Crafting transaction...");

    // Explicilty check that we're registering an off-chain tool. This is mainly
    // for when we implement logic for on-chain so that we don't forget to
    // adjust the transaction.
    if ident_check.on_chain.is_some() {
        todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>");
    }

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    match tool::register_off_chain(
        &mut tx,
        &meta,
        collateral_coin,
        tool_registry,
        workflow_pkg_id,
    ) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    // Sign and submit the TX.
    let response = sign_transaction(&sui, &wallet, tx_data).await?;

    json_output(&json!({ "digest": response.digest }))?;

    Ok(())
}

/// Fetch the gas and collateral coins from the Sui client. On Localnet, Devnet
/// and Testnet, we can use the faucet to get the coins. On Mainnet, this fails
/// if the coins are not present.
async fn fetch_gas_and_collateral_coins(
    sui: &sui::Client,
    sui_net: SuiNet,
    addr: sui::Address,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_collateral_coin: Option<sui::ObjectID>,
) -> AnyResult<(sui::Coin, sui::Coin), NexusCliError> {
    let mut coins = fetch_all_coins_for_address(sui, addr).await?;

    // We need at least 2 coins. We can create those on Localnet, Devnet and
    // Testnet.
    match sui_net {
        SuiNet::Localnet | SuiNet::Devnet | SuiNet::Testnet if coins.len() < 2 => {
            // Only call once because on Localnet and Devnet, we get 5 coins and
            // on Testnet this will be rate-limited.
            request_tokens_from_faucet(sui_net, addr).await?;

            coins = fetch_all_coins_for_address(sui, addr).await?;
        }
        SuiNet::Mainnet if coins.len() < 2 => {
            return Err(NexusCliError::Any(anyhow!(
                "The wallet does not have enough coins to register the tool"
            )));
        }
        _ => (),
    }

    if coins.len() < 2 {
        return Err(NexusCliError::Any(anyhow!(
            "The wallet does not have enough coins to register the tool"
        )));
    }

    // If object IDs were specified, use them. If any of the specified coins is
    // not found, return error.
    let gas_coin = match sui_gas_coin {
        Some(id) => coins
            .iter()
            .find(|coin| coin.coin_object_id == id)
            .cloned()
            .ok_or_else(|| NexusCliError::Any(anyhow!("Coin '{id}' not found in wallet")))?,
        None => coins.remove(0),
    };

    let collateral_coin = match sui_collateral_coin {
        Some(id) => coins
            .iter()
            .find(|coin| coin.coin_object_id == id)
            .cloned()
            .ok_or_else(|| NexusCliError::Any(anyhow!("Coin '{id}' not found in wallet")))?,
        None => coins.remove(0),
    };

    Ok((gas_coin, collateral_coin))
}
