use crate::{sui, types::NexusObjects};

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

/// Create a new [`sui::EventID`] with random values.
pub fn mock_nexus_objects() -> NexusObjects {
    NexusObjects {
        workflow_pkg_id: sui::ObjectID::random(),
        primitives_pkg_id: sui::ObjectID::random(),
        interface_pkg_id: sui::ObjectID::random(),
        network_id: sui::ObjectID::random(),
        tool_registry: mock_sui_object_ref(),
        default_tap: mock_sui_object_ref(),
        gas_service: mock_sui_object_ref(),
        pre_key_vault: mock_sui_object_ref(),
    }
}

/// Generate a new Sui address and its corresponding mnemonic.
pub fn mock_sui_mnemonic() -> (sui::Address, String) {
    let derivation_path = None;
    let word_length = None;

    let (addr, _, _, secret_mnemonic) =
        sui::generate_new_key(sui::SignatureScheme::ED25519, derivation_path, word_length)
            .expect("Failed to generate key.");

    (addr, secret_mnemonic)
}
