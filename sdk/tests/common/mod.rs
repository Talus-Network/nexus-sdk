//! Shared Move package compilation for SDK tests.

use {
    anyhow::Context,
    nexus_sdk::sui::{self, MovePackageArtifact},
    std::{
        env,
        path::{Path, PathBuf},
        sync::OnceLock,
    },
    tempfile::{Builder, TempDir},
};

fn test_artifact_temp_root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-temp");
    std::fs::create_dir_all(&root).expect("failed to create test temp root");
    ensure_test_move_home(&root);
    root
}

fn ensure_test_move_home(root: &Path) {
    static MOVE_HOME: OnceLock<PathBuf> = OnceLock::new();

    let move_home = MOVE_HOME.get_or_init(|| {
        env::var_os("MOVE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                let move_home = root.join(".move");
                std::fs::create_dir_all(&move_home).expect("failed to create test Move home");
                env::set_var("MOVE_HOME", &move_home);
                move_home
            })
    });
    std::fs::create_dir_all(move_home).expect("failed to create test Move home");
}

fn build_tempdir() -> TempDir {
    Builder::new()
        .prefix("nexus-move-package-")
        .tempdir_in(test_artifact_temp_root())
        .expect("failed to create test directory")
}

fn copy_dir_recursive(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("failed to create destination directory");

    for entry in std::fs::read_dir(source).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let file_type = entry.file_type().expect("failed to read entry type");
        let file_name = entry.file_name();

        if file_type.is_dir() && file_name == "build" {
            continue;
        }

        let destination_path = destination.join(&file_name);
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &destination_path);
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), destination_path).expect("failed to copy file");
        }
    }
}

pub(crate) fn compile_move_package(package_path: &Path) -> anyhow::Result<MovePackageArtifact> {
    let temp_package_root = build_tempdir();
    let install_dir = temp_package_root.path().join("package");
    copy_dir_recursive(package_path, &install_dir);
    let _ = std::fs::remove_file(install_dir.join("Move.lock"));
    let _ = std::fs::remove_file(install_dir.join("Published.toml"));

    let mut build_config = sui_move_build::BuildConfig::new_for_testing();
    build_config.config.environment = Some("testnet".to_string());
    build_config.print_diags_to_stderr = false;
    let package = build_config.build(&install_dir)?;
    let dependency_ids = package
        .get_dependency_storage_package_ids()
        .iter()
        .map(|id| {
            id.to_string()
                .parse::<sui::types::Address>()
                .with_context(|| format!("failed to convert dependency package ID '{id}'"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(MovePackageArtifact {
        modules: package.package.get_package_bytes(false),
        dependency_ids,
    })
}
