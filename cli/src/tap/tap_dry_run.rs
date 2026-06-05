use super::*;

pub(crate) async fn dry_run_skill(config: PathBuf) -> AnyResult<(), NexusCliError> {
    let config = validate_skill(config, None).await?;
    let digest = config
        .digest_input()
        .digest_hex()
        .map_err(NexusCliError::Any)?;
    json_output(&dry_run_result_json(&config, digest))
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
