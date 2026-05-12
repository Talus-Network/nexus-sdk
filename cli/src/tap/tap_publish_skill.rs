use super::*;

pub(crate) async fn publish_skill(
    config_path: PathBuf,
    out: Option<PathBuf>,
    tap_package_override: Option<PathBuf>,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config_path.clone(), tap_package_override).await?;
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
