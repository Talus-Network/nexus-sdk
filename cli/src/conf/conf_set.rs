use {
    crate::{command_title, display::json_output, loading, prelude::*, sui::resolve_wallet_path},
    nexus_sdk::{
        types::StorageKind,
        walrus::{WALRUS_AGGREGATOR_URL, WALRUS_PUBLISHER_URL},
    },
};

/// Set the Nexus CLI configuration from the provided arguments.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn set_nexus_conf(
    sui_net: Option<SuiNet>,
    sui_wallet_path: Option<PathBuf>,
    sui_rpc_url: Option<reqwest::Url>,
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
    conf.sui.rpc_url = sui_rpc_url.or(conf.sui.rpc_url);

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

        assert!(!tokio::fs::try_exists(&path).await.unwrap());

        let nexus_objects_instance = NexusObjects {
            workflow_pkg_id: sui::ObjectID::random(),
            primitives_pkg_id: sui::ObjectID::random(),
            interface_pkg_id: sui::ObjectID::random(),
            network_id: sui::ObjectID::random(),
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
            Some(SuiNet::Mainnet),
            Some(tempdir.join("wallet")),
            Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap()),
            Some(tempdir.join("objects.toml")),
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

        assert_eq!(conf.sui.net, SuiNet::Mainnet);
        assert_eq!(conf.sui.wallet_path, tempdir.join("wallet"));
        assert_eq!(
            conf.sui.rpc_url,
            Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap())
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
            Some(SuiNet::Testnet),
            None,
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

        assert_eq!(conf.sui.net, SuiNet::Testnet);
        assert_eq!(conf.sui.wallet_path, tempdir.join("wallet"));
        assert_eq!(
            conf.sui.rpc_url,
            Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap())
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
            Some(SuiNet::Testnet),
            Some(tempdir.join("wallet")),
            Some(reqwest::Url::parse("https://testnet.sui.io").unwrap()),
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
            Some(SuiNet::Mainnet),
            Some(tempdir.join("wallet")),
            Some(reqwest::Url::parse("https://mainnet.sui.io").unwrap()),
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
