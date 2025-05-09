use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::*,
        tool::{tool_validate::*, ToolIdent},
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        transactions::tool,
    },
};

/// Validate and then register a new Tool.
pub(crate) async fn register_tool(
    ident: ToolIdent,
    collateral_coin: Option<sui::ObjectID>,
    invocation_cost: u64,
    batch: bool,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let ident_check = ident.clone();

    // Validate either a single tool or a batch of tools if the `batch` flag is
    // provided.
    let idents = if batch {
        let Some(url) = &ident.off_chain else {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>");
        };

        // Fetch all tools on the webserver.
        let response = reqwest::Client::new()
            .get(url.join("/tools").expect("Joining URL must be valid"))
            .send()
            .await
            .map_err(|e| NexusCliError::Http(e))?
            .json::<Vec<String>>()
            .await
            .map_err(|e| NexusCliError::Http(e))?;

        response
            .iter()
            .filter_map(|s| match url.join(s) {
                Ok(url) => Some(ToolIdent {
                    off_chain: Some(url),
                    on_chain: None,
                }),
                Err(_) => None,
            })
            .collect::<Vec<_>>()
    } else {
        vec![ident]
    };

    let mut registration_results = Vec::with_capacity(idents.len());

    for ident in idents {
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
            primitives_pkg_id,
            tool_registry,
            gas_service,
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
            collateral_coin,
        )
        .await?;

        if gas_coin.coin_object_id == collateral_coin.coin_object_id {
            return Err(NexusCliError::Any(anyhow!(
                "Gas and collateral coins must be different."
            )));
        }

        // Fetch reference gas price.
        let reference_gas_price = fetch_reference_gas_price(&sui).await?;

        // Craft a TX to register the tool.
        let tx_handle = loading!("Crafting transaction...");

        // Explicitly check that we're registering an off-chain tool. This is mainly
        // for when we implement logic for on-chain so that we don't forget to
        // adjust the transaction.
        if ident_check.on_chain.is_some() {
            todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>");
        }

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        match tool::register_off_chain_for_self(
            &mut tx,
            &meta,
            address.into(),
            &collateral_coin,
            invocation_cost,
            tool_registry,
            gas_service,
            *workflow_pkg_id,
            *primitives_pkg_id,
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
                } if object_type.address == **primitives_pkg_id
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

        registration_results.push(json!({
            "digest": response.digest,
            "tool_fqn": meta.fqn,
            "owner_cap_over_tool_id": over_tool_id,
            "owner_cap_over_gas_id": over_gas_id,
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
