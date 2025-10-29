//! Commands related to handling cryptographic operations in Nexus.
//!
//! - [`CryptoActions::handshake`] to perform a crypto handshake with Nexus
//!   and retrieve a session for secure communication.

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

        let raw_pre_key = fetch_one::<Structure<RawPreKey>>(sui_client, pre_key_object_id)
            .await
            .map_err(NexusError::Rpc)?;

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

#[cfg(test)]
mod tests {
    use {
        crate::{
            crypto::x3dh::{IdentityKey, PreKeyBundle},
            idents::workflow,
            sui,
            test_utils::{nexus_mocks, sui_mocks},
        },
        std::collections::BTreeMap,
    };

    #[tokio::test]
    async fn test_crypto_actions_handshake() {
        let (mut server, nexus_client) = nexus_mocks::mock_nexus_client().await;
        let ik = IdentityKey::generate();
        let address = nexus_client
            .signer
            .get_active_address()
            .await
            .expect("Failed to get active address");

        let receiver_id = IdentityKey::generate();
        let spk_secret = IdentityKey::generate().secret().clone();
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let pre_key_object_ref = sui::ObjectRef {
            object_id: sui::ObjectID::random(),
            version: sui::SequenceNumber::from_u64(1),
            digest: sui::ObjectDigest::random(),
        };

        let claim_tx_digest = sui::TransactionDigest::random();
        let (claim_execute_call, claim_confirm_call) =
            sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
                &mut server,
                claim_tx_digest,
                Some(sui_mocks::mock_sui_transaction_block_effects(
                    None,
                    None,
                    Some(vec![sui::OwnedObjectRef {
                        owner: sui::Owner::AddressOwner(address),
                        reference: pre_key_object_ref.clone(),
                    }]),
                    None,
                )),
                None,
                None,
                None,
            );

        let pre_key_object = sui::ParsedMoveObject {
            type_: sui::MoveStructTag {
                address: *nexus_client.nexus_objects.workflow_pkg_id,
                module: workflow::PreKeyVault::PRE_KEY.module.into(),
                name: workflow::PreKeyVault::PRE_KEY.name.into(),
                type_params: vec![],
            },
            has_public_transfer: false,
            fields: sui::MoveStruct::WithFields(BTreeMap::from([(
                "bytes".into(),
                sui::MoveValue::Vector(
                    bincode::serialize(&bundle)
                        .expect("Failed to serialize PreKeyBundle")
                        .into_iter()
                        .map(|b| sui::MoveValue::Number(b.into()))
                        .collect(),
                ),
            )])),
        };

        let get_object_call = sui_mocks::rpc::mock_read_api_get_object(
            &mut server,
            pre_key_object_ref.object_id,
            pre_key_object,
        );

        let associate_tx_digest = sui::TransactionDigest::random();
        let (associate_execute_call, associate_confirm_call) =
            sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
                &mut server,
                associate_tx_digest,
                None,
                None,
                None,
                None,
            );

        let result = nexus_client
            .crypto()
            .handshake(&ik)
            .await
            .expect("Failed to perform handshake");

        claim_execute_call.assert_async().await;
        claim_confirm_call.assert_async().await;

        get_object_call.assert_async().await;

        associate_execute_call.assert_async().await;
        associate_confirm_call.assert_async().await;

        assert_eq!(result.claim_tx_digest, claim_tx_digest);
        assert_eq!(result.associate_tx_digest, associate_tx_digest);
    }
}
