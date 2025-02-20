use {
    crate::{command_title, loading, prelude::*, utils::*},
    move_core_types::ident_str,
};

/// Sui `std::ascii::string`
const SUI_ASCII_MODULE: &sui::MoveIdentStr = ident_str!("ascii");
const SUI_ASCII_FROM_STRING: &sui::MoveIdentStr = ident_str!("string");

// Nexus `tool_registry::claim_collateral_for_off_chain_tool`
const NEXUS_TOOL_REGISTRY_MODULE: &sui::MoveIdentStr = ident_str!("tool_registry");
const NEXUS_CLAIM_COLLATERAL_FOR_OFF_CHAIN_TOOL: &sui::MoveIdentStr =
    ident_str!("claim_collateral_for_off_chain_tool");

/// Claim collateral for a Tool based on the provided FQN.
pub(crate) async fn claim_collateral(
    tool_fqn: ToolFqn,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Claiming collateral for Tool '{tool_fqn}'");

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

    // Fetch gas coin object.
    let gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Fetch the tool registry object.
    let tool_registry = fetch_object_by_id(&sui, tool_registry_object_id).await?;

    // Craft a TX to claim the collaters for a Tool.
    let tx_handle = loading!("Crafting transaction...");

    let tx = match prepare_transaction(&tool_fqn, tool_registry, workflow_pkg_id) {
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

/// Fetch the gas coin from the Sui client. On Localnet, Devnet and Testnet, we
/// can use the faucet to get the coin. On Mainnet, this fails if the coin is
/// not present.
async fn fetch_gas_coin(
    sui: &sui::Client,
    sui_net: SuiNet,
    addr: sui::Address,
    sui_gas_coin: Option<sui::ObjectID>,
) -> AnyResult<sui::Coin, NexusCliError> {
    let mut coins = fetch_all_coins_for_address(sui, addr).await?;

    // We need at least 1 coin. We can create it on Localnet, Devnet and Testnet.
    match sui_net {
        SuiNet::Localnet | SuiNet::Devnet | SuiNet::Testnet if coins.is_empty() => {
            request_tokens_from_faucet(sui_net, addr).await?;

            coins = fetch_all_coins_for_address(sui, addr).await?;
        }
        SuiNet::Mainnet if coins.is_empty() => {
            return Err(NexusCliError::Any(anyhow!(
                "The wallet does not have enough coins to claim the collateral for a Tool"
            )));
        }
        _ => (),
    }

    if coins.len() < 2 {
        return Err(NexusCliError::Any(anyhow!(
            "The wallet does not have enough coins to claim the collateral for a Tool"
        )));
    }

    // If object IDs were specified, use them.
    let gas_coin = sui_gas_coin
        .and_then(|id| coins.iter().find(|coin| coin.coin_object_id == id))
        .cloned()
        .unwrap_or_else(|| coins.remove(0));

    Ok(gas_coin)
}

/// Build a programmable transaction to claim the collateral for a tool.
fn prepare_transaction(
    tool_fqn: &ToolFqn,
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
    let fqn = tx.pure(tool_fqn.to_string().as_bytes())?;

    let fqn = tx.programmable_move_call(
        sui::MOVE_STDLIB_PACKAGE_ID,
        SUI_ASCII_MODULE.into(),
        SUI_ASCII_FROM_STRING.into(),
        vec![],
        vec![fqn],
    );

    // `clock: &Clock`
    let clock = tx.obj(sui::ObjectArg::SharedObject {
        id: sui::CLOCK_OBJECT_ID,
        initial_shared_version: sui::CLOCK_OBJECT_SHARED_VERSION,
        mutable: false,
    })?;

    // `nexus::tool_registry::unregister_off_chain_tool()`
    tx.programmable_move_call(
        workflow_pkg_id,
        NEXUS_TOOL_REGISTRY_MODULE.into(),
        NEXUS_CLAIM_COLLATERAL_FOR_OFF_CHAIN_TOOL.into(),
        vec![],
        vec![tool_registry, fqn, clock],
    );

    Ok(tx)
}
