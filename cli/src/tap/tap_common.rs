use {
    super::*,
    crate::types::AgentId,
    nexus_sdk::{move_bindings::interface::agent::SkillSchedulePolicy, nexus::client::NexusClient},
};

pub(crate) async fn read_artifact(path: PathBuf) -> AnyResult<TapPublishArtifact, NexusCliError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(NexusCliError::Io)?;
    serde_json::from_str(&text).map_err(|e| NexusCliError::Any(e.into()))
}

pub(crate) fn schedule_policy_from_cli(
    recurrence_kind: &str,
    min_interval_ms: u64,
    max_occurrences: u64,
) -> AnyResult<SkillSchedulePolicy, NexusCliError> {
    match recurrence_kind {
        "once" => Ok(SkillSchedulePolicy::Once),
        "recurring" => Ok(SkillSchedulePolicy::Recurring {
            min_interval_ms,
            max_occurrences: nexus_sdk::move_bindings::move_std::option::Option::from_option(
                (max_occurrences != 0).then_some(max_occurrences),
            ),
        }),
        other => Err(NexusCliError::Any(anyhow!(
            "invalid recurrence-kind '{other}': expected 'once' or 'recurring'"
        ))),
    }
}

pub(crate) fn agent_id_from_alias_or_arg(
    conf: &CliConf,
    alias: Option<String>,
    agent_id: Option<sui::types::Address>,
) -> AnyResult<AgentId, NexusCliError> {
    if let Some(agent_id) = agent_id {
        return Ok(agent_id);
    }
    if let Some(alias) = alias {
        let agent_id = conf.agents.get(&alias).copied().ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No Talus agent alias '{alias}' found in CLI config"
            ))
        })?;
        return Ok(agent_id);
    }
    Err(NexusCliError::Any(anyhow!(
        "provide either --agent-id or --alias"
    )))
}

pub(crate) async fn ensure_cli_agent_owner(
    nexus_client: &NexusClient,
    agent_id: AgentId,
) -> AnyResult<(), NexusCliError> {
    ensure_cli_agent_access(nexus_client, agent_id, AgentAccess::Immutable).await
}

pub(crate) async fn ensure_cli_mutable_agent(
    nexus_client: &NexusClient,
    agent_id: AgentId,
) -> AnyResult<(), NexusCliError> {
    ensure_cli_agent_access(nexus_client, agent_id, AgentAccess::Mutable).await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentAccess {
    Immutable,
    Mutable,
}

pub(crate) async fn ensure_cli_agent_access(
    nexus_client: &NexusClient,
    agent_id: AgentId,
    access: AgentAccess,
) -> AnyResult<(), NexusCliError> {
    let signer = nexus_client.signer().get_active_address();
    let metadata = nexus_client
        .crawler()
        .get_object_metadata(agent_id)
        .await
        .map_err(NexusCliError::Any)?;
    ensure_agent_owner_allowed(agent_id, &metadata.owner, signer, access)
}

pub(crate) fn ensure_agent_owner_allowed(
    agent_id: AgentId,
    owner: &sui::types::Owner,
    signer: sui::types::Address,
    access: AgentAccess,
) -> AnyResult<(), NexusCliError> {
    match owner {
        sui::types::Owner::Address(owner) if *owner == signer => Ok(()),
        sui::types::Owner::Shared(_) => Ok(()),
        sui::types::Owner::Immutable if access == AgentAccess::Immutable => Ok(()),
        sui::types::Owner::Immutable => Err(NexusCliError::Any(anyhow!(
            "agent '{agent_id}' is immutable; this command requires a mutable agent object"
        ))),
        _ => {
            let expected = match access {
                AgentAccess::Immutable => {
                    format!("active wallet {signer}, shared, or immutable ownership")
                }
                AgentAccess::Mutable => format!("active wallet {signer} or shared ownership"),
            };
            Err(NexusCliError::Any(anyhow!(
                "agent '{agent_id}' is owned by {owner:?}; expected {expected}"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_policy_from_cli_accepts_supported_recurrence_values() {
        assert_eq!(
            schedule_policy_from_cli("once", 50, 3).unwrap(),
            SkillSchedulePolicy::Once,
        );
        assert_eq!(
            schedule_policy_from_cli("recurring", 50, 0).unwrap(),
            SkillSchedulePolicy::Recurring {
                min_interval_ms: 50,
                max_occurrences: nexus_sdk::move_bindings::move_std::option::Option::from_option(
                    None
                ),
            }
        );
    }

    #[test]
    fn schedule_policy_from_cli_rejects_unknown_recurrence_values() {
        let error = schedule_policy_from_cli("Recurring", 50, 3)
            .expect_err("unknown recurrence kind should fail");
        assert!(
            error
                .to_string()
                .contains("invalid recurrence-kind 'Recurring'"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn agent_owner_guard_accepts_sender_shared_and_immutable_for_immutable_access() {
        let signer = sui::types::Address::from_static("0xa");
        let agent_id = sui::types::Address::from_static("0xb");

        ensure_agent_owner_allowed(
            agent_id,
            &sui::types::Owner::Address(signer),
            signer,
            AgentAccess::Mutable,
        )
        .expect("sender-owned agent is accepted");
        ensure_agent_owner_allowed(
            agent_id,
            &sui::types::Owner::Shared(1),
            signer,
            AgentAccess::Mutable,
        )
        .expect("shared agent is accepted");
        ensure_agent_owner_allowed(
            agent_id,
            &sui::types::Owner::Immutable,
            signer,
            AgentAccess::Immutable,
        )
        .expect("immutable agent is accepted");
    }

    #[test]
    fn agent_owner_guard_rejects_immutable_for_mutable_access() {
        let signer = sui::types::Address::from_static("0xa");
        let agent_id = sui::types::Address::from_static("0xb");
        let error = ensure_agent_owner_allowed(
            agent_id,
            &sui::types::Owner::Immutable,
            signer,
            AgentAccess::Mutable,
        )
        .expect_err("immutable agent cannot satisfy mutable access");

        assert!(
            error
                .to_string()
                .contains("requires a mutable agent object"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn agent_owner_guard_rejects_other_address_owner() {
        let signer = sui::types::Address::from_static("0xa");
        let agent_id = sui::types::Address::from_static("0xb");
        let error = ensure_agent_owner_allowed(
            agent_id,
            &sui::types::Owner::Address(sui::types::Address::from_static("0xc")),
            signer,
            AgentAccess::Immutable,
        )
        .expect_err("other address owner is rejected");

        assert!(
            error.to_string().contains("expected active wallet"),
            "unexpected error: {error}"
        );
    }
}
