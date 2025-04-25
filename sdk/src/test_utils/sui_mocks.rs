use crate::sui;

/// Create a new [`sui::Coin`] with random values.
pub fn mock_sui_coin(balance: u64) -> sui::Coin {
    sui::Coin {
        coin_type: "Sui".to_string(),
        coin_object_id: sui::ObjectID::random(),
        version: sui::SequenceNumber::new(),
        digest: sui::ObjectDigest::random(),
        balance,
        previous_transaction: sui::TransactionDigest::random(),
    }
}

/// Create a new [`sui::ObjectRef`] with random values.
pub fn mock_sui_object_ref() -> sui::ObjectRef {
    sui::ObjectRef {
        object_id: sui::ObjectID::random(),
        version: sui::SequenceNumber::new(),
        digest: sui::ObjectDigest::random(),
    }
}

/// Create a new [`sui::EventID`] with random values.
pub fn mock_sui_event_id() -> sui::EventID {
    sui::EventID {
        tx_digest: sui::TransactionDigest::random(),
        event_seq: 0,
    }
}
