//! Move package submission support for integration tests.

use {
    crate::{
        nexus::signer::{ExecutedTransaction, Signer},
        sui::{self, MovePackageArtifact},
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
    let signer = Signer::new(
        Arc::new(client.clone()),
        key.clone(),
        std::time::Duration::from_secs(30),
        None,
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
