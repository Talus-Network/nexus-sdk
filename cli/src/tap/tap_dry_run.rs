use super::*;

pub(crate) async fn dry_run_skill(config: PathBuf) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config).await?;
    let digest = config
        .digest_input()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn dry_run_valid_scaffolded_skill_computes_zero_package_digest() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        scaffold_tap_skill("weather skill".to_string(), tempdir.path().to_path_buf())
            .await
            .expect("scaffold succeeds");

        dry_run_skill(tempdir.path().join("weather-skill/skill.tap.json"))
            .await
            .expect("dry-run validates local package");
    }
}
