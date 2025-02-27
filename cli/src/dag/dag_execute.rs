use crate::{command_title, prelude::*};

/// Execute a Nexus DAG based on the provided object ID and initial input data.
pub(crate) async fn execute_dag(
    dag_id: sui::ObjectID,
    _input_data: serde_json::Value,
    _sui_gas_coin: Option<sui::ObjectID>,
    _sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Executing Nexus DAG '{dag_id}'");

    // ...

    Ok(())
}
