//! TAP execution lifecycle commands.

use {
    super::*,
    crate::tap::tap_output::{
        execution_abort_result_json,
        execution_resolve_expired_walk_result_json,
        execution_settle_result_json,
    },
    nexus_sdk::nexus::workflow::{ResolveExpiredWalkParams, SettleCommittedToolResultParams},
};

pub(crate) async fn handle_execution_command(
    command: ExecutionCommand,
) -> AnyResult<(), NexusCliError> {
    match command {
        ExecutionCommand::ResolveExpiredWalk {
            execution_id,
            walk_index,
            tool_gas_id,
            gas,
        } => {
            resolve_expired_walk(
                execution_id,
                walk_index,
                tool_gas_id,
                gas.sui_gas_coin,
                gas.sui_gas_budget,
            )
            .await
        }
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

async fn resolve_expired_walk(
    execution_id: sui::types::Address,
    walk_index: u64,
    tool_gas_id: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Resolving expired TAP DAGExecution walk '{execution_id}::{walk_index}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let handle = loading!("Inspecting walk state and submitting matching timeout resolution...");
    let result = match nexus_client
        .workflow()
        .resolve_expired_walk(ResolveExpiredWalkParams {
            dag_execution_id: execution_id,
            walk_index,
            tool_gas_id,
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

    if let Some(digest) = result.tx_digest {
        notify_success!(
            "Timeout resolution submitted: {digest}",
            digest = digest.to_string().truecolor(100, 100, 100)
        );
    } else if let Some(reason) = result.resolution_kind.skip_reason() {
        notify_success!("Timeout resolution skipped: {reason}");
    }
    json_output(&execution_resolve_expired_walk_result_json(&result))
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
