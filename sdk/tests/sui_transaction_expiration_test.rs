#![cfg(feature = "types")]

use nexus_sdk::sui::{grpc, types};

#[test]
fn decodes_simulation_transactions_with_both_validity_formats() {
    // Fixed BCS transaction data with an empty PTB, address balance gas, and
    // sender 0x42. The expiration is appended below using its wire variant.
    let transaction_prefix = concat!(
        "00000000",
        "0000000000000000000000000000000000000000000000000000000000000042",
        "00",
        "0000000000000000000000000000000000000000000000000000000000000042",
        "e803000000000000",
        "40420f0000000000",
    );
    let validity_window = concat!(
        "010700000000000000", // Minimum epoch 7.
        "010800000000000000", // Maximum epoch 8.
        "0000",               // No timestamp bounds.
        "200707070707070707070707070707070707070707070707070707070707070707",
        "09000000", // Nonce 9.
    );
    let chain = types::Digest::from([7; 32]);
    let cases = [
        (
            "02",
            "",
            types::TransactionExpiration::ValidDuring {
                min_epoch: Some(7),
                max_epoch: Some(8),
                min_timestamp: None,
                max_timestamp: None,
                chain,
                nonce: 9,
            },
        ),
        (
            "03",
            "010700000000000000020100000003000000",
            types::TransactionExpiration::Validity {
                min_epoch: Some(7),
                max_epoch: Some(8),
                min_timestamp: None,
                max_timestamp: None,
                chain,
                nonce: 9,
                allowed_proposers: Some(types::AllowedProposers {
                    epoch: 7,
                    proposers: vec![1, 3],
                }),
            },
        ),
    ];

    for (variant, proposers, expiration) in cases {
        let bytes = hex::decode(format!(
            "{transaction_prefix}{variant}{validity_window}{proposers}"
        ))
        .unwrap();
        let response =
            grpc::Transaction::default().with_bcs(grpc::Bcs::default().with_value(bytes));
        let transaction = types::Transaction::try_from(&response)
            .expect("simulation transaction BCS must decode");

        assert_eq!(transaction.sender, types::Address::from_static("0x42"));
        assert!(transaction.gas_payment.objects.is_empty());
        assert_eq!(transaction.gas_payment.price, 1_000);
        assert_eq!(transaction.gas_payment.budget, 1_000_000);
        assert_eq!(transaction.expiration, expiration);

        // The structured RPC representation must preserve the same expiration
        // and proposer restrictions when the BCS field is absent.
        let mut structured = grpc::Transaction::from(transaction.clone());
        structured.bcs = None;
        assert_eq!(
            types::Transaction::try_from(&structured).unwrap(),
            transaction
        );
    }
}
