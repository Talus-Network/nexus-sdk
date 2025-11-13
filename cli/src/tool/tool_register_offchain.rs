use {
    crate::{
        command_title, display::json_output, loading, notify_error, notify_success, prelude::*,
        sui::*, tool::tool_validate::validate_off_chain_tool,
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        transactions::tool,
    },
};

/// Validate and then register a new offchain Tool.
pub(crate) async fn register_off_chain_tool(
    url: reqwest::Url,
    collateral_coin: Option<sui::ObjectID>,
    invocation_cost: u64,
    batch: bool,
    no_save: bool,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    // Validate either a single tool or a batch of tools if the `batch` flag is
    // provided.
    let urls = if batch {
        // Fetch all tools on the webserver.
        let response = reqwest::Client::new()
            .get(url.join("/tools").expect("Joining URL must be valid"))
            .send()
            .await
            .map_err(NexusCliError::Http)?
            .json::<Vec<String>>()
            .await
            .map_err(NexusCliError::Http)?;

        response
            .iter()
            .filter_map(|s| url.join(s).ok())
            .collect::<Vec<_>>()
    } else {
        vec![url]
    };

    let mut registration_results = Vec::with_capacity(urls.len());

    for tool_url in urls {
        let meta = validate_off_chain_tool(tool_url).await?;

        command_title!(
            "Registering Tool '{fqn}' at '{url}'",
            fqn = meta.fqn,
            url = meta.url
        );

        // Load CLI configuration.
        let mut conf = CliConf::load().await.unwrap_or_default();

        // Nexus objects must be present in the configuration.
        let objects = &get_nexus_objects(&mut conf).await?;

        // Create wallet context, Sui client and find the active address.
        let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
        let sui = build_sui_client(&conf.sui).await?;
        let address = wallet.active_address().map_err(NexusCliError::Any)?;

        // Fetch gas and collateral coin objects.
        let (gas_coin, collateral_coin) =
            fetch_gas_and_collateral_coins(&sui, address, sui_gas_coin, collateral_coin).await?;

        if gas_coin.coin_object_id == collateral_coin.coin_object_id {
            return Err(NexusCliError::Any(anyhow!(
                "Gas and collateral coins must be different."
            )));
        }

        // Fetch reference gas price.
        let reference_gas_price = fetch_reference_gas_price(&sui).await?;

        // Craft a TX to register the tool.
        let tx_handle = loading!("Crafting transaction...");

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        if let Err(e) = tool::register_off_chain_for_self(
            &mut tx,
            objects,
            &meta,
            address.into(),
            &collateral_coin,
            invocation_cost,
        ) {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }

        tx_handle.success();

        let tx_data = sui::TransactionData::new_programmable(
            address,
            vec![gas_coin.object_ref()],
            tx.finish(),
            sui_gas_budget,
            reference_gas_price,
        );

        // Sign and submit the TX.
        let response = match sign_and_execute_transaction(&sui, &wallet, tx_data).await {
            Ok(response) => response,
            // If the tool is already registered, we don't want to fail the
            // command.
            Err(NexusCliError::Any(e)) if e.to_string().contains("register_off_chain_tool_") => {
                notify_error!(
                    "Tool '{fqn}' is already registered.",
                    fqn = meta.fqn.to_string().truecolor(100, 100, 100)
                );

                registration_results.push(json!({
                    "tool_fqn": meta.fqn,
                    "already_registered": true,
                }));

                continue;
            }
            // Any other error fails the tool registration but continues the
            // loop.
            Err(e) => {
                notify_error!(
                    "Failed to register tool '{fqn}': {error}",
                    fqn = meta.fqn.to_string().truecolor(100, 100, 100),
                    error = e
                );

                registration_results.push(json!({
                    "tool_fqn": meta.fqn,
                    "error": e.to_string(),
                }));

                continue;
            }
        };

        // Parse the owner cap object IDs from the response.
        let owner_caps = response
            .object_changes
            .unwrap_or_default()
            .into_iter()
            .filter_map(|change| match change {
                sui::ObjectChange::Created {
                    object_type,
                    object_id,
                    ..
                } if object_type.address == *objects.primitives_pkg_id
                    && object_type.module
                        == primitives::OwnerCap::CLONEABLE_OWNER_CAP.module.into()
                    && object_type.name
                        == primitives::OwnerCap::CLONEABLE_OWNER_CAP.name.into() =>
                {
                    Some((object_id, object_type))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        // Find `CloneableOwnerCap<OverTool>` object ID.
        let over_tool = owner_caps.iter().find_map(|(object_id, object_type)| {
            match object_type.type_params.first() {
                Some(sui::MoveTypeTag::Struct(what_for))
                    if what_for.module == workflow::ToolRegistry::OVER_TOOL.module.into()
                        && what_for.name == workflow::ToolRegistry::OVER_TOOL.name.into() =>
                {
                    Some(object_id)
                }
                _ => None,
            }
        });

        let Some(over_tool_id) = over_tool else {
            return Err(NexusCliError::Any(anyhow!(
                "Could not find the OwnerCap<OverTool> object ID in the transaction response."
            )));
        };

        // Find `CloneableOwnerCap<OverGas>` object ID.
        let over_gas = owner_caps.iter().find_map(|(object_id, object_type)| {
            match object_type.type_params.first() {
                Some(sui::MoveTypeTag::Struct(what_for))
                    if what_for.module == workflow::Gas::OVER_GAS.module.into()
                        && what_for.name == workflow::Gas::OVER_GAS.name.into() =>
                {
                    Some(object_id)
                }
                _ => None,
            }
        });

        let Some(over_gas_id) = over_gas else {
            return Err(NexusCliError::Any(anyhow!(
                "Could not find the OwnerCap<OverGas> object ID in the transaction response."
            )));
        };

        notify_success!(
            "OwnerCap<OverTool> object ID: {id}",
            id = over_tool_id.to_string().truecolor(100, 100, 100)
        );

        notify_success!(
            "OwnerCap<OverGas> object ID: {id}",
            id = over_gas_id.to_string().truecolor(100, 100, 100)
        );

        // Save the owner caps to the CLI conf.
        if !no_save {
            let save_handle = loading!("Saving the owner caps to the CLI configuration...");

            let mut conf = CliConf::load().await.unwrap_or_default();

            conf.tools.insert(
                meta.fqn.clone(),
                ToolOwnerCaps {
                    over_tool: *over_tool_id,
                    over_gas: Some(*over_gas_id),
                },
            );

            if let Err(e) = conf.save().await {
                save_handle.error();

                return Err(NexusCliError::Any(e));
            }

            save_handle.success();
        }

        registration_results.push(json!({
            "digest": response.digest,
            "tool_fqn": meta.fqn,
            "owner_cap_over_tool_id": over_tool_id,
            "owner_cap_over_gas_id": over_gas_id,
            "already_registered": false,
        }))
    }

    json_output(&registration_results)?;

    Ok(())
}

/// Fetch the gas and collateral coins from the Sui client. On Localnet, Devnet
/// and Testnet, we can use the faucet to get the coins. On Mainnet, this fails
/// if the coins are not present.
async fn fetch_gas_and_collateral_coins(
    sui: &sui::Client,
    addr: sui::Address,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_collateral_coin: Option<sui::ObjectID>,
) -> AnyResult<(sui::Coin, sui::Coin), NexusCliError> {
    let mut coins = fetch_all_coins_for_address(sui, addr).await?;

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
