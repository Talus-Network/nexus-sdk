use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
        tool::tool_validate::validate_off_chain_tool,
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        nexus::error::NexusError,
        transactions::tool,
    },
};

/// Validate and then register a new offchain Tool.
pub(crate) async fn register_off_chain_tool(
    url: reqwest::Url,
    collateral_coin: Option<sui::types::Address>,
    invocation_cost: u64,
    batch: bool,
    no_save: bool,
    sui_gas_coin: Option<sui::types::Address>,
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

    if collateral_coin.is_some() && collateral_coin == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "The coin used for collateral cannot be the same as the gas coin."
        )));
    }

    let conf = CliConf::load().await.unwrap_or_default();
    let client = build_sui_grpc_client(&conf).await?;
    let pk = get_signing_key(&conf).await?;
    let owner = pk.public_key().derive_address();

    for tool_url in urls {
        let meta = validate_off_chain_tool(tool_url).await?;

        command_title!(
            "Registering Tool '{fqn}' at '{url}'",
            fqn = meta.fqn,
            url = meta.url
        );

        let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
        let signer = nexus_client.signer();
        let gas_config = nexus_client.gas_config();
        let address = signer.get_active_address();
        let nexus_objects = &*nexus_client.get_nexus_objects();
        let collateral_coin = fetch_coin(client.clone(), owner, collateral_coin, 1).await?;

        // Craft a TX to register the tool.
        let tx_handle = loading!("Crafting transaction...");

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = tool::register_off_chain_for_self(
            &mut tx,
            nexus_objects,
            &meta,
            address,
            &collateral_coin,
            invocation_cost,
        ) {
            tx_handle.error();

            return Err(NexusCliError::Any(e));
        }

        tx_handle.success();

        let mut gas_coin = gas_config.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(gas_config.get_budget());
        tx.set_gas_price(nexus_client.get_reference_gas_price());

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx.finish().map_err(|e| NexusCliError::Any(e.into()))?;

        let signature = signer.sign_tx(&tx).await.map_err(NexusCliError::Nexus)?;

        // Sign and submit the TX.
        let response = match signer.execute_tx(tx, signature, &mut gas_coin).await {
            Ok(response) => {
                gas_config.release_gas_coin(gas_coin).await;

                response
            }
            // If the tool is already registered, we don't want to fail the
            // command.
            Err(NexusError::Wallet(e)) if e.to_string().contains("register_off_chain_tool_") => {
                gas_config.release_gas_coin(gas_coin).await;

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
                gas_config.release_gas_coin(gas_coin).await;

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
            .objects
            .iter()
            .filter_map(|obj| {
                let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                    return None;
                };

                if *object_type.address() == nexus_objects.primitives_pkg_id
                    && *object_type.module() == primitives::OwnerCap::CLONEABLE_OWNER_CAP.module
                    && *object_type.name() == primitives::OwnerCap::CLONEABLE_OWNER_CAP.name
                {
                    Some((obj.object_id(), object_type))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        // Find `CloneableOwnerCap<OverTool>` object ID.
        let over_tool = owner_caps.iter().find_map(|(object_id, object_type)| {
            match object_type.type_params().first() {
                Some(sui::types::TypeTag::Struct(what_for))
                    if *what_for.module() == workflow::ToolRegistry::OVER_TOOL.module
                        && *what_for.name() == workflow::ToolRegistry::OVER_TOOL.name =>
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
            match object_type.type_params().first() {
                Some(sui::types::TypeTag::Struct(what_for))
                    if *what_for.module() == workflow::Gas::OVER_GAS.module
                        && *what_for.name() == workflow::Gas::OVER_GAS.name =>
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
