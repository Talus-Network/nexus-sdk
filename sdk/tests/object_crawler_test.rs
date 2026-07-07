#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        move_bindings::{
            move_std::ascii::String as MoveString,
            sui_framework::{
                bag::Bag,
                linked_table::{LinkedTable, Node as LinkedTableNode},
                object_bag::ObjectBag,
                object_table::ObjectTable,
                table_vec::TableVec,
                vec_map::VecMap,
                vec_set::VecSet,
            },
        },
        nexus::crawler::{Crawler, Map},
        sui,
        test_utils,
    },
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{
        collections::{HashMap, HashSet},
        str::FromStr,
        sync::Arc,
    },
    tokio::sync::Mutex,
};

#[derive(Clone, Debug, Deserialize)]
struct Guy {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    name: String,
    age: u8,
    hobbies: VecSet<MoveString>,
    groups: VecMap<Name, Vec<Name>>,
    timetable: ObjectTable<Name, Value>,
    friends: ObjectBag,
    bag: Bag,
    heterogeneous: Bag,
    sequence: TableVec<Name>,
    linked_table: LinkedTable<Name, Name>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Name {
    name: String,
}

impl sui_move::MoveType for Name {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0x0"),
            sui::types::Identifier::from_static("main"),
            sui::types::Identifier::from_static("Name"),
            vec![],
        )))
    }
}

impl sui_move::HasCopy for Name {}
impl sui_move::HasDrop for Name {}
impl sui_move::HasStore for Name {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Value {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    value: Name,
    pouch: ObjectBag,
}

impl sui_move::MoveType for Value {
    fn type_tag_static() -> sui::types::TypeTag {
        sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0x0"),
            sui::types::Identifier::from_static("main"),
            sui::types::Identifier::from_static("Value"),
            vec![],
        )))
    }
}

impl sui_move::HasKey for Value {}
impl sui_move::HasStore for Value {}

#[derive(Clone, Debug, Deserialize)]
struct PlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    value: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
struct AnotherPlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    another_value: Vec<u8>,
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
        guy.hobbies
            .contents
            .into_iter()
            .map(MoveString::into_string)
            .collect::<HashSet<_>>(),
        vec!["Reading".to_string(), "Swimming".to_string()]
            .into_iter()
            .collect()
    );

    // Check map and nested vector fetched correctly.
    let groups = guy.groups.into_hash_map();
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
        .get_dynamic_object_fields::<Name, Value>(guy.timetable.id())
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
        .get_dynamic_object_fields::<Name, PlainValue>(monday.data.pouch.id())
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
        .get_dynamic_object_fields::<Name, PlainValue>(tuesday.data.pouch.id())
        .await
        .unwrap();
    assert_eq!(pouch.len(), 1);
    let (key, value) = pouch.into_iter().next().unwrap();
    assert_eq!(key.name, "Pouch Code");
    assert_eq!(value.data.value, b"MOREDATA15");

    // Fetch friends which is an ObjectBag.
    assert_eq!(guy.friends.size(), 2);
    let friends = crawler
        .get_dynamic_object_fields::<Name, PlainValue>(guy.friends.id())
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
    let bag = crawler
        .get_dynamic_fields::<Name, PlainValue>(guy.bag.id(), guy.bag.size())
        .await
        .unwrap();
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
        .get_dynamic_field_refs_matching_key::<Name>(guy.heterogeneous.id())
        .await
        .unwrap();
    assert_eq!(heterogeneous.len(), 2);

    for field in heterogeneous {
        if field.name.name == "Bag Item" {
            let value = crawler
                .get_dynamic_field_value_by_id::<Name, PlainValue>(field.field_id)
                .await
                .unwrap();
            assert_eq!(value.value, b"Bag Data");
        } else if field.name.name == "Another Bag Item" {
            let value = crawler
                .get_dynamic_field_value_by_id::<Name, AnotherPlainValue>(field.field_id)
                .await
                .unwrap();
            assert_eq!(value.another_value, b"Another Bag Data");
        } else {
            panic!("Unexpected key in heterogeneous bag: {:?}", field.name);
        }
    }

    // Fetch linked table.
    assert_eq!(guy.linked_table.size(), 1);
    let linked_table = crawler
        .get_dynamic_fields::<Name, LinkedTableNode<Name, Name>>(
            guy.linked_table.id(),
            guy.linked_table.size(),
        )
        .await
        .unwrap();
    assert_eq!(linked_table.len(), 1);

    // Fetch first value from linked table.
    let linked_item = linked_table
        .get(&Name {
            name: "Key 1".to_string(),
        })
        .unwrap();

    assert_eq!(linked_item.value.name, "Value 1");

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
        let mut ptb = sui_move_ptb::PtbBuilder::new();
        let guy_arg = ptb
            .input(sui::types::Input::Shared(sui::types::SharedInput::new(
                guy_id,
                initial_shared_version,
                sui::types::Mutability::Mutable,
            )))
            .expect("shared Guy input should build");
        let target = sui_move_call::CallTarget::new(pkg_id, "main", "bump_age")
            .expect("bump_age target should build");
        ptb.call_target(target, vec![guy_arg])
            .expect("bump_age call should build");

        let tx = sui::types::Transaction {
            kind: sui::types::TransactionKind::ProgrammableTransaction(ptb.finish()),
            sender: addr,
            gas_payment: sui::types::GasPayment {
                objects: vec![gas_coin_ref.clone()],
                owner: addr,
                price: reference_gas_price,
                budget: 1_000_000_000,
            },
            expiration: sui::types::TransactionExpiration::None,
        };
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
