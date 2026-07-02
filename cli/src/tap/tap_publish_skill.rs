use {
    super::*,
    crate::tap::tap_validate_skill::{tap_package_path_for_config, validate_tap_package_manifest},
    nexus_sdk::{dag::json::parse_dag_spec, sui::build::Environment},
};

/// Parse `<package_path>/Move.toml` and pick the `[environments]` entry whose
/// chain id matches the connected RPC's chain id. Returns `Some(Environment)`
/// when a match is found; `None` is impossible here because `validate-skill`
/// has already enforced the presence of an `[environments]` table, but we
/// still surface a clean error if the entries don't match the connected RPC.
fn pick_publish_environment(
    package_path: &std::path::Path,
    chain_id: &str,
) -> AnyResult<Option<Environment>, NexusCliError> {
    let manifest_path = package_path.join("Move.toml");
    let manifest_text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        NexusCliError::Any(anyhow!(
            "failed to read TAP package manifest '{}': {error}",
            manifest_path.display()
        ))
    })?;
    let manifest: toml::Value = toml::from_str(&manifest_text).map_err(|error| {
        NexusCliError::Any(anyhow!(
            "failed to parse TAP package manifest '{}': {error}",
            manifest_path.display()
        ))
    })?;
    let Some(environments) = manifest.get("environments").and_then(toml::Value::as_table) else {
        return Ok(None);
    };
    let matching = environments
        .iter()
        .filter_map(|(name, value)| value.as_str().map(|cid| (name.clone(), cid.to_string())))
        .find(|(_, cid)| cid == chain_id);
    let (alias, _) = matching.ok_or_else(|| {
        let configured = environments
            .iter()
            .filter_map(|(name, value)| value.as_str().map(|cid| format!("{name} = \"{cid}\"")))
            .collect::<Vec<_>>()
            .join(", ");
        NexusCliError::Any(anyhow!(
            "TAP package manifest '{}' has no [environments] entry matching the connected chain \
             id '{chain_id}'. Configured: [{configured}]. Add or update an entry so it maps your \
             target env alias to '{chain_id}'.",
            manifest_path.display()
        ))
    })?;
    Ok(Some(Environment::new(alias, chain_id.to_string())))
}

pub(crate) async fn publish_skill(
    config_path: PathBuf,
    out: Option<PathBuf>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config_path.clone()).await?;
    let dag_path = resolve_relative(&config_path, config.dag_path.clone());
    let tap_package_path = tap_package_path_for_config(&config_path);
    let dag_text = tokio::fs::read_to_string(&dag_path)
        .await
        .map_err(NexusCliError::Io)?;
    let dag = parse_dag_spec(&dag_text).map_err(|error| NexusCliError::Any(error.into()))?;

    command_title!("Publishing TAP skill");
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    // New-style 2024 Sui Move packages resolve each dependency's
    // `Published.toml` via the active build environment. Pick the
    // `[environments]` entry whose chain id matches the connected RPC's
    // chain id so Sui emits the right dep addresses; otherwise the publish
    // tx aborts with `PublishUpgradeMissingDependency`.
    let chain_id = nexus_client
        .crawler()
        .get_chain_id()
        .await
        .map_err(NexusCliError::Any)?;
    let package_name = validate_tap_package_manifest(&tap_package_path.join("Move.toml"))
        .map_err(NexusCliError::Any)?;
    let environment = pick_publish_environment(&tap_package_path, &chain_id)?;
    let publish = nexus_client
        .tap()
        .publish_skill(
            &config,
            dag,
            TapPackagePublishOptions {
                package_path: tap_package_path,
                named_address_overrides: vec![(package_name, sui::types::Address::ZERO)],
                environment: environment.map(|environment| environment.name().clone()),
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
        // `tokio::fs::write` opens, writes the full buffer, and closes (flushing
        // to the OS) before it returns. The previous `File::create` +
        // `write_all` left the data in `tokio::fs::File`'s internal buffer:
        // dropping the file does not flush it, so a reader racing the drop
        // could observe a truncated/empty file (see the flaky
        // `publish_artifact_flow_writes_revision_metadata` test).
        tokio::fs::write(&out, artifact_json.as_bytes())
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
