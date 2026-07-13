use super::*;

pub(crate) async fn handle_agent_command(command: AgentCommand) -> AnyResult<(), NexusCliError> {
    match command {
        AgentCommand::Save { name, agent_id } => {
            let mut conf = CliConf::load().await.unwrap_or_default();
            conf.agents.insert(name.clone(), agent_id);
            conf.save().await.map_err(NexusCliError::Any)?;
            notify_success!("Saved Talus agent alias {name}");
            json_output(&agent_save_result_json(&name, agent_id))
        }
        AgentCommand::List => {
            let conf = CliConf::load().await.unwrap_or_default();
            let mut agents = conf.agents.into_iter().collect::<Vec<_>>();
            agents.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
            json_output(&agent_list_result_json(&agents))
        }
        AgentCommand::Remove { name } => {
            let mut conf = CliConf::load().await.unwrap_or_default();
            let removed = conf.agents.remove(&name);
            conf.save().await.map_err(NexusCliError::Any)?;
            json_output(&agent_remove_result_json(&name, removed))
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, std::ffi::OsString};

    struct EnvGuard {
        home: Option<OsString>,
    }

    impl EnvGuard {
        fn with_home(path: &std::path::Path) -> Self {
            let guard = Self {
                home: std::env::var_os("HOME"),
            };
            std::env::set_var("HOME", path);
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.home.take() {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn agent_alias_commands_persist_list_and_remove() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::with_home(temp_home.path());
        let agent_id = sui::types::Address::from_static("0xa");

        handle_agent_command(AgentCommand::Save {
            name: "primary".to_string(),
            agent_id,
        })
        .await
        .expect("save alias");

        let conf = CliConf::load().await.expect("saved config");
        assert_eq!(conf.agents.get("primary"), Some(&agent_id));

        handle_agent_command(AgentCommand::List)
            .await
            .expect("list aliases");
        handle_agent_command(AgentCommand::Remove {
            name: "primary".to_string(),
        })
        .await
        .expect("remove alias");

        let conf = CliConf::load().await.expect("updated config");
        assert!(!conf.agents.contains_key("primary"));
    }
}
