#![cfg(feature = "sui_types")]

#[test]
fn reexports_sui_transaction_builder() {
    let type_name = std::any::type_name::<nexus_sdk::sui_transaction_builder::TransactionBuilder>();

    assert!(type_name.contains("TransactionBuilder"));
}
