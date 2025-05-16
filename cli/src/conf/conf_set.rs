use crate::{command_title, display::json_output, loading, prelude::*, sui::resolve_wallet_path};

/// Set the Nexus CLI configuration from the provided arguments.
pub(crate) async fn set_nexus_conf(
    sui_net: Option<SuiNet>,
    sui_wallet_path: Option<PathBuf>,
    sui_auth_user: Option<String>,
    sui_auth_password: Option<String>,
    nexus_objects_path: Option<PathBuf>,
    conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    let mut conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_default();

    command_title!("Updating Nexus CLI Configuration");

    if sui_auth_user.is_some() != sui_auth_password.is_some() {
        return Err(NexusCliError::Any(anyhow!(
            "Both --sui.basic-auth-user and --sui.basic-auth-password must be provided together."
        )));
    }

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

    json_output(&serde_json::to_value(&conf).unwrap())?;

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
    use {super::*, assert_matches::assert_matches, nexus_sdk::test_utils::sui_mocks};

    #[tokio::test]
    async fn test_conf_loads_and_saves() {
        let tempdir = tempfile::tempdir().unwrap().into_path();
        let path = tempdir.join("conf.toml");
        let objects_path = tempdir.join("objects.toml");

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_objects_instance = NexusObjects {
            workflow_pkg_id: sui::ObjectID::random(),
            primitives_pkg_id: sui::ObjectID::random(),
            interface_pkg_id: sui::ObjectID::random(),
            network_id: sui::ObjectID::random(),
            tool_registry: sui_mocks::mock_sui_object_ref(),
            default_sap: sui_mocks::mock_sui_object_ref(),
            gas_service: sui_mocks::mock_sui_object_ref(),
        };

        // Serialize the NexusObjects instance to a TOML string.
        let toml_str = toml::to_string(&nexus_objects_instance)
            .expect("Failed to serialize NexusObjects to TOML");

        // Write the TOML string to the objects.toml file.
        tokio::fs::write(&objects_path, toml_str)
            .await
            .expect("Failed to write objects.toml");

        // Command saves values.
        let result = set_nexus_conf(
            Some(SuiNet::Mainnet),
            Some(tempdir.join("wallet")),
            Some("user".to_string()),
            Some("pass".to_string()),
            Some(tempdir.join("objects.toml")),
            path.clone(),
        )
        .await;

        assert_matches!(result, Ok(()));

        // Check that file was written with the correct contents.
        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        let conf = toml::from_str::<CliConf>(&contents).unwrap();
        let objects = conf.nexus.unwrap();

        assert_eq!(conf.sui.net, SuiNet::Mainnet);
        assert_eq!(conf.sui.wallet_path, tempdir.join("wallet"));
        assert_eq!(conf.sui.auth_user, Some("user".to_string()));
        assert_eq!(conf.sui.auth_password, Some("pass".to_string()));
        assert_eq!(objects, nexus_objects_instance);

        // Overriding one value will save that one value and leave other values intact.
        let result =
            set_nexus_conf(Some(SuiNet::Testnet), None, None, None, None, path.clone()).await;

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
