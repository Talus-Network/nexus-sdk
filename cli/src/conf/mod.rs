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
        long = "nexus.workflow-pkg-id",
        help = "Set the Nexus Workflow package ID",
        value_name = "PKG_ID"
    )]
    nexus_workflow_pkg_id: Option<sui::ObjectID>,

    #[arg(
        long = "nexus.primitives-pkg-id",
        help = "Set the Nexus Primitives package ID",
        value_name = "PKG_ID"
    )]
    nexus_primitives_pkg_id: Option<sui::ObjectID>,

    #[arg(
        long = "nexus.tool-registry-object-id",
        help = "Set the Nexus Tool Registry object ID",
        value_name = "OBJECT_ID"
    )]
    nexus_tool_registry_object_id: Option<sui::ObjectID>,

    #[arg(
        long = "nexus.default-sap-object-id",
        help = "Set the Nexus Default SAP object ID",
        value_name = "OBJECT_ID"
    )]
    nexus_default_sap_object_id: Option<sui::ObjectID>,

    #[arg(
        long = "nexus.network_id",
        help = "Set the Nexus Network ID",
        value_name = "OBJECT_ID"
    )]
    nexus_network_id: Option<sui::ObjectID>,

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

/// Handle the provided conf command. The [ConfCommand] instance is passed from
/// [crate::main].
pub(crate) async fn handle(
    ConfCommand {
        sui_net,
        sui_wallet_path,
        sui_auth_user,
        sui_auth_password,
        nexus_objects_path,
        nexus_workflow_pkg_id,
        nexus_primitives_pkg_id,
        nexus_tool_registry_object_id,
        nexus_default_sap_object_id,
        nexus_network_id,
        conf_path,
    }: ConfCommand,
) -> AnyResult<(), NexusCliError> {
    let mut conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_else(|_| CliConf::default());

    // If all fields are None, display the current configuration.
    if sui_net.is_none()
        && sui_wallet_path.is_none()
        && nexus_workflow_pkg_id.is_none()
        && nexus_primitives_pkg_id.is_none()
        && nexus_tool_registry_object_id.is_none()
        && nexus_default_sap_object_id.is_none()
        && nexus_network_id.is_none()
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

        conf.nexus.workflow_pkg_id = nexus_workflow_pkg_id.or(Some(objects.workflow_pkg_id));
        conf.nexus.primitives_pkg_id = nexus_primitives_pkg_id.or(Some(objects.primitives_pkg_id));
        conf.nexus.tool_registry_object_id =
            nexus_tool_registry_object_id.or(Some(objects.tool_registry_object_id));
        conf.nexus.default_sap_object_id =
            nexus_default_sap_object_id.or(Some(objects.default_sap_object_id));
        conf.nexus.network_id = nexus_network_id.or(Some(objects.network_id));
    }

    conf.sui.net = sui_net.unwrap_or(conf.sui.net);
    conf.sui.wallet_path = resolve_wallet_path(sui_wallet_path, &conf.sui)?;
    conf.sui.auth_user = sui_auth_user.or(conf.sui.auth_user);
    conf.sui.auth_password = sui_auth_password.or(conf.sui.auth_password);
    conf.nexus.workflow_pkg_id = nexus_workflow_pkg_id.or(conf.nexus.workflow_pkg_id);
    conf.nexus.primitives_pkg_id = nexus_primitives_pkg_id.or(conf.nexus.primitives_pkg_id);
    conf.nexus.tool_registry_object_id =
        nexus_tool_registry_object_id.or(conf.nexus.tool_registry_object_id);
    conf.nexus.default_sap_object_id =
        nexus_default_sap_object_id.or(conf.nexus.default_sap_object_id);
    conf.nexus.network_id = nexus_network_id.or(conf.nexus.network_id);

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
    use {super::*, assert_fs::prelude::*, assert_matches::assert_matches};

    #[tokio::test]
    async fn test_conf_loads_and_saves() {
        let temp = assert_fs::TempDir::new().expect("Failed to create temp dir");
        let conf_path = temp.child("conf.toml");
        let objects_path = temp.child("objects.toml");
        let wallet_path = temp.child("wallet");

        let nexus_workflow_pkg_id = Some(sui::ObjectID::random());
        let nexus_primitives_pkg_id = Some(sui::ObjectID::random());
        let nexus_tool_registry_object_id = Some(sui::ObjectID::random());
        let nexus_default_sap_object_id = Some(sui::ObjectID::random());
        let nexus_network_id = Some(sui::ObjectID::random());

        let nexus_objects_instance = NexusObjects {
            workflow_pkg_id: nexus_workflow_pkg_id.unwrap(),
            primitives_pkg_id: nexus_primitives_pkg_id.unwrap(),
            tool_registry_object_id: nexus_tool_registry_object_id.unwrap(),
            default_sap_object_id: nexus_default_sap_object_id.unwrap(),
            network_id: nexus_network_id.unwrap(),
        };

        // Serialize the NexusObjects instance to a TOML string.
        let toml_str = toml::to_string(&nexus_objects_instance)
            .expect("Failed to serialize NexusObjects to TOML");

        // Write the TOML string to the objects.toml file.
        objects_path
            .write_str(&toml_str)
            .expect("Failed to write objects.toml");

        let command = ConfCommand {
            sui_net: Some(SuiNet::Mainnet),
            sui_wallet_path: Some(wallet_path.to_path_buf()),
            sui_auth_user: Some("user".to_string()),
            sui_auth_password: Some("pass".to_string()),
            nexus_objects_path: Some(objects_path.to_path_buf()),
            nexus_workflow_pkg_id,
            nexus_primitives_pkg_id,
            nexus_tool_registry_object_id,
            nexus_default_sap_object_id,
            nexus_network_id,
            conf_path: conf_path.to_path_buf(),
        };

        // Command saves values.
        let result = handle(command).await;

        assert_matches!(result, Ok(()));

        // Check that file was written to `/tmp/.nexus/conf.toml` with the correct contents.
        let contents = tokio::fs::read_to_string(&conf_path.path()).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();

        assert_eq!(conf.sui.net, SuiNet::Mainnet);
        assert_eq!(conf.sui.wallet_path, wallet_path.to_path_buf());
        assert_eq!(conf.sui.auth_user, Some("user".to_string()));
        assert_eq!(conf.sui.auth_password, Some("pass".to_string()));
        assert_eq!(conf.nexus.workflow_pkg_id, nexus_workflow_pkg_id);
        assert_eq!(conf.nexus.primitives_pkg_id, nexus_primitives_pkg_id);
        assert_eq!(
            conf.nexus.tool_registry_object_id,
            nexus_tool_registry_object_id
        );
        assert_eq!(
            conf.nexus.default_sap_object_id,
            nexus_default_sap_object_id
        );
        assert_eq!(conf.nexus.network_id, nexus_network_id);

        // Overriding one value will save that one value and leave other values intact.
        let command = ConfCommand {
            sui_net: Some(SuiNet::Testnet),
            sui_wallet_path: None,
            sui_auth_user: None,
            sui_auth_password: None,
            nexus_objects_path: None,
            nexus_workflow_pkg_id: None,
            nexus_primitives_pkg_id: None,
            nexus_tool_registry_object_id: None,
            nexus_default_sap_object_id: None,
            nexus_network_id: None,
            conf_path: conf_path.to_path_buf(),
        };

        let result = handle(command).await;

        assert_matches!(result, Ok(()));

        let contents = tokio::fs::read_to_string(conf_path.path()).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();

        assert_eq!(conf.sui.net, SuiNet::Testnet);
        assert_eq!(conf.sui.wallet_path, wallet_path.to_path_buf());
        assert_eq!(conf.sui.auth_user, Some("user".to_string()));
        assert_eq!(conf.sui.auth_password, Some("pass".to_string()));
        assert_eq!(conf.nexus.workflow_pkg_id, nexus_workflow_pkg_id);
        assert_eq!(conf.nexus.primitives_pkg_id, nexus_primitives_pkg_id);
        assert_eq!(
            conf.nexus.tool_registry_object_id,
            nexus_tool_registry_object_id
        );
        assert_eq!(
            conf.nexus.default_sap_object_id,
            nexus_default_sap_object_id
        );
        assert_eq!(conf.nexus.network_id, nexus_network_id);

        // Remove any leftover artifacts.
        temp.close().expect("Failed to close temp dir");
    }
}
