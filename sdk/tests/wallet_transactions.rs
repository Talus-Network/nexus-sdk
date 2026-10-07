#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        events::NexusEventDecoder,
        nexus::{
            address_balance::{finish_transaction, SubmissionContext},
            client::NexusClient,
            crawler::Crawler,
            error::NexusError,
            signer::Signer,
            state::StateResolver,
            wallet::WalletClient,
        },
        sui::{self, crypto::SuiVerifier as _},
        test_utils::sui_mocks::{
            self,
            grpc::{
                self,
                MockLedgerService,
                MockSubscriptionService,
                MockTransactionExecutionService,
                ServerMocks,
            },
        },
    },
    std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    },
};

fn empty_ptb() -> sui::types::ProgrammableTransaction {
    sui::types::ProgrammableTransaction {
        inputs: vec![],
        commands: vec![],
    }
}

fn signer(rpc_url: &str) -> Signer {
    let client = Arc::new(sui::grpc::client(rpc_url).unwrap());
    Signer::new(
        Arc::clone(&client),
        sui::crypto::Ed25519PrivateKey::new([17; 32]),
        Duration::from_secs(3),
        NexusEventDecoder::new(
            StateResolver::new(Arc::new(Crawler::new(client))),
            Arc::new(sui_mocks::mock_nexus_objects()),
        ),
    )
}

fn transaction(signer: &Signer, chain: sui::types::Digest) -> sui::types::Transaction {
    finish_transaction(
        empty_ptb(),
        signer.get_active_address(),
        1_000_000,
        SubmissionContext {
            reference_gas_price: 1_000,
            epoch: 7,
            chain,
        },
        1,
    )
}

#[tokio::test]
async fn wallet_metadata_is_lazy_and_shared_across_signer_clones() {
    let chain = sui::types::Digest::new([9; 32]);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_service_info().returning(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(tonic::Response::new(
            sui::grpc::GetServiceInfoResponse::default()
                .with_chain_id(chain)
                .with_chain("testnet"),
        ))
    });
    let url = grpc::mock_server(ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let signer = signer(&url);
    let transaction = transaction(&signer, chain);
    let signature = signer.sign_tx(&transaction).await.unwrap();
    sui::crypto::ed25519::Ed25519VerifyingKey::new(
        &sui::crypto::Ed25519PrivateKey::new([17; 32]).public_key(),
    )
    .unwrap()
    .verify_transaction(&transaction, &signature)
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let cloned = signer.clone();
    let (chain_id, network) = tokio::join!(signer.wallet().chain_id(), cloned.wallet().chain());
    assert_eq!(chain_id.unwrap(), chain.to_string());
    assert_eq!(network.unwrap(), "testnet");
    assert_eq!(cloned.wallet().chain_id().await.unwrap(), chain.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn wallet_retries_failed_metadata_lookups_and_reports_a_missing_network() {
    let chain = sui::types::Digest::new([9; 32]);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_service_info().returning(move |_| {
        match observed.fetch_add(1, Ordering::SeqCst) {
            0 => Err(tonic::Status::permission_denied("service info denied")),
            1 => Ok(tonic::Response::new(
                sui::grpc::GetServiceInfoResponse::default(),
            )),
            _ => Ok(tonic::Response::new(
                sui::grpc::GetServiceInfoResponse::default().with_chain_id(chain),
            )),
        }
    });
    let url = grpc::mock_server(ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let signer = signer(&url);
    let wallet = signer.wallet();
    assert!(matches!(wallet.chain_id().await, Err(NexusError::Rpc(_))));
    assert!(matches!(wallet.chain_id().await,
        Err(NexusError::Configuration(message)) if message.contains("chain ID")));
    assert_eq!(wallet.chain_id().await.unwrap(), chain.to_string());
    assert!(matches!(wallet.chain().await,
        Err(NexusError::Configuration(message)) if message.contains("network")));
    assert_eq!(wallet.clone().chain_id().await.unwrap(), chain.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn wallet_loads_checkpoint_support_on_the_first_signer_submission() {
    let chain = sui::types::Digest::new([9; 32]);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_service_info().returning(move |_| {
        observed.fetch_add(1, Ordering::SeqCst);
        let mut response =
            tonic::Response::new(sui::grpc::GetServiceInfoResponse::default().with_chain_id(chain));
        response
            .metadata_mut()
            .insert("x-sui-checkpoint-wait", "true".parse().unwrap());
        Ok(response)
    });
    let mut execution = MockTransactionExecutionService::new();
    let mut subscription = MockSubscriptionService::new();
    let submitted = grpc::mock_checkpointed_execute_transaction_without_gas(
        &mut execution,
        &mut subscription,
        &mut ledger,
        true,
        vec![],
        vec![],
        vec![],
        |_| {},
    );
    let url = grpc::mock_server(ServerMocks {
        ledger_service_mock: Some(ledger),
        execution_service_mock: Some(execution),
        subscription_service_mock: Some(subscription),
        ..Default::default()
    });
    let signer = signer(&url);
    let transaction = transaction(&signer, chain);
    let signature = signer.sign_tx(&transaction).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let result = signer
        .execute_tx_without_gas_coin(transaction, signature)
        .await
        .unwrap();
    assert_eq!(result.digest, submitted.digest());
    assert_eq!(result.checkpoint, 1);
    assert_eq!(signer.wallet().chain_id().await.unwrap(), chain.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn builder_initializes_wallet_metadata_once_and_rechecks_shared_wallets() {
    let original = sui::types::Digest::new([9; 32]);
    let changed = sui::types::Digest::new([8; 32]);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_service_info().returning(move |_| {
        let chain = if observed.fetch_add(1, Ordering::SeqCst) == 0 {
            original
        } else {
            changed
        };
        Ok(tonic::Response::new(
            sui::grpc::GetServiceInfoResponse::default().with_chain_id(chain),
        ))
    });
    let url = grpc::mock_server(ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    let mut objects = sui_mocks::mock_nexus_objects();
    objects.chain_id = original.to_string();
    let client = NexusClient::builder()
        .with_rpc_url(&url)
        .with_private_key(sui::crypto::Ed25519PrivateKey::new([17; 32]))
        .with_nexus_objects(objects.clone())
        .build()
        .await
        .unwrap();
    let wallet = client.wallet().unwrap().clone();
    assert_eq!(wallet.chain_id().await.unwrap(), original.to_string());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let result = NexusClient::builder()
        .with_wallet(wallet)
        .with_nexus_objects(objects)
        .build()
        .await;
    assert!(matches!(result,
        Err(NexusError::ChainMismatch { expected, actual })
            if expected == original.to_string() && actual == changed.to_string()));
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn shared_wallet_prepares_signs_and_confirms_transactions() {
    for succeeds in [true, false] {
        let key = sui::crypto::Ed25519PrivateKey::new([17; 32]);
        let owner = key.public_key().derive_address();
        let chain = sui::types::Digest::new([9; 32]);
        let mut ledger = MockLedgerService::new();
        grpc::mock_submission_context(&mut ledger, 1_000, 7);
        let mut execution = MockTransactionExecutionService::new();
        execution
            .expect_execute_transaction()
            .times(1)
            .returning(move |request| {
                assert_eq!(
                    request.metadata().get("x-sui-checkpoint-wait").unwrap(),
                    "true"
                );
                let request = request.into_inner();
                let transaction = sui::types::Transaction::try_from(request.transaction()).unwrap();
                assert_eq!(transaction.sender, owner);
                assert_eq!(transaction.gas_payment.owner, owner);
                assert!(transaction.gas_payment.objects.is_empty());
                assert_eq!(transaction.gas_payment.budget, 1_000_000);
                assert_eq!(transaction.gas_payment.price, 1_000);
                assert_eq!(request.signatures.len(), 1);
                let signature =
                    sui::types::UserSignature::try_from(&request.signatures[0]).unwrap();
                sui::crypto::ed25519::Ed25519VerifyingKey::new(&key.public_key())
                    .unwrap()
                    .verify_transaction(&transaction, &signature)
                    .unwrap();
                let digest = transaction.digest();
                let effects = sui::types::TransactionEffects::V2(Box::new(
                    sui::types::TransactionEffectsV2 {
                        status: if succeeds {
                            sui::types::ExecutionStatus::Success
                        } else {
                            sui::types::ExecutionStatus::Failure {
                                error: sui::types::ExecutionError::InsufficientGas,
                                command: Some(0),
                            }
                        },
                        epoch: 7,
                        gas_used: sui::types::GasCostSummary {
                            computation_cost: 1_000,
                            storage_cost: 0,
                            storage_rebate: 0,
                            non_refundable_storage_fee: 0,
                        },
                        transaction_digest: digest,
                        gas_object_index: None,
                        events_digest: None,
                        dependencies: vec![],
                        lamport_version: 1,
                        changed_objects: vec![],
                        unchanged_consensus_objects: vec![],
                        auxiliary_data_digest: None,
                    },
                ));
                let response = sui::grpc::ExecutedTransaction::default()
                    .with_digest(digest)
                    .with_checkpoint(42)
                    .with_effects(
                        sui::grpc::TransactionEffects::default()
                            .with_bcs(bcs::to_bytes(&effects).unwrap()),
                    );
                Ok(tonic::Response::new(
                    sui::grpc::ExecuteTransactionResponse::default().with_transaction(response),
                ))
            });
        let url = grpc::mock_server(ServerMocks {
            chain_id: chain,
            checkpoint_wait_supported: true,
            ledger_service_mock: Some(ledger),
            execution_service_mock: Some(execution),
            ..Default::default()
        });
        let wallet = WalletClient::connect(&url, sui::crypto::Ed25519PrivateKey::new([17; 32]))
            .await
            .unwrap()
            .with_transaction_timeout(Duration::from_secs(3));
        let shared_wallet = wallet.clone();
        assert!(Arc::ptr_eq(
            &wallet.grpc_client(),
            &shared_wallet.grpc_client()
        ));
        assert_eq!(shared_wallet.chain_id().await.unwrap(), chain.to_string());
        assert_eq!(shared_wallet.rpc_url(), format!("{url}/"));
        assert_eq!(shared_wallet.transaction_timeout(), Duration::from_secs(3));
        let transaction = shared_wallet
            .prepare_transaction(empty_ptb(), 1_000_000)
            .await
            .unwrap();
        let digest = transaction.digest();
        let result = shared_wallet.execute_transaction(transaction).await;
        if succeeds {
            let executed = result.unwrap();
            assert_eq!(executed.digest(), digest.to_string());
            assert_eq!(executed.checkpoint(), 42);
        } else {
            let Err(NexusError::Transaction(error)) = result else {
                panic!("expected a confirmed execution failure")
            };
            assert!(matches!(*error,
                nexus_sdk::nexus::error::TransactionError::ExecutionFailed { digest: actual, checkpoint: 42, command: Some(0), .. }
                if actual == digest));
        }
    }
}

#[tokio::test]
async fn wallet_rejects_changed_chain_and_zero_gas_before_submission() {
    let original = sui::types::Digest::new([9; 32]);
    let changed = sui::types::Digest::new([8; 32]);
    let mut ledger = MockLedgerService::new();
    ledger
        .expect_get_service_info()
        .times(1)
        .returning(move |_| {
            Ok(tonic::Response::new(
                sui::grpc::GetServiceInfoResponse::default().with_chain_id(original),
            ))
        });
    grpc::mock_submission_context(&mut ledger, 1_000, 7);
    let mut execution = MockTransactionExecutionService::new();
    execution.expect_execute_transaction().times(0);
    let url = grpc::mock_server(ServerMocks {
        chain_id: changed,
        ledger_service_mock: Some(ledger),
        execution_service_mock: Some(execution),
        ..Default::default()
    });
    let wallet = WalletClient::connect(&url, sui::crypto::Ed25519PrivateKey::new([17; 32]))
        .await
        .unwrap();
    assert!(matches!(
        wallet.prepare_transaction(empty_ptb(), 0).await,
        Err(NexusError::Configuration(_))
    ));
    assert!(
        matches!(wallet.prepare_transaction(empty_ptb(), 1_000_000).await,
        Err(NexusError::ChainMismatch { expected, actual }) if expected == original.to_string() && actual == changed.to_string())
    );
}

#[tokio::test]
async fn wallet_connection_requires_chain_identity() {
    let mut ledger = MockLedgerService::new();
    ledger.expect_get_service_info().times(1).returning(|_| {
        Ok(tonic::Response::new(
            sui::grpc::GetServiceInfoResponse::default(),
        ))
    });
    let url = grpc::mock_server(ServerMocks {
        ledger_service_mock: Some(ledger),
        ..Default::default()
    });
    assert!(
        matches!(WalletClient::connect(&url, sui::crypto::Ed25519PrivateKey::new([17; 32])).await,
        Err(NexusError::Configuration(message)) if message.contains("chain ID"))
    );
}
