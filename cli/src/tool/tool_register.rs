use {
    crate::{
        command_title,
        prelude::*,
        tool::{tool_validate::*, ToolIdent},
        utils::*,
    },
    std::path::PathBuf,
};

/// Validate and then register a new Tool.
pub(crate) async fn register_tool(
    ident: ToolIdent,
    sui_wallet_path: PathBuf,
    sui_net: SuiNet,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_collateral_coin: Option<sui::ObjectID>,
) -> AnyResult<(), NexusCliError> {
    let meta = validate_tool(ident).await?;

    command_title!(
        "Registering Tool '{fqn}' at '{url}' on {sui_net}",
        fqn = meta.fqn,
        url = meta.url
    );

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&sui_wallet_path, &sui_net).await?;
    let sui = build_sui_client(&sui_net).await?;

    let address = match wallet.active_address() {
        Ok(address) => address,
        Err(e) => {
            return Err(NexusCliError::AnyError(e));
        }
    };

    // Fetch gas and collateral coin objects.
    let (gas_coin, collateral_coin) =
        fetch_gas_and_collateral_coins(&sui, sui_net, address, sui_gas_coin, sui_collateral_coin)
            .await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Craft a TX to register the tool.
    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // `self: &mut ToolRegistry`
    let tool_registry = fetch_object_by_id(
        &sui,
        "0x741d0cd3cd69d375790bebd1eb603448e0b157250b8568db8f42cf292460da86"
            .parse()
            .unwrap(),
    )
    .await?;

    let tool_registry = tx
        .obj(sui::ObjectArg::SharedObject {
            id: tool_registry.object_id,
            initial_shared_version: tool_registry.version,
            mutable: true,
        })
        .unwrap();

    // `fqn: AsciiString`
    let fqn = tx.pure(meta.fqn.as_bytes()).unwrap();

    let fqn = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        sui::Identifier::new("ascii").unwrap(),
        sui::Identifier::new("string").unwrap(),
        vec![],
        vec![fqn],
    );

    // `url: vector<u8>`
    let url = tx.pure(meta.url.as_bytes()).unwrap();

    // `input_schema: vector<u8>`
    let input_schema = tx.pure(meta.input_schema.to_string().as_bytes()).unwrap();

    // `output_schema: vector<u8>`
    let output_schema = tx.pure(meta.output_schema.to_string().as_bytes()).unwrap();

    // `pay_with: Coin<SUI>`
    let pay_with = tx
        .obj(sui::ObjectArg::ImmOrOwnedObject(
            collateral_coin.object_ref(),
        ))
        .unwrap();

    // `register_off_chain_tool()`
    tx.programmable_move_call(
        // Workflow package ID.
        "0x6f907d922b802b199b8638f15d18f1b6ba929772bb02fa1c0256617b67ed1e8a"
            .parse()
            .unwrap(),
        sui::Identifier::new("tool_registry").unwrap(),
        sui::Identifier::new("register_off_chain_tool").unwrap(),
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

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        // TODO: Should this be a command arg?
        sui::MIST_PER_SUI / 10,
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
