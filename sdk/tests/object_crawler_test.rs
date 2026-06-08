#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        nexus::crawler::{
            Bag,
            Crawler,
            DynamicMap,
            DynamicObjectMap,
            Map,
            ObjectBag,
            Set,
            TableVec,
        },
        sui,
        test_utils,
        types::deserialize_encoded_bytes,
    },
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{collections::HashMap, str::FromStr, sync::Arc},
    tokio::sync::Mutex,
};

#[derive(Clone, Debug, Deserialize)]
struct Guy {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    name: String,
    age: u8,
    hobbies: Set<String>,
    groups: Map<Name, Vec<Name>>,
    timetable: DynamicObjectMap<Name, Value>,
    friends: DynamicObjectMap<Name, PlainValue>,
    bag: DynamicMap<Name, PlainValue>,
    heterogeneous: DynamicMap<Name, HeterogeneousValue>,
    linked_table: DynamicMap<Name, Name>,
    sequence: TableVec<Name>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Name {
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Value {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    value: Name,
    pouch: DynamicObjectMap<Name, PlainValue>,
}

#[derive(Clone, Debug, Deserialize)]
struct PlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    #[serde(deserialize_with = "deserialize_encoded_bytes")]
    value: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
struct AnotherPlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    #[serde(
        deserialize_with = "deserialize_encoded_bytes",
        serialize_with = "serialize_encoded_bytes"
    )]
    another_value: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum HeterogeneousValue {
    Value(PlainValue),
    AnotherValue(AnotherPlainValue),
}

#[tokio::test]
async fn test_object_crawler() {
    // Spin up the Sui instance.
    let test_utils::containers::SuiInstance {
        rpc_port,
        faucet_port,
        container: _container,
        ..
    } = test_utils::containers::setup_sui_instance().await;

    let rpc_url = format!("http://127.0.0.1:{rpc_port}");
    let faucet_url = format!("http://127.0.0.1:{faucet_port}/gas");

    let mut rng = rand::thread_rng();

    // Create a wallet and request some gas tokens.
    let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
    let addr = pk.public_key().derive_address();

    test_utils::faucet::request_tokens(&faucet_url, addr)
        .await
        .expect("Failed to request tokens from faucet.");

    let gas_coins = test_utils::gas::fetch_gas_coins(&rpc_url, addr)
        .await
        .expect("Failed to fetch gas coins.");

    // Publish test contract and fetch some IDs.
    let response = test_utils::contracts::publish_move_package(
        &pk,
        &rpc_url,
        "tests/move/object_crawler_test",
        gas_coins.first().cloned().unwrap().0,
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let guy = response
        .objects
        .iter()
        .find_map(|obj| {
            let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                return None;
            };

            if *object_type.name() == sui::types::Identifier::from_static("Guy") {
                Some(obj.object_id())
            } else {
                None
            }
        })
        .expect("Guy object must be created");

    let guy = sui::types::Address::from_str(&guy.to_string()).unwrap();

    let grpc = sui::grpc::Client::new(format!("http://127.0.0.1:{rpc_port}"))
        .expect("Could not create gRPC client");

    let crawler = Crawler::new(Arc::new(Mutex::new(grpc)));

    let guy = crawler
        .get_object::<Guy>(guy)
        .await
        .expect("Could not fetch Guy object.");

    assert!(guy.is_shared());

    let guy = guy.data;

    assert_eq!(guy.name, "John Doe");
    assert_eq!(guy.age, 30);
    assert_eq!(
        guy.hobbies.into_inner(),
        vec!["Reading".to_string(), "Swimming".to_string()]
            .into_iter()
            .collect()
    );

    // Check map and nested vector fetched correctly.
    let groups = guy.groups.into_inner();
    assert_eq!(groups.len(), 2);

    // Contains book club with the correct people.
    let group = groups.clone().into_iter().find(|(group, people)| {
        group.clone().name == "Book Club"
            && people.iter().any(|p| p.name == "Alice")
            && people.iter().any(|p| p.name == "Bob")
    });

    assert!(group.is_some());

    // Contains swimming club with the correct people.
    let group = groups.clone().into_iter().find(|(group, people)| {
        group.clone().name == "Swimming Club"
            && people.iter().any(|p| p.name == "Charlie")
            && people.iter().any(|p| p.name == "David")
    });

    assert!(group.is_some());

    // Fetch timetable that is an ObjectTable and has a nested ObjectBag.
    assert_eq!(guy.timetable.size(), 2);
    let timetable = crawler
        .get_dynamic_field_objects(&guy.timetable)
        .await
        .unwrap();
    assert_eq!(timetable.len(), 2);

    // Fetch monday.
    let monday = timetable
        .get(&Name {
            name: "Monday".to_string(),
        })
        .unwrap();

    assert_eq!(monday.data.value.name, "Meeting");

    assert_eq!(monday.data.pouch.size(), 1);
    let pouch = crawler
        .get_dynamic_field_objects(&monday.data.pouch)
        .await
        .unwrap();

    assert_eq!(pouch.len(), 1);
    let (key, value) = pouch.into_iter().next().unwrap();
    assert_eq!(key.name, "Pouch Item");
    assert_eq!(value.data.value, b"Pouch Data");

    // Fetch tuesday
    let tuesday = timetable
        .get(&Name {
            name: "Tuesday".to_string(),
        })
        .unwrap();

    assert_eq!(tuesday.data.value.name, "Code Review");

    assert_eq!(tuesday.data.pouch.size(), 1);
    let pouch = crawler
        .get_dynamic_field_objects(&tuesday.data.pouch)
        .await
        .unwrap();
    assert_eq!(pouch.len(), 1);
    let (key, value) = pouch.into_iter().next().unwrap();
    assert_eq!(key.name, "Pouch Code");
    assert_eq!(value.data.value, b"MOREDATA15");

    // Fetch friends which is an ObjectBag.
    assert_eq!(guy.friends.size(), 2);
    let friends = crawler
        .get_dynamic_field_objects(&guy.friends)
        .await
        .unwrap();
    assert_eq!(friends.len(), 2);

    // Fetch first friend.
    let charlie = friends
        .get(&Name {
            name: "Charlie".to_string(),
        })
        .unwrap();

    assert_eq!(charlie.data.value.clone(), b"Never Seen");

    // Fetch second friend.
    let david = friends
        .get(&Name {
            name: "David".to_string(),
        })
        .unwrap();

    assert_eq!(david.data.value.clone(), b"Definitely Imagination");

    // Now fetch bag which is a Bag. Finally.
    assert_eq!(guy.bag.size(), 2);
    let bag = crawler.get_dynamic_fields(&guy.bag).await.unwrap();
    assert_eq!(bag.len(), 2);

    // Fetch first item from bag.
    let item1 = bag
        .get(&Name {
            name: "Bag Item".to_string(),
        })
        .unwrap();

    assert_eq!(item1.value.clone(), b"Bag Data");

    // Fetch second item from bag.
    let item2 = bag
        .get(&Name {
            name: "Bag Item 2".to_string(),
        })
        .unwrap();

    assert_eq!(item2.value.clone(), b"Bag Data 2");

    // Fetch heterogeneous Bag.
    assert_eq!(guy.heterogeneous.size(), 2);
    let heterogeneous = crawler
        .get_dynamic_fields(&guy.heterogeneous)
        .await
        .unwrap();
    assert!(heterogeneous.len() == 2);

    for (key, value) in heterogeneous {
        if key.name == "Bag Item" {
            assert!(
                matches!(value, HeterogeneousValue::Value(v) if v.value.clone() == b"Bag Data")
            );
        } else if key.name == "Another Bag Item" {
            assert!(
                matches!(value, HeterogeneousValue::AnotherValue(v) if v.another_value.clone() == b"Another Bag Data")
            );
        } else {
            panic!("Unexpected key in heterogeneous bag: {:?}", key);
        }
    }

    // Fetch linked table.
    assert_eq!(guy.linked_table.size(), 1);
    let linked_table = crawler.get_dynamic_fields(&guy.linked_table).await.unwrap();
    assert_eq!(linked_table.len(), 1);

    // Fetch first value from linked table.
    let linked_item = linked_table
        .get(&Name {
            name: "Key 1".to_string(),
        })
        .unwrap();

    assert_eq!(linked_item.name, "Value 1");

    // Fetch TableVec.
    assert_eq!(guy.sequence.size(), 3);
    assert_eq!(guy.sequence.size_u64(), 3);
    let sequence_id = guy.sequence.id();
    assert_ne!(sequence_id, sui::types::Address::from_static("0x0"));

    let sequence = crawler.get_table_vec(&guy.sequence).await.unwrap();
    assert_eq!(sequence.len(), 3);
    assert_eq!(sequence[0].name, "First");
    assert_eq!(sequence[1].name, "Second");
    assert_eq!(sequence[2].name, "Third");
}

#[test]
fn crawler_wrapper_accessors_cover_id_and_size_helpers() {
    let id = nexus_sdk::sui::types::Address::from_static("0x123");

    let bag = Bag::new(id, 2);
    assert_eq!(bag.id(), id);
    assert_eq!(bag.size_u64(), 2);
    assert_eq!(bag.size(), 2);

    let object_bag = ObjectBag::new(id, 3);
    assert_eq!(object_bag.id(), id);
    assert_eq!(object_bag.size_u64(), 3);
    assert_eq!(object_bag.size(), 3);

    let table_vec: TableVec<u64> = TableVec::new(id, 5);
    assert_eq!(table_vec.id(), id);
    assert_eq!(table_vec.size_u64(), 5);
    assert_eq!(table_vec.size(), 5);
}

/// End-to-end check for [`Crawler::get_object_creation_checkpoint`].
///
/// Publishes the test contract (which creates a shared `Guy` object in its
/// `init`), then submits three follow-up `bump_age` transactions against
/// `Guy`. Each follow-up tx lands in a later checkpoint than the publish,
/// so the LATEST `previous_transaction` on the shared object is guaranteed
/// to differ from the creation tx digest. We then call
/// `get_object_creation_checkpoint(guy_id)` and assert it returns the
/// publish-tx's checkpoint — proving the helper walks
/// `Owner::Shared(initial_shared_version) → version-pinned
/// previous_transaction → checkpoint` rather than just reading the
/// object's current `previous_transaction`.
#[tokio::test]
async fn test_object_crawler_get_object_creation_checkpoint() {
    use nexus_sdk::{nexus::signer::Signer, test_utils::sui_mocks};

    let test_utils::containers::SuiInstance {
        rpc_port,
        faucet_port,
        container: _container,
        ..
    } = test_utils::containers::setup_sui_instance().await;

    let rpc_url = format!("http://127.0.0.1:{rpc_port}");
    let faucet_url = format!("http://127.0.0.1:{faucet_port}/gas");

    let mut rng = rand::thread_rng();
    let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
    let addr = pk.public_key().derive_address();

    test_utils::faucet::request_tokens(&faucet_url, addr)
        .await
        .expect("Failed to request tokens from faucet.");

    let gas_coins = test_utils::gas::fetch_gas_coins(&rpc_url, addr)
        .await
        .expect("Failed to fetch gas coins.");

    let publish_response = test_utils::contracts::publish_move_package(
        &pk,
        &rpc_url,
        "tests/move/object_crawler_test",
        gas_coins.first().cloned().unwrap().0,
    )
    .await;

    let creation_checkpoint = publish_response.checkpoint;
    let pkg_id = publish_response
        .objects
        .iter()
        .find_map(|object| match object.data() {
            sui::types::ObjectData::Package(package) => Some(package.id),
            _ => None,
        })
        .expect("Move package must be published");
    let guy_id = publish_response
        .objects
        .iter()
        .find_map(|object| {
            let sui::types::ObjectType::Struct(object_type) = object.object_type() else {
                return None;
            };
            (*object_type.name() == sui::types::Identifier::from_static("Guy"))
                .then_some(object.object_id())
        })
        .expect("Guy object must be created");
    let guy_id = sui::types::Address::from_str(&guy_id.to_string()).expect("Guy id parses");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let grpc = sui::grpc::Client::new(format!("http://127.0.0.1:{rpc_port}"))
        .expect("Could not create gRPC client");
    let crawler = Crawler::new(Arc::new(Mutex::new(grpc.clone())));

    // Read the freshly-published `Guy` to capture its `initial_shared_version`,
    // which is needed to feed the shared input to subsequent PTBs and is the
    // exact value the helper under test is supposed to derive internally.
    let guy_metadata = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Guy metadata after publish");
    let initial_shared_version = match guy_metadata.owner {
        sui::types::Owner::Shared(v) => v,
        other => panic!("Guy must be a shared object after publish, got {other:?}"),
    };
    let initial_version_at_publish = guy_metadata.version;

    // Bump `Guy` three times. Each call lands in a later checkpoint than the
    // publish, so the most-recent `previous_transaction` on `guy_id` will be
    // the third `bump_age` tx — exactly the value the helper must NOT return.
    let mut gas_coin_ref = crawler
        .get_object_metadata(*gas_coins.first().unwrap().0.object_id())
        .await
        .expect("gas coin metadata")
        .object_ref();
    let reference_gas_price = grpc
        .clone()
        .get_reference_gas_price()
        .await
        .expect("Failed to get reference gas price.");
    let signer = Signer::new(
        Arc::new(Mutex::new(grpc.clone())),
        pk.clone(),
        std::time::Duration::from_secs(30),
        Arc::new(sui_mocks::mock_nexus_objects()),
    );

    let mut bump_checkpoints = Vec::new();
    for _ in 0..3 {
        let mut tx = sui::tx::TransactionBuilder::new();
        let guy_arg = tx.input(sui::tx::Input::shared(guy_id, initial_shared_version, true));
        tx.move_call(
            sui::tx::Function::new(
                pkg_id,
                sui::types::Identifier::from_static("main"),
                sui::types::Identifier::from_static("bump_age"),
                vec![],
            ),
            vec![guy_arg],
        );
        tx.set_sender(addr);
        tx.set_gas_budget(1_000_000_000);
        tx.set_gas_price(reference_gas_price);
        tx.add_gas_objects(vec![sui::tx::Input::owned(
            *gas_coin_ref.object_id(),
            gas_coin_ref.version(),
            *gas_coin_ref.digest(),
        )]);
        let tx = tx.finish().expect("Failed to finish bump_age transaction.");
        let signature = signer
            .sign_tx(&tx)
            .await
            .expect("Failed to sign bump_age transaction.");
        let executed = signer
            .execute_tx(tx, signature, &mut gas_coin_ref)
            .await
            .expect("Failed to execute bump_age transaction.");
        bump_checkpoints.push(executed.checkpoint);
    }

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // The helper under test: should return the publish-tx's checkpoint, not
    // any of the bump_age checkpoints we just produced.
    let resolved = crawler
        .get_object_creation_checkpoint(guy_id)
        .await
        .expect("creation checkpoint resolves");

    assert_eq!(
        resolved, creation_checkpoint,
        "expected creation checkpoint to match the publish tx's checkpoint"
    );
    for bump_checkpoint in &bump_checkpoints {
        assert_ne!(
            resolved, *bump_checkpoint,
            "creation checkpoint must NOT match any of the post-publish bump_age tx checkpoints"
        );
    }

    // Sanity check: the bumps actually advanced the object's version, so this
    // test would have failed loudly if the helper accidentally returned the
    // latest-tx checkpoint (e.g. by reading `previous_transaction` off the
    // current object instead of the version-pinned one).
    let post_bump_metadata = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Guy metadata after bumps");
    assert!(
        post_bump_metadata.version > initial_version_at_publish,
        "bump_age calls must have advanced Guy's version (initial {initial_version_at_publish}, \
         post-bump {})",
        post_bump_metadata.version,
    );
}

#[test]
fn map_deserializes_from_object_contents_variant() {
    let value = json!({
        "contents": {
            "a": 1,
            "b": 2,
        }
    });

    let parsed: Map<String, u64> = serde_json::from_value(value).expect("deserialize Map");
    assert_eq!(
        parsed.into_map(),
        HashMap::from([("a".to_string(), 1), ("b".to_string(), 2)])
    );
}
