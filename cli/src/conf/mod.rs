use crate::{command_title, loading, prelude::*, sui::resolve_wallet_path};
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
        long = "sui.basic-auth-user",
        help = "Set an user for basic authentication to the Sui node",
        value_name = "USER",
        requires = "sui.basic-auth-password"
    )]
    sui_auth_user: Option<String>,

    #[arg(
        long = "sui.basic-auth-password",
        help = "Set a password for basic authentication to the Sui node",
        value_name = "PASSWORD",
        requires = "sui.basic-auth-user"
    )]
    sui_auth_password: Option<String>,

    #[arg(
        long = "nexus.objects",
        help = "Path to a TOML file containing Nexus objects",
        value_name = "PATH",
        value_parser = ValueParser::from(expand_tilde)
    )]
    nexus_objects_path: Option<PathBuf>,

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

// TODO: adjust begin_exec.
// TODO: new commands (add budget, set cost, add ticket, claim...)

/// Handle the provided conf command. The [ConfCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(
    ConfCommand {
        sui_net,
        sui_wallet_path,
        sui_auth_user,
        sui_auth_password,
        nexus_objects_path,
        conf_path,
    }: ConfCommand,
) -> AnyResult<(), NexusCliError> {
    let mut conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_else(|_| CliConf::default());

    // If all fields are None, display the current configuration.
    if sui_net.is_none()
        && sui_wallet_path.is_none()
        && sui_auth_user.is_none()
        && sui_auth_password.is_none()
        && nexus_objects_path.is_none()
    {
        command_title!("Current Nexus CLI Configuration");
        println!("{:#?}", conf);
        return Ok(());
    }

    command_title!("Updating Nexus CLI Configuration");
    let conf_handle = loading!("Updating configuration...");

    // If a nexus.objects file is provided, load the file and update configuration.
    if let Some(objects_path) = nexus_objects_path {
        let content = std::fs::read_to_string(&objects_path).map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to read objects file {}: {}",
                objects_path.display(),
                e
            ))
        })?;

        let objects: NexusObjects = toml::from_str(&content).map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to parse objects file {}: {}",
                objects_path.display(),
                e
            ))
        })?;

        conf.nexus = Some(objects);
    }

    conf.sui.net = sui_net.unwrap_or(conf.sui.net);
    conf.sui.wallet_path = resolve_wallet_path(sui_wallet_path, &conf.sui)?;
    conf.sui.auth_user = sui_auth_user.or(conf.sui.auth_user);
    conf.sui.auth_password = sui_auth_password.or(conf.sui.auth_password);

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
    use {
        super::*,
        assert_matches::assert_matches,
        nexus_sdk::test_utils::sui_mocks::mock_sui_object_ref,
    };

    #[tokio::test]
    async fn test_conf_loads_and_saves() {
        let tempdir = tempfile::tempdir().unwrap().into_path();
        let path = tempdir.join("conf.toml");
        let objects_path = tempdir.join("objects.toml");

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_objects_instance = NexusObjects {
            workflow_pkg_id: sui::ObjectID::random(),
            primitives_pkg_id: sui::ObjectID::random(),
            inteface_pkg_id: sui::ObjectID::random(),
            network_id: sui::ObjectID::random(),
            tool_registry: mock_sui_object_ref(),
            default_sap: mock_sui_object_ref(),
            gas_service: mock_sui_object_ref(),
        };

        // Serialize the NexusObjects instance to a TOML string.
        let toml_str = toml::to_string(&nexus_objects_instance)
            .expect("Failed to serialize NexusObjects to TOML");

        // Write the TOML string to the objects.toml file.
        tokio::fs::write(&objects_path, toml_str)
            .await
            .expect("Failed to write objects.toml");

        let command = ConfCommand {
            sui_net: Some(SuiNet::Mainnet),
            sui_wallet_path: Some(tempdir.join("wallet")),
            sui_auth_user: Some("user".to_string()),
            sui_auth_password: Some("pass".to_string()),
            nexus_objects_path: Some(tempdir.join("objects.toml")),
            conf_path: path.clone(),
        };

        // Command saves values.
        let result = handle(command).await;

        assert_matches!(result, Ok(()));

        // Check that file was writte with the correct contents.
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();
        let objects = conf.nexus.unwrap();

        assert_eq!(conf.sui.net, SuiNet::Mainnet);
        assert_eq!(conf.sui.wallet_path, tempdir.join("wallet"));
        assert_eq!(conf.sui.auth_user, Some("user".to_string()));
        assert_eq!(conf.sui.auth_password, Some("pass".to_string()));
        assert_eq!(objects, nexus_objects_instance);

        // Overriding one value will save that one value and leave other values intact.
        let command = ConfCommand {
            sui_net: Some(SuiNet::Testnet),
            sui_wallet_path: None,
            sui_auth_user: None,
            sui_auth_password: None,
            nexus_objects_path: None,
            conf_path: path.clone(),
        };

        let result = handle(command).await;

        assert_matches!(result, Ok(()));

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();
        let objects = conf.nexus.unwrap();

        assert_eq!(conf.sui.net, SuiNet::Testnet);
        assert_eq!(conf.sui.wallet_path, tempdir.join("wallet"));
        assert_eq!(conf.sui.auth_user, Some("user".to_string()));
        assert_eq!(conf.sui.auth_password, Some("pass".to_string()));
        assert_eq!(objects, nexus_objects_instance);
    }
}
