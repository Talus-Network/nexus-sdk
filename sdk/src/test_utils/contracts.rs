//! Move package submission support for integration tests.

use {
    crate::{
        events::NexusEventDecoder,
        nexus::{
            crawler::Crawler,
            signer::{ExecutedTransaction, Signer},
            state::StateResolver,
        },
        sui::{self, MovePackageArtifact},
        test_utils::sui_mocks,
        transactions::tap::publish_package_ptb,
    },
    std::sync::Arc,
};

/// Publishes [`MovePackageArtifact`] to Sui for integration tests.
pub async fn publish_move_package(
    key: &sui::crypto::Ed25519PrivateKey,
    rpc_url: &str,
    package: MovePackageArtifact,
    gas_coin: sui::types::ObjectReference,
) -> ExecutedTransaction {
    let mut client = sui::grpc::client(rpc_url).expect("could not create gRPC client");
    let address = key.public_key().derive_address();
    let signer_client = Arc::new(client.clone());
    let signer = Signer::new(
        Arc::clone(&signer_client),
        key.clone(),
        std::time::Duration::from_secs(30),
        NexusEventDecoder::new(
            StateResolver::new(Arc::new(Crawler::new(signer_client))),
            Arc::new(sui_mocks::mock_nexus_objects()),
        ),
    );
    let reference_gas_price = client
        .get_reference_gas_price()
        .await
        .expect("failed to get reference gas price");
    let transaction = publish_package_ptb(package, address).expect("publish command should build");
    let transaction = sui::types::Transaction {
        kind: sui::types::TransactionKind::ProgrammableTransaction(transaction),
        sender: address,
        gas_payment: sui::types::GasPayment {
            objects: vec![gas_coin.clone()],
            owner: address,
            price: reference_gas_price,
            budget: 1_000_000_000,
        },
        expiration: sui::types::TransactionExpiration::None,
    };
    let signature = signer
        .sign_tx(&transaction)
        .await
        .expect("failed to sign transaction");

    signer
        .execute_tx(transaction, signature, &mut gas_coin.clone())
        .await
        .expect("failed to execute transaction")
}
