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

/// Create a new [`sui::TransactionBlockEffects`] with random values. Note that
/// this function can be changed in the future to allow more customization.
pub fn mock_sui_transaction_block_effects(
    created: Option<Vec<sui::OwnedObjectRef>>,
    mutated: Option<Vec<sui::OwnedObjectRef>>,
    unwrapped: Option<Vec<sui::OwnedObjectRef>>,
    deleted: Option<Vec<sui::ObjectRef>>,
) -> sui::TransactionBlockEffects {
    sui::TransactionBlockEffects::V1(sui::TransactionBlockEffectsV1 {
        status: sui::ExecutionStatus::Success,
        executed_epoch: 1,
        gas_used: sui::GasCostSummary {
            computation_cost: 0,
            storage_cost: 0,
            storage_rebate: 0,
            non_refundable_storage_fee: 0,
        },
        modified_at_versions: vec![],
        shared_objects: vec![],
        transaction_digest: sui::TransactionDigest::random(),
        created: created.unwrap_or_default(),
        mutated: mutated.unwrap_or_default(),
        unwrapped: unwrapped.unwrap_or_default(),
        deleted: deleted.unwrap_or_default(),
        unwrapped_then_deleted: vec![],
        wrapped: vec![],
        gas_object: sui::OwnedObjectRef {
            owner: sui::Owner::AddressOwner(sui::ObjectID::random().into()),
            reference: mock_sui_object_ref(),
        },
        events_digest: None,
        dependencies: vec![],
    })
}

/// Mocking RPC endpoints for deeper testing.
pub mod rpc {
    use {
        crate::{events::NexusEventKind, idents::primitives, sui, test_utils::sui_mocks},
        mockito::{Matcher, Mock, ServerGuard},
        serde_json::json,
    };

    #[derive(serde::Deserialize)]
    struct PartialRequest {
        jsonrpc: String,
        id: u64,
    }

    pub fn mock_rpc_discover(server: &mut ServerGuard) -> Mock {
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "rpc.discover"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": {
                        "openrpc": "1.2.6",
                        "info": { "version": "1.58.3" },
                        "methods": []
                    },
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .expect(1)
            .create()
    }

    pub fn mock_reference_gas_price(server: &mut ServerGuard, price: String) -> Mock {
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "suix_getReferenceGasPrice"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": price,
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .expect(1)
            .create()
    }

    pub fn mock_event_api_query_events(
        server: &mut ServerGuard,
        events: Vec<(String, NexusEventKind)>,
    ) -> Mock {
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "suix_queryEvents"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": {
                        "data": events
                            .iter()
                            .map(|(event_name, event)|
                                sui::Event {
                                    id: sui_mocks::mock_sui_event_id(),
                                    package_id: sui::ObjectID::random(),
                                    parsed_json: serde_json::to_value(event).expect("Failed to serialize event"),
                                    transaction_module: sui::move_ident_str!("test").into(),
                                    sender: sui::ObjectID::random().into(),
                                    type_: sui::MoveStructTag {
                                        address: sui::ObjectID::random().into(),
                                        module: primitives::Event::EVENT_WRAPPER.module.into(),
                                        name: primitives::Event::EVENT_WRAPPER.name.into(),
                                        type_params: vec![
                                            sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                                                address: sui::ObjectID::random().into(),
                                                module: sui::move_ident_str!("test").into(),
                                                name: sui::Identifier::new(event_name.clone()).expect("Failed to parse event name"),
                                                type_params: vec![],
                                            })),
                                        ],
                                    },
                                    bcs: sui::BcsEvent::Base64 { bcs: vec![] },
                                    timestamp_ms: None,
                                }
                            )
                            .collect::<Vec<sui::Event>>(),
                        "nextCursor": null,
                        "hasNextPage": false,
                    },
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .create()
    }

    pub fn mock_read_api_get_object(
        server: &mut ServerGuard,
        object_id: sui::ObjectID,
        object: sui::ParsedMoveObject,
    ) -> Mock {
        server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "sui_getObject"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": sui::ObjectResponse::new_with_data(
                        sui::ObjectData {
                            object_id,
                            version: sui::SequenceNumber::from_u64(1),
                            digest: sui::ObjectDigest::random(),
                            type_: None,
                            owner: Some(sui::Owner::AddressOwner(sui::ObjectID::random().into())),
                            previous_transaction: None,
                            storage_rebate: None,
                            display: None,
                            content: Some(sui::ParsedData::MoveObject(object.clone())),
                            bcs: None,
                        }
                    ),
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .create()
    }

    pub fn mock_governance_api_execute_execute_transaction_block(
        server: &mut ServerGuard,
        digest: sui::TransactionDigest,
        effects: Option<sui::TransactionBlockEffects>,
        events: Option<sui::TransactionBlockEvents>,
        balance_changes: Option<Vec<sui::BalanceChange>>,
        object_changes: Option<Vec<sui::ObjectChange>>,
    ) -> (Mock, Mock) {
        let effects_execute = effects.or(Some(sui_mocks::mock_sui_transaction_block_effects(
            None, None, None, None,
        )));
        let effects_confirm = effects_execute.clone();
        let events_confirm = events.clone();
        let object_changes_confirm = object_changes.clone();
        let balance_changes_confirm = balance_changes.clone();

        let execute_mock = server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "sui_executeTransactionBlock"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": sui::TransactionBlockResponse {
                        digest,
                        transaction: None,
                        raw_transaction: vec![],
                        effects: effects_execute.clone(),
                        events: events.clone(),
                        object_changes: object_changes.clone(),
                        balance_changes: balance_changes.clone(),
                        timestamp_ms: None,
                        confirmed_local_execution: Some(true),
                        checkpoint: None,
                        errors: vec![],
                        raw_effects: vec![],
                    },
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .expect(1)
            .create();

        let confirm_mock = server
            .mock("POST", "/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::PartialJson(json!({
                "method": "sui_getTransactionBlock"
            })))
            .with_status(200)
            .with_body_from_request(move |req| {
                let req: PartialRequest = serde_json::from_str(
                    &req.utf8_lossy_body().expect("Failed to parse request body"),
                )
                .expect("Failed to parse PartialRequest");

                json!({
                    "result": sui::TransactionBlockResponse {
                        digest,
                        transaction: None,
                        raw_transaction: vec![],
                        effects: effects_confirm.clone(),
                        events: events_confirm.clone(),
                        object_changes: object_changes_confirm.clone(),
                        balance_changes: balance_changes_confirm.clone(),
                        timestamp_ms: None,
                        confirmed_local_execution: Some(true),
                        checkpoint: None,
                        errors: vec![],
                        raw_effects: vec![],
                    },
                    "jsonrpc": req.jsonrpc,
                    "id": req.id
                })
                .to_string()
                .into()
            })
            .expect(1)
            .create();

        (execute_mock, confirm_mock)
    }
}
