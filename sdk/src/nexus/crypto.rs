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
        events::{NexusEvent, NexusEventKind},
        idents::primitives,
        nexus::{client::NexusClient, error::NexusError},
        sui,
        transactions::crypto,
    },
    anyhow::anyhow,
    std::time::{Duration, Instant},
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

        // == Wait for pre key fulfillment ==

        let cursor = claim_response
            .events
            .as_ref()
            .and_then(|events| events.data.last())
            .map(|event| event.id);

        let fulfilled_event = self
            .wait_for_pre_key_fulfilled_event(address, cursor)
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

        let mut tx = sui::ProgrammableTransactionBuilder::new();

        if let Err(e) = crypto::associate_pre_key_with_sender(&mut tx, nexus_objects, message) {
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

    async fn wait_for_pre_key_fulfilled_event(
        &self,
        requested_by: sui::Address,
        cursor: Option<sui::EventID>,
    ) -> Result<crate::events::PreKeyFulfilledEvent, NexusError> {
        let primitives_pkg_id = self.client.nexus_objects.primitives_pkg_id;

        let filter = sui::EventFilter::MoveEventModule {
            package: primitives_pkg_id,
            module: primitives::Event::EVENT_WRAPPER.module.into(),
        };

        self.poll_event_until(filter, cursor, |event| match event.data {
            NexusEventKind::PreKeyFulfilled(e) if e.requested_by == requested_by => Some(e),
            _ => None,
        })
        .await
    }

    async fn poll_event_until<T, F>(
        &self,
        filter: sui::EventFilter,
        mut cursor: Option<sui::EventID>,
        mut predicate: F,
    ) -> Result<T, NexusError>
    where
        F: FnMut(NexusEvent) -> Option<T>,
    {
        let timeout = Duration::from_secs(300);
        let mut poll_interval = Duration::from_millis(100);
        let max_poll_interval = Duration::from_secs(2);
        let started = Instant::now();

        let sui_client = self.client.signer.get_client().await?;

        loop {
            if started.elapsed() > timeout {
                return Err(NexusError::Timeout(anyhow!(
                    "Timeout {timeout:?} reached while waiting for event"
                )));
            }

            let limit = None;
            let descending_order = false;

            let page = sui_client
                .event_api()
                .query_events(filter.clone(), cursor, limit, descending_order)
                .await
                .map_err(|e| NexusError::Rpc(e.into()))?;

            cursor = page.next_cursor;

            let mut found_event = false;

            for event in page.data {
                let Ok(event): anyhow::Result<NexusEvent> = event.try_into() else {
                    continue;
                };

                if let Some(result) = predicate(event) {
                    return Ok(result);
                }

                found_event = true;
            }

            if found_event {
                poll_interval = Duration::from_millis(100);
            } else {
                poll_interval = (poll_interval * 2).min(max_poll_interval);
            }

            tokio::time::sleep(poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        crypto::x3dh::{IdentityKey, PreKeyBundle},
        events::{NexusEventKind, PreKeyFulfilledEvent},
        idents::{primitives, workflow},
        sui,
        test_utils::{nexus_mocks, sui_mocks},
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

        let claim_tx_digest = sui::TransactionDigest::random();
        let (claim_execute_call, claim_confirm_call) =
            sui_mocks::rpc::mock_governance_api_execute_execute_transaction_block(
                &mut server,
                claim_tx_digest,
                Some(sui_mocks::mock_sui_transaction_block_effects(
                    None, None, None, None,
                )),
                Some(sui::TransactionBlockEvents {
                    data: vec![sui::Event {
                        id: sui::EventID {
                            tx_digest: claim_tx_digest,
                            event_seq: 0,
                        },
                        package_id: nexus_client.nexus_objects.primitives_pkg_id,
                        transaction_module: primitives::Event::EVENT_WRAPPER.module.into(),
                        sender: address,
                        bcs: sui::BcsEvent::new(vec![]),
                        timestamp_ms: None,
                        type_: sui::MoveStructTag {
                            address: nexus_client.nexus_objects.primitives_pkg_id.into(),
                            module: primitives::Event::EVENT_WRAPPER.module.into(),
                            name: primitives::Event::EVENT_WRAPPER.name.into(),
                            type_params: vec![sui::MoveTypeTag::Struct(Box::new(
                                sui::MoveStructTag {
                                    address: nexus_client.nexus_objects.workflow_pkg_id.into(),
                                    module: workflow::PreKeyVault::PRE_KEY_VAULT.module.into(),
                                    name: sui::move_ident_str!("PreKeyRequestedEvent").into(),
                                    type_params: vec![],
                                },
                            ))],
                        },
                        parsed_json: serde_json::json!({
                            "event": {
                                "requested_by": address.to_string(),
                            }
                        }),
                    }],
                }),
                None,
                None,
            );

        let _event_query = sui_mocks::rpc::mock_event_api_query_events(
            &mut server,
            vec![(
                "PreKeyFulfilledEvent".to_string(),
                NexusEventKind::PreKeyFulfilled(PreKeyFulfilledEvent {
                    requested_by: address,
                    pre_key_bytes: bincode::serialize(&bundle).unwrap(),
                }),
            )],
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

        associate_execute_call.assert_async().await;
        associate_confirm_call.assert_async().await;

        assert_eq!(result.claim_tx_digest, claim_tx_digest);
        assert_eq!(result.associate_tx_digest, associate_tx_digest);
    }
}
