use crate::{command_title, dag::dag_validate::validate_dag, prelude::*};

/// Publish the provided Nexus DAG to the currently active Sui net. This also
/// performs validation on the DAG before publishing.
pub(crate) async fn publish_dag(
    path: PathBuf,
    _sui_gas_coin: Option<sui::ObjectID>,
    _sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let _dag = validate_dag(path).await?;

    command_title!("Publishing Nexus DAG");

    // ...

    Ok(())
}
