use {super::*, std::path::Path};

pub(crate) async fn read_skill_config(path: &PathBuf) -> AnyResult<TapSkillConfig, NexusCliError> {
    let text = tokio::fs::read_to_string(path)
        .await
        .map_err(NexusCliError::Io)?;
    serde_json::from_str(&text).map_err(|e| NexusCliError::Any(e.into()))
}

pub(crate) fn resolve_relative(base_file: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    base_file
        .parent()
        .map(|parent| parent.join(&path))
        .unwrap_or(path)
}

pub(crate) fn validate_dag_file(dag_path: &std::path::Path) -> AnyResult<()> {
    let dag_text = std::fs::read_to_string(dag_path)
        .map_err(|error| anyhow!("failed to read DAG file '{}': {error}", dag_path.display()))?;
    let dag: JsonDag = serde_json::from_str(&dag_text)
        .map_err(|error| anyhow!("failed to parse DAG JSON '{}': {error}", dag_path.display()))?;

    dag_validator::validate(dag).map_err(|error| {
        anyhow!(
            "DAG validation failed for '{}': {error}",
            dag_path.display()
        )
    })
}

pub(crate) fn validate_tap_package_manifest(
    manifest_path: &std::path::Path,
    config: &TapSkillConfig,
) -> AnyResult<()> {
    let manifest_text = std::fs::read_to_string(manifest_path).map_err(|error| {
        anyhow!(
            "failed to read TAP package manifest '{}': {error}",
            manifest_path.display()
        )
    })?;
    let manifest: toml::Value = toml::from_str(&manifest_text).map_err(|error| {
        anyhow!(
            "failed to parse TAP package manifest '{}': {error}",
            manifest_path.display()
        )
    })?;

    let package_name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            anyhow!(
                "TAP package manifest '{}' is missing [package].name",
                manifest_path.display()
            )
        })?;
    if package_name != config.tap_package_name {
        bail!(
            "TAP package manifest '{}' has package.name='{}' but skill config expects '{}'",
            manifest_path.display(),
            package_name,
            config.tap_package_name
        );
    }

    let addresses = manifest
        .get("addresses")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| {
            anyhow!(
                "TAP package manifest '{}' is missing [addresses]",
                manifest_path.display()
            )
        })?;
    if !addresses.contains_key(&config.tap_package_name) {
        bail!(
            "TAP package manifest '{}' must define [addresses].{}",
            manifest_path.display(),
            config.tap_package_name
        );
    }

    Ok(())
}

pub(crate) fn collect_move_source_files(root: &std::path::Path) -> AnyResult<Vec<PathBuf>> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();

    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|error| {
            anyhow!(
                "failed to read source directory '{}': {error}",
                dir.display()
            )
        })? {
            let entry = entry.map_err(|error| {
                anyhow!(
                    "failed to inspect TAP package source entry in '{}': {error}",
                    dir.display()
                )
            })?;
            let file_type = entry.file_type().map_err(|error| {
                anyhow!(
                    "failed to read TAP package source entry type '{}': {error}",
                    entry.path().display()
                )
            })?;
            let path = entry.path();

            if file_type.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("move") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

pub(crate) fn validate_tap_package_sources(
    tap_package_path: &std::path::Path,
    config: &TapSkillConfig,
) -> AnyResult<()> {
    let sources_dir = tap_package_path.join("sources");
    if !sources_dir.exists() {
        bail!(
            "TAP package sources directory '{}' does not exist",
            sources_dir.display()
        );
    }

    let source_files = collect_move_source_files(&sources_dir)?;
    if source_files.is_empty() {
        bail!(
            "TAP package '{}' has no Move source files under '{}'",
            config.tap_package_name,
            sources_dir.display()
        );
    }

    let module_pattern = Regex::new(&format!(
        r"(?m)^\s*module\s+{}::[A-Za-z_][A-Za-z0-9_]*\s*;",
        regex::escape(&config.tap_package_name)
    ))?;

    for source_file in &source_files {
        let source_text = std::fs::read_to_string(source_file).map_err(|error| {
            anyhow!(
                "failed to read TAP source file '{}': {error}",
                source_file.display()
            )
        })?;
        if module_pattern.is_match(&source_text) {
            return Ok(());
        }
    }

    bail!(
        "TAP package '{}' has no source file declaring `module {}::...;` under '{}'",
        config.tap_package_name,
        config.tap_package_name,
        sources_dir.display()
    );
}

pub(crate) async fn validate_skill(
    config_path: PathBuf,
    tap_package_override: Option<PathBuf>,
) -> AnyResult<TapSkillConfig, NexusCliError> {
    command_title!("Validating TAP skill config '{}'", config_path.display());

    let handle = loading!("Validating TAP skill config...");
    let mut config = read_skill_config(&config_path).await?;
    if let Some(path) = tap_package_override {
        config.tap_package_path = path;
    }

    config
        .validate()
        .map_err(|e| NexusCliError::Any(anyhow!(e)))?;

    let dag_path = resolve_relative(&config_path, config.dag_path.clone());
    if !dag_path.exists() {
        handle.error();
        return Err(NexusCliError::Any(anyhow!(
            "DAG file '{}' does not exist",
            dag_path.display()
        )));
    }
    if let Err(error) = validate_dag_file(&dag_path) {
        handle.error();
        return Err(NexusCliError::Any(error));
    }

    let tap_package = resolve_relative(&config_path, config.tap_package_path.clone());
    let move_toml = tap_package.join("Move.toml");
    if !move_toml.exists() {
        handle.error();
        return Err(NexusCliError::Any(anyhow!(
            "TAP package Move.toml '{}' does not exist",
            move_toml.display()
        )));
    }
    if let Err(error) = validate_tap_package_manifest(&move_toml, &config) {
        handle.error();
        return Err(NexusCliError::Any(error));
    }
    if let Err(error) = validate_tap_package_sources(&tap_package, &config) {
        handle.error();
        return Err(NexusCliError::Any(error));
    }

    handle.success();
    json_output(&json!({
        "valid": true,
        "skill_name": config.name,
        "tap_package_name": config.tap_package_name,
        "interface_revision": config.interface_revision,
    }))?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_skill_config_reports_json_parse_errors() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let path = tempdir.path().join("skill.tap.json");
        tokio::fs::write(&path, "{")
            .await
            .expect("write invalid JSON");

        let error = read_skill_config(&path)
            .await
            .expect_err("invalid config JSON should fail");

        assert!(
            error.to_string().contains("EOF while parsing"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn resolve_relative_without_parent_uses_input_path() {
        assert_eq!(
            resolve_relative(std::path::Path::new("skill.tap.json"), PathBuf::from("tap")),
            PathBuf::from("tap")
        );
    }
}
