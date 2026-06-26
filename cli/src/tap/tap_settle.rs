//! TAP execution lifecycle commands.

use {
    super::*,
    nexus_sdk::nexus::workflow::{
        AbortExecutionResult,
        CommittedToolResultSettlementResult,
        SettleCommittedToolResultParams,
    },
};

pub(crate) async fn handle_execution_command(
    command: SettleCommand,
) -> AnyResult<(), NexusCliError> {
    match command {
        SettleCommand::Settle {
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
        SettleCommand::Abort { execution_id, gas } => {
            abort_execution(execution_id, gas.sui_gas_coin, gas.sui_gas_budget).await
        }
    }
}

fn execution_settle_result_json(result: &CommittedToolResultSettlementResult) -> serde_json::Value {
    json!({
        "function": "settle_committed_tool_result_for_walk",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "dag_id": result.dag_id,
        "execution_id": result.dag_execution_id,
        "walk_index": result.walk_index,
    })
}

fn execution_abort_result_json(result: &AbortExecutionResult) -> serde_json::Value {
    json!({
        "function": "abort_expired_execution",
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "dag_id": result.dag_id,
        "execution_id": result.dag_execution_id,
    })
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

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::sui};

    #[test]
    fn execution_settle_result_json_includes_stable_fields() {
        let result = CommittedToolResultSettlementResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 7,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
            walk_index: 3,
        };

        let json = execution_settle_result_json(&result);

        assert_eq!(json["function"], "settle_committed_tool_result_for_walk");
        assert_eq!(json["tx_checkpoint"], 7);
        assert_eq!(
            json["dag_id"],
            sui::types::Address::from_static("0xda6").to_string()
        );
        assert_eq!(
            json["execution_id"],
            sui::types::Address::from_static("0xe").to_string()
        );
        assert_eq!(json["walk_index"], 3);
    }

    #[test]
    fn execution_abort_result_json_includes_stable_fields() {
        let result = AbortExecutionResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 9,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
        };

        let json = execution_abort_result_json(&result);

        assert_eq!(json["function"], "abort_expired_execution");
        assert_eq!(json["tx_checkpoint"], 9);
        assert_eq!(
            json["dag_id"],
            sui::types::Address::from_static("0xda6").to_string()
        );
        assert_eq!(
            json["execution_id"],
            sui::types::Address::from_static("0xe").to_string()
        );
    }
}
