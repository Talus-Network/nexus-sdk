use {
    crate::{
        command_title,
        loading,
        prelude::*,
        tool::{tool_validate::*, ToolIdent, ToolMeta},
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
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Workflow package and tool registry IDs must be present.
    let NexusObjects {
        workflow_pkg_id,
        tool_registry_object_id,
    } = get_nexus_objects(&conf)?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(conf.sui.net).await?;

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
    // adjust `prepare_transaction`.
    if ident_check.on_chain.is_some() {
        todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>");
    }

    let tx = match prepare_transaction(meta, collateral_coin, tool_registry, workflow_pkg_id) {
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
    sign_transaction(&sui, &wallet, tx_data).await
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

/// Build a programmable transaction to register a new off-chain Tool.
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
    let fqn = tx.pure(meta.fqn.to_string().as_bytes())?;

    let fqn = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        SUI_ASCII_MODULE.into(),
        SUI_ASCII_FROM_STRING.into(),
        vec![],
        vec![fqn],
    );

    // `url: vector<u8>`
    let url = tx.pure(meta.url.to_string().as_bytes())?;

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
