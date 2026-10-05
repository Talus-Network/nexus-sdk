//! Local persistence for SDK references and upload recovery.

use {
    crate::{display::json_output, prelude::*},
    anyhow::{ensure, Context as _},
    nexus_sdk::walrus::WalrusReference,
    std::{io::Write as _, path::Path},
    tokio::io::AsyncReadExt as _,
};

pub(crate) async fn load(path: &Path) -> AnyResult<WalrusReference> {
    let bytes = read_bounded(path, 256 * 1024).await?;
    let reference: WalrusReference =
        serde_json::from_slice(&bytes).context("invalid Walrus reference")?;
    reference.nexus_data()?;
    Ok(reference)
}

pub(crate) fn save(reference: &WalrusReference, path: &Path, replace: bool) -> AnyResult<()> {
    reference.nexus_data()?;
    atomic_write(path, &serde_json::to_vec_pretty(reference)?, replace)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> AnyResult<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    if replace {
        file.persist(path).map_err(|error| error.error)?;
    } else {
        file.persist_noclobber(path).map_err(|error| error.error)?;
    }
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Journal {
    pub(crate) registration: nexus_sdk::walrus::UploadRegistration,
    pub(crate) pending: Option<nexus_sdk::walrus::PendingUpload>,
    pub(crate) stored: Option<nexus_sdk::walrus::StoredBlob>,
}

impl Journal {
    pub(crate) fn save(&self, path: &Path, replace: bool) -> AnyResult<()> {
        atomic_write(path, &serde_json::to_vec_pretty(self)?, replace)
    }

    pub(crate) async fn load(path: &Path) -> AnyResult<Self> {
        let bytes = read_bounded(path, 1024 * 1024).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub(crate) fn check(&self, bytes: &[u8]) -> AnyResult<()> {
        use sha2::{Digest as _, Sha256};
        ensure!(
            self.registration.size == bytes.len()
                && self.registration.sha256 == hex::encode(Sha256::digest(bytes)),
            "upload recovery contents differ from the original file"
        );
        Ok(())
    }
}

pub(crate) async fn read_bounded(path: &Path, max: usize) -> AnyResult<Vec<u8>> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("cannot read {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(max as u64 + 1).read_to_end(&mut bytes).await?;
    ensure!(
        bytes.len() <= max,
        "{} exceeds the {max} byte limit",
        path.display()
    );
    Ok(bytes)
}

/// Adds local artifact paths to the normal task receipt for machine consumers.
pub(crate) fn print_task_receipt(
    receipt: &impl Serialize,
    references: &std::collections::BTreeMap<String, PathBuf>,
) -> Result<(), NexusCliError> {
    let mut value =
        serde_json::to_value(receipt).map_err(|error| NexusCliError::Any(error.into()))?;
    if !references.is_empty() {
        value["walrus_references"] =
            serde_json::to_value(references).map_err(|error| NexusCliError::Any(error.into()))?;
    }
    json_output(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_writes_preserve_existing_references_unless_replacing_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reference.json");
        atomic_write(&path, b"original", false).unwrap();
        assert!(atomic_write(&path, b"replacement", false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        atomic_write(&path, b"replacement", true).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    }
}
