//! Resolve storage defaults against the active Sui wallet and network.

use {
    crate::prelude::*,
    anyhow::{ensure, Context as _},
    nexus_sdk::{
        execution_limits::MAX_RESOLVED_DATA_BYTES,
        nexus::wallet::WalletClient,
        walrus::{WalrusNetwork, WalrusReader, WalrusReference},
    },
};

#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub(crate) network: WalrusNetwork,
    pub(crate) chain_id: String,
    rpc_url: String,
    pub(super) aggregator: String,
    pub(super) epochs: u32,
}

impl Settings {
    pub(crate) async fn load(conf: &CliConf) -> AnyResult<Self> {
        let rpc_url = std::env::var("SUI_RPC_URL")
            .ok()
            .or_else(|| conf.sui.rpc_url.as_ref().map(ToString::to_string))
            .context("Set the Sui RPC URL with nexus conf set --sui.rpc-url")?;
        let info = sui::grpc::client(&rpc_url)?
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await?
            .into_inner();
        Self::new(
            &conf.data_storage,
            rpc_url,
            info.chain_id.context("Sui RPC omitted its chain ID")?,
            info.chain
                .as_deref()
                .context("Sui RPC omitted its network")?,
        )
    }

    pub(crate) fn for_wallet(conf: &DataStorageConf, wallet: &WalletClient) -> AnyResult<Self> {
        Self::new(
            conf,
            wallet.rpc_url().into(),
            wallet.chain_id().into(),
            wallet.chain(),
        )
    }

    fn new(
        conf: &DataStorageConf,
        rpc_url: String,
        chain_id: String,
        chain: &str,
    ) -> AnyResult<Self> {
        let network = chain.parse()?;
        let aggregator = aggregator_for(conf, network)?;
        let epochs = u32::from(conf.walrus_save_for_epochs.unwrap_or(2));
        ensure!(
            (1..=53).contains(&epochs),
            "Walrus epochs must be between 1 and 53"
        );
        WalrusReader::new(&aggregator, MAX_RESOLVED_DATA_BYTES)?;
        Ok(Self {
            network,
            chain_id,
            rpc_url,
            aggregator,
            epochs,
        })
    }

    pub(crate) fn reader(&self) -> AnyResult<WalrusReader> {
        WalrusReader::new(&self.aggregator, MAX_RESOLVED_DATA_BYTES)
    }

    pub(super) async fn wallet(&self, conf: &CliConf) -> AnyResult<WalletClient> {
        let wallet =
            WalletClient::connect(&self.rpc_url, crate::sui::get_signing_key(conf).await?).await?;
        ensure!(
            wallet.chain_id() == self.chain_id,
            "Sui network changed while loading the wallet"
        );
        Ok(wallet)
    }

    pub(crate) async fn verify(&self, reference: &WalrusReference) -> AnyResult<Vec<Vec<u8>>> {
        reference.check_network(self.network, &self.chain_id)?;
        reference.download(&self.reader()?).await
    }
}

fn aggregator_for(conf: &DataStorageConf, network: WalrusNetwork) -> AnyResult<String> {
    let Some(url) = &conf.walrus_aggregator_url else {
        return Ok(network.aggregator_url().into());
    };
    if [WalrusNetwork::Testnet, WalrusNetwork::Mainnet]
        .iter()
        .any(|net| url.as_str().trim_end_matches('/') == net.aggregator_url())
    {
        return Ok(network.aggregator_url().into());
    }
    ensure!(conf.walrus_network == Some(network), "Custom aggregator requires a matching network; use nexus walrus configure --aggregator URL --network testnet|mainnet");
    Ok(url.to_string())
}

pub(crate) async fn load_conf() -> AnyResult<CliConf> {
    match CliConf::load().await {
        Ok(conf) => Ok(conf),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(CliConf::default())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_aggregators_follow_the_rpc_and_custom_readers_are_network_bound() {
        let mut conf = DataStorageConf {
            walrus_aggregator_url: Some(WalrusNetwork::Testnet.aggregator_url().parse().unwrap()),
            ..Default::default()
        };
        assert_eq!(
            aggregator_for(&conf, WalrusNetwork::Mainnet).unwrap(),
            WalrusNetwork::Mainnet.aggregator_url()
        );
        conf.walrus_aggregator_url = Some("https://storage.example".parse().unwrap());
        assert!(aggregator_for(&conf, WalrusNetwork::Mainnet).is_err());
        conf.walrus_network = Some(WalrusNetwork::Mainnet);
        assert_eq!(
            aggregator_for(&conf, WalrusNetwork::Mainnet).unwrap(),
            "https://storage.example/"
        );
    }

    #[tokio::test]
    async fn reference_reads_need_no_wallet_and_reject_a_different_network() {
        use sha2::{Digest as _, Sha256};
        let mut server = mockito::Server::new_async().await;
        let bytes = b"  \"public data\"\n";
        let blob_id = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let get = server
            .mock("GET", format!("/v1/blobs/{blob_id}").as_str())
            .with_body(bytes)
            .expect(1)
            .create_async()
            .await;
        let settings = Settings {
            network: WalrusNetwork::Testnet,
            chain_id: "chain".into(),
            rpc_url: "unused".into(),
            aggregator: server.url(),
            epochs: 2,
        };
        let reference = WalrusReference {
            version: 1,
            network: WalrusNetwork::Testnet,
            chain_id: "chain".into(),
            many: false,
            blobs: vec![nexus_sdk::walrus::StoredBlob {
                blob_id: blob_id.into(),
                sha256: hex::encode(Sha256::digest(bytes)),
                size: bytes.len(),
                object_id: sui::types::Address::TWO,
                owner: sui::types::Address::TWO,
                end_epoch: 10,
                deletable: false,
            }],
        };
        assert_eq!(
            settings.verify(&reference).await.unwrap(),
            vec![bytes.to_vec()]
        );
        assert!(settings
            .verify(&WalrusReference {
                network: WalrusNetwork::Mainnet,
                ..reference
            })
            .await
            .is_err());
        get.assert_async().await;
    }
}
