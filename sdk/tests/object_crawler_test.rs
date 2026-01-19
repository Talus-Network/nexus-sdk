#![cfg(feature = "test_utils")]

use {
    futures::future::join_all,
    nexus_sdk::{
        nexus::{
            crawler::{Bag, Crawler, DynamicMap, DynamicObjectMap, Map, ObjectBag, Set, TableVec},
            signer::Signer,
        },
        sui,
        test_utils::{self, sui_mocks},
        types::{deserialize_encoded_bytes, NexusObjects},
    },
    serde::{Deserialize, Serialize},
    serde_json::json,
    std::{collections::HashMap, str::FromStr, sync::Arc},
    tokio::{sync::Mutex, time::Instant},
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

// TODO: <https://github.com/Talus-Network/nexus-sdk/issues/318>
#[tokio::test]
async fn test_object_crawler() {
    // // Spin up the Sui instance.
    // let test_utils::containers::SuiInstance {
    //     rpc_port,
    //     faucet_port,
    //     pg: _pg,
    //     container: _container,
    //     ..
    // } = test_utils::containers::setup_sui_instance().await;

    let rpc_url = format!("https://grpc.ssfn.devnet.production.taluslabs.dev");
    let faucet_url = format!("https://faucet.devnet.production.taluslabs.dev/gas");

    let mut rng = rand::thread_rng();

    // Create a wallet and request some gas tokens.
    let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
    let addr = pk.public_key().derive_address();

    for _ in 0..10 {
        test_utils::faucet::request_tokens(&faucet_url, addr)
            .await
            .expect("Failed to request tokens from faucet.");
    }

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

    let gas_coins = test_utils::gas::fetch_gas_coins(&rpc_url, addr)
        .await
        .expect("Failed to fetch gas coins.");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let pkg_id = response
        .objects
        .iter()
        .find_map(|c| match c.data() {
            sui::types::ObjectData::Package(m) => Some(m.id),
            _ => None,
        })
        .expect("Move package must be published");

    let guy = response
        .objects
        .iter()
        .find_map(|obj| {
            let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                return None;
            };

            if *object_type.name() == sui::types::Identifier::from_static("Guy") {
                Some(obj)
            } else {
                None
            }
        })
        .expect("Guy object must be created");

    let crawler = Crawler::new(Arc::new(Mutex::new(
        sui::grpc::Client::new(rpc_url.clone()).expect("Could not create gRPC client"),
    )));
    let guy_id = guy.object_id();
    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy starting version: {guy_version}");

    // Controls shared object mutability in the tx.
    // - `mutable: false` -> shared immutable/read-only
    // - `mutable: true`  -> shared mutable/exclusive
    let guy_imm = sui::tx::Input::shared(guy.object_id(), guy.version(), false);
    let guy_mut = sui::tx::Input::shared(guy.object_id(), guy.version(), true);

    let mut grpc =
        sui::grpc::Client::new(format!("https://grpc.ssfn.devnet.production.taluslabs.dev"))
            .expect("Could not create gRPC client");
    let addr = pk.public_key().derive_address();
    let signer = Signer::new(
        Arc::new(Mutex::new(grpc.clone())),
        pk.clone(),
        std::time::Duration::from_secs(30),
        Arc::new(sui_mocks::mock_nexus_objects()),
    );

    let reference_gas_price = grpc
        .get_reference_gas_price()
        .await
        .expect("Failed to get reference gas price.");

    const TASKS_PER_RUN: usize = 5;
    let mut gas_iter = gas_coins.into_iter();
    let shared_signer_gas: Vec<_> = gas_iter.by_ref().take(TASKS_PER_RUN).collect();
    let per_task_signer_gas: Vec<_> = gas_iter.by_ref().take(TASKS_PER_RUN).collect();
    let read_imm_gas: Vec<_> = gas_iter.by_ref().take(TASKS_PER_RUN).collect();
    let read_mut_gas: Vec<_> = gas_iter.by_ref().take(TASKS_PER_RUN).collect();
    let noop_mut_gas: Vec<_> = gas_iter.by_ref().take(TASKS_PER_RUN).collect();
    let serial_mut_gas: Vec<_> = gas_iter.by_ref().take(TASKS_PER_RUN).collect();

    assert_eq!(shared_signer_gas.len(), TASKS_PER_RUN);
    assert_eq!(per_task_signer_gas.len(), TASKS_PER_RUN);
    assert_eq!(read_imm_gas.len(), TASKS_PER_RUN);
    assert_eq!(read_mut_gas.len(), TASKS_PER_RUN);
    assert_eq!(noop_mut_gas.len(), TASKS_PER_RUN);
    assert_eq!(serial_mut_gas.len(), TASKS_PER_RUN);

    println!("=== shared signer run ({TASKS_PER_RUN} txs) ===");
    let batch_start = Instant::now();
    let tasks = shared_signer_gas.into_iter().map(|(gas_coin, _)| {
        let signer = signer.clone();
        let addr = addr;
        let reference_gas_price = reference_gas_price;
        let gas_coin = gas_coin.clone();
        let guy = guy_imm.clone();

        tokio::spawn(async move {
            println!("Starting tx execution task...");

            let mut tx = sui::tx::TransactionBuilder::new();

            let guy = tx.input(guy);

            tx.move_call(
                sui::tx::Function::new(
                    pkg_id,
                    sui::types::Identifier::from_static("main"),
                    sui::types::Identifier::from_static("test_serial"),
                    vec![],
                ),
                vec![guy],
            );

            tx.set_sender(addr);
            tx.set_gas_budget(1_000_000_000);
            tx.set_gas_price(reference_gas_price);
            tx.add_gas_objects(vec![sui::tx::Input::owned(
                *gas_coin.object_id(),
                gas_coin.version(),
                *gas_coin.digest(),
            )]);

            let tx = tx.finish().expect("Failed to finish transaction.");

            let signature = signer
                .sign_tx(&tx)
                .await
                .expect("Failed to sign transaction.");

            let now = Instant::now();

            println!("Executing transaction...");

            match signer
                .execute_tx(tx, signature, &mut gas_coin.clone())
                .await
            {
                Ok(_) => println!("Executed tx (ok) in {:?}", now.elapsed()),
                Err(e) => println!("Executed tx (err) in {:?}: {e}", now.elapsed()),
            }
        })
    });

    join_all(tasks).await;
    println!("Shared signer batch took {:?}", batch_start.elapsed());

    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy version after shared signer run: {guy_version}");

    // Now do the same run but with a dedicated gRPC client per task to remove the
    // `Arc<Mutex<Client>>` contention from the timing measurement.
    let nexus_objects = Arc::new(sui_mocks::mock_nexus_objects());
    println!("=== per-task signer run ({TASKS_PER_RUN} txs) ===");
    let batch_start = Instant::now();
    let tasks = per_task_signer_gas.into_iter().map(|(gas_coin, _)| {
        let pk = pk.clone();
        let addr = addr;
        let rpc_url = rpc_url.clone();
        let reference_gas_price = reference_gas_price;
        let gas_coin = gas_coin.clone();
        let guy = guy_imm.clone();
        let nexus_objects = nexus_objects.clone();

        tokio::spawn(async move {
            let signer = Signer::new(
                Arc::new(Mutex::new(
                    sui::grpc::Client::new(rpc_url).expect("Could not create gRPC client"),
                )),
                pk,
                std::time::Duration::from_secs(30),
                nexus_objects,
            );

            let mut tx = sui::tx::TransactionBuilder::new();
            let guy = tx.input(guy);
            tx.move_call(
                sui::tx::Function::new(
                    pkg_id,
                    sui::types::Identifier::from_static("main"),
                    sui::types::Identifier::from_static("test_serial"),
                    vec![],
                ),
                vec![guy],
            );

            tx.set_sender(addr);
            tx.set_gas_budget(1_000_000_000);
            tx.set_gas_price(reference_gas_price);
            tx.add_gas_objects(vec![sui::tx::Input::owned(
                *gas_coin.object_id(),
                gas_coin.version(),
                *gas_coin.digest(),
            )]);

            let tx = tx.finish().expect("Failed to finish transaction.");
            let signature = signer
                .sign_tx(&tx)
                .await
                .expect("Failed to sign transaction.");

            let now = Instant::now();
            match signer
                .execute_tx(tx, signature, &mut gas_coin.clone())
                .await
            {
                Ok(_) => println!("Executed tx (ok) in {:?}", now.elapsed()),
                Err(e) => println!("Executed tx (err) in {:?}: {e}", now.elapsed()),
            }
        })
    });

    join_all(tasks).await;
    println!("Per-task signer batch took {:?}", batch_start.elapsed());

    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy version after per-task signer run: {guy_version}");

    async fn per_task_signer_batch(
        label: &str,
        function: &'static str,
        guy: sui::tx::Input,
        gas_coins: Vec<(sui::types::ObjectReference, u64)>,
        pk: sui::crypto::Ed25519PrivateKey,
        addr: sui::types::Address,
        rpc_url: String,
        pkg_id: sui::types::Address,
        reference_gas_price: u64,
        nexus_objects: Arc<NexusObjects>,
    ) {
        println!("=== {label} ({}) ===", gas_coins.len());
        let batch_start = Instant::now();

        let tasks = gas_coins.into_iter().map(|(gas_coin, _)| {
            let signer = Signer::new(
                Arc::new(Mutex::new(
                    sui::grpc::Client::new(rpc_url.clone()).expect("Could not create gRPC client"),
                )),
                pk.clone(),
                std::time::Duration::from_secs(30),
                nexus_objects.clone(),
            );

            let guy = guy.clone();
            tokio::spawn(async move {
                let mut tx = sui::tx::TransactionBuilder::new();
                let guy = tx.input(guy);
                tx.move_call(
                    sui::tx::Function::new(
                        pkg_id,
                        sui::types::Identifier::from_static("main"),
                        sui::types::Identifier::from_static(function),
                        vec![],
                    ),
                    vec![guy],
                );

                tx.set_sender(addr);
                tx.set_gas_budget(1_000_000_000);
                tx.set_gas_price(reference_gas_price);
                tx.add_gas_objects(vec![sui::tx::Input::owned(
                    *gas_coin.object_id(),
                    gas_coin.version(),
                    *gas_coin.digest(),
                )]);

                let tx = tx.finish().expect("Failed to finish transaction.");
                let signature = signer
                    .sign_tx(&tx)
                    .await
                    .expect("Failed to sign transaction.");

                let now = Instant::now();
                match signer
                    .execute_tx(tx, signature, &mut gas_coin.clone())
                    .await
                {
                    Ok(_) => println!("Executed tx (ok) in {:?}", now.elapsed()),
                    Err(e) => println!("Executed tx (err) in {:?}: {e}", now.elapsed()),
                }
            })
        });

        join_all(tasks).await;
        println!("{label} batch took {:?}", batch_start.elapsed());
    }

    // On-chain `&T` + tx `mutable: false`: expected parallel.
    per_task_signer_batch(
        "read-only, shared immutable (&Guy, mutable: false)",
        "test_read",
        guy_imm.clone(),
        read_imm_gas,
        pk.clone(),
        addr,
        rpc_url.clone(),
        pkg_id,
        reference_gas_price,
        nexus_objects.clone(),
    )
    .await;

    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy version after read-only immutable: {guy_version}");

    // On-chain `&T` + tx `mutable: true`: should succeed but will be treated as exclusive by consensus.
    per_task_signer_batch(
        "read-only, shared mutable (&Guy, mutable: true)",
        "test_read",
        guy_mut.clone(),
        read_mut_gas,
        pk.clone(),
        addr,
        rpc_url.clone(),
        pkg_id,
        reference_gas_price,
        nexus_objects.clone(),
    )
    .await;

    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy version after read-only mutable: {guy_version}");

    // On-chain `&mut T` + tx `mutable: true` but no writes: still exclusive.
    per_task_signer_batch(
        "noop &mut, shared mutable (&mut Guy, mutable: true)",
        "test_noop_mut",
        guy_mut.clone(),
        noop_mut_gas,
        pk.clone(),
        addr,
        rpc_url.clone(),
        pkg_id,
        reference_gas_price,
        nexus_objects.clone(),
    )
    .await;

    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy version after noop &mut: {guy_version}");

    // On-chain `&mut T` + tx `mutable: true` and a write.
    per_task_signer_batch(
        "write &mut, shared mutable (&mut Guy, mutable: true)",
        "test_serial",
        guy_mut.clone(),
        serial_mut_gas,
        pk.clone(),
        addr,
        rpc_url.clone(),
        pkg_id,
        reference_gas_price,
        nexus_objects.clone(),
    )
    .await;

    let guy_version = crawler
        .get_object_metadata(guy_id)
        .await
        .expect("Failed to fetch Guy metadata")
        .version;
    println!("Guy version after write &mut: {guy_version}");

    //     let crawler = Crawler::new(Arc::new(Mutex::new(grpc)));

    //     let guy = crawler
    //         .get_object::<Guy>(guy)
    //         .await
    //         .expect("Could not fetch Guy object.");

    //     assert!(guy.is_shared());

    //     let guy = guy.data;

    //     assert_eq!(guy.name, "John Doe");
    //     assert_eq!(guy.age, 30);
    //     assert_eq!(
    //         guy.hobbies.into_inner(),
    //         vec!["Reading".to_string(), "Swimming".to_string()]
    //             .into_iter()
    //             .collect()
    //     );

    //     // Check map and nested vector fetched correctly.
    //     let groups = guy.groups.into_inner();
    //     assert_eq!(groups.len(), 2);

    //     // Contains book club with the correct people.
    //     let group = groups.clone().into_iter().find(|(group, people)| {
    //         group.clone().name == "Book Club"
    //             && people.iter().any(|p| p.name == "Alice")
    //             && people.iter().any(|p| p.name == "Bob")
    //     });

    //     assert!(group.is_some());

    //     // Contains swimming club with the correct people.
    //     let group = groups.clone().into_iter().find(|(group, people)| {
    //         group.clone().name == "Swimming Club"
    //             && people.iter().any(|p| p.name == "Charlie")
    //             && people.iter().any(|p| p.name == "David")
    //     });

    //     assert!(group.is_some());

    //     // Fetch timetable that is an ObjectTable and has a nested ObjectBag.
    //     assert_eq!(guy.timetable.size(), 2);
    //     let timetable = crawler
    //         .get_dynamic_field_objects(&guy.timetable)
    //         .await
    //         .unwrap();
    //     assert_eq!(timetable.len(), 2);

    //     // Fetch monday.
    //     let monday = timetable
    //         .get(&Name {
    //             name: "Monday".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(monday.data.value.name, "Meeting");

    //     assert_eq!(monday.data.pouch.size(), 1);
    //     let pouch = crawler
    //         .get_dynamic_field_objects(&monday.data.pouch)
    //         .await
    //         .unwrap();

    //     assert_eq!(pouch.len(), 1);
    //     let (key, value) = pouch.into_iter().next().unwrap();
    //     assert_eq!(key.name, "Pouch Item");
    //     assert_eq!(value.data.value, b"Pouch Data");

    //     // Fetch tuesday
    //     let tuesday = timetable
    //         .get(&Name {
    //             name: "Tuesday".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(tuesday.data.value.name, "Code Review");

    //     assert_eq!(tuesday.data.pouch.size(), 1);
    //     let pouch = crawler
    //         .get_dynamic_field_objects(&tuesday.data.pouch)
    //         .await
    //         .unwrap();
    //     assert_eq!(pouch.len(), 1);
    //     let (key, value) = pouch.into_iter().next().unwrap();
    //     assert_eq!(key.name, "Pouch Code");
    //     assert_eq!(value.data.value, b"MOREDATA15");

    //     // Fetch friends which is an ObjectBag.
    //     assert_eq!(guy.friends.size(), 2);
    //     let friends = crawler
    //         .get_dynamic_field_objects(&guy.friends)
    //         .await
    //         .unwrap();
    //     assert_eq!(friends.len(), 2);

    //     // Fetch first friend.
    //     let charlie = friends
    //         .get(&Name {
    //             name: "Charlie".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(charlie.data.value.clone(), b"Never Seen");

    //     // Fetch second friend.
    //     let david = friends
    //         .get(&Name {
    //             name: "David".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(david.data.value.clone(), b"Definitely Imagination");

    //     // Now fetch bag which is a Bag. Finally.
    //     assert_eq!(guy.bag.size(), 2);
    //     let bag = crawler.get_dynamic_fields(&guy.bag).await.unwrap();
    //     assert_eq!(bag.len(), 2);

    //     // Fetch first item from bag.
    //     let item1 = bag
    //         .get(&Name {
    //             name: "Bag Item".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(item1.value.clone(), b"Bag Data");

    //     // Fetch second item from bag.
    //     let item2 = bag
    //         .get(&Name {
    //             name: "Bag Item 2".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(item2.value.clone(), b"Bag Data 2");

    //     // Fetch heterogeneous Bag.
    //     assert_eq!(guy.heterogeneous.size(), 2);
    //     let heterogeneous = crawler
    //         .get_dynamic_fields(&guy.heterogeneous)
    //         .await
    //         .unwrap();
    //     assert!(heterogeneous.len() == 2);

    //     for (key, value) in heterogeneous {
    //         if key.name == "Bag Item" {
    //             assert!(
    //                 matches!(value, HeterogeneousValue::Value(v) if v.value.clone() == b"Bag Data")
    //             );
    //         } else if key.name == "Another Bag Item" {
    //             assert!(
    //                 matches!(value, HeterogeneousValue::AnotherValue(v) if v.another_value.clone() == b"Another Bag Data")
    //             );
    //         } else {
    //             panic!("Unexpected key in heterogeneous bag: {:?}", key);
    //         }
    //     }

    //     // Fetch linked table.
    //     assert_eq!(guy.linked_table.size(), 1);
    //     let linked_table = crawler.get_dynamic_fields(&guy.linked_table).await.unwrap();
    //     assert_eq!(linked_table.len(), 1);

    //     // Fetch first value from linked table.
    //     let linked_item = linked_table
    //         .get(&Name {
    //             name: "Key 1".to_string(),
    //         })
    //         .unwrap();

    //     assert_eq!(linked_item.name, "Value 1");

    //     // Fetch TableVec.
    //     assert_eq!(guy.sequence.size(), 3);
    //     assert_eq!(guy.sequence.size_u64(), 3);
    //     let sequence_id = guy.sequence.id();
    //     assert_ne!(sequence_id, sui::types::Address::from_static("0x0"));

    //     let sequence = crawler.get_table_vec(&guy.sequence).await.unwrap();
    //     assert_eq!(sequence.len(), 3);
    //     assert_eq!(sequence[0].name, "First");
    //     assert_eq!(sequence[1].name, "Second");
    //     assert_eq!(sequence[2].name, "Third");
    // }

    // #[test]
    // fn crawler_wrapper_accessors_cover_id_and_size_helpers() {
    //     let id = nexus_sdk::sui::types::Address::from_static("0x123");

    //     let bag = Bag::new(id, 2);
    //     assert_eq!(bag.id(), id);
    //     assert_eq!(bag.size_u64(), 2);
    //     assert_eq!(bag.size(), 2);

    //     let object_bag = ObjectBag::new(id, 3);
    //     assert_eq!(object_bag.id(), id);
    //     assert_eq!(object_bag.size_u64(), 3);
    //     assert_eq!(object_bag.size(), 3);

    //     let table_vec: TableVec<u64> = TableVec::new(id, 5);
    //     assert_eq!(table_vec.id(), id);
    //     assert_eq!(table_vec.size_u64(), 5);
    //     assert_eq!(table_vec.size(), 5);
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
