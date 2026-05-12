use super::*;

pub(crate) async fn dry_run_skill(config: PathBuf) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config, None).await?;
    let package_id = sui::types::Address::ZERO;
    let digest = config
        .digest_input(package_id)
        .digest_hex()
        .map_err(NexusCliError::Any)?;
    json_output(&json!({
        "dry_run": true,
        "valid": true,
        "skill_name": config.name,
        "interface_revision": config.interface_revision,
        "config_digest_hex_with_zero_package": digest,
        "next_step": "publish TAP plus DAG, then create-agent and register-skill"
    }))
}
