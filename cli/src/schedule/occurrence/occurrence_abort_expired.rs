use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::{
        nexus::{
            scheduler::OccurrenceRef,
            workflow::{AbortExpiredExecutionResult, ToolGasAbortCandidateWalk},
        },
        sui,
    },
};

fn abort_walk_json(walk: &ToolGasAbortCandidateWalk) -> serde_json::Value {
    json!({
        "walk_index": walk.walk_index,
        "vertex": walk.vertex,
        "payment_vertex_key": hex::encode(&walk.payment_vertex_key),
    })
}

pub(crate) fn abort_expired_occurrence_result_json(
    occurrence: OccurrenceRef,
    result: &AbortExpiredExecutionResult,
) -> serde_json::Value {
    json!({
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
        "task_id": occurrence.task_id,
        "occurrence_id": occurrence.occurrence_id,
        "dag_id": result.dag_id,
        "execution_id": result.dag_execution_id,
        "tool_fqn": result.selected_candidate.tool_fqn.to_string(),
        "tool_gas_id": result.selected_candidate.tool_gas_ref.object_id(),
        "matching_walks": result
            .selected_candidate
            .matching_walks
            .iter()
            .map(abort_walk_json)
            .collect::<Vec<_>>(),
    })
}

pub(crate) async fn abort_expired_occurrence(
    task_id: sui::types::Address,
    occurrence_id: u64,
    tool_gas_id: Option<sui::types::Address>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Aborting expired occurrence '{occurrence_id}' in Task '{task_id}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let occurrence = OccurrenceRef::new(task_id, occurrence_id);

    let tx_handle = loading!("Finding ToolGas candidate and submitting abort transaction...");
    let result = match nexus_client
        .scheduler()
        .abort_expired_occurrence_with_tool_gas(occurrence, tool_gas_id)
        .await
        .map_err(NexusCliError::Nexus)
    {
        Ok(result) => result,
        Err(error) => {
            tx_handle.error();
            return Err(error);
        }
    };
    tx_handle.success();

    notify_success!(
        "Abort transaction submitted: {digest}",
        digest = result.tx_digest.to_string().truecolor(100, 100, 100)
    );
    notify_success!(
        "Selected ToolGas: {tool_gas}",
        tool_gas = result
            .selected_candidate
            .tool_gas_ref
            .object_id()
            .to_string()
            .truecolor(100, 100, 100)
    );

    json_output(&abort_expired_occurrence_result_json(occurrence, &result))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            move_bindings::interface::graph::RuntimeVertex,
            nexus::workflow::{ToolGasAbortCandidate, ToolGasAbortCandidateWalk},
        },
    };

    #[test]
    fn abort_expired_occurrence_json_includes_selected_tool_gas_and_walks() {
        let result = AbortExpiredExecutionResult {
            tx_digest: sui::types::Digest::default(),
            tx_checkpoint: 42,
            dag_id: sui::types::Address::from_static("0xda6"),
            dag_execution_id: sui::types::Address::from_static("0xe"),
            selected_candidate: ToolGasAbortCandidate {
                tool_fqn: "xyz.taluslabs.payable@1".parse().expect("tool fqn"),
                tool_gas_ref: sui::types::ObjectReference::new(
                    sui::types::Address::from_static("0x9a5"),
                    7,
                    sui::types::Digest::default(),
                ),
                matching_walks: vec![ToolGasAbortCandidateWalk {
                    walk_index: 3,
                    vertex: RuntimeVertex::plain("vertex"),
                    payment_vertex_key: vec![0xab, 0xcd],
                }],
            },
        };

        let occurrence = OccurrenceRef::new(sui::types::Address::from_static("0x81"), 3);
        let json = abort_expired_occurrence_result_json(occurrence, &result);

        assert_eq!(json["tx_checkpoint"], 42);
        assert_eq!(json["task_id"], occurrence.task_id.to_string());
        assert_eq!(json["occurrence_id"], occurrence.occurrence_id);
        assert_eq!(
            json["dag_id"],
            sui::types::Address::from_static("0xda6").to_string()
        );
        assert_eq!(
            json["execution_id"],
            sui::types::Address::from_static("0xe").to_string()
        );
        assert_eq!(json["tool_fqn"], "xyz.taluslabs.payable@1");
        assert_eq!(
            json["tool_gas_id"],
            sui::types::Address::from_static("0x9a5").to_string()
        );
        assert_eq!(json["matching_walks"][0]["walk_index"], 3);
        assert_eq!(json["matching_walks"][0]["payment_vertex_key"], "abcd");
    }
}
