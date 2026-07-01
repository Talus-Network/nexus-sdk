//! TAP execution lifecycle commands.

use {
    super::*,
    crate::tap::tap_output::{execution_abort_result_json, execution_settle_result_json},
    nexus_sdk::nexus::workflow::SettleCommittedToolResultParams,
};

pub(crate) async fn handle_execution_command(
    command: ExecutionCommand,
) -> AnyResult<(), NexusCliError> {
    match command {
        ExecutionCommand::Settle {
            execution_id,
            walk_index,
            gas,
        } => {
            settle_committed_result(
                execution_id,
                walk_index,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
        ExecutionCommand::Abort { execution_id, gas } => {
            abort_execution(execution_id, gas.sui_gas_coin, gas.sui_gas_budget).await
        }
    }
}

async fn settle_committed_result(
    execution_id: sui::types::Address,
    walk_index: u64,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Settling committed TAP execution result for DAGExecution '{execution_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let handle = loading!("Submitting permissionless committed-result settlement transaction...");
    let result = match nexus_client
        .workflow()
        .settle_committed_tool_result_for_walk(SettleCommittedToolResultParams {
            dag_execution_id: execution_id,
            walk_index,
        })
        .await
        .map_err(NexusCliError::Nexus)
    {
        Ok(result) => result,
        Err(error) => {
            handle.error();
            return Err(error);
        }
    };
    handle.success();

    notify_success!(
        "Settlement transaction submitted: {digest}",
        digest = result.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&execution_settle_result_json(&result))
}

async fn abort_execution(
    execution_id: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Aborting expired TAP DAGExecution '{execution_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let handle = loading!("Submitting permissionless execution abort transaction...");
    let result = match nexus_client
        .workflow()
        .abort_expired_execution(execution_id)
        .await
        .map_err(NexusCliError::Nexus)
    {
        Ok(result) => result,
        Err(error) => {
            handle.error();
            return Err(error);
        }
    };
    handle.success();

    notify_success!(
        "Abort transaction submitted: {digest}",
        digest = result.tx_digest.to_string().truecolor(100, 100, 100)
    );
    json_output(&execution_abort_result_json(&result))
}
