//! Validated payloads prepared before any storage payment.

use {
    crate::{
        execution_limits::MAX_RESOLVED_INPUT_BYTES,
        types::{NexusData, NexusValue},
    },
    anyhow::{ensure, Context as _},
    serde_json::Value,
};

/// The bytes and cardinality of one remote Nexus data port.
#[derive(Clone, Debug)]
pub struct WalrusUploadData {
    values: Vec<Vec<u8>>,
    many: bool,
}

impl WalrusUploadData {
    pub fn new(values: Vec<Vec<u8>>, many: bool) -> anyhow::Result<Self> {
        let data = Self { values, many };
        data.preflight_reference()?;
        let total = data
            .values
            .iter()
            .try_fold(0usize, |total, value| total.checked_add(value.len()))
            .context("upload size overflow")?;
        ensure!(
            total <= MAX_RESOLVED_INPUT_BYTES,
            "remote input exceeds the execution byte limit"
        );
        Ok(data)
    }

    /// Keeps the exact document bytes for One; Many encodes each JSON array item.
    pub fn from_json_document(bytes: Vec<u8>, many: bool) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() <= MAX_RESOLVED_INPUT_BYTES,
            "JSON document exceeds the execution byte limit"
        );
        let value: Value =
            serde_json::from_slice(&bytes).context("task data must be a JSON document")?;
        let values = if many {
            value
                .as_array()
                .context("Many data requires a JSON array")?
                .iter()
                .map(serde_json::to_vec)
                .collect::<Result<_, _>>()?
        } else {
            vec![bytes]
        };
        Self::new(values, many)
    }

    pub fn values(&self) -> &[Vec<u8>] {
        &self.values
    }

    pub fn is_many(&self) -> bool {
        self.many
    }

    pub fn byte_len(&self) -> usize {
        self.values.iter().map(Vec::len).sum()
    }

    /// Checks that an uploader returned commitments for exactly these payloads.
    pub fn verify_reference(&self, reference: &NexusData) -> anyhow::Result<()> {
        use sha2::{Digest as _, Sha256};
        ensure!(
            reference.is_well_formed(),
            "upload returned a malformed reference"
        );
        let values = reference.values()?;
        ensure!(
            reference.is_many() == self.many && values.len() == self.values.len(),
            "upload changed the port cardinality"
        );
        for (value, bytes) in values.iter().zip(&self.values) {
            let NexusValue::WalrusData { content_digest, .. } = value else {
                anyhow::bail!("upload must return Walrus references")
            };
            ensure!(
                content_digest.as_slice() == &Sha256::digest(bytes)[..],
                "upload reference commits different content"
            );
        }
        Ok(())
    }

    /// Placeholder commitments for schema and transaction size preflight only.
    /// Never submit them; replace them with the certified upload references.
    pub fn preflight_reference(&self) -> anyhow::Result<NexusData> {
        NexusData::from_values(
            self.values
                .iter()
                .map(|_| {
                    NexusValue::walrus_data(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", [0; 32])
                })
                .collect::<anyhow::Result<_>>()?,
            self.many,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_large_documents_without_constructing_inline_data() {
        let bytes = format!(
            "  {}\n",
            serde_json::to_string(&"x".repeat(100_000)).unwrap()
        )
        .into_bytes();
        assert!(NexusData::inline_data(bytes.clone()).is_err());
        let data = WalrusUploadData::from_json_document(bytes.clone(), false).unwrap();
        assert_eq!(data.values(), &[bytes]);
        assert!(data.preflight_reference().unwrap().has_walrus());
    }

    #[test]
    fn rejects_invalid_cardinality_and_combined_byte_overflow_before_payment() {
        assert!(WalrusUploadData::new(vec![], true).is_err());
        assert!(WalrusUploadData::new(vec![vec![], vec![]], false).is_err());
        assert!(WalrusUploadData::new(vec![vec![]; 257], true).is_err());
        assert!(
            WalrusUploadData::new(vec![vec![0; MAX_RESOLVED_INPUT_BYTES / 2 + 1]; 2], true)
                .is_err()
        );
    }
}
