use crate::{command_title, prelude::*};

/// Print the current Nexus CLI configuration.
pub(crate) async fn get_nexus_conf(conf_path: PathBuf) -> AnyResult<CliConf, NexusCliError> {
    let conf = CliConf::load_from_path(&conf_path).await.map_err(|e| {
        NexusCliError::Any(anyhow!(
            "Failed to load Nexus CLI configuration from {}: {e}",
            conf_path.display(),
        ))
    })?;

    command_title!("Current Nexus CLI Configuration");

    Ok(conf)
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::test_utils::sui_mocks};

    #[tokio::test]
    async fn test_get_nexus_conf() {
        let mut rng = rand::thread_rng();
        let tempdir = tempfile::tempdir().unwrap().keep();
        let path = tempdir.join("conf.toml");

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_objects = NexusObjects {
            workflow_pkg_id: sui::types::Address::generate(&mut rng),
            primitives_pkg_id: sui::types::Address::generate(&mut rng),
            interface_pkg_id: sui::types::Address::generate(&mut rng),
            network_id: sui::types::Address::generate(&mut rng),
            tool_registry: sui_mocks::mock_sui_object_ref(),
            network_auth: sui_mocks::mock_sui_object_ref(),
            default_tap: sui_mocks::mock_sui_object_ref(),
            gas_service: sui_mocks::mock_sui_object_ref(),
            pre_key_vault: sui_mocks::mock_sui_object_ref(),
        };

        let sui_conf = SuiConf {
            pk: Some("123".to_string().into()),
            rpc_url: Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap()),
            gql_url: Some(reqwest::Url::parse("https://mainnet.sui.io/graphql").unwrap()),
        };

        let tools = HashMap::new();

        let data_storage_conf = DataStorageConf {
            walrus_aggregator_url: None,
            walrus_publisher_url: None,
            walrus_save_for_epochs: None,
            preferred_remote_storage: None,
        };

        let conf = CliConf {
            sui: sui_conf.clone(),
            nexus: Some(nexus_objects.clone()),
            tools: tools.clone(),
            data_storage: data_storage_conf.clone(),
        };

        // Write the configuration to the file.
        let toml_str = toml::to_string(&conf).expect("Failed to serialize NexusObjects to TOML");

        tokio::fs::write(&path, toml_str)
            .await
            .expect("Failed to write conf.toml");

        // Ensure the command returns the correct string.
        let result = get_nexus_conf(path).await.expect("Failed to print config");

        assert_eq!(result, conf);

        // Test loading config without crypto field
        let conf_without_crypto = CliConf {
            sui: sui_conf.clone(),
            nexus: Some(nexus_objects.clone()),
            tools: tools.clone(),
            ..Default::default()
        };

        let path_no_crypto = tempdir.join("conf_no_crypto.toml");
        let toml_str_no_crypto = toml::to_string(&conf_without_crypto)
            .expect("Failed to serialize config without crypto to TOML");
        tokio::fs::write(&path_no_crypto, toml_str_no_crypto)
            .await
            .expect("Failed to write conf_no_crypto.toml");

        let result_no_crypto = get_nexus_conf(path_no_crypto)
            .await
            .expect("Failed to load config without crypto");
        assert_eq!(result_no_crypto, conf_without_crypto);
    }
}
