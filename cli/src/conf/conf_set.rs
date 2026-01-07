use {
    crate::{command_title, display::json_output, loading, prelude::*},
    nexus_sdk::{
        types::StorageKind,
        walrus::{WALRUS_AGGREGATOR_URL, WALRUS_PUBLISHER_URL},
    },
};

/// Set the Nexus CLI configuration from the provided arguments.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn set_nexus_conf(
    sui_pk: Option<String>,
    sui_rpc_url: Option<reqwest::Url>,
    sui_gql_url: Option<reqwest::Url>,
    nexus_objects_path: Option<PathBuf>,
    data_storage_walrus_aggregator_url: Option<reqwest::Url>,
    data_storage_walrus_publisher_url: Option<reqwest::Url>,
    data_storage_walrus_save_for_epochs: Option<u8>,
    data_storage_preferred_remote_storage: Option<StorageKind>,
    data_storage_testnet: bool,
    conf_path: PathBuf,
) -> AnyResult<(), NexusCliError> {
    let mut conf = CliConf::load_from_path(&conf_path)
        .await
        .unwrap_or_default();

    command_title!("Updating Nexus CLI Configuration");
    let conf_handle = loading!("Updating configuration...");

    // If a nexus.objects file is provided, load the file and update configuration.
    if let Some(objects_path) = nexus_objects_path {
        let content = std::fs::read_to_string(&objects_path).map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to read objects file {}: {e}",
                objects_path.display(),
            ))
        })?;

        let objects: NexusObjects = toml::from_str(&content).map_err(|e| {
            NexusCliError::Any(anyhow!(
                "Failed to parse objects file {}: {e}",
                objects_path.display(),
            ))
        })?;

        conf.nexus = Some(objects);
    }

    conf.sui.pk = sui_pk.or(conf.sui.pk);
    conf.sui.rpc_url = sui_rpc_url.or(conf.sui.rpc_url);
    conf.sui.gql_url = sui_gql_url.or(conf.sui.gql_url);

    // Preferred remote storage cannot be inline.
    if matches!(
        data_storage_preferred_remote_storage,
        Some(StorageKind::Inline)
    ) {
        conf_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "Preferred remote storage cannot be 'Inline'"
        )));
    }

    conf.data_storage.walrus_aggregator_url =
        data_storage_walrus_aggregator_url.or(conf.data_storage.walrus_aggregator_url);
    conf.data_storage.walrus_publisher_url =
        data_storage_walrus_publisher_url.or(conf.data_storage.walrus_publisher_url);
    conf.data_storage.walrus_save_for_epochs =
        data_storage_walrus_save_for_epochs.or(conf.data_storage.walrus_save_for_epochs);
    conf.data_storage.preferred_remote_storage =
        data_storage_preferred_remote_storage.or(conf.data_storage.preferred_remote_storage);

    if data_storage_testnet {
        conf.data_storage = DataStorageConf {
            walrus_aggregator_url: Some(WALRUS_AGGREGATOR_URL.parse().expect("valid URL")),
            walrus_publisher_url: Some(WALRUS_PUBLISHER_URL.parse().expect("valid URL")),
            walrus_save_for_epochs: Some(2),
            preferred_remote_storage: Some(StorageKind::Walrus),
        };
    }

    json_output(&serde_json::to_value(&conf).unwrap())?;

    match conf.save_to_path(&conf_path).await {
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
        let tempdir = tempfile::tempdir().unwrap().keep();
        let path = tempdir.join("conf.toml");
        let objects_path = tempdir.join("objects.toml");
        let mut rng = rand::thread_rng();

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_objects_instance = NexusObjects {
            workflow_pkg_id: sui::types::Address::generate(&mut rng),
            primitives_pkg_id: sui::types::Address::generate(&mut rng),
            interface_pkg_id: sui::types::Address::generate(&mut rng),
            network_id: sui::types::Address::generate(&mut rng),
            tool_registry: sui_mocks::mock_sui_object_ref(),
            default_tap: sui_mocks::mock_sui_object_ref(),
            gas_service: sui_mocks::mock_sui_object_ref(),
            pre_key_vault: sui_mocks::mock_sui_object_ref(),
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
            Some("123".to_string()),
            Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap()),
            Some(reqwest::Url::parse("https://mainnet.sui.io/graphql").unwrap()),
            Some(objects_path),
            Some(reqwest::Url::parse("https://aggregator.url").unwrap()),
            Some(reqwest::Url::parse("https://publisher.url").unwrap()),
            Some(42),
            Some(StorageKind::Walrus),
            false,
            path.clone(),
        )
        .await;

        assert_matches!(result, Ok(()));

        // Check that file was written with the correct contents.
        let conf = CliConf::load_from_path(&path).await.unwrap();
        let objects = conf.nexus.unwrap();

        assert_eq!(conf.sui.pk, Some("123".to_string()));
        assert_eq!(
            conf.sui.rpc_url,
            Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap())
        );
        assert_eq!(
            conf.sui.gql_url,
            Some(reqwest::Url::parse("https://mainnet.sui.io/graphql").unwrap())
        );
        assert_eq!(objects, nexus_objects_instance);
        assert_eq!(
            conf.data_storage.walrus_aggregator_url,
            Some(reqwest::Url::parse("https://aggregator.url").unwrap())
        );
        assert_eq!(
            conf.data_storage.walrus_publisher_url,
            Some(reqwest::Url::parse("https://publisher.url").unwrap())
        );
        assert_eq!(conf.data_storage.walrus_save_for_epochs, Some(42));
        assert_eq!(
            conf.data_storage.preferred_remote_storage,
            Some(StorageKind::Walrus)
        );

        // Overriding one value will save that one value and leave other values intact.
        let result = set_nexus_conf(
            None,
            Some(reqwest::Url::parse("https://testnet.sui.io").unwrap()),
            None,
            None,
            None,
            None,
            None,
            None,
            false,
            path.clone(),
        )
        .await;

        assert_matches!(result, Ok(()));

        let conf = CliConf::load_from_path(&path).await.unwrap();
        let objects = conf.nexus.unwrap();

        assert_eq!(conf.sui.pk, Some("123".to_string()));
        assert_eq!(
            conf.sui.rpc_url,
            Some(reqwest::Url::parse("https://testnet.sui.io").unwrap())
        );
        assert_eq!(
            conf.sui.gql_url,
            Some(reqwest::Url::parse("https://mainnet.sui.io/graphql").unwrap())
        );
        assert_eq!(objects, nexus_objects_instance);
        assert_eq!(
            conf.data_storage.walrus_aggregator_url,
            Some(reqwest::Url::parse("https://aggregator.url").unwrap())
        );
        assert_eq!(
            conf.data_storage.walrus_publisher_url,
            Some(reqwest::Url::parse("https://publisher.url").unwrap())
        );
        assert_eq!(conf.data_storage.walrus_save_for_epochs, Some(42));
        assert_eq!(
            conf.data_storage.preferred_remote_storage,
            Some(StorageKind::Walrus)
        );
    }

    #[tokio::test]
    async fn test_data_storage_testnet_preset() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let path = tempdir.join("conf_testnet.toml");

        // Run with data_storage_testnet = true
        let result = set_nexus_conf(
            None,
            Some(reqwest::Url::parse("https://testnet.sui.io").unwrap()),
            Some(reqwest::Url::parse("https://testnet.sui.io/graphql").unwrap()),
            None,
            None,
            None,
            None,
            None,
            true,
            path.clone(),
        )
        .await;

        assert_matches!(result, Ok(()));

        let conf = CliConf::load_from_path(&path).await.unwrap();
        assert_eq!(
            conf.data_storage.walrus_aggregator_url,
            Some(WALRUS_AGGREGATOR_URL.parse().unwrap())
        );
        assert_eq!(
            conf.data_storage.walrus_publisher_url,
            Some(WALRUS_PUBLISHER_URL.parse().unwrap())
        );
        assert_eq!(conf.data_storage.walrus_save_for_epochs, Some(2));
        assert_eq!(
            conf.data_storage.preferred_remote_storage,
            Some(StorageKind::Walrus)
        );
    }

    #[tokio::test]
    async fn test_inline_preferred_storage_error() {
        let tempdir = tempfile::tempdir().unwrap().keep();
        let path = tempdir.join("conf_inline.toml");

        let result = set_nexus_conf(
            None,
            Some(reqwest::Url::parse("https://testnet.sui.io").unwrap()),
            Some(reqwest::Url::parse("https://testnet.sui.io/graphql").unwrap()),
            None,
            None,
            None,
            None,
            Some(StorageKind::Inline),
            false,
            path.clone(),
        )
        .await;

        assert_matches!(result, Err(NexusCliError::Any(_)));
    }
}
