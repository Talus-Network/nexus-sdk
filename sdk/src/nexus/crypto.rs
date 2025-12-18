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
        events::{NexusEventKind, PreKeyFulfilledEvent},
        nexus::{client::NexusClient, error::NexusError},
        sui,
        transactions::crypto,
    },
    anyhow::anyhow,
    std::time::Duration,
};

pub struct HandshakeResult {
    pub session: Session,
    pub claim_tx_digest: sui::types::Digest,
    pub associate_tx_digest: sui::types::Digest,
}

pub struct CryptoActions {
    pub(super) client: NexusClient,
}

impl CryptoActions {
    /// Perform crypto handshake with Nexus and return the [`Session`] and
    /// the [`IdentityKey`] generated.
    pub async fn handshake(&self, ik: &IdentityKey) -> Result<HandshakeResult, NexusError> {
        let address = self.client.signer.get_active_address();
        let nexus_objects = &self.client.nexus_objects;

        // == Claim PreKey transaction ==

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = crypto::claim_pre_key_for_self(&mut tx, nexus_objects) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let claim_response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        // == Wait for pre key fulfillment ==

        let fulfilled_event = self
            .wait_for_pre_key_fulfilled_event(address, claim_response.checkpoint)
            .await?;

        let pre_key_bytes = fulfilled_event.pre_key_bytes;

        let bundle = bincode::deserialize::<PreKeyBundle>(&pre_key_bytes)
            .map_err(|e| NexusError::Parsing(e.into()))?;

        let (Message::Initial(message), session) =
            Session::initiate(ik, &bundle, b"nexus auth").map_err(NexusError::Crypto)?
        else {
            unreachable!("Session::initiate must return Initial message");
        };

        // == Associate PreKey transaction ==

        let mut tx = sui::tx::TransactionBuilder::new();

        if let Err(e) = crypto::associate_pre_key_with_sender(&mut tx, nexus_objects, message) {
            return Err(NexusError::TransactionBuilding(e));
        }

        let mut gas_coin = self.client.gas.acquire_gas_coin().await;

        tx.set_sender(address);
        tx.set_gas_budget(self.client.gas.get_budget());
        tx.set_gas_price(self.client.reference_gas_price);

        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin.object_id(),
            gas_coin.version(),
            *gas_coin.digest(),
        )]);

        let tx = tx
            .finish()
            .map_err(|e| NexusError::TransactionBuilding(e.into()))?;

        let signature = self.client.signer.sign_tx(&tx).await?;

        let associate_response = self
            .client
            .signer
            .execute_tx(tx, signature, &mut gas_coin)
            .await?;

        self.client.gas.release_gas_coin(gas_coin).await;

        Ok(HandshakeResult {
            session,
            claim_tx_digest: claim_response.digest,
            associate_tx_digest: associate_response.digest,
        })
    }

    async fn wait_for_pre_key_fulfilled_event(
        &self,
        requested_by: sui::types::Address,
        checkpoint: u64,
    ) -> Result<PreKeyFulfilledEvent, NexusError> {
        let fetcher = self.client.event_fetcher();
        let timeout = tokio::time::sleep(Duration::from_secs(20));

        let (_poller, mut next_page) = fetcher.poll_nexus_events(None, Some(checkpoint));

        tokio::pin!(timeout);

        loop {
            tokio::select! {
                result = next_page.recv() => {
                    let page = match result {
                        Some(page) => page,
                        None => {
                            return Err(NexusError::Channel(anyhow!("Event fetcher stopped unexpectedly")));
                        }
                    };

                    for event in page.events {
                        if let NexusEventKind::PreKeyFulfilled(e) = event.data {
                            if e.requested_by == requested_by {
                                return Ok(e);
                            }
                        }
                    }
                }

                _ = &mut timeout => {
                    return Err(NexusError::Timeout(anyhow!("Timeout reached while waiting for PreKeyFulfilledEvent")));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        crate::{
            crypto::x3dh::{IdentityKey, PreKeyBundle},
            events::{NexusEventKind, PreKeyFulfilledEvent},
            sui,
            test_utils::{nexus_mocks, sui_mocks},
        },
        mockito::Server,
    };

    #[tokio::test]
    async fn test_crypto_actions_handshake() {
        let mut rng = rand::thread_rng();
        let claim_tx_digest = sui::types::Digest::generate(&mut rng);
        let associate_tx_digest = sui::types::Digest::generate(&mut rng);
        let gas_coin_ref = sui_mocks::mock_sui_object_ref();
        let nexus_objects = sui_mocks::mock_nexus_objects();

        let ik = IdentityKey::generate();
        let receiver_id = IdentityKey::generate();
        let spk_secret = IdentityKey::generate().secret().clone();
        let bundle = PreKeyBundle::new(&receiver_id, 1, &spk_secret, None, None);

        let mut server = Server::new_async().await;
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut tx_service_mock = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut sub_service_mock = sui_mocks::grpc::MockSubscriptionService::new();

        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service_mock, 1000);

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            claim_tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut tx_service_mock,
            &mut sub_service_mock,
            &mut ledger_service_mock,
            associate_tx_digest,
            gas_coin_ref.clone(),
            vec![],
            vec![],
            vec![],
        );

        let grpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            execution_service_mock: Some(tx_service_mock),
            subscription_service_mock: Some(sub_service_mock),
        });

        let client = nexus_mocks::mock_nexus_client(
            &nexus_objects,
            &grpc_url,
            Some(&format!("{}/graphql", server.url())),
        )
        .await;

        let pre_key_fulfilled_event = NexusEventKind::PreKeyFulfilled(PreKeyFulfilledEvent {
            requested_by: client.signer.get_active_address(),
            pre_key_bytes: bincode::serialize(&bundle).unwrap(),
        });

        let event_mock = sui_mocks::gql::mock_event_query(
            &mut server,
            nexus_objects.primitives_pkg_id,
            vec![pre_key_fulfilled_event],
            None,
            None,
        );

        let result = client
            .crypto()
            .handshake(&ik)
            .await
            .expect("Failed to perform handshake");

        event_mock.assert_async().await;

        assert_eq!(result.claim_tx_digest, claim_tx_digest);
        assert_eq!(result.associate_tx_digest, associate_tx_digest);
    }
}
