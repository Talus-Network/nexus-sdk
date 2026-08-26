//! Commands related to Nexus network economics.

use {
    crate::{
        events::NexusEventKind,
        move_bindings::{
            registry::{
                era::V1 as RegistryWitnessV1,
                leader::{Leader, LeaderRegistry, LeaderRegistryInnerV1},
                priority_fee_vault::{
                    PriorityFeeDeposit,
                    PriorityFeeVault,
                    PriorityFeeVaultInnerV1,
                },
            },
            sui_framework::object::ID,
        },
        nexus::{client::NexusClient, crawler::Response, error::NexusError},
        sui,
        transactions::{leader, network},
        types::PriorityFeeWithdrawalQuote,
    },
    std::collections::HashMap,
};

pub const MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE: usize = 128;

pub struct ConfigurePriorityFeeVaultResult {
    pub tx_digest: sui::types::Digest,
}

/// Result of applying one complete leader registry policy.
pub struct ConfigureLeaderRegistryResult {
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

/// A deposit left under the vault because its embedded leader is no longer registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedPriorityFeeDeposit {
    pub deposit_id: sui::types::Address,
    pub leader_cap_id: sui::types::Address,
}

/// Collection outcome across one finite leader-scoped run.
///
/// Existing one-digest fee results cannot represent bounded multi-transaction collection,
/// removed-leader skips, or deposits consumed concurrently by another collector.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollectPriorityFeeDepositsResult {
    pub tx_digests: Vec<sui::types::Digest>,
    pub collected_deposit_ids: Vec<sui::types::Address>,
    pub skipped_old_leader_deposits: Vec<SkippedPriorityFeeDeposit>,
    pub unavailable_deposit_ids: Vec<sui::types::Address>,
}

/// Operations over Nexus network policy and fee state.
pub struct NetworkActions {
    pub(super) client: NexusClient,
}

impl NetworkActions {
    /// Apply a complete leader registry policy in one transaction.
    ///
    /// The operation resolves current object ownership before building the
    /// transaction, so an administration capability moved to consensus
    /// ownership remains usable after publication and upgrade.
    pub async fn configure_leader_registry(
        &self,
        unbonding_duration_ms: u64,
        min_stake_us: u64,
        max_transaction_budget_mist: u64,
    ) -> Result<ConfigureLeaderRegistryResult, NexusError> {
        let client = &self.client;
        let address = client.owner()?;
        let context = client
            .context_for_root(&client.nexus_objects.leader_registry)
            .await?;
        let admin_cap = client
            .crawler()
            .get_object_metadata(client.nexus_objects.leader_admin_cap.object_id())
            .await
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch leader administration capability metadata: {error}"
                ))
            })?;
        let transaction = leader::configure_registry_ptb(
            &context,
            &admin_cap.object_ref(),
            &admin_cap.owner,
            unbonding_duration_ms,
            min_stake_us,
            max_transaction_budget_mist,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;

        Ok(ConfigureLeaderRegistryResult {
            tx_digest: response.digest,
        })
    }

    /// Fetch and decode the priority fee vault state.
    pub async fn fetch_priority_fee_vault_state(
        &self,
    ) -> Result<PriorityFeeVaultInnerV1, NexusError> {
        Self::fetch_priority_fee_vault_state_with(&self.client).await
    }

    async fn fetch_priority_fee_vault_state_with(
        client: &NexusClient,
    ) -> Result<PriorityFeeVaultInnerV1, NexusError> {
        let context = client
            .context_for_root(&client.nexus_objects.priority_fee_vault)
            .await
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch priority fee vault state: {error}"
                ))
            })?;
        client
            .state_resolver()
            .load_inner::<PriorityFeeVault, RegistryWitnessV1, PriorityFeeVaultInnerV1>(
                client.nexus_objects.priority_fee_vault.object_id(),
                &context,
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
        let state = Self::fetch_priority_fee_vault_state_with(&self.client).await?;
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
        let state = Self::fetch_priority_fee_vault_state_with(&self.client).await?;
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
        let client = &self.client;
        let address = client.owner()?;
        let context = client
            .context_for_root(&client.nexus_objects.priority_fee_vault)
            .await?;
        let owner_cap = client
            .object_reference(
                client
                    .nexus_objects
                    .priority_fee_vault_owner_cap
                    .object_id(),
            )
            .await?;
        let transaction = network::configure_priority_fee_vault(
            &context,
            &owner_cap,
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
        Self::swap_us_for_sui_with(&self.client, us_coin, min_sui_out).await
    }

    async fn swap_us_for_sui_with(
        client: &NexusClient,
        us_coin: sui::types::Address,
        min_sui_out: u64,
    ) -> Result<SwapUsForSuiResult, NexusError> {
        let address = client.owner()?;
        let context = client
            .context_for_root(&client.nexus_objects.priority_fee_vault)
            .await?;
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
        let transaction = network::swap_us_for_sui(&context, &us_coin, min_sui_out, address)
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
        let client = &self.client;
        let state = Self::fetch_priority_fee_vault_state_with(client).await?;
        let quote = state.quote_sui_drain().ok_or_else(|| {
            NexusError::Configuration(
                "Priority fee vault must have a configured exchange rate and positive SUI balance to drain"
                    .to_owned(),
            )
        })?;
        let min_sui_out = quote.sui_out;
        let result = Self::swap_us_for_sui_with(client, us_coin, min_sui_out).await?;

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
        let client = &self.client;
        let address = client.owner()?;
        let context = client
            .context_for_object_with_roots(
                client.nexus_objects.priority_fee_vault.object_id(),
                std::slice::from_ref(&client.nexus_objects.leader_registry),
            )
            .await?;
        let leader_cap = client
            .crawler()
            .get_object_metadata(leader_cap)
            .await
            .map_err(|error| {
                NexusError::Rpc(anyhow::anyhow!(
                    "Failed to fetch leader capability metadata: {error}"
                ))
            })?;
        let leader_cap_ref = leader_cap.object_ref();
        let transaction = network::withdraw_priority_fee(
            &context,
            &leader_cap_ref,
            &leader_cap.owner,
            share_to_withdraw,
            address,
        )
        .map_err(NexusError::TransactionBuilding)?;
        let response = client.submit_transaction(transaction, address).await?;

        Ok(WithdrawPriorityFeeResult {
            tx_digest: response.digest,
        })
    }

    /// Collect the finite priority-fee deposit set visible for one leader capability.
    ///
    /// Each batch rescans only to refresh references or observe concurrent consumption. Deposits
    /// created after the initial scan are intentionally deferred to the next invocation.
    pub async fn collect_priority_fee_deposits(
        &self,
        leader_cap_id: sui::types::Address,
        batch_size: usize,
    ) -> Result<CollectPriorityFeeDepositsResult, NexusError> {
        validate_priority_fee_batch_size(batch_size)?;

        let client = &self.client;
        let frozen_ids = discover_priority_fee_deposits(client)
            .await?
            .into_iter()
            .filter(|deposit| deposit.data.leader_cap_id.bytes == leader_cap_id)
            .map(|deposit| deposit.object_id)
            .collect::<Vec<_>>();
        let mut result = CollectPriorityFeeDepositsResult::default();

        for batch in frozen_ids.chunks(batch_size) {
            let batch_result =
                collect_priority_fee_deposit_batch(client, batch.to_vec(), false).await?;
            result.tx_digests.extend(batch_result.tx_digests);
            result
                .collected_deposit_ids
                .extend(batch_result.collected_deposit_ids);
            result
                .skipped_old_leader_deposits
                .extend(batch_result.skipped_old_leader_deposits);
            result
                .unavailable_deposit_ids
                .extend(batch_result.unavailable_deposit_ids);
        }

        Ok(result)
    }
}

fn validate_priority_fee_batch_size(batch_size: usize) -> Result<(), NexusError> {
    if !(1..=MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE).contains(&batch_size) {
        return Err(NexusError::Configuration(format!(
            "priority fee deposit batch size must be in 1..={MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE}, got {batch_size}"
        )));
    }
    Ok(())
}

async fn discover_priority_fee_deposits(
    client: &NexusClient,
) -> Result<Vec<Response<PriorityFeeDeposit>>, NexusError> {
    let vault_id = client.nexus_objects.priority_fee_vault.object_id();
    let context = client
        .context_for_root(&client.nexus_objects.priority_fee_vault)
        .await?;
    let object_type = crate::move_bindings::struct_tag::<PriorityFeeDeposit>(&context);
    let deposits = client
        .crawler()
        .get_owned_objects::<PriorityFeeDeposit>(vault_id, object_type)
        .await
        .map_err(|error| {
            NexusError::Rpc(anyhow::anyhow!(
                "Failed to discover priority fee deposits owned by vault '{vault_id}': {error}"
            ))
        })?;

    Ok(deposits)
}

async fn prepare_priority_fee_deposit_batch(
    client: &NexusClient,
    target_ids: &[sui::types::Address],
    reject_missing: bool,
) -> Result<
    (
        Vec<Response<PriorityFeeDeposit>>,
        Vec<SkippedPriorityFeeDeposit>,
        Vec<sui::types::Address>,
    ),
    NexusError,
> {
    let mut discovered = discover_priority_fee_deposits(client)
        .await?
        .into_iter()
        .map(|deposit| (deposit.object_id, deposit))
        .collect::<HashMap<_, _>>();
    let mut selected = Vec::with_capacity(target_ids.len());
    let mut unavailable = Vec::new();
    for target_id in target_ids {
        match discovered.remove(target_id) {
            Some(deposit) => selected.push(deposit),
            None => unavailable.push(*target_id),
        }
    }
    if reject_missing && !unavailable.is_empty() {
        let missing = unavailable
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(NexusError::Configuration(format!(
            "priority fee deposits are not present at the configured vault address: {missing}"
        )));
    }
    if selected.is_empty() {
        return Ok((selected, Vec::new(), unavailable));
    }

    let context = client
        .context_for_object_with_roots(
            client.nexus_objects.priority_fee_vault.object_id(),
            std::slice::from_ref(&client.nexus_objects.leader_registry),
        )
        .await?;
    let leader_registry = client
        .state_resolver()
        .load_inner::<LeaderRegistry, RegistryWitnessV1, LeaderRegistryInnerV1>(
            client.nexus_objects.leader_registry.object_id(),
            &context,
        )
        .await
        .map_err(|error| {
            NexusError::Rpc(anyhow::anyhow!(
                "Failed to fetch leader registry state for priority fee collection: {error}"
            ))
        })?;
    let leader_ids = selected
        .iter()
        .map(|deposit| deposit.data.leader_cap_id)
        .collect::<Vec<_>>();
    if leader_registry.data.records.size() == 0 {
        let skipped = selected
            .into_iter()
            .map(|deposit| SkippedPriorityFeeDeposit {
                deposit_id: deposit.object_id,
                leader_cap_id: deposit.data.leader_cap_id.bytes,
            })
            .collect();
        return Ok((Vec::new(), skipped, unavailable));
    }
    let leader_id_type = crate::move_bindings::type_tag::<ID>(&context);
    let registered = client
        .crawler()
        .get_dynamic_fields_by_keys::<ID, Leader, _>(
            leader_registry.data.records.id(),
            leader_ids,
            &leader_id_type,
        )
        .await
        .map_err(|error| {
            NexusError::Rpc(anyhow::anyhow!(
                "Failed to validate priority fee deposit leaders: {error}"
            ))
        })?;

    let mut valid = Vec::with_capacity(selected.len());
    let mut skipped = Vec::new();
    for deposit in selected {
        if registered.contains_key(&deposit.data.leader_cap_id) {
            valid.push(deposit);
        } else {
            skipped.push(SkippedPriorityFeeDeposit {
                deposit_id: deposit.object_id,
                leader_cap_id: deposit.data.leader_cap_id.bytes,
            });
        }
    }
    Ok((valid, skipped, unavailable))
}

async fn collect_priority_fee_deposit_batch(
    client: &NexusClient,
    target_ids: Vec<sui::types::Address>,
    reject_initial_missing: bool,
) -> Result<CollectPriorityFeeDepositsResult, NexusError> {
    let (deposits, mut skipped, unavailable) =
        prepare_priority_fee_deposit_batch(client, &target_ids, reject_initial_missing).await?;
    if deposits.is_empty() {
        return Ok(CollectPriorityFeeDepositsResult {
            skipped_old_leader_deposits: skipped,
            unavailable_deposit_ids: unavailable,
            ..Default::default()
        });
    }

    let owner = client.owner()?;
    let deposit_ids = deposits
        .iter()
        .map(|deposit| deposit.object_id)
        .collect::<Vec<_>>();
    let deposit_refs = deposits
        .iter()
        .map(Response::object_ref)
        .collect::<Vec<_>>();
    let context = client
        .context_for_object_with_roots(
            client.nexus_objects.priority_fee_vault.object_id(),
            std::slice::from_ref(&client.nexus_objects.leader_registry),
        )
        .await?;
    let transaction = network::collect_priority_fee_deposits(&context, &deposit_refs)
        .map_err(NexusError::TransactionBuilding)?;
    match client.submit_transaction(transaction, owner).await {
        Ok(response) => Ok(CollectPriorityFeeDepositsResult {
            tx_digests: vec![response.digest],
            collected_deposit_ids: deposit_ids,
            skipped_old_leader_deposits: skipped,
            unavailable_deposit_ids: unavailable,
        }),
        Err(_) => {
            let (refreshed, retry_skipped, retry_unavailable) =
                prepare_priority_fee_deposit_batch(client, &deposit_ids, false).await?;
            skipped.extend(retry_skipped);
            let mut unavailable = unavailable;
            unavailable.extend(retry_unavailable);
            if refreshed.is_empty() {
                return Ok(CollectPriorityFeeDepositsResult {
                    skipped_old_leader_deposits: skipped,
                    unavailable_deposit_ids: unavailable,
                    ..Default::default()
                });
            }

            let collected_deposit_ids = refreshed
                .iter()
                .map(|deposit| deposit.object_id)
                .collect::<Vec<_>>();
            let refreshed_refs = refreshed
                .iter()
                .map(Response::object_ref)
                .collect::<Vec<_>>();
            let context = client
                .context_for_object_with_roots(
                    client.nexus_objects.priority_fee_vault.object_id(),
                    std::slice::from_ref(&client.nexus_objects.leader_registry),
                )
                .await?;
            let transaction = network::collect_priority_fee_deposits(&context, &refreshed_refs)
                .map_err(NexusError::TransactionBuilding)?;
            let response = client.submit_transaction(transaction, owner).await?;
            Ok(CollectPriorityFeeDepositsResult {
                tx_digests: vec![response.digest],
                collected_deposit_ids,
                skipped_old_leader_deposits: skipped,
                unavailable_deposit_ids: unavailable,
            })
        }
    }
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                primitives::{data::NexusData, event::EventWrapper},
                registry::{
                    leader::{LeaderStatus, Metadata, StakeManager},
                    priority_fee_vault::{PriorityFeeAccount, PriorityFeeSwapEvent},
                },
                sui_framework::{
                    balance::Balance,
                    object::{ID, UID},
                    sui::SUI,
                    table::Table,
                    vec_map::{Entry, VecMap},
                },
                talus::us::US,
            },
            test_utils::{nexus_mocks, sui_mocks},
            types::{NexusContext, PackageRole},
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
    ) -> PriorityFeeVaultInnerV1 {
        PriorityFeeVaultInnerV1::new(
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
        state_service: &mut sui_mocks::grpc::MockStateService,
        objects: &NexusContext,
        state: &PriorityFeeVaultInnerV1,
    ) {
        let vault_id = objects.priority_fee_vault.object_id();
        sui_mocks::grpc::mock_object_state::<
            PriorityFeeVault,
            RegistryWitnessV1,
            PriorityFeeVaultInnerV1,
        >(
            ledger_service,
            state_service,
            objects,
            sui_mocks::object_ref_for_id(vault_id),
            sui::types::Owner::Shared(objects.priority_fee_vault.initial_shared_version),
            PriorityFeeVault::new(UID::new(vault_id)),
            state.clone(),
        );
    }

    #[derive(Serialize)]
    struct Wrapper<T> {
        event: T,
    }

    fn swap_event(
        objects: &NexusContext,
        us_in: u64,
        us_refunded: u64,
        sui_out: u64,
    ) -> sui::types::Event {
        let event = PriorityFeeSwapEvent::new(
            ID::new(objects.priority_fee_vault.object_id()),
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
            objects
                .require_package(PackageRole::Registry)
                .unwrap()
                .storage_id,
            wrapper,
            bcs::to_bytes(&Wrapper { event }).expect("swap event serializes"),
        )
    }

    async fn mutating_client(
        objects: &NexusContext,
        metadata: Vec<(sui::types::ObjectReference, sui::types::Owner)>,
        events: Vec<sui::types::Event>,
        require_leader_registry: bool,
    ) -> (NexusClient, sui_mocks::grpc::SubmittedTransaction) {
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        let mut transaction_service = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service = sui_mocks::grpc::MockSubscriptionService::new();
        mock_vault_reads(
            &mut ledger_service,
            &mut state_service,
            objects,
            &PriorityFeeVaultInnerV1::new(
                Balance::<SUI>::new(0),
                Balance::<US>::new(0),
                0,
                0,
                VecMap::new(vec![]),
            ),
        );
        if require_leader_registry {
            mock_leader_registry_state(
                &mut ledger_service,
                &mut state_service,
                objects,
                sui::types::Address::from_static("0x503"),
                0,
            );
        }
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            objects.packages(),
        );
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
            package_service_mock: Some(package_service),
            execution_service_mock: Some(transaction_service),
            subscription_service_mock: Some(subscription_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(objects, &rpc_url).await;
        (client, submitted)
    }

    fn mock_priority_fee_deposit_scans(
        state_service: &mut sui_mocks::grpc::MockStateService,
        objects: &NexusContext,
        scans: Vec<Vec<(sui::types::ObjectReference, sui::types::Address)>>,
    ) {
        let vault_id = objects.priority_fee_vault.object_id();
        let object_type = crate::move_bindings::struct_tag::<PriorityFeeDeposit>(objects);
        let grpc_scans = scans
            .into_iter()
            .map(|deposits| {
                deposits
                    .into_iter()
                    .map(|(object_ref, leader_cap_id)| {
                        let deposit = PriorityFeeDeposit::new(
                            UID::new(*object_ref.object_id()),
                            Balance::<SUI>::new(1),
                            ID::new(leader_cap_id),
                        );
                        let mut object = sui::grpc::Object::default();
                        object.set_object_id(object_ref.object_id().to_string());
                        object.set_version(object_ref.version());
                        object.set_digest(*object_ref.digest());
                        object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Address(
                            vault_id,
                        )));
                        object.set_object_type(object_type.to_string());
                        let mut contents = sui::grpc::Bcs::default();
                        contents.set_name(object_type.to_string());
                        contents.set_value(bcs::to_bytes(&deposit).expect("deposit serializes"));
                        object.contents = Some(contents);
                        object
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let scan_count = grpc_scans.len();
        let mut grpc_scans = grpc_scans.into_iter();
        let expected_owner = vault_id.to_string();
        let expected_type = object_type.to_string();
        state_service
            .expect_list_owned_objects()
            .times(scan_count)
            .returning(move |request| {
                let request = request.get_ref();
                assert_eq!(request.owner.as_deref(), Some(expected_owner.as_str()));
                assert_eq!(request.object_type.as_deref(), Some(expected_type.as_str()));
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(grpc_scans.next().expect("configured deposit scan"));
                Ok(tonic::Response::new(response))
            });
    }

    fn mock_leader_registry_state(
        ledger_service: &mut sui_mocks::grpc::MockLedgerService,
        state_service: &mut sui_mocks::grpc::MockStateService,
        objects: &NexusContext,
        records_id: sui::types::Address,
        record_count: u64,
    ) {
        let registry_id = objects.leader_registry.object_id();
        let mut state = LeaderRegistryInnerV1::new_for_test(registry_id, objects.network_id);
        state.records = Table::new(records_id, record_count);
        sui_mocks::grpc::mock_object_state::<
            LeaderRegistry,
            RegistryWitnessV1,
            LeaderRegistryInnerV1,
        >(
            ledger_service,
            state_service,
            objects,
            sui_mocks::object_ref_for_id(registry_id),
            sui::types::Owner::Shared(objects.leader_registry.initial_shared_version),
            LeaderRegistry::new(UID::new(registry_id)),
            state,
        );
    }

    fn active_leader(records_id: sui::types::Address) -> Leader {
        Leader::new(
            LeaderStatus::Active,
            Metadata::new(VecMap::new(vec![])),
            StakeManager::<US>::new(Balance::<US>::new(0), 0, Table::new(records_id, 0)),
            vec![],
        )
    }

    #[test]
    fn priority_fee_batch_size_validation_enforces_bounds() {
        assert!(validate_priority_fee_batch_size(0).is_err());
        assert!(validate_priority_fee_batch_size(MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE + 1).is_err());
        assert!(validate_priority_fee_batch_size(1).is_ok());
        assert!(validate_priority_fee_batch_size(MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE).is_ok());
    }

    #[tokio::test]
    async fn priority_fee_collection_skips_a_removed_leader_without_submitting() {
        let objects = sui_mocks::mock_nexus_context();
        let deposit_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x572"));
        let removed_leader = sui::types::Address::from_static("0x573");
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_priority_fee_deposit_scans(
            &mut state_service,
            &objects,
            vec![
                vec![(deposit_ref.clone(), removed_leader)],
                vec![(deposit_ref.clone(), removed_leader)],
            ],
        );
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        mock_leader_registry_state(
            &mut ledger_service,
            &mut state_service,
            &objects,
            sui::types::Address::from_static("0x574"),
            0,
        );
        mock_vault_reads(
            &mut ledger_service,
            &mut state_service,
            &objects,
            &PriorityFeeVaultInnerV1::new(
                Balance::<SUI>::new(0),
                Balance::<US>::new(0),
                0,
                0,
                VecMap::new(vec![]),
            ),
        );
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            objects.packages(),
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client_without_coins(&objects, &rpc_url).await;

        let result = client
            .network()
            .collect_priority_fee_deposits(removed_leader, MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE)
            .await
            .expect("removed leader deposits are reported");

        assert!(result.tx_digests.is_empty());
        assert!(result.collected_deposit_ids.is_empty());
        assert_eq!(
            result.skipped_old_leader_deposits,
            vec![SkippedPriorityFeeDeposit {
                deposit_id: *deposit_ref.object_id(),
                leader_cap_id: removed_leader,
            }]
        );
        assert!(result.unavailable_deposit_ids.is_empty());
    }

    #[tokio::test]
    async fn priority_fee_collection_submits_registered_deposits() {
        let objects = sui_mocks::mock_nexus_context();
        let deposit_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x575"));
        let unrelated_ref =
            sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x5751"));
        let leader_cap_id = sui::types::Address::from_static("0x576");
        let unrelated_leader = sui::types::Address::from_static("0x5761");
        let records_id = sui::types::Address::from_static("0x577");
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_priority_fee_deposit_scans(
            &mut state_service,
            &objects,
            vec![
                vec![
                    (deposit_ref.clone(), leader_cap_id),
                    (unrelated_ref.clone(), unrelated_leader),
                ],
                vec![
                    (deposit_ref.clone(), leader_cap_id),
                    (unrelated_ref, unrelated_leader),
                ],
            ],
        );

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        mock_leader_registry_state(
            &mut ledger_service,
            &mut state_service,
            &objects,
            records_id,
            1,
        );
        mock_vault_reads(
            &mut ledger_service,
            &mut state_service,
            &objects,
            &PriorityFeeVaultInnerV1::new(
                Balance::<SUI>::new(0),
                Balance::<US>::new(0),
                0,
                0,
                VecMap::new(vec![]),
            ),
        );
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            objects.packages(),
        );
        let leader_id = ID::new(leader_cap_id);
        let leader_id_type = crate::move_bindings::type_tag::<ID>(&objects);
        let field_id = records_id.derive_dynamic_child_id(
            &leader_id_type,
            &bcs::to_bytes(&leader_id).expect("leader ID serializes"),
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service,
            vec![(
                sui_mocks::object_ref_for_id(field_id),
                sui::types::Owner::Object(records_id),
                leader_id,
                active_leader(sui::types::Address::from_static("0x578")),
            )],
        );
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 1_000);
        let mut transaction_service = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service = sui_mocks::grpc::MockSubscriptionService::new();
        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut transaction_service,
            &mut subscription_service,
            &mut ledger_service,
            sui_mocks::mock_sui_object_ref(),
            vec![],
            vec![],
            vec![],
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            execution_service_mock: Some(transaction_service),
            subscription_service_mock: Some(subscription_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&objects, &rpc_url).await;

        let result = client
            .network()
            .collect_priority_fee_deposits(leader_cap_id, MAX_PRIORITY_FEE_DEPOSIT_BATCH_SIZE)
            .await
            .expect("registered deposit collection succeeds");

        assert_eq!(result.tx_digests, vec![submitted.digest()]);
        assert_eq!(result.collected_deposit_ids, vec![*deposit_ref.object_id()]);
        assert!(result.skipped_old_leader_deposits.is_empty());
        assert!(result.unavailable_deposit_ids.is_empty());
    }

    #[tokio::test]
    async fn leader_collection_freezes_initial_ids_and_reports_concurrent_unavailability() {
        let objects = sui_mocks::mock_nexus_context();
        let first_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x579"));
        let second_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x57b"));
        let later_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x57c"));
        let unrelated_ref =
            sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x57c1"));
        let leader_cap_id = sui::types::Address::from_static("0x57d");
        let unrelated_leader = sui::types::Address::from_static("0x57d1");
        let records_id = sui::types::Address::from_static("0x57e");
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_priority_fee_deposit_scans(
            &mut state_service,
            &objects,
            vec![
                vec![
                    (first_ref.clone(), leader_cap_id),
                    (second_ref.clone(), leader_cap_id),
                    (unrelated_ref.clone(), unrelated_leader),
                ],
                vec![
                    (first_ref.clone(), leader_cap_id),
                    (unrelated_ref.clone(), unrelated_leader),
                ],
                vec![
                    (later_ref, leader_cap_id),
                    (unrelated_ref, unrelated_leader),
                ],
            ],
        );

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        mock_leader_registry_state(
            &mut ledger_service,
            &mut state_service,
            &objects,
            records_id,
            1,
        );
        mock_vault_reads(
            &mut ledger_service,
            &mut state_service,
            &objects,
            &PriorityFeeVaultInnerV1::new(
                Balance::<SUI>::new(0),
                Balance::<US>::new(0),
                0,
                0,
                VecMap::new(vec![]),
            ),
        );
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            objects.packages(),
        );
        let leader_id = ID::new(leader_cap_id);
        let leader_id_type = crate::move_bindings::type_tag::<ID>(&objects);
        let field_id = records_id.derive_dynamic_child_id(
            &leader_id_type,
            &bcs::to_bytes(&leader_id).expect("leader ID serializes"),
        );
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service,
            vec![(
                sui_mocks::object_ref_for_id(field_id),
                sui::types::Owner::Object(records_id),
                leader_id,
                active_leader(sui::types::Address::from_static("0x57f")),
            )],
        );
        sui_mocks::grpc::mock_reference_gas_price(&mut ledger_service, 1_000);
        let mut transaction_service = sui_mocks::grpc::MockTransactionExecutionService::new();
        let mut subscription_service = sui_mocks::grpc::MockSubscriptionService::new();
        let submitted = sui_mocks::grpc::mock_execute_transaction_and_wait_for_checkpoint(
            &mut transaction_service,
            &mut subscription_service,
            &mut ledger_service,
            sui_mocks::mock_sui_object_ref(),
            vec![],
            vec![],
            vec![],
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            execution_service_mock: Some(transaction_service),
            subscription_service_mock: Some(subscription_service),
            state_service_mock: Some(state_service),
            ..Default::default()
        });
        let client = nexus_mocks::mock_nexus_client(&objects, &rpc_url).await;

        let result = client
            .network()
            .collect_priority_fee_deposits(leader_cap_id, 1)
            .await
            .expect("finite leader collection succeeds");

        assert_eq!(result.tx_digests, vec![submitted.digest()]);
        assert_eq!(result.collected_deposit_ids, vec![*first_ref.object_id()]);
        assert_eq!(
            result.unavailable_deposit_ids,
            vec![*second_ref.object_id()]
        );
        assert!(result.skipped_old_leader_deposits.is_empty());
    }

    #[tokio::test]
    async fn priority_fee_reads_decode_state_and_classify_invalid_requests() {
        let objects = sui_mocks::mock_nexus_context();
        let leader_cap = sui::types::Address::from_static("0x511");
        let state = vault_state(leader_cap, 10, 0, 90, 3, 30);
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_vault_reads(&mut ledger_service, &mut state_service, &objects, &state);
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            objects.packages(),
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
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
        let objects = sui_mocks::mock_nexus_context();
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
        let objects = sui_mocks::mock_nexus_context();
        let state = vault_state(sui::types::Address::from_static("0x521"), 1, 0, 9, 0, 1);
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        let mut package_service = sui_mocks::grpc::MockMovePackageService::new();
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        mock_vault_reads(&mut ledger_service, &mut state_service, &objects, &state);
        sui_mocks::grpc::mock_nexus_package_graph(
            &mut ledger_service,
            &mut package_service,
            objects.packages(),
        );
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            package_service_mock: Some(package_service),
            state_service_mock: Some(state_service),
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
        let objects = sui_mocks::mock_nexus_context();
        let owner_cap = objects.priority_fee_vault_owner_cap.object_id();
        let (client, submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(owner_cap),
                sui::types::Owner::Address(sui::types::Address::ZERO),
            )],
            vec![],
            false,
        )
        .await;

        let result = client
            .network()
            .configure_priority_fee_vault(17)
            .await
            .expect("vault configuration succeeds");

        assert_eq!(result.tx_digest, submitted.digest());
    }

    #[tokio::test]
    async fn swap_decodes_amounts_from_canonical_event() {
        let objects = sui_mocks::mock_nexus_context();
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
            false,
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
        let objects = sui_mocks::mock_nexus_context();
        let coin_id = sui::types::Address::from_static("0x541");
        let (client, _submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(coin_id),
                sui::types::Owner::Address(sui::types::Address::from_static("0x542")),
            )],
            vec![],
            false,
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
        let objects = sui_mocks::mock_nexus_context();
        let coin_id = sui::types::Address::from_static("0x551");
        let event = swap_event(&objects, 10, 11, 1);
        let (client, _submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(coin_id),
                sui::types::Owner::Address(sui::types::Address::from_static("0x552")),
            )],
            vec![event],
            false,
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
        let objects = sui_mocks::mock_nexus_context();
        let leader_cap = sui::types::Address::from_static("0x561");
        let (client, submitted) = mutating_client(
            &objects,
            vec![(
                sui_mocks::object_ref_for_id(leader_cap),
                sui::types::Owner::Address(sui::types::Address::from_static("0x562")),
            )],
            vec![],
            true,
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
