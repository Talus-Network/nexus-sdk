use {
    super::ToolMeta,
    crate::{
        command_title,
        loading,
        prelude::*,
        tool::{tool_validate::*, ToolIdent},
        utils::*,
    },
    move_core_types::ident_str,
};

/// Sui `std::ascii::string`
const SUI_ASCII_MODULE: &sui::MoveIdentStr = ident_str!("ascii");
const SUI_ASCII_FROM_STRING: &sui::MoveIdentStr = ident_str!("string");

// Nexus `tool_registry::register_off_chain_tool`
const NEXUS_TOOL_REGISTRY_MODULE: &sui::MoveIdentStr = ident_str!("tool_registry");
const NEXUS_REGISTER_OFF_CHAIN_TOOL: &sui::MoveIdentStr = ident_str!("register_off_chain_tool");

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
    let config_handle = loading!("Loading CLI configuration...");

    let conf = match CliConf::load().await {
        Ok(conf) => conf,
        Err(e) => {
            config_handle.error();

            return Err(NexusCliError::AnyError(e));
        }
    };

    // Workflow package and tool registry IDs must be present.
    //
    // TODO: <https://github.com/Talus-Network/nexus-sdk/issues/20>
    let (workflow_pkg_id, tool_registry_object_id) = match (
        conf.nexus.workflow_id,
        conf.nexus.tool_registry_id,
    ) {
        (Some(wid), Some(trid)) => (wid, trid),
        _ => {
            config_handle.error();

            return Err(NexusCliError::AnyError(anyhow!(
                "{message}\n\n{workflow_command}\n{tool_registry_command}",
                message = "The Nexus Workflow package ID and Tool Registry object ID must be set. Use the following commands to update the configuration:",
                workflow_command = "$ nexus conf --nexus.workflow-id <ID>".bold(),
                tool_registry_command = "$ nexus conf --nexus.tool-registry-id <ID>".bold()
            )));
        }
    };

    config_handle.success();

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(conf.sui.net).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::AnyError(e));
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
    // adjust `prepare_transaction`.
    if ident_check.on_chain.is_some() {
        todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>");
    }

    let tx = match prepare_transaction(meta, collateral_coin, tool_registry, workflow_pkg_id) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();

            return Err(NexusCliError::AnyError(e));
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
    sign_transaction(&sui, &wallet, tx_data).await
}

/// Fetch the gas and collateral coins from the Sui client. On Localnet and
/// Testnet, we can use the faucet to get the coins. On Mainnet, this fails if
/// the coins are not present.
async fn fetch_gas_and_collateral_coins(
    sui: &sui::Client,
    sui_net: SuiNet,
    addr: sui::Address,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_collateral_coin: Option<sui::ObjectID>,
) -> AnyResult<(sui::Coin, sui::Coin), NexusCliError> {
    let mut coins = fetch_all_coins_for_address(sui, addr).await?;

    // We need at least 2 coins. We can create those on localnet and testnet.
    match sui_net {
        SuiNet::Localnet | SuiNet::Testnet if coins.len() < 2 => {
            for _ in coins.len()..2 {
                request_tokens_from_faucet(sui_net, addr).await?;
            }

            coins = fetch_all_coins_for_address(sui, addr).await?;
        }
        SuiNet::Mainnet if coins.len() < 2 => {
            return Err(NexusCliError::AnyError(anyhow!(
                "The wallet does not have enough coins to register the tool"
            )));
        }
        _ => (),
    }

    if coins.len() < 2 {
        return Err(NexusCliError::AnyError(anyhow!(
            "The wallet does not have enough coins to register the tool"
        )));
    }

    // If object IDs were specified, use them.
    let gas_coin = sui_gas_coin
        .and_then(|id| coins.iter().find(|coin| coin.coin_object_id == id))
        .cloned()
        .unwrap_or_else(|| coins.remove(0));

    let collateral_coin = sui_collateral_coin
        .and_then(|id| coins.iter().find(|coin| coin.coin_object_id == id))
        .cloned()
        .unwrap_or_else(|| coins.remove(0));

    Ok((gas_coin, collateral_coin))
}

/// Build a programmable transaction to register a new off-chain tool.
fn prepare_transaction(
    meta: ToolMeta,
    collateral_coin: sui::Coin,
    tool_registry: sui::ObjectRef,
    workflow_pkg_id: sui::ObjectID,
) -> AnyResult<sui::ProgrammableTransactionBuilder> {
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // `self: &mut ToolRegistry`
    let tool_registry = tx.obj(sui::ObjectArg::SharedObject {
        id: tool_registry.object_id,
        initial_shared_version: tool_registry.version,
        mutable: true,
    })?;

    // `fqn: AsciiString`
    let fqn = tx.pure(meta.fqn.as_bytes())?;

    let fqn = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        SUI_ASCII_MODULE.into(),
        SUI_ASCII_FROM_STRING.into(),
        vec![],
        vec![fqn],
    );

    // `url: vector<u8>`
    let url = tx.pure(meta.url.as_bytes())?;

    // `input_schema: vector<u8>`
    let input_schema = tx.pure(meta.input_schema.to_string().as_bytes())?;

    // `output_schema: vector<u8>`
    let output_schema = tx.pure(meta.output_schema.to_string().as_bytes())?;

    // `pay_with: Coin<SUI>`
    let pay_with = tx.obj(sui::ObjectArg::ImmOrOwnedObject(
        collateral_coin.object_ref(),
    ))?;

    // `nexus::tool_registry::register_off_chain_tool()`
    tx.programmable_move_call(
        workflow_pkg_id,
        NEXUS_TOOL_REGISTRY_MODULE.into(),
        NEXUS_REGISTER_OFF_CHAIN_TOOL.into(),
        vec![],
        vec![
            tool_registry,
            fqn,
            url,
            input_schema,
            output_schema,
            pay_with,
        ],
    );

    Ok(tx)
}
