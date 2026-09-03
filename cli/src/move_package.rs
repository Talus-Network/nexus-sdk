//! Move package compilation for command line deployment flows.

use {
    anyhow::Context,
    nexus_sdk::sui::{self, MovePackageArtifact},
    std::path::Path,
};

/// Compiles a Move project into [`MovePackageArtifact`].
pub(crate) fn compile_move_package(
    package_path: &Path,
    named_address_overrides: &[(String, sui::types::Address)],
    environment: Option<String>,
) -> anyhow::Result<MovePackageArtifact> {
    let named_address_overrides = named_address_overrides
        .iter()
        .map(|(name, address)| {
            let address = address
                .to_string()
                .parse()
                .with_context(|| format!("failed to convert named address '{name}'"))?;
            Ok((name.clone(), address))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut build_config =
        sui_move_build::BuildConfig::new_for_testing_replace_addresses(named_address_overrides);
    build_config.config.environment = environment;
    build_config.print_diags_to_stderr = false;
    let package = build_config.build(package_path).with_context(|| {
        format!(
            "failed to compile Move package at '{}'",
            package_path.display()
        )
    })?;
    let dependency_ids = package
        .get_dependency_storage_package_ids()
        .iter()
        .map(|id| {
            id.to_string()
                .parse()
                .with_context(|| format!("failed to convert dependency package ID '{id}'"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(MovePackageArtifact {
        modules: package.package.get_package_bytes(false),
        dependency_ids,
    })
}

#[cfg(test)]
pub(crate) fn compile_move_package_fixture(
    package_path: &Path,
    named_address_overrides: &[(String, sui::types::Address)],
    environment: Option<String>,
) -> anyhow::Result<MovePackageArtifact> {
    fn copy_fixture(source: &Path, destination: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(destination).with_context(|| {
            format!(
                "failed to create Move fixture directory '{}'",
                destination.display()
            )
        })?;
        for entry in std::fs::read_dir(source)
            .with_context(|| format!("failed to read Move fixture '{}'", source.display()))?
        {
            let entry = entry.context("failed to read Move fixture entry")?;
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some("build" | "Move.lock" | "Published.toml")
            ) {
                continue;
            }
            let target = destination.join(&name);
            let kind = entry
                .file_type()
                .with_context(|| format!("failed to inspect Move fixture entry '{name:?}'"))?;
            if kind.is_dir() {
                copy_fixture(&entry.path(), &target)?;
            } else if kind.is_file() {
                std::fs::copy(entry.path(), &target)
                    .with_context(|| format!("failed to copy Move fixture entry '{name:?}'"))?;
            }
        }
        Ok(())
    }

    let directory = tempfile::tempdir().context("failed to create Move fixture directory")?;
    let isolated_package = directory.path().join("package");
    copy_fixture(package_path, &isolated_package)?;
    compile_move_package(&isolated_package, named_address_overrides, environment)
}

#[cfg(test)]
mod tests {
    use {super::*, std::fs, tempfile::tempdir};

    #[test]
    fn compiles_move_package_into_artifact() {
        let package = tempdir().unwrap();
        fs::create_dir(package.path().join("sources")).unwrap();
        fs::write(
            package.path().join("Move.toml"),
            r#"[package]
name = "artifact_test"
version = "0.0.0"
edition = "2024"

[environments]
testnet = "4c78adac"

[addresses]
artifact_test = "0x0"
"#,
        )
        .unwrap();
        fs::write(
            package.path().join("sources/artifact_test.move"),
            "module artifact_test::artifact_test { public fun value(): u64 { 1 } }\n",
        )
        .unwrap();

        let artifact = compile_move_package(
            package.path(),
            &[(
                "artifact_test".to_string(),
                sui::types::Address::from_static("0x42"),
            )],
            Some("testnet".to_string()),
        )
        .unwrap();

        assert!(!artifact.modules.is_empty());
    }
}
