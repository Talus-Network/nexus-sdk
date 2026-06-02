use super::*;

pub(crate) async fn publish_skill(
    config_path: PathBuf,
    out: Option<PathBuf>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config_path.clone()).await?;
    let dag_path = resolve_relative(&config_path, config.dag_path.clone());
    let tap_package_path = resolve_relative(&config_path, config.tap_package_path.clone());
    let dag_text = tokio::fs::read_to_string(&dag_path)
        .await
        .map_err(NexusCliError::Io)?;
    let dag: JsonDag = serde_json::from_str(&dag_text).map_err(|e| NexusCliError::Any(e.into()))?;

    command_title!("Publishing TAP skill");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let publish = nexus_client
        .tap()
        .publish_skill(
            &config,
            dag,
            TapPackagePublishOptions {
                package_path: tap_package_path,
                named_address_overrides: vec![(
                    config.tap_package_name.clone(),
                    sui::types::Address::ZERO,
                )],
            },
        )
        .await
        .map_err(NexusCliError::Nexus)?;

    let artifact_json = serde_json::to_string_pretty(&publish.artifact)
        .map_err(|e| NexusCliError::Any(e.into()))?;
    if let Some(out) = out {
        if let Some(parent) = out.parent() {
            create_dir_all(parent).await.map_err(NexusCliError::Io)?;
        }
        let mut file = File::create(&out).await.map_err(NexusCliError::Io)?;
        file.write_all(artifact_json.as_bytes())
            .await
            .map_err(NexusCliError::Io)?;
        notify_success!("Wrote TAP publish artifact to {}", out.display());
    }

    json_output(&publish_skill_result_json(&publish))
}

#[cfg(test)]
mod tests {
    use {super::*, std::ffi::OsString};

    struct EnvGuard {
        home: Option<OsString>,
        rpc: Option<OsString>,
        pk: Option<OsString>,
    }

    impl EnvGuard {
        fn without_sui_credentials(path: &std::path::Path) -> Self {
            let guard = Self {
                home: std::env::var_os("HOME"),
                rpc: std::env::var_os("SUI_RPC_URL"),
                pk: std::env::var_os("SUI_PK"),
            };
            std::env::set_var("HOME", path);
            std::env::remove_var("SUI_RPC_URL");
            std::env::remove_var("SUI_PK");
            guard
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.home.take() {
                Some(value) => std::env::set_var("HOME", value),
                None => std::env::remove_var("HOME"),
            }
            match self.rpc.take() {
                Some(value) => std::env::set_var("SUI_RPC_URL", value),
                None => std::env::remove_var("SUI_RPC_URL"),
            }
            match self.pk.take() {
                Some(value) => std::env::set_var("SUI_PK", value),
                None => std::env::remove_var("SUI_PK"),
            }
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn publish_skill_validates_local_inputs_before_rpc_client() {
        let temp_home = tempfile::tempdir().expect("temp home");
        let _env = EnvGuard::without_sui_credentials(temp_home.path());
        let tempdir = tempfile::tempdir().expect("tempdir");
        scaffold_tap_skill("weather skill".to_string(), tempdir.path().to_path_buf())
            .await
            .expect("scaffold succeeds");

        let error = publish_skill(
            tempdir.path().join("weather-skill/skill.tap.json"),
            None,
            None,
            DEFAULT_GAS_BUDGET,
        )
        .await
        .expect_err("missing RPC should fail after local validation");

        assert!(
            error.to_string().contains("Sui RPC URL is not configured"),
            "unexpected error: {error}"
        );
    }
}
