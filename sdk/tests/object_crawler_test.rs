#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        nexus::crawler::{Bytes, Crawler, DynamicMap, DynamicObjectMap, Map, Set},
        sui,
        test_utils,
    },
    serde::{Deserialize, Serialize},
    std::{str::FromStr, sync::Arc},
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
    chair: DynamicMap<Name, Name>,
    timetable: DynamicObjectMap<Name, Value>,
    friends: DynamicObjectMap<Name, PlainValue>,
    bag: DynamicMap<Name, PlainValue>,
    heterogeneous: DynamicMap<Name, HeterogeneousValue>,
    linked_table: DynamicMap<Name, Name>,
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
    value: Bytes,
}

#[derive(Clone, Debug, Deserialize)]
struct AnotherPlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::types::Address,
    another_value: Bytes,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum HeterogeneousValue {
    Value(PlainValue),
    AnotherValue(AnotherPlainValue),
}

// TODO: <https://github.com/Talus-Network/nexus-sdk/issues/318>
#[tokio::test]
async fn test_object_crawler() {
    // Spin up the Sui instance.
    let (_container, rpc_port, faucet_port) = test_utils::containers::setup_sui_instance().await;

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
        gas_coins.iter().nth(0).cloned().unwrap(),
    )
    .await;

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

    let grpc = sui::grpc::Client::new(&format!("http://127.0.0.1:{rpc_port}"))
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

    // Check VecMap and nested vector fetched correctly.
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
    assert_eq!(value.data.value.into_inner(), b"Pouch Data");

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
    assert_eq!(value.data.value.into_inner(), b"MOREDATA15");

    // Fetch chair which is a Table. Weirdly.
    assert_eq!(guy.chair.size(), 2);
    let chair = crawler.get_dynamic_fields(&guy.chair).await.unwrap();
    assert_eq!(chair.len(), 2);

    // Fetch chairman.
    let chairman = chair
        .get(&Name {
            name: "Chairman".to_string(),
        })
        .unwrap();

    assert_eq!(chairman.name, "John Doe");

    // Fetch vice chairman.
    let vice_chairman = chair
        .get(&Name {
            name: "Vice Chairman".to_string(),
        })
        .unwrap();

    assert_eq!(vice_chairman.name, "Alice");

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

    assert_eq!(charlie.data.value.clone().into_inner(), b"Never Seen");

    // Fetch second friend.
    let david = friends
        .get(&Name {
            name: "David".to_string(),
        })
        .unwrap();

    assert_eq!(
        david.data.value.clone().into_inner(),
        b"Definitely Imagination"
    );

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

    assert_eq!(item1.value.clone().into_inner(), b"Bag Data");

    // Fetch second item from bag.
    let item2 = bag
        .get(&Name {
            name: "Bag Item 2".to_string(),
        })
        .unwrap();

    assert_eq!(item2.value.clone().into_inner(), b"Bag Data 2");

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
                matches!(value, HeterogeneousValue::Value(v) if v.value.clone().into_inner() == b"Bag Data")
            );
        } else if key.name == "Another Bag Item" {
            assert!(
                matches!(value, HeterogeneousValue::AnotherValue(v) if v.another_value.clone().into_inner() == b"Another Bag Data")
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
}
