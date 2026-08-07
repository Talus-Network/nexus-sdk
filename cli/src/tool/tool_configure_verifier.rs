use {
    super::ConfigureVerifierCommand,
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
};

pub(crate) async fn configure_verifier(
    command: ConfigureVerifierCommand,
) -> AnyResult<(), NexusCliError> {
    match command {
        ConfigureVerifierCommand::RegisteredKey {
            tool_fqn,
            owner_cap,
            gas,
        } => {
            command_title!("Configuring RegisteredKey verifier for Tool '{tool_fqn}'");
            let owner_cap = resolve_owner_cap(&tool_fqn, owner_cap).await?;
            let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
            let handle = loading!("Submitting RegisteredKey verifier configuration...");
            let result = client
                .tool()
                .configure_registered_key_verifier(&tool_fqn, owner_cap)
                .await
                .map_err(NexusCliError::Nexus)?;
            handle.success();
            notify_success!("Configured RegisteredKey verifier for Tool '{tool_fqn}'.");
            json_output(&json!({
                "digest": result.tx_digest,
                "tool_fqn": tool_fqn,
                "tool_id": result.tool_id,
                "verifier": "registered_key",
            }))
        }
        ConfigureVerifierCommand::External {
            tool_fqn,
            owner_cap,
            package,
            module,
            function,
            verifier_objects,
            gas,
        } => {
            command_title!("Configuring External verifier for Tool '{tool_fqn}'");
            let owner_cap = resolve_owner_cap(&tool_fqn, owner_cap).await?;
            let client = get_nexus_client(gas.sui_gas_coin, gas.sui_gas_budget).await?;
            let handle = loading!("Preflighting and registering External verifier...");
            let result = client
                .tool()
                .configure_external_verifier(
                    &tool_fqn,
                    owner_cap,
                    package,
                    module.as_str(),
                    function.as_str(),
                    &verifier_objects,
                )
                .await
                .map_err(NexusCliError::Nexus)?;
            handle.success();
            notify_success!("Configured External verifier for Tool '{tool_fqn}'.");
            json_output(&json!({
                "digest": result.tx_digest,
                "tool_fqn": tool_fqn,
                "tool_id": result.tool_id,
                "verifier": "external",
                "package": package,
                "module": module,
                "function": function,
                "verifier_objects": verifier_objects,
            }))
        }
    }
}

async fn resolve_owner_cap(
    tool_fqn: &ToolFqn,
    owner_cap: Option<sui::types::Address>,
) -> AnyResult<sui::types::Address, NexusCliError> {
    let conf = CliConf::load().await.unwrap_or_default();
    owner_cap
        .or_else(|| conf.tools.get(tool_fqn).map(|tool| tool.over_tool))
        .ok_or_else(|| {
            NexusCliError::Any(anyhow!(
                "No OwnerCap<OverTool> object ID found for tool '{tool_fqn}'."
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_registered_key_configuration() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "configure-verifier",
            "registered-key",
            "--tool-fqn",
            "xyz.demo.tool@1",
            "--owner-cap",
            "0x10",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            crate::Command::Tool(crate::tool::ToolCommand::ConfigureVerifier {
                verifier: ConfigureVerifierCommand::RegisteredKey { .. }
            })
        ));
    }

    #[test]
    fn parses_external_configuration_with_ordered_objects() {
        let cli = crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "configure-verifier",
            "external",
            "--tool-fqn",
            "xyz.demo.tool@1",
            "--owner-cap",
            "0x10",
            "--package",
            "0x20",
            "--module",
            "verifier",
            "--function",
            "verify",
            "--verifier-object",
            "0x30",
            "--verifier-object",
            "0x31",
        ])
        .unwrap();
        let crate::Command::Tool(crate::tool::ToolCommand::ConfigureVerifier {
            verifier:
                ConfigureVerifierCommand::External {
                    verifier_objects, ..
                },
        }) = cli.command
        else {
            panic!("expected External verifier command");
        };
        assert_eq!(
            verifier_objects,
            vec![
                sui::types::Address::from_static("0x30"),
                sui::types::Address::from_static("0x31"),
            ]
        );
    }

    #[test]
    fn external_configuration_requires_a_witness_object() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "configure-verifier",
            "external",
            "--tool-fqn",
            "xyz.demo.tool@1",
            "--package",
            "0x20",
            "--module",
            "verifier",
            "--function",
            "verify",
        ])
        .is_err());
    }
}
