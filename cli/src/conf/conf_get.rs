use crate::{command_title, prelude::*};

/// Print the current Nexus CLI configuration.
pub(crate) async fn get_nexus_conf(conf_path: PathBuf) -> AnyResult<CliConf, NexusCliError> {
    let conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_default();

    command_title!("Current Nexus CLI Configuration");

    Ok(conf)
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::test_utils::sui_mocks};

    #[tokio::test]
    async fn test_get_nexus_conf() {
        let tempdir = tempfile::tempdir().unwrap().into_path();
        let path = tempdir.join("conf.toml");

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_objects = NexusObjects {
            workflow_pkg_id: sui::ObjectID::random(),
            primitives_pkg_id: sui::ObjectID::random(),
            interface_pkg_id: sui::ObjectID::random(),
            network_id: sui::ObjectID::random(),
            tool_registry: sui_mocks::mock_sui_object_ref(),
            default_sap: sui_mocks::mock_sui_object_ref(),
            gas_service: sui_mocks::mock_sui_object_ref(),
        };

        let sui_conf = SuiConf {
            net: SuiNet::Mainnet,
            wallet_path: tempdir.join("wallet"),
            auth_user: Some("user".to_string()),
            auth_password: Some("pass".to_string()),
        };

        let tools = HashMap::new();

        let conf = CliConf {
            sui: sui_conf,
            nexus: Some(nexus_objects),
            tools,
        };

        // Write the configuration to the file.
        let toml_str = toml::to_string(&conf).expect("Failed to serialize NexusObjects to TOML");

        tokio::fs::write(&path, toml_str)
            .await
            .expect("Failed to write conf.toml");

        // Ensure the command returns the correct string.
        let result = get_nexus_conf(path).await.expect("Failed to print config");

        assert_eq!(result, conf);
    }
}
