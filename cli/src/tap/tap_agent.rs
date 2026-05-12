use super::*;

pub(crate) async fn handle_agent_command(command: AgentCommand) -> AnyResult<(), NexusCliError> {
    match command {
        AgentCommand::Save { name, agent_id } => {
            let mut conf = CliConf::load().await.unwrap_or_default();
            conf.agents.insert(name.clone(), agent_id);
            conf.save().await.map_err(NexusCliError::Any)?;
            notify_success!("Saved Talus agent alias {name}");
            json_output(&json!({ "name": name, "agent_id": agent_id }))
        }
        AgentCommand::List => {
            let conf = CliConf::load().await.unwrap_or_default();
            let mut agents = conf.agents.into_iter().collect::<Vec<_>>();
            agents.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
            json_output(&json!({
                "agents": agents.into_iter().map(|(name, agent_id)| {
                    json!({ "name": name, "agent_id": agent_id })
                }).collect::<Vec<_>>()
            }))
        }
        AgentCommand::Remove { name } => {
            let mut conf = CliConf::load().await.unwrap_or_default();
            let removed = conf.agents.remove(&name);
            conf.save().await.map_err(NexusCliError::Any)?;
            json_output(&json!({ "name": name, "removed": removed }))
        }
    }
}
