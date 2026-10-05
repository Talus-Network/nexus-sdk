//! Local persistence for SDK references and upload recovery.

use {
    crate::prelude::*,
    anyhow::{ensure, Context as _},
    nexus_sdk::walrus::WalrusReference,
    std::{io::Write as _, path::Path},
};

pub(crate) async fn load(path: &Path) -> AnyResult<WalrusReference> {
    let bytes = super::read_bounded(path, 256 * 1024).await?;
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
        let bytes = super::read_bounded(path, 1024 * 1024).await?;
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
