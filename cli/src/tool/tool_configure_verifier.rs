use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    clap::Subcommand,
};

#[derive(Subcommand)]
pub(crate) enum VerifierCommand {
    #[command(about = "Enable RegisteredKey response verification")]
    RegisteredKey {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "owner-cap", short = 'o', value_name = "OBJECT_ID")]
        owner_cap: Option<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
    #[command(about = "Install one external response verifier")]
    External {
        #[arg(long = "tool-fqn", short = 't', value_name = "FQN")]
        tool_fqn: ToolFqn,
        #[arg(long = "owner-cap", short = 'o', value_name = "OBJECT_ID")]
        owner_cap: Option<sui::types::Address>,
        #[arg(long = "package", value_name = "PACKAGE_ID")]
        package: sui::types::Address,
        #[arg(long = "module", value_name = "MODULE")]
        module: sui::types::Identifier,
        #[arg(long = "function", value_name = "FUNCTION")]
        function: sui::types::Identifier,
        #[arg(
            long = "verifier-object",
            value_name = "OBJECT_ID",
            required = true,
            num_args = 1..
        )]
        verifier_objects: Vec<sui::types::Address>,
        #[command(flatten)]
        gas: GasArgs,
    },
}

pub(crate) async fn configure_verifier(command: VerifierCommand) -> AnyResult<(), NexusCliError> {
    match command {
        VerifierCommand::RegisteredKey {
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
                "verifier": "registered_key",
            }))
        }
        VerifierCommand::External {
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
            let handle = loading!("Validating and registering External verifier...");
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
                "No OwnerCap<OverTool> object ID found for Tool '{tool_fqn}'."
            ))
        })
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

    #[test]
    fn external_configuration_accepts_ordered_objects_after_one_flag() {
        let cli = crate::Cli::try_parse_from([
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
            "--verifier-object",
            "0x30",
            "0x31",
        ])
        .expect("ordered verifier objects after one flag should parse");

        let crate::Command::Tool(crate::tool::ToolCommand::ConfigureVerifier(
            VerifierCommand::External {
                verifier_objects, ..
            },
        )) = cli.command
        else {
            panic!("expected External verifier configuration");
        };
        assert_eq!(
            verifier_objects,
            vec![
                sui::types::Address::from_static("0x30"),
                sui::types::Address::from_static("0x31"),
            ]
        );
    }
}
