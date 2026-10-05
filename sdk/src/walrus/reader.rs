//! Bounded, digest verified reads for transient HTTP tool inputs.

use {
    super::{WalrusClient, WalrusContentDigestMismatch},
    crate::{
        execution_limits::MAX_RESOLVED_INPUT_BYTES,
        types::{NexusData, NexusValue},
    },
    anyhow::{ensure, Context as _},
    sha2::{Digest as _, Sha256},
    std::collections::{BTreeMap, HashMap},
};

/// A reader has no publisher configuration or upload credentials.
pub struct WalrusReader {
    client: WalrusClient,
    max_bytes: usize,
}

impl WalrusReader {
    /// Sets a total byte budget for one resolution, including inline values.
    pub fn new(aggregator_url: &str, max_bytes: usize) -> anyhow::Result<Self> {
        let url = reqwest::Url::parse(aggregator_url).context("invalid Walrus aggregator URL")?;
        ensure!(
            matches!(url.scheme(), "http" | "https"),
            "Walrus aggregator must use HTTP or HTTPS"
        );
        ensure!(
            url.username().is_empty() && url.password().is_none(),
            "Walrus aggregator URL must not contain credentials"
        );
        ensure!(
            (1..=MAX_RESOLVED_INPUT_BYTES).contains(&max_bytes),
            "Walrus read budget must be between 1 and {MAX_RESOLVED_INPUT_BYTES} bytes"
        );
        Ok(Self {
            client: WalrusClient::builder()
                .with_aggregator_url(aggregator_url.trim_end_matches('/'))
                .build(),
            max_bytes,
        })
    }

    /// Downloads at most the specified bytes and checks the committed digest.
    pub async fn read_verified(
        &self,
        blob_id: &str,
        digest: &[u8],
        max_bytes: usize,
    ) -> anyhow::Result<Vec<u8>> {
        NexusValue::walrus_data(blob_id.as_bytes(), digest)?;
        let bytes = self
            .client
            .read_file_bounded(blob_id, max_bytes.min(self.max_bytes))
            .await?;
        if &Sha256::digest(&bytes)[..] != digest {
            return Err(WalrusContentDigestMismatch {
                blob_id: blob_id.to_owned(),
            }
            .into());
        }
        Ok(bytes)
    }

    /// Resolves canonical ports into transient values. Chain limits still apply to
    /// the original references; a separate budget applies to their contents.
    /// Sequential reads keep the total allocation within the operation budget.
    pub async fn resolve_ports(
        &self,
        ports: HashMap<String, NexusData>,
    ) -> anyhow::Result<HashMap<String, Vec<NexusValue>>> {
        ensure!(
            ports.values().all(NexusData::is_well_formed),
            "cannot resolve malformed NexusData"
        );
        let mut remaining = self.max_bytes;
        let mut resolved = HashMap::with_capacity(ports.len());
        for (name, port) in ports.into_iter().collect::<BTreeMap<_, _>>() {
            let mut values = Vec::new();
            for value in port.into_values()? {
                let value = match value {
                    NexusValue::WalrusData {
                        blob_id,
                        content_digest,
                    } => {
                        let blob_id = std::str::from_utf8(&blob_id)?;
                        let bytes = self
                            .read_verified(blob_id, &content_digest, remaining)
                            .await?;
                        NexusValue::InlineData { bytes }
                    }
                    value => value,
                };
                let len = match &value {
                    NexusValue::InlineData { bytes } => bytes.len(),
                    NexusValue::Object { .. } => 32,
                    NexusValue::WalrusData { .. } => unreachable!("reference was resolved"),
                };
                remaining = remaining
                    .checked_sub(len)
                    .context("resolved inputs exceed the Walrus read budget")?;
                values.push(value);
            }
            resolved.insert(name, values);
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, mockito::Server};

    const BLOB: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    #[tokio::test]
    async fn large_reference_resolves_without_relaxing_chain_limits() {
        let mut server = Server::new_async().await;
        let bytes = vec![b'a'; 100_000];
        assert!(NexusData::inline_data(bytes.clone()).is_err());
        let reference =
            NexusData::walrus_data(BLOB.as_bytes(), Sha256::digest(&bytes).to_vec()).unwrap();
        let mock = server
            .mock("GET", format!("/v1/blobs/{BLOB}").as_str())
            .with_body(bytes.clone())
            .expect(1)
            .create_async()
            .await;
        let resolved = WalrusReader::new(&server.url(), 200_000)
            .unwrap()
            .resolve_ports(HashMap::from([("data".into(), reference)]))
            .await
            .unwrap();
        assert_eq!(resolved["data"], vec![NexusValue::InlineData { bytes }]);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn checks_digest_and_combined_budget() {
        let mut server = Server::new_async().await;
        let bytes = vec![7; 80];
        let mock = server
            .mock("GET", format!("/v1/blobs/{BLOB}").as_str())
            .with_body(bytes.clone())
            .expect(3)
            .create_async()
            .await;
        let reader = WalrusReader::new(&server.url(), 100).unwrap();
        assert!(reader
            .read_verified(BLOB, &[0; 32], 100)
            .await
            .unwrap_err()
            .is::<WalrusContentDigestMismatch>());
        let reference =
            NexusData::walrus_data(BLOB.as_bytes(), Sha256::digest(&bytes).to_vec()).unwrap();
        assert!(reader
            .resolve_ports(HashMap::from([
                ("a".into(), reference.clone()),
                ("b".into(), reference),
            ]))
            .await
            .is_err());
        mock.assert_async().await;
    }
}
