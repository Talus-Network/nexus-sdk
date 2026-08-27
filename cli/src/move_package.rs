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
