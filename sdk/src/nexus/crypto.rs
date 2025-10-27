//! Commands related to handling cryptographic operations in Nexus.

use {
    crate::{
        crypto::{
            session::{Message, Session},
            x3dh::{IdentityKey, PreKeyBundle},
        },
        nexus::{client::NexusClient, error::NexusError},
        object_crawler::{fetch_one, Structure},
        sui,
        transactions::crypto,
    },
    anyhow::anyhow,
    sui_sdk::rpc_types::SuiTransactionBlockEffectsAPI,
};

/// Struct that results from [`CryptoActions::handshake`].
pub struct HandshakeResult {
    pub session: Session,
    pub claim_tx_digest: sui::TransactionDigest,
    pub associate_tx_digest: sui::TransactionDigest,
}

pub struct CryptoActions {
    pub(super) client: NexusClient,
}

impl CryptoActions {
    /// Perform crypto handshake with Nexus and return the [`Session`] and
    /// the [`IdentityKey`] generated.
    pub async fn handshake(&self, ik: &IdentityKey) -> Result<HandshakeResult, NexusError> {
        let address = self.client.signer.get_active_address().await?;
        let nexus_objects = &self.client.nexus_objects;
        let sui_client = &self.client.signer.get_client().await?;

        // == Claim PreKey transaction ==

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        if let Err(e) = crypto::claim_pre_key_for_self(&mut tx, nexus_objects) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        let tx_data = sui::TransactionData::new_programmable(
            address,
            vec![gas_coin.to_object_ref()],
            tx.finish(),
            self.client.gas.get_budget(),
            self.client.reference_gas_price,
        );

        let envelope = self.client.signer.sign_tx(tx_data).await?;

        let claim_response = self
            .client
            .signer
            .execute_tx(envelope, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        // == Find the claimed PreKey object ID ==

        let effects = claim_response.effects.expect("Effects must be present");

        let pre_key_object_id = effects
            .unwrapped()
            .iter()
            .find_map(|object| {
                if object.owner.get_owner_address() == Ok(address) {
                    return Some(object.object_id());
                }

                None
            })
            .ok_or_else(|| NexusError::Parsing(anyhow!("No PreKey object found")))?;

        // == Parse into PreKeyBundle and initiate the session ==

        #[derive(serde::Deserialize, Debug)]
        struct RawPreKey {
            bytes: Vec<u8>,
        }

        let raw_pre_key = fetch_one::<Structure<RawPreKey>>(&sui_client, pre_key_object_id)
            .await
            .map_err(NexusError::Rpc)?;

        println!(
            "Fetched PreKey object: {:#?}",
            raw_pre_key.data.inner().bytes
        );

        let bundle = bincode::deserialize::<PreKeyBundle>(&raw_pre_key.data.inner().bytes)
            .map_err(|e| NexusError::Parsing(e.into()))?;

        let (Message::Initial(message), session) =
            Session::initiate(ik, &bundle, b"nexus auth").map_err(NexusError::Crypto)?
        else {
            unreachable!("Session::initiate must return Initial message");
        };

        // == Associate PreKey transaction ==

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        if let Err(e) = crypto::associate_pre_key_with_sender(
            &mut tx,
            nexus_objects,
            &raw_pre_key.object_ref(),
            message,
        ) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        let tx_data = sui::TransactionData::new_programmable(
            address,
            vec![gas_coin.to_object_ref()],
            tx.finish(),
            self.client.gas.get_budget(),
            self.client.reference_gas_price,
        );

        let envelope = self.client.signer.sign_tx(tx_data).await?;

        let associate_response = self
            .client
            .signer
            .execute_tx(envelope, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(HandshakeResult {
            session,
            claim_tx_digest: claim_response.digest,
            associate_tx_digest: associate_response.digest,
        })
    }
}
