use {
    super::*,
    crate::types::AgentId,
    nexus_sdk::types::{TapRecurrenceKind, TapSchedulePolicy},
};

pub(crate) async fn read_artifact(path: PathBuf) -> AnyResult<TapPublishArtifact, NexusCliError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(NexusCliError::Io)?;
    serde_json::from_str(&text).map_err(|e| NexusCliError::Any(e.into()))
}

pub(crate) fn decode_hex_arg(value: &str, name: &str) -> AnyResult<Vec<u8>, NexusCliError> {
    hex::decode(value.trim_start_matches("0x"))
        .map_err(|e| NexusCliError::Any(anyhow!("invalid {name} hex: {e}")))
}

pub(crate) fn decode_optional_hex_arg(
    value: Option<String>,
    name: &str,
) -> AnyResult<Option<Vec<u8>>, NexusCliError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| decode_hex_arg(&value, name))
        .transpose()
}

pub(crate) fn agent_execute_options_from_cli(
    payment_source_hex: String,
    payment_max_budget: u64,
    authorization_plan_commitment_hex: Option<String>,
) -> AnyResult<AgentDagExecuteOptions, NexusCliError> {
    Ok(AgentDagExecuteOptions {
        payment_source: decode_hex_arg(&payment_source_hex, "payment-source")?,
        payment_coin: None,
        payment_coin_balance: None,
        payment_max_budget,
        authorization_plan_commitment: decode_optional_hex_arg(
            authorization_plan_commitment_hex,
            "authorization-plan-hash",
        )?,
        authorization_plan: Vec::new(),
    })
}

pub(crate) fn schedule_policy_from_cli(
    recurrence_kind: &str,
    min_interval_ms: u64,
    max_occurrences: u64,
    allow_recursive: bool,
) -> AnyResult<TapSchedulePolicy, NexusCliError> {
    let recurrence = match recurrence_kind {
        "once" => TapRecurrenceKind::Once,
        "recursive" => TapRecurrenceKind::Recursive {
            min_interval_ms,
            max_occurrences: (max_occurrences != 0).then_some(max_occurrences),
        },
        other => {
            return Err(NexusCliError::Any(anyhow!(
                "invalid recurrence-kind '{other}': expected 'once' or 'recursive'"
            )));
        }
    };

    Ok(TapSchedulePolicy {
        recurrence,
        allow_recursive,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_hex_helpers_report_named_argument_errors() {
        let error = decode_hex_arg("0xnot-hex", "payment-source").expect_err("invalid hex");
        assert!(
            error.to_string().contains("invalid payment-source hex"),
            "unexpected error: {error}"
        );

        assert_eq!(
            decode_optional_hex_arg(Some(String::new()), "authorization-plan").unwrap(),
            None
        );
        assert_eq!(
            decode_optional_hex_arg(Some("0x0102".to_string()), "authorization-plan").unwrap(),
            Some(vec![1, 2])
        );
    }

    #[test]
    fn schedule_policy_from_cli_accepts_supported_recurrence_values() {
        assert_eq!(
            schedule_policy_from_cli("once", 50, 3, false).unwrap(),
            TapSchedulePolicy {
                recurrence: TapRecurrenceKind::Once,
                allow_recursive: false,
            }
        );
        assert_eq!(
            schedule_policy_from_cli("recursive", 50, 0, true).unwrap(),
            TapSchedulePolicy {
                recurrence: TapRecurrenceKind::Recursive {
                    min_interval_ms: 50,
                    max_occurrences: None,
                },
                allow_recursive: true,
            }
        );
    }

    #[test]
    fn schedule_policy_from_cli_rejects_unknown_recurrence_values() {
        let error = schedule_policy_from_cli("Recursive", 50, 3, false)
            .expect_err("unknown recurrence kind should fail");
        assert!(
            error
                .to_string()
                .contains("invalid recurrence-kind 'Recursive'"),
            "unexpected error: {error}"
        );
    }
}
