use crate::{
    command_title,
    dag::{dag_validate::validate_dag, parser::Vertex},
    prelude::*,
    sui::*,
};

/// Publish the provided Nexus DAG to the currently active Sui net. This also
/// performs validation on the DAG before publishing.
pub(crate) async fn publish_dag(
    path: PathBuf,
    sui_gas_coin: Option<sui::ObjectID>,
    _sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let _dag = validate_dag(path).await?;

    command_title!("Publishing Nexus DAG");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Workflow package and tool registry IDs must be present.
    // TODO: registry does not need to be present but primitives likely do.
    // This can be changed to n different helpers.
    let NexusObjects { .. } = get_nexus_objects(&conf)?;

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
    let _gas_coin = fetch_gas_coin(&sui, conf.sui.net, address, sui_gas_coin).await?;

    // Fetch reference gas price.
    let _reference_gas_price = fetch_reference_gas_price(&sui).await?;

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    // 1. stuff too specific, we need <T> tools?
    // 2. can we even pass API keys as they're discoverable on chain?

    Ok(())
}

pub(crate) fn get_tx_vertex(
    tx: &mut sui::ProgrammableTransactionBuilder,
    vertex: &Vertex,
) -> AnyResult<(), NexusCliError> {
    //

    Ok(())
}
