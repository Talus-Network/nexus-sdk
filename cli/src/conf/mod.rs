use crate::{command_title, loading, prelude::*};

#[derive(Args, Clone, Debug)]
pub(crate) struct ConfCommand {
    #[arg(
        long = "sui.net",
        help = "Set the Sui network",
        value_enum,
        value_name = "NET"
    )]
    sui_net: Option<SuiNet>,
    #[arg(
        long = "sui.wallet-path",
        help = "Set the Sui wallet path",
        value_name = "PATH",
        value_parser = ValueParser::from(expand_tilde)
    )]
    sui_wallet_path: Option<PathBuf>,
    #[arg(
        long = "nexus.workflow-id",
        help = "Set the Nexus Workflow package ID",
        value_name = "ID"
    )]
    nexus_workflow_id: Option<sui::ObjectID>,
    #[arg(
        long = "nexus.tool-registry-id",
        help = "Set the Nexus Tool Registry object ID",
        value_name = "ID"
    )]
    nexus_tool_registry_id: Option<sui::ObjectID>,
}

/// Handle the provided conf command. The [ConfCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(command: ConfCommand) -> AnyResult<(), NexusCliError> {
    let ConfCommand {
        sui_net,
        sui_wallet_path,
        nexus_workflow_id,
        nexus_tool_registry_id,
    } = command;

    let mut conf = CliConf::load().unwrap_or(CliConf::default());

    // If all fields are None, we just want to display the current config.
    if sui_net.is_none()
        && sui_wallet_path.is_none()
        && nexus_workflow_id.is_none()
        && nexus_tool_registry_id.is_none()
    {
        command_title!("Current Nexus CLI Configuration");

        println!("{:#?}", conf);

        return Ok(());
    }

    command_title!("Updating Nexus CLI Configuration");

    let config_handle = loading!("Updating configuration...");

    conf.sui.net = sui_net.unwrap_or(conf.sui.net);
    conf.sui.wallet_path = sui_wallet_path.unwrap_or(conf.sui.wallet_path);
    conf.nexus.workflow_id = nexus_workflow_id.or(conf.nexus.workflow_id);
    conf.nexus.tool_registry_id = nexus_tool_registry_id.or(conf.nexus.tool_registry_id);

    match conf.save() {
        Ok(()) => {
            config_handle.success();

            Ok(())
        }
        Err(e) => {
            config_handle.error();

            Err(NexusCliError::AnyError(e))
        }
    }
}
