use {
    crate::{
        execution_limits::MAX_RESOLVED_INPUT_BYTES,
        sui,
        types::{NexusData, NexusValue},
    },
    anyhow::{bail, ensure, Context as _},
    serde::{Deserialize, Serialize},
    sha2::{Digest as _, Sha256},
    std::str::FromStr,
};

/// A supported Walrus deployment, selected from the connected Sui chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WalrusNetwork {
    Testnet,
    Mainnet,
}

impl FromStr for WalrusNetwork {
    type Err = anyhow::Error;

    fn from_str(chain: &str) -> Result<Self, Self::Err> {
        match chain {
            "testnet" => Ok(Self::Testnet),
            "mainnet" => Ok(Self::Mainnet),
            _ => bail!("Walrus requires Sui testnet or mainnet, got '{chain}'"),
        }
    }
}

impl WalrusNetwork {
    pub fn aggregator_url(self) -> &'static str {
        match self {
            Self::Testnet => "https://aggregator.walrus-testnet.walrus.space",
            Self::Mainnet => "https://aggregator.walrus-mainnet.walrus.space",
        }
    }

    /// System and staking objects from the official Walrus network configuration.
    pub fn contracts(self) -> (&'static str, &'static str) {
        match self {
            Self::Mainnet => (
                "0x2134d52768ea07e8c43570ef975eb3e4c27a39fa6396bef985b5abc58d03ddd2",
                "0x10b9d30c28448939ce6c4d6c6e0ffce4a7f8a4ada8248bdad09ef8b70e4a3904",
            ),
            Self::Testnet => (
                "0x6c2547cbbc38025cf3adac45f63cb0a8d12ecf777cdc75a4971612bf97fdf6af",
                "0xbe46180321c30aab2f8b3501e24048377287fa708018a5b7c2792b35fe339ee3",
            ),
        }
    }
}

/// Content commitment and lifecycle facts for one certified upload.
/// Only the blob ID and digest become a Nexus protocol value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredBlob {
    pub blob_id: String,
    pub sha256: String,
    pub size: usize,
    pub object_id: sui::types::Address,
    pub owner: sui::types::Address,
    pub end_epoch: u32,
    pub deletable: bool,
}

impl StoredBlob {
    pub fn nexus_value(&self) -> anyhow::Result<NexusValue> {
        ensure!(
            self.size <= MAX_RESOLVED_INPUT_BYTES,
            "blob exceeds execution byte limit"
        );
        NexusValue::walrus_data(self.blob_id.as_bytes(), hex::decode(&self.sha256)?)
    }

    pub fn nexus_data(&self) -> anyhow::Result<NexusData> {
        NexusData::from_values(vec![self.nexus_value()?], false)
    }

    pub fn verify_bytes(&self, bytes: &[u8]) -> anyhow::Result<()> {
        ensure!(
            bytes.len() == self.size
                && hex::decode(&self.sha256)? == Sha256::digest(bytes).to_vec(),
            "blob contents do not match the reference"
        );
        Ok(())
    }
}

/// Portable local reference. Endpoints and wallet configuration are never included.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalrusReference {
    pub version: u8,
    pub network: WalrusNetwork,
    pub chain_id: String,
    pub many: bool,
    pub blobs: Vec<StoredBlob>,
}

impl WalrusReference {
    pub fn nexus_data(&self) -> anyhow::Result<NexusData> {
        ensure!(
            self.version == 1,
            "unsupported Walrus reference version {}",
            self.version
        );
        ensure!(
            !self.chain_id.is_empty(),
            "Walrus reference has no chain identity"
        );
        let mut remaining = MAX_RESOLVED_INPUT_BYTES;
        let values = self
            .blobs
            .iter()
            .map(|blob| {
                remaining = remaining
                    .checked_sub(blob.size)
                    .context("reference exceeds execution byte limit")?;
                blob.nexus_value()
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        NexusData::from_values(values, self.many)
    }

    pub fn check_network(&self, network: WalrusNetwork, chain_id: &str) -> anyhow::Result<()> {
        ensure!(
            self.network == network && self.chain_id == chain_id,
            "Walrus reference belongs to a different Sui network"
        );
        Ok(())
    }

    /// Reads require only an aggregator. Both digest and saved size are checked.
    pub async fn download(&self, reader: &super::WalrusReader) -> anyhow::Result<Vec<Vec<u8>>> {
        let ports = reader
            .resolve_ports(std::collections::HashMap::from([(
                "data".into(),
                self.nexus_data()?,
            )]))
            .await?;
        let mut values = Vec::new();
        for (blob, value) in self.blobs.iter().zip(&ports["data"]) {
            let NexusValue::InlineData { bytes } = value else {
                unreachable!("resolved data reference")
            };
            blob.verify_bytes(bytes)?;
            values.push(bytes.clone());
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference() -> WalrusReference {
        WalrusReference {
            version: 1,
            network: WalrusNetwork::Testnet,
            chain_id: "testnet chain".into(),
            many: false,
            blobs: vec![StoredBlob {
                blob_id: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
                sha256: hex::encode(Sha256::digest(b"contents")),
                size: 8,
                object_id: sui::types::Address::TWO,
                owner: sui::types::Address::TWO,
                end_epoch: 10,
                deletable: false,
            }],
        }
    }

    #[test]
    fn only_content_commitments_enter_the_protocol() {
        let saved = reference();
        let data = saved.nexus_data().unwrap();
        let expected = NexusData::walrus_data(
            saved.blobs[0].blob_id.as_bytes(),
            Sha256::digest(b"contents").to_vec(),
        )
        .unwrap();
        assert_eq!(data, expected);
        assert!(saved
            .check_network(WalrusNetwork::Testnet, "testnet chain")
            .is_ok());
        assert!(saved
            .check_network(WalrusNetwork::Mainnet, "testnet chain")
            .is_err());
        assert!(saved
            .check_network(WalrusNetwork::Testnet, "different chain")
            .is_err());
        let mut unexpected = serde_json::to_value(saved).unwrap();
        unexpected["publisher"] = serde_json::json!("https://untrusted.example");
        assert!(serde_json::from_value::<WalrusReference>(unexpected).is_err());
    }

    #[test]
    fn receipt_verification_checks_size_digest_version_and_cardinality() {
        let mut saved = reference();
        saved.blobs[0].verify_bytes(b"contents").unwrap();
        assert!(saved.blobs[0].verify_bytes(b"tampered").is_err());
        saved.blobs[0].size += 1;
        assert!(saved.blobs[0].verify_bytes(b"contents").is_err());
        saved.version = 2;
        assert!(saved.nexus_data().is_err());
        saved.version = 1;
        saved.blobs.push(saved.blobs[0].clone());
        assert!(saved.nexus_data().is_err());
        saved.many = true;
        assert!(saved.nexus_data().is_ok());
    }
}
