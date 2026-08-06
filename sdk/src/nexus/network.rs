//! Commands related to Nexus network economics.

use crate::{
    events::NexusEventKind,
    move_bindings::registry::priority_fee_vault::{PriorityFeeVault, PriorityFeeVaultStateV1},
    nexus::{client::NexusClient, error::NexusError},
    sui,
    transactions::network,
    types::PriorityFeeWithdrawalQuote,
};

pub struct ConfigurePriorityFeeVaultResult {
    pub tx_digest: sui::types::Digest,
}

pub struct SwapUsForSuiResult {
    pub tx_digest: sui::types::Digest,
    pub us_spent: u64,
    pub us_refunded: u64,
    pub sui_withdrawn: u64,
}

#[derive(Debug)]
pub struct DrainPriorityFeeVaultSuiResult {
    pub tx_digest: sui::types::Digest,
    pub exchange_rate_million_mists_us: u64,
    pub sui_balance_before: u64,
    pub min_sui_out: u64,
}

pub struct WithdrawPriorityFeeResult {
    pub tx_digest: sui::types::Digest,
}

/// Operations over Nexus network fee state.
pub struct NetworkActions {
    pub(super) client: NexusClient,
}

impl NetworkActions {
    /// Fetch and decode the priority fee vault state.
    pub async fn fetch_priority_fee_vault_state(
        &self,
    ) -> Result<PriorityFeeVaultStateV1, NexusError> {
        let client = self.client.operation_client().await?;
        Self::fetch_priority_fee_vault_state_with(&client).await
    }

    async fn fetch_priority_fee_vault_state_with(
        client: &NexusClient,
    ) -> Result<PriorityFeeVaultStateV1, NexusError> {
        client
            .crawler()
            .get_versioned_object::<PriorityFeeVault, PriorityFeeVaultStateV1>(
                *client.nexus_objects.priority_fee_vault.object_id(),
                1,
            )
            .await
            .map(|response| response.data)
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch priority fee vault state: {error}"
                ))
            })
    }

    /// Return the current vault share for a leader capability object ID.
    pub async fn priority_fee_share(
        &self,
        leader_cap: sui::types::Address,
    ) -> Result<u64, NexusError> {
        let client = self.client.operation_client().await?;
        let state = Self::fetch_priority_fee_vault_state_with(&client).await?;
        state.leader_share(leader_cap).ok_or_else(|| {
            NexusError::Configuration(format!(
                "Leader capability '{leader_cap}' has no priority fee share in the vault"
            ))
        })
    }

    /// Quote a leader priority fee withdrawal before constructing the transaction.
    pub async fn quote_priority_fee_withdrawal(
        &self,
        leader_cap: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Result<PriorityFeeWithdrawalQuote, NexusError> {
        let client = self.client.operation_client().await?;
        let state = Self::fetch_priority_fee_vault_state_with(&client).await?;
        state
            .quote_leader_withdrawal(leader_cap, share_to_withdraw)
            .ok_or_else(|| {
                NexusError::Configuration(format!(
                    "Invalid priority fee withdrawal for leader capability '{leader_cap}' and share '{share_to_withdraw}'"
                ))
            })
    }

    /// Configure the priority fee vault exchange rate.
    pub async fn configure_priority_fee_vault(
        &self,
        exchange_rate_million_mists_us: u64,
    ) -> Result<ConfigurePriorityFeeVaultResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let transaction = network::configure_priority_fee_vault(
            &client.nexus_objects,
            exchange_rate_million_mists_us,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;

        Ok(ConfigurePriorityFeeVaultResult {
            tx_digest: response.digest,
        })
    }

    /// Swap an owned `$US` coin for SUI and transfer both returned coins to the sender.
    pub async fn swap_us_for_sui(
        &self,
        us_coin: sui::types::Address,
        min_sui_out: u64,
    ) -> Result<SwapUsForSuiResult, NexusError> {
        let client = self.client.operation_client().await?;
        Self::swap_us_for_sui_with(&client, us_coin, min_sui_out).await
    }

    async fn swap_us_for_sui_with(
        client: &NexusClient,
        us_coin: sui::types::Address,
        min_sui_out: u64,
    ) -> Result<SwapUsForSuiResult, NexusError> {
        let address = client.owner()?;
        let us_coin = client
            .crawler()
            .get_object_metadata(us_coin)
            .await
            .map(|response| response.object_ref())
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch `$US` coin metadata: {error}"
                ))
            })?;
        let transaction =
            network::swap_us_for_sui(&client.nexus_objects, &us_coin, min_sui_out, address)
                .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;
        let swap = response
            .events
            .iter()
            .find_map(|event| match &event.data {
                NexusEventKind::PriorityFeeSwap(swap) => Some(swap),
                _ => None,
            })
            .ok_or_else(|| {
                NexusError::Parsing(anyhow::anyhow!(
                    "PriorityFeeSwapEvent not found in swap transaction response"
                ))
            })?;
        let us_spent = swap.us_in.checked_sub(swap.us_refunded).ok_or_else(|| {
            NexusError::Parsing(anyhow::anyhow!(
                "PriorityFeeSwapEvent refund exceeds its `$US` input"
            ))
        })?;

        Ok(SwapUsForSuiResult {
            tx_digest: response.digest,
            us_spent,
            us_refunded: swap.us_refunded,
            sui_withdrawn: swap.sui_out,
        })
    }

    /// Drain all available SUI by swapping `$US` with a strict minimum output.
    pub async fn drain_priority_fee_vault_sui(
        &self,
        us_coin: sui::types::Address,
    ) -> Result<DrainPriorityFeeVaultSuiResult, NexusError> {
        let client = self.client.operation_client().await?;
        let state = Self::fetch_priority_fee_vault_state_with(&client).await?;
        let quote = state.quote_sui_drain().ok_or_else(|| {
            NexusError::Configuration(
                "Priority fee vault must have a configured exchange rate and positive SUI balance to drain"
                    .to_owned(),
            )
        })?;
        let min_sui_out = quote.sui_out;
        let result = Self::swap_us_for_sui_with(&client, us_coin, min_sui_out).await?;

        Ok(DrainPriorityFeeVaultSuiResult {
            tx_digest: result.tx_digest,
            exchange_rate_million_mists_us: quote.exchange_rate_million_mists_us,
            sui_balance_before: quote.sui_out,
            min_sui_out,
        })
    }

    /// Withdraw a leader's `$US` priority fee share to the sender.
    pub async fn withdraw_priority_fee(
        &self,
        leader_cap: sui::types::Address,
        share_to_withdraw: u64,
    ) -> Result<WithdrawPriorityFeeResult, NexusError> {
        let client = self.client.operation_client().await?;
        let address = client.owner()?;
        let leader_cap = client
            .crawler()
            .get_object_metadata(leader_cap)
            .await
            .map(|response| response.object_ref())
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch leader capability metadata: {error}"
                ))
            })?;
        let transaction = network::withdraw_priority_fee(
            &client.nexus_objects,
            &leader_cap,
            share_to_withdraw,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;

        Ok(WithdrawPriorityFeeResult {
            tx_digest: response.digest,
        })
    }
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                primitives::{data::NexusData, event::EventWrapper},
                registry::priority_fee_vault::{PriorityFeeAccount, PriorityFeeSwapEvent},
                sui_framework::{
                    balance::Balance,
                    object::{ID, UID},
                    sui::SUI,
                    vec_map::{Entry, VecMap},
                    versioned::Versioned,
                },
                talus::us::US,
            },
            test_utils::{nexus_mocks, sui_mocks},
            types::NexusObjects,
        },
        serde::Serialize,
    };

    fn vault_state(
        leader_cap: sui::types::Address,
        leader_share: u64,
        sui_balance: u64,
        us_balance: u64,
        exchange_rate_million_mists_us: u64,
        total_share: u64,
    ) -> PriorityFeeVaultStateV1 {
        PriorityFeeVaultStateV1::new(
            ID::new(sui::types::Address::from_static("0x501")),
            1,
            Balance::<SUI>::new(sui_balance),
            Balance::<US>::new(us_balance),
            exchange_rate_million_mists_us,
            total_share,
            VecMap::new(vec![Entry::new(
                ID::new(leader_cap),
                PriorityFeeAccount::new(leader_share),
            )]),
        )
    }

    fn mock_vault_reads(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        objects: &NexusObjects,
        state: &PriorityFeeVaultStateV1,
        reads: usize,
    ) {
        let state_id = sui::types::Address::from_static("0x502");
        let vault = PriorityFeeVault::new(
            UID::new(*objects.priority_fee_vault.object_id()),
            Versioned::new(UID::new(state_id), 1),
        );
        for _ in 0..reads {
            sui_mocks::grpc::mock_get_object_bcs(
                ledger_service,
                objects.priority_fee_vault.clone(),
                sui::types::Owner::Shared(objects.priority_fee_vault.version()),
                bcs::to_bytes(&vault).expect("vault serializes"),
            );
            sui_mocks::grpc::mock_versioned_payload(ledger_service, state_id, 1, state.clone());
        }
    }

    #[derive(Serialize)]
    struct Wrapper<T> {
        event: T,
    }

    fn swap_event(
        objects: &NexusObjects,
        us_in: u64,
        us_refunded: u64,
        sui_out: u64,
    ) -> sui::types::Event {
        let event = PriorityFeeSwapEvent::new(
            ID::new(*objects.priority_fee_vault.object_id()),
            us_in,
            us_refunded,
            sui_out,
        );
        let inner = crate::move_bindings::struct_tag::<PriorityFeeSwapEvent>(objects);
        let wrapper = crate::move_bindings::struct_tag::<EventWrapper<NexusData>>(objects);
        let wrapper = sui::types::StructTag::new(
            *wrapper.address(),
            wrapper.module().clone(),
            wrapper.name().clone(),
            vec![sui::types::TypeTag::Struct(Box::new(inner))],
        );
        sui_mocks::mock_sui_event(
            objects.packages.registry.storage_id,
            wrapper,
            bcs::to_bytes(&Wrapper { event }).expect("swap event serializes"),
        )
    }

    async fn mutating_client(
        objects: &NexusObjects,
        metadata: Vec<(sui::types::ObjectReference, sui::types::Owner)>,
        events: Vec<sui::types::Event>,
    ) -> (NexusClient, sui_mocks::grpc::SubmittedTransaction) {
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut transaction_service = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service = sui_mocks::grpc::MockSubscriptionService::new();
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 1_000);
        for (object_ref, owner) in metadata {
            sui_mocks::grpc::mock_get_object_metadata(&mut ledger_service, object_ref, owner, None);
        }
        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut transaction_service,
            &mut subscription_service,
            &mut ledger_service,
            sui_mocks::mock_sui_object_ref(),
            vec![],
            vec![],
            events,
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            execution_service_mock: Some(transaction_service),
            subscription_service_mock: Some(subscription_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(objects, &rpc_url).await;
        (client, submitted)
    }

    #[tokio::test]
    async fn priority_fee_reads_decode_state_and_classify_invalid_requests() {
        let objects = sui_mocks::mock_nexus_objects();
        let leader_cap = sui::types::Address::from_static("0x511");
        let state = vault_state(leader_cap, 10, 0, 90, 3, 30);
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        mock_vault_reads(&mut ledger_service, &objects, &state, 5);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&objects, &rpc_url).await;
        let actions = client.network();

        let decoded = actions
            .fetch_priority_fee_vault_state()
            .await
            .expect("vault state decodes");
        assert_eq!(decoded.leader_share(leader_cap), Some(10));
        assert_eq!(actions.priority_fee_share(leader_cap).await.unwrap(), 10);
        assert_eq!(
            actions
                .quote_priority_fee_withdrawal(leader_cap, 10)
                .await
                .unwrap(),
            PriorityFeeWithdrawalQuote {
                share_to_withdraw: 10,
                us_out: 30,
                us_refunded: 60,
            }
        );

        let unknown = actions
            .priority_fee_share(sui::types::Address::from_static("0x512"))
            .await
            .expect_err("unknown leader is rejected");
        assert!(matches!(unknown, NexusError::Configuration(_)));
        let excessive = actions
            .quote_priority_fee_withdrawal(leader_cap, 11)
            .await
            .expect_err("excessive withdrawal is rejected");
        assert!(matches!(excessive, NexusError::Configuration(_)));
    }

    #[tokio::test]
    async fn priority_fee_read_failure_keeps_rpc_context() {
        let objects = sui_mocks::mock_nexus_objects();
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = nexus_mocks::mock_nexus_client_without_coins(&objects, &rpc_url).await;

        let error = client
            .network()
            .fetch_priority_fee_vault_state()
            .await
            .expect_err("missing vault is rejected");

        assert!(matches!(error, NexusError::Rpc(_)));
        assert!(error
            .to_string()
            .contains("Failed to fetch priority fee vault state"));
    }

    #[tokio::test]
    async fn drain_requires_configured_rate_and_positive_sui_balance() {
        let objects = sui_mocks::mock_nexus_objects();
        let state = vault_state(sui::types::Address::from_static("0x521"), 1, 0, 9, 0, 1);
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        mock_vault_reads(&mut ledger_service, &objects, &state, 1);
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&objects, &rpc_url).await;

        let error = client
            .network()
            .drain_priority_fee_vault_sui(sui::types::Address::from_static("0x522"))
            .await
            .expect_err("empty unconfigured vault cannot be drained");

        assert!(matches!(error, NexusError::Configuration(_)));
    }

    #[tokio::test]
    async fn configure_priority_fee_vault_submits_transaction() {
        let objects = sui_mocks::mock_nexus_objects();
        let (client, submitted) = mutating_client(&objects, vec![], vec![]).await;

        let result = client
            .network()
            .configure_priority_fee_vault(17)
            .await
            .expect("vault configuration succeeds");

        assert_eq!(result.tx_digest, submitted.digest());
    }

    #[tokio::test]
    async fn swap_decodes_amounts_from_canonical_event() {
        let objects = sui_mocks::mock_nexus_objects();
        let coin_id = sui::types::Address::from_static("0x531");
        let coin_ref = sui_mocks::object_ref_for_id(coin_id);
        let event = swap_event(&objects, 100, 20, 70);
        let (client, submitted) = mutating_client(
            &objects,
            vec![(
                coin_ref,
                sui::types::Owner::Address(sui::types::Address::from_static("0x532")),
            )],
            vec![event],
        )
        .await;

        let result = client
            .network()
            .swap_us_for_sui(coin_id, 60)
            .await
            .expect("swap succeeds");

        assert_eq!(result.tx_digest, submitted.digest());
        assert_eq!(result.us_spent, 80);
        assert_eq!(result.us_refunded, 20);
        assert_eq!(result.sui_withdrawn, 70);
    }

    #[tokio::test]
    async fn swap_requires_canonical_event() {
        let objects = sui_mocks::mock_nexus_objects();
        let coin_id = sui::types::Address::from_static("0x541");
        let (client, _submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(coin_id),
                sui::types::Owner::Address(sui::types::Address::from_static("0x542")),
            )],
            vec![],
        )
        .await;

        let error = client
            .network()
            .swap_us_for_sui(coin_id, 1)
            .await
            .err()
            .expect("eventless swap is rejected");

        assert!(matches!(error, NexusError::Parsing(_)));
        assert!(error.to_string().contains("PriorityFeeSwapEvent not found"));
    }

    #[tokio::test]
    async fn swap_rejects_refund_larger_than_input() {
        let objects = sui_mocks::mock_nexus_objects();
        let coin_id = sui::types::Address::from_static("0x551");
        let event = swap_event(&objects, 10, 11, 1);
        let (client, _submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(coin_id),
                sui::types::Owner::Address(sui::types::Address::from_static("0x552")),
            )],
            vec![event],
        )
        .await;

        let error = client
            .network()
            .swap_us_for_sui(coin_id, 1)
            .await
            .err()
            .expect("invalid refund is rejected");

        assert!(matches!(error, NexusError::Parsing(_)));
        assert!(error.to_string().contains("refund exceeds"));
    }

    #[tokio::test]
    async fn priority_fee_withdrawal_resolves_cap_and_submits() {
        let objects = sui_mocks::mock_nexus_objects();
        let leader_cap = sui::types::Address::from_static("0x561");
        let (client, submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(leader_cap),
                sui::types::Owner::Address(sui::types::Address::from_static("0x562")),
            )],
            vec![],
        )
        .await;

        let result = client
            .network()
            .withdraw_priority_fee(leader_cap, 5)
            .await
            .expect("priority fee withdrawal succeeds");

        assert_eq!(result.tx_digest, submitted.digest());
    }
}
