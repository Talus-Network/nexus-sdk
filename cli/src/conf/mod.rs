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
        long = "nexus.primitives-id",
        help = "Set the Nexus Primitives package ID",
        value_name = "ID"
    )]
    nexus_primitives_id: Option<sui::ObjectID>,
    #[arg(
        long = "nexus.tool-registry-id",
        help = "Set the Nexus Tool Registry object ID",
        value_name = "ID"
    )]
    nexus_tool_registry_id: Option<sui::ObjectID>,
    /// Hidden argument used for testing to set the path of the configuration
    /// file.
    #[arg(
        long = "conf-path",
        hide = true,
        default_value = CLI_CONF_PATH,
        value_parser = ValueParser::from(expand_tilde)
    )]
    conf_path: PathBuf,
}

/// Handle the provided conf command. The [ConfCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(
    ConfCommand {
        sui_net,
        sui_wallet_path,
        nexus_workflow_id,
        nexus_primitives_id,
        nexus_tool_registry_id,
        conf_path,
    }: ConfCommand,
) -> AnyResult<(), NexusCliError> {
    let mut conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_else(|_| CliConf::default());

    // If all fields are None, we just want to display the current configuration.
    if sui_net.is_none()
        && sui_wallet_path.is_none()
        && nexus_workflow_id.is_none()
        && nexus_tool_registry_id.is_none()
        && nexus_primitives_id.is_none()
    {
        command_title!("Current Nexus CLI Configuration");

        println!("{:#?}", conf);

        return Ok(());
    }

    command_title!("Updating Nexus CLI Configuration");

    let conf_handle = loading!("Updating configuration...");

    conf.sui.net = sui_net.unwrap_or(conf.sui.net);
    conf.sui.wallet_path = sui_wallet_path.unwrap_or(conf.sui.wallet_path);
    conf.nexus.workflow_id = nexus_workflow_id.or(conf.nexus.workflow_id);
    conf.nexus.primitives_id = nexus_primitives_id.or(conf.nexus.primitives_id);
    conf.nexus.tool_registry_id = nexus_tool_registry_id.or(conf.nexus.tool_registry_id);

    match conf.save(&conf_path).await {
        Ok(()) => {
            conf_handle.success();

            Ok(())
        }
        Err(e) => {
            conf_handle.error();

            Err(NexusCliError::Any(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, assert_matches::assert_matches};

    #[tokio::test]
    async fn test_conf_loads_and_saves() {
        let path = PathBuf::from("/tmp/.nexus/conf.toml");

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_workflow_id = Some(sui::ObjectID::random());
        let nexus_primitives_id = Some(sui::ObjectID::random());
        let nexus_tool_registry_id = Some(sui::ObjectID::random());

        let command = ConfCommand {
            sui_net: Some(SuiNet::Mainnet),
            sui_wallet_path: Some(PathBuf::from("/tmp/.nexus/wallet")),
            nexus_workflow_id,
            nexus_primitives_id,
            nexus_tool_registry_id,
            conf_path: path.clone(),
        };

        // Command saves values.
        let result = handle(command).await;

        assert_matches!(result, Ok(()));

        // Check that file was written to `/tmp/.nexus/conf.toml` with the correct contents.
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();

        assert_eq!(conf.sui.net, SuiNet::Mainnet);
        assert_eq!(conf.sui.wallet_path, PathBuf::from("/tmp/.nexus/wallet"));
        assert_eq!(conf.nexus.workflow_id, nexus_workflow_id);
        assert_eq!(conf.nexus.primitives_id, nexus_primitives_id);
        assert_eq!(conf.nexus.tool_registry_id, nexus_tool_registry_id);

        // Overriding one value will save that one value and leave other values intact.
        let command = ConfCommand {
            sui_net: Some(SuiNet::Testnet),
            sui_wallet_path: None,
            nexus_workflow_id: None,
            nexus_primitives_id: None,
            nexus_tool_registry_id: None,
            conf_path: path.clone(),
        };

        let result = handle(command).await;

        assert_matches!(result, Ok(()));

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();

        assert_eq!(conf.sui.net, SuiNet::Testnet);
        assert_eq!(conf.sui.wallet_path, PathBuf::from("/tmp/.nexus/wallet"));
        assert_eq!(conf.nexus.workflow_id, nexus_workflow_id);
        assert_eq!(conf.nexus.primitives_id, nexus_primitives_id);
        assert_eq!(conf.nexus.tool_registry_id, nexus_tool_registry_id);

        // Remove any leftover artifacts.
        tokio::fs::remove_dir_all("/tmp/.nexus").await.unwrap();
    }
}
