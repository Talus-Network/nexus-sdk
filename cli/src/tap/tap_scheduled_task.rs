use {
    crate::{
        command_title,
        display::json_output,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::nexus::tap::{AgentTaskStateAction, SetAgentTaskStateResult},
    serde_json::json,
};

pub(crate) async fn set_agent_task_state(
    task_id: sui::types::Address,
    agent_id: sui::types::Address,
    gas: GasArgs,
    action: AgentTaskStateAction,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "{verb} TAP scheduled task '{task_id}'",
        verb = agent_task_state_action_verb(action)
    );

    let nexus_client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .set_agent_task_state(task_id, agent_id, action)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!("TAP scheduled task state updated");

    json_output(&scheduled_task_state_result_json(&result))?;

    Ok(())
}

fn scheduled_task_state_result_json(result: &SetAgentTaskStateResult) -> serde_json::Value {
    json!({
        "digest": result.tx_digest,
        "checkpoint": result.tx_checkpoint,
        "scheduled_task_id": result.task_id,
        "agent_id": result.agent_id,
        "state": agent_task_state_action_state(result.state),
    })
}

fn agent_task_state_action_verb(action: AgentTaskStateAction) -> &'static str {
    match action {
        AgentTaskStateAction::Pause => "Pausing",
        AgentTaskStateAction::Resume => "Resuming",
        AgentTaskStateAction::Cancel => "Canceling",
    }
}

fn agent_task_state_action_state(action: AgentTaskStateAction) -> &'static str {
    match action {
        AgentTaskStateAction::Pause => "paused",
        AgentTaskStateAction::Resume => "resumed",
        AgentTaskStateAction::Cancel => "canceled",
    }
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::sui};

    #[test]
    fn scheduled_task_state_result_json_exposes_required_agent_status_keys() {
        let task_id = sui::types::Address::from_static("0xabc");
        let agent_id = sui::types::Address::from_static("0xa11ce");
        let result = SetAgentTaskStateResult {
            tx_digest: sui::types::Digest::ZERO,
            tx_checkpoint: 42,
            task_id,
            agent_id,
            state: AgentTaskStateAction::Pause,
        };

        let json = scheduled_task_state_result_json(&result);

        assert_eq!(json["digest"], result.tx_digest.to_string());
        assert_eq!(json["checkpoint"], 42);
        assert_eq!(json["scheduled_task_id"], task_id.to_string());
        assert_eq!(json["agent_id"], agent_id.to_string());
        assert_eq!(json["state"], "paused");
    }
}
