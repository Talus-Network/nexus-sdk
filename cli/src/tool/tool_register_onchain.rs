use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        transactions::tool,
    },
};

/// Register a new onchain tool.
/// todo: merge this function with the existing `tool_register.rs` function.
pub(crate) async fn register_onchain_tool(
    package_address: sui::ObjectID,
    module_name: String,
    input_schema: String,
    fqn: ToolFqn,
    description: String,
    witness_id: sui::ObjectID,
    no_save: bool,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Registering Onchain Tool '{fqn}' from package '{package_address}'",
        fqn = fqn,
        package_address = package_address
    );

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft a TX to register the tool.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Err(e) = tool::register_on_chain_for_self(
        &mut tx,
        objects,
        package_address,
        module_name.clone(),
        input_schema.clone(),
        &fqn,
        description.clone(),
        witness_id,
        address.into(),
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
        Err(NexusCliError::Any(e)) if e.to_string().contains("register_on_chain_tool_") => {
            notify_error!(
                "Tool '{fqn}' is already registered.",
                fqn = fqn.to_string().truecolor(100, 100, 100)
            );

            json_output(&json!({
                "tool_fqn": fqn,
                "already_registered": true,
            }))?;

            return Ok(());
        }
        // Any other error fails the tool registration.
        Err(e) => {
            notify_error!(
                "Failed to register tool '{fqn}': {error}",
                fqn = fqn.to_string().truecolor(100, 100, 100),
                error = e
            );

            return Err(e);
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

    notify_success!(
        "OwnerCap<OverTool> object ID: {id}",
        id = over_tool_id.to_string().truecolor(100, 100, 100)
    );

    // Note: Onchain tools don't create OverGas caps, so we only save the OverTool cap
    notify_success!("Onchain tools use a different gas model - no OverGas cap created.");

    // Save the owner caps to the CLI conf.
    if !no_save {
        let save_handle = loading!("Saving the owner cap to the CLI configuration...");

        let mut conf = CliConf::load().await.unwrap_or_default();

        // For onchain tools, we only have OverTool cap, use a placeholder for OverGas
        // TODO: Update ToolOwnerCaps structure to support onchain-only tools
        conf.tools.insert(
            fqn.clone(),
            ToolOwnerCaps {
                over_tool: *over_tool_id,
                over_gas: sui::ObjectID::ZERO, // Placeholder since onchain tools don't have OverGas caps
            },
        );

        if let Err(e) = conf.save().await {
            save_handle.error();

            return Err(NexusCliError::Any(e));
        }

        save_handle.success();
    }

    json_output(&json!({
        "digest": response.digest,
        "tool_fqn": fqn,
        "package_address": package_address.to_string(),
        "module_name": module_name,
        "witness_id": witness_id.to_string(),
        "description": description,
        "input_schema": input_schema,
        "owner_cap_over_tool_id": over_tool_id,
        "owner_cap_over_gas_id": null, // Onchain tools don't have OverGas caps
        "already_registered": false,
    }))?;

    Ok(())
} 