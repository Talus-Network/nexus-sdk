#![cfg(feature = "test_utils")]

use {
    assert_matches::assert_matches,
    nexus_sdk::{object_crawler::*, sui, test_utils},
    serde::{Deserialize, Serialize},
};

#[derive(Clone, Debug, Deserialize)]
struct Guy {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::UID,
    name: String,
    age: u8,
    hobbies: VecSet<String>,
    groups: VecMap<Structure<Name>, Vec<Structure<Name>>>,
    chair: Table<Name, Structure<Name>>,
    timetable: ObjectTable<Name, Structure<Value>>,
    friends: ObjectBag<Name, Structure<PlainValue>>,
    bag: Bag<Name, Structure<PlainValue>>,
    heterogeneous: Bag<HeterogeneousKey, Structure<HeterogeneousValue>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct Name {
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct AnotherName {
    another_name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Value {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::UID,
    value: Structure<Name>,
    pouch: ObjectBag<Name, Structure<PlainValue>>,
}

#[derive(Clone, Debug, Deserialize)]
struct PlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::UID,
    value: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
struct AnotherPlainValue {
    // Test UID deser.
    #[allow(dead_code)]
    id: sui::UID,
    another_value: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
enum HeterogeneousKey {
    Name(Name),
    AnotherName(AnotherName),
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
    let (container, rpc_port, faucet_port) = test_utils::containers::setup_sui_instance().await;

    // Build Sui client.
    let sui = sui::ClientBuilder::default()
        .build(format!("http://127.0.0.1:{}", rpc_port))
        .await
        .expect("Failed to build Sui client");

    // Create a wallet and request some gas tokens.
    let (keystore, addr) =
        test_utils::wallet::create_test_wallet().expect("Failed to create a wallet.");

    test_utils::faucet::request_tokens(addr, &format!("http://127.0.0.1:{faucet_port}/gas"))
        .await
        .expect("Failed to request tokens from faucet.");

    let gas_coin = test_utils::gas::fetch_gas_coin(&sui, addr)
        .await
        .expect("Failed to fetch gas coin.");

    // Publish test contract and fetch some IDs.
    let response = test_utils::contracts::publish_move_package(
        &sui,
        addr,
        &keystore,
        "tests/move/object_crawler_test",
        gas_coin,
    )
    .await;

    let changes = response
        .object_changes
        .expect("TX response must have object changes");

    let pkg_id = changes
        .iter()
        .find_map(|c| match c {
            sui::ObjectChange::Published { package_id, .. } => Some(package_id),
            _ => None,
        })
        .expect("Move package must be published")
        .clone();

    let guy = changes
        .iter()
        .find_map(|c| match c {
            sui::ObjectChange::Created {
                object_id,
                object_type,
                ..
            } if object_type.name == sui::move_ident_str!("Guy").into() => Some(object_id),
            _ => None,
        })
        .expect("Guy object must be created")
        .clone();

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Name type tag.
    let name_tag = sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
        address: *pkg_id,
        module: sui::move_ident_str!("main").into(),
        name: sui::move_ident_str!("Name").into(),
        type_params: vec![],
    }));

    // Fetch the base object.
    let guy = fetch_one::<Structure<Guy>>(&sui, guy)
        .await
        .unwrap()
        .data
        .into_inner();

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
        group.clone().into_inner().name == "Book Club"
            && people.iter().find(|p| p.inner().name == "Alice").is_some()
            && people.iter().find(|p| p.inner().name == "Bob").is_some()
    });

    assert!(group.is_some());

    // Contains swimming club with the correct people.
    let group = groups.clone().into_iter().find(|(group, people)| {
        group.clone().into_inner().name == "Swimming Club"
            && people
                .iter()
                .find(|p| p.inner().name == "Charlie")
                .is_some()
            && people.iter().find(|p| p.inner().name == "David").is_some()
    });

    assert!(group.is_some());

    // Fetch timetable that is an ObjectTable and has a nested ObjectBag.
    let timetable = guy.timetable.fetch_all(&sui).await.unwrap();
    assert_eq!(timetable.len(), 2);

    // Fetch monday.
    let monday = guy
        .timetable
        .fetch_one(
            &sui,
            Name {
                name: "Monday".to_string(),
            },
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(monday.value.into_inner().name, "Meeting");

    let pouch = monday.pouch.fetch_all(&sui).await.unwrap();
    assert_eq!(pouch.len(), 1);
    let (key, value) = pouch.into_iter().next().unwrap();
    assert_eq!(key.name, "Pouch Item");
    assert_eq!(value.into_inner().value, b"Pouch Data");

    // Fetch tuesday
    let monday = guy
        .timetable
        .fetch_one(
            &sui,
            Name {
                name: "Tuesday".to_string(),
            },
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(monday.value.into_inner().name, "Code Review");

    let pouch = monday.pouch.fetch_all(&sui).await.unwrap();
    assert_eq!(pouch.len(), 1);
    let (key, value) = pouch.into_iter().next().unwrap();
    assert_eq!(key.name, "Pouch Code");
    assert_eq!(value.into_inner().value, b"MOREDATA15");

    // Fetch chair which is a Table. Weirdly.
    let chair = guy.chair.fetch_all(&sui).await.unwrap();
    assert_eq!(chair.len(), 2);

    // Fetch chairman.
    let chairmain = guy
        .chair
        .fetch_one(
            &sui,
            Name {
                name: "Chairman".to_string(),
            },
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(chairmain.name, "John Doe");

    // Fetch vice chairman.
    let vice_chairman = guy
        .chair
        .fetch_one(
            &sui,
            Name {
                name: "Vice Chairman".to_string(),
            },
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(vice_chairman.name, "Alice");

    // Fetch friends which is an ObjectBag.
    let friends = guy.friends.fetch_all(&sui).await.unwrap();
    assert_eq!(friends.len(), 2);

    // Fetch frist friend.
    let charlie = guy
        .friends
        .fetch_one(
            &sui,
            Name {
                name: "Charlie".to_string(),
            },
            name_tag.clone(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(charlie.value, b"Never Seen");

    // Fetch second friend.
    let david = guy
        .friends
        .fetch_one(
            &sui,
            Name {
                name: "David".to_string(),
            },
            name_tag.clone(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(david.value, b"Definitely Imagination");

    // Now fetch bag which is a Bag. Finally.
    let bag = guy.bag.fetch_all(&sui).await.unwrap();
    assert_eq!(bag.len(), 2);

    // Fetch first item from bag.
    let item1 = guy
        .bag
        .fetch_one(
            &sui,
            Name {
                name: "Bag Item".to_string(),
            },
            name_tag.clone(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(item1.value, b"Bag Data");

    // Fetch second item from bag.
    let item2 = guy
        .bag
        .fetch_one(
            &sui,
            Name {
                name: "Bag Item 2".to_string(),
            },
            name_tag.clone(),
        )
        .await
        .unwrap()
        .into_inner();
    assert_eq!(item2.value, b"Bag Data 2");

    // Fetch heterogeneous Bag.
    let heterogeneous = guy.heterogeneous.fetch_all(&sui).await.unwrap();
    assert!(heterogeneous.len() == 2);

    for (key, value) in heterogeneous {
        match key {
            HeterogeneousKey::Name(name) => {
                assert_eq!(name.name, "Bag Item");
                assert_matches!(value.into_inner(), HeterogeneousValue::Value(v) if v.value == b"Bag Data");
            }
            HeterogeneousKey::AnotherName(name) => {
                assert_eq!(name.another_name, "Another Bag Item");
                assert_matches!(value.into_inner(), HeterogeneousValue::AnotherValue(v) if v.another_value == b"Another Bag Data");
            }
        }
    }

    drop(container);
}
