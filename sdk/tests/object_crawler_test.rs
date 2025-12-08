#![cfg(feature = "test_utils")]

use {
    nexus_sdk::{
        nexus::{
            client::NexusClient,
            crawler::{Bytes, Crawler, DynamicMap, DynamicObjectMap, Map, Set},
        },
        sui,
        test_utils,
    },
    serde::{Deserialize, Serialize},
    std::{str::FromStr, sync::Arc},
    sui_keys::keystore::AccountKeystore,
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

    // Create a wallet and request some gas tokens.
    let (mut wallet, _) = test_utils::wallet::create_ephemeral_wallet_context(rpc_port)
        .expect("Failed to create a wallet.");
    let sui = wallet.get_client().await.expect("Could not get Sui client");

    let addr = wallet
        .active_address()
        .expect("Failed to get active address.");

    test_utils::faucet::request_tokens(&format!("http://127.0.0.1:{faucet_port}/gas"), addr)
        .await
        .expect("Failed to request tokens from faucet.");
    // let gas_coins = test_utils::gas::fetch_gas_coins(&sui, addr)
    //     .await
    //     .expect("Failed to fetch gas coins.");
    //
    // Publish test contract and fetch some IDs.
    // let response = test_utils::contracts::publish_move_package(
    //     &mut wallet,
    //     "tests/move/object_crawler_test",
    //     gas_coins.iter().nth(0).cloned().unwrap(),
    // )
    // .await;

    let sui::KeyPair::Ed25519(a) = wallet
        .config
        .keystore
        .get_key(&addr)
        .expect("TODO: remove old sdk")
    else {
        todo!("TODO: remove old sdk")
    };

    // TODO: remove wallet.
    let mut raw_pk = [0u8; 32];
    raw_pk.copy_from_slice(a.as_ref());
    let pk = sui::crypto::Ed25519PrivateKey::new(raw_pk);

    let cp = publish_core_packages(&mut wallet, faucet_port).await;
    let (nexus_objects, crypto_cap) =
        deploy_nexus_contracts(&mut wallet, faucet_port, "test", &cp).await;

    let gas_coins = test_utils::gas::fetch_gas_coins(&sui, addr)
        .await
        .expect("Failed to fetch gas coins.");

    let gas = gas_coins.iter().nth(1).cloned().unwrap();
    let client = NexusClient::builder()
        .with_private_key(pk)
        .with_grpc_url(&format!("http://127.0.0.1:{rpc_port}"))
        .with_nexus_objects(nexus_objects)
        .with_gas(
            vec![sui::types::ObjectReference::new(
                gas.coin_object_id.to_string().parse().unwrap(),
                gas.version.value(),
                gas.digest.to_string().parse().unwrap(),
            )],
            MIST_PER_SUI / 10,
        )
        .build()
        .await
        .unwrap();

    // let nexus_gas = gas_coins.iter().nth(2).cloned().unwrap();
    // let coin_id = nexus_gas.coin_object_id.to_string().parse().unwrap();

    // let res = client.gas().add_budget(coin_id).await.unwrap();

    let dag = serde_json::from_str(include_str!("../src/dag/_dags/math_branching.json")).unwrap();

    let res = client.workflow().publish(dag).await.unwrap();

    println!("digest: {:?}", res.tx_digest);
    println!("dag_id: {:?}", res.dag_object_id);

    // TODO: extract helper to mock tx execution.
    // TODO: extract helper to mock get object.
    // TODO: make it work here end to end with crypto auth -- needs leader so tough luck

    // let changes = response
    //     .object_changes
    //     .expect("TX response must have object changes");

    // let guy = *changes
    //     .iter()
    //     .find_map(|c| match c {
    //         sui::ObjectChange::Created {
    //             object_id,
    //             object_type,
    //             ..
    //         } if object_type.name == sui::move_ident_str!("Guy").into() => Some(object_id),
    //         _ => None,
    //     })
    //     .expect("Guy object must be created");

    // let guy = sui::types::Address::from_str(&guy.to_string()).unwrap();

    // let grpc = sui::grpc::Client::new(&format!("http://127.0.0.1:{rpc_port}"))
    //     .expect("Could not create gRPC client");

    // let crawler = Crawler::new(Arc::new(Mutex::new(grpc)));

    // let guy = crawler
    //     .get_object::<Guy>(guy)
    //     .await
    //     .expect("Could not fetch Guy object.");

    // assert!(guy.is_shared());

    // let guy = guy.data;

    // assert_eq!(guy.name, "John Doe");
    // assert_eq!(guy.age, 30);
    // assert_eq!(
    //     guy.hobbies.into_inner(),
    //     vec!["Reading".to_string(), "Swimming".to_string()]
    //         .into_iter()
    //         .collect()
    // );

    // // Check VecMap and nested vector fetched correctly.
    // let groups = guy.groups.into_inner();
    // assert_eq!(groups.len(), 2);

    // // Contains book club with the correct people.
    // let group = groups.clone().into_iter().find(|(group, people)| {
    //     group.clone().name == "Book Club"
    //         && people.iter().any(|p| p.name == "Alice")
    //         && people.iter().any(|p| p.name == "Bob")
    // });

    // assert!(group.is_some());

    // // Contains swimming club with the correct people.
    // let group = groups.clone().into_iter().find(|(group, people)| {
    //     group.clone().name == "Swimming Club"
    //         && people.iter().any(|p| p.name == "Charlie")
    //         && people.iter().any(|p| p.name == "David")
    // });

    // assert!(group.is_some());

    // // Fetch timetable that is an ObjectTable and has a nested ObjectBag.
    // assert_eq!(guy.timetable.size(), 2);
    // let timetable = crawler
    //     .get_dynamic_field_objects(&guy.timetable)
    //     .await
    //     .unwrap();
    // assert_eq!(timetable.len(), 2);

    // // Fetch monday.
    // let monday = timetable
    //     .get(&Name {
    //         name: "Monday".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(monday.data.value.name, "Meeting");

    // assert_eq!(monday.data.pouch.size(), 1);
    // let pouch = crawler
    //     .get_dynamic_field_objects(&monday.data.pouch)
    //     .await
    //     .unwrap();

    // assert_eq!(pouch.len(), 1);
    // let (key, value) = pouch.into_iter().next().unwrap();
    // assert_eq!(key.name, "Pouch Item");
    // assert_eq!(value.data.value.into_inner(), b"Pouch Data");

    // // Fetch tuesday
    // let tuesday = timetable
    //     .get(&Name {
    //         name: "Tuesday".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(tuesday.data.value.name, "Code Review");

    // assert_eq!(tuesday.data.pouch.size(), 1);
    // let pouch = crawler
    //     .get_dynamic_field_objects(&tuesday.data.pouch)
    //     .await
    //     .unwrap();
    // assert_eq!(pouch.len(), 1);
    // let (key, value) = pouch.into_iter().next().unwrap();
    // assert_eq!(key.name, "Pouch Code");
    // assert_eq!(value.data.value.into_inner(), b"MOREDATA15");

    // // Fetch chair which is a Table. Weirdly.
    // assert_eq!(guy.chair.size(), 2);
    // let chair = crawler.get_dynamic_fields(&guy.chair).await.unwrap();
    // assert_eq!(chair.len(), 2);

    // // Fetch chairman.
    // let chairman = chair
    //     .get(&Name {
    //         name: "Chairman".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(chairman.name, "John Doe");

    // // Fetch vice chairman.
    // let vice_chairman = chair
    //     .get(&Name {
    //         name: "Vice Chairman".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(vice_chairman.name, "Alice");

    // // Fetch friends which is an ObjectBag.
    // assert_eq!(guy.friends.size(), 2);
    // let friends = crawler
    //     .get_dynamic_field_objects(&guy.friends)
    //     .await
    //     .unwrap();
    // assert_eq!(friends.len(), 2);

    // // Fetch first friend.
    // let charlie = friends
    //     .get(&Name {
    //         name: "Charlie".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(charlie.data.value.clone().into_inner(), b"Never Seen");

    // // Fetch second friend.
    // let david = friends
    //     .get(&Name {
    //         name: "David".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(
    //     david.data.value.clone().into_inner(),
    //     b"Definitely Imagination"
    // );

    // // Now fetch bag which is a Bag. Finally.
    // assert_eq!(guy.bag.size(), 2);
    // let bag = crawler.get_dynamic_fields(&guy.bag).await.unwrap();
    // assert_eq!(bag.len(), 2);

    // // Fetch first item from bag.
    // let item1 = bag
    //     .get(&Name {
    //         name: "Bag Item".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(item1.value.clone().into_inner(), b"Bag Data");

    // // Fetch second item from bag.
    // let item2 = bag
    //     .get(&Name {
    //         name: "Bag Item 2".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(item2.value.clone().into_inner(), b"Bag Data 2");

    // // Fetch heterogeneous Bag.
    // assert_eq!(guy.heterogeneous.size(), 2);
    // let heterogeneous = crawler
    //     .get_dynamic_fields(&guy.heterogeneous)
    //     .await
    //     .unwrap();
    // assert!(heterogeneous.len() == 2);

    // for (key, value) in heterogeneous {
    //     if key.name == "Bag Item" {
    //         assert!(
    //             matches!(value, HeterogeneousValue::Value(v) if v.value.clone().into_inner() == b"Bag Data")
    //         );
    //     } else if key.name == "Another Bag Item" {
    //         assert!(
    //             matches!(value, HeterogeneousValue::AnotherValue(v) if v.another_value.clone().into_inner() == b"Another Bag Data")
    //         );
    //     } else {
    //         panic!("Unexpected key in heterogeneous bag: {:?}", key);
    //     }
    // }

    // // Fetch linked table.
    // assert_eq!(guy.linked_table.size(), 1);
    // let linked_table = crawler.get_dynamic_fields(&guy.linked_table).await.unwrap();
    // assert_eq!(linked_table.len(), 1);

    // // Fetch first value from linked table.
    // let linked_item = linked_table
    //     .get(&Name {
    //         name: "Key 1".to_string(),
    //     })
    //     .unwrap();

    // assert_eq!(linked_item.name, "Value 1");
}

use {
    anyhow::{bail, Result as AnyhowResult},
    nexus_sdk::{
        events::{FoundingLeaderCapCreatedEvent, NexusEvent, NexusEventKind},
        sui::{traits::TransactionBlockEffectsAPI, MIST_PER_SUI},
        test_utils::*,
        types::NexusObjects,
    },
    sha2::{Digest, Sha256},
    std::{
        collections::BTreeMap,
        fs::{self, File, OpenOptions},
        io::{Read, Write},
        path::{Path, PathBuf},
    },
    sui_move_build::implicit_deps,
    sui_package_management::{self, system_package_versions::latest_system_packages},
    tempfile::{Builder, NamedTempFile, TempDir},
};

/// The amount of gas coins to use for testing.
const DESIRED_GAS_COINS: usize = 10;
/// The minimum balance required for a coin to be split.
const MIN_SPLITTABLE_BALANCE: u64 = 1_000_000_000;
/// The amount of reserve to keep when splitting a coin.
const SPLIT_RESERVE: u64 = MIN_SPLITTABLE_BALANCE * 2;
/// The version of the core package cache.
const CORE_PACKAGE_CACHE_VERSION: u32 = 1;
/// The file name for the core package cache.
const CORE_PACKAGE_CACHE_FILE: &str = "nexus-core-packages-cache.json";

/// Compiled Move package bytes plus the IDs of its on-chain dependencies.
#[derive(Clone, Debug)]
struct CompiledPackage {
    /// The compiled Move package bytes.
    modules: Vec<Vec<u8>>,
    /// The IDs of the on-chain dependencies of the compiled packages.
    dependencies: Vec<sui::ObjectID>,
}

/// On-disk cache for frequently reused core Move packages compiled for tests.
#[derive(Serialize, Deserialize)]
struct PackageCacheFile {
    /// The version of the cache file.
    version: u32,
    /// The chain ID of the network the cache was generated for.
    chain_id: String,
    /// The hash of the primitives package.
    primitives_hash: String,
    /// The compiled primitives package.
    primitives: Option<PackageCacheEntry>,
    /// The hash of the interface package.
    interface_hash: String,
    /// The compiled interface packages.
    #[serde(default)]
    interfaces: BTreeMap<String, PackageCacheEntry>,
    /// The compiled workflow packages.
    /// BTreeMap provides deterministic ordering of keys which is important for reproducibility.
    #[serde(default)]
    workflows: BTreeMap<String, PackageCacheEntry>,
}

/// Serialized representation of a compiled package suitable for persistence.
#[derive(Clone, Serialize, Deserialize)]
struct PackageCacheEntry {
    #[serde(with = "serde_bytes_vec")]
    modules: Vec<Vec<u8>>,
    dependencies: Vec<String>,
}

mod serde_bytes_vec {
    use {
        serde::{
            de::{SeqAccess, Visitor},
            ser::SerializeSeq,
            Deserializer,
            Serializer,
        },
        serde_bytes::{ByteBuf, Bytes},
        std::fmt,
    };

    pub(super) fn serialize<S>(value: &Vec<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(value.len()))?;

        for bytes in value {
            seq.serialize_element(&Bytes::new(bytes))?;
        }

        seq.end()
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VecVisitor;

        impl<'de> Visitor<'de> for VecVisitor {
            type Value = Vec<Vec<u8>>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of byte buffers")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut out = Vec::new();

                while let Some(buf) = seq.next_element::<ByteBuf>()? {
                    out.push(buf.into_vec());
                }

                Ok(out)
            }
        }

        deserializer.deserialize_seq(VecVisitor)
    }
}

impl PackageCacheEntry {
    fn from_package(package: &CompiledPackage) -> Self {
        let modules = package.modules.to_vec();
        let dependencies = package
            .dependencies
            .iter()
            .map(ToString::to_string)
            .collect();

        PackageCacheEntry {
            modules,
            dependencies,
        }
    }

    fn into_compiled_package(self) -> CompiledPackage {
        let dependencies = self
            .dependencies
            .into_iter()
            .map(|dep| sui::ObjectID::from_str(&dep).expect("Failed to parse cached ObjectID"))
            .collect();

        CompiledPackage {
            modules: self.modules,
            dependencies,
        }
    }

    fn to_package(&self) -> CompiledPackage {
        self.clone().into_compiled_package()
    }
}

/// Load an existing cache file or initialise a fresh one for this chain + code hash.
fn initialize_cache(
    existing: Option<PackageCacheFile>,
    chain_id: &str,
    primitives_hash: &str,
    interface_hash: &str,
) -> PackageCacheFile {
    if let Some(cache) = existing {
        if cache.version == CORE_PACKAGE_CACHE_VERSION
            && cache.chain_id == chain_id
            && cache.primitives_hash == primitives_hash
            && cache.interface_hash == interface_hash
        {
            return cache;
        }
    }

    PackageCacheFile {
        version: CORE_PACKAGE_CACHE_VERSION,
        chain_id: chain_id.to_string(),
        primitives_hash: primitives_hash.to_string(),
        primitives: None,
        interface_hash: interface_hash.to_string(),
        interfaces: BTreeMap::new(),
        workflows: BTreeMap::new(),
    }
}

fn interface_cache_key(interface_hash: &str, primitives_pkg_id: &sui::ObjectID) -> String {
    format!("{interface_hash}:{primitives_pkg_id}")
}

/// Compose a stable key for workflow cache entries.
fn workflow_cache_key(workflow_hash: &str, interface_pkg_id: &sui::ObjectID) -> String {
    format!("{workflow_hash}:{interface_pkg_id}")
}

fn load_package_cache(path: &Path) -> Option<PackageCacheFile> {
    let file = File::open(path).ok()?;
    serde_json::from_reader::<_, PackageCacheFile>(file).ok()
}

fn save_package_cache(path: &Path, cache: &PackageCacheFile) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut tmp = if let Some(parent) = path.parent() {
        NamedTempFile::new_in(parent)?
    } else {
        NamedTempFile::new()?
    };

    serde_json::to_writer_pretty(tmp.as_file_mut(), cache)?;
    tmp.as_file_mut().flush()?;

    if path.exists() {
        fs::remove_file(path)?;
    }

    tmp.persist(path)?;

    Ok(())
}

fn compile_package(package_dir: &Path, chain_id: &str) -> AnyhowResult<CompiledPackage> {
    compile_package_with_overrides(package_dir, chain_id, &[])
}

/// Compile a Move package with optional named-address overrides.
fn compile_package_with_overrides(
    package_dir: &Path,
    chain_id: &str,
    overrides: &[(String, sui::ObjectID)],
) -> AnyhowResult<CompiledPackage> {
    let mut build_config = if overrides.is_empty() {
        sui_move_build::BuildConfig::new_for_testing()
    } else {
        let owned: Vec<(String, sui::ObjectID)> = overrides.to_vec();
        sui_move_build::BuildConfig::new_for_testing_replace_addresses(owned)
    };

    build_config.chain_id = Some(chain_id.to_string());
    build_config.config.implicit_dependencies = implicit_deps(latest_system_packages());

    let package = build_config.build(package_dir)?;

    Ok(CompiledPackage {
        modules: package.get_package_bytes(false),
        dependencies: package.get_dependency_storage_package_ids(),
    })
}

/// Reuse or rebuild the cached primitives package, updating the cache on misses.
fn load_or_compile_primitives(
    cache: &mut PackageCacheFile,
    primitives_dir: &Path,
    chain_id: &str,
) -> CompiledPackage {
    if let Some(entry) = cache.primitives.as_ref() {
        return entry.to_package();
    }

    let package =
        compile_package(primitives_dir, chain_id).expect("Failed to build primitives package");
    cache.primitives = Some(PackageCacheEntry::from_package(&package));
    package
}

/// Reuse or rebuild the cached interface package, accounting for the primitives it references.
fn load_or_compile_interface(
    cache: &mut PackageCacheFile,
    interface_hash: &str,
    chain_id: &str,
    primitives_pkg_id: &sui::ObjectID,
    interface_build_path: &Path,
) -> CompiledPackage {
    let key = interface_cache_key(interface_hash, primitives_pkg_id);

    if let Some(entry) = cache.interfaces.get(&key) {
        return entry.to_package();
    }

    let package =
        compile_package(interface_build_path, chain_id).expect("Failed to build interface package");

    cache
        .interfaces
        .insert(key, PackageCacheEntry::from_package(&package));

    package
}

/// Reuse or rebuild the cached workflow package, keyed to the published interface package.
fn load_or_compile_workflow(
    cache: &mut PackageCacheFile,
    workflow_hash: &str,
    chain_id: &str,
    interface_pkg_id: &sui::ObjectID,
    workflow_build_path: &Path,
) -> CompiledPackage {
    let key = workflow_cache_key(workflow_hash, interface_pkg_id);

    if let Some(entry) = cache.workflows.get(&key) {
        return entry.to_package();
    }

    let package =
        compile_package(workflow_build_path, chain_id).expect("Failed to build workflow package");

    cache
        .workflows
        .insert(key, PackageCacheEntry::from_package(&package));

    package
}

/// Hash package sources so the cache invalidates automatically on edits.
fn compute_package_hash(package_root: &Path) -> AnyhowResult<String> {
    let mut hasher = Sha256::new();

    for file in source_files(package_root) {
        let mut buf = Vec::new();
        File::open(&file)?.read_to_end(&mut buf)?;

        let relative = file
            .strip_prefix(package_root)?
            .to_string_lossy()
            .replace('\\', "/");
        hasher.update(relative.as_bytes());
        hasher.update(&buf);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn source_files(package_root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    let manifest = package_root.join("Move.toml");
    if manifest.exists() {
        files.push(manifest);
    }

    let lock = package_root.join("Move.lock");
    if lock.exists() {
        files.push(lock);
    }

    collect_sources(&package_root.join("sources"), &mut files);
    files.sort();
    files
}

fn collect_sources(dir: &Path, files: &mut Vec<PathBuf>) {
    if !dir.exists() {
        return;
    }

    for entry in fs::read_dir(dir).expect("Failed to read sources directory") {
        let entry = entry.expect("Failed to read directory entry");
        let path = entry.path();
        let file_type = entry.file_type().expect("Failed to read file type");

        if file_type.is_dir() {
            collect_sources(&path, files);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
}

async fn publish_compiled_package(
    wallet: &mut sui::WalletContext,
    package: &CompiledPackage,
    package_path: &Path,
    gas_coin: sui::Coin,
) -> sui::TransactionBlockResponse {
    let sui = wallet.get_client().await.expect("Failed to get Sui client");
    let addr = wallet
        .active_address()
        .expect("Failed to get active address.");

    let tx_kind = sui
        .transaction_builder()
        .publish_tx_kind(addr, package.modules.clone(), package.dependencies.clone())
        .await
        .expect("Failed to build publish transaction");

    let reference_gas_price = sui
        .read_api()
        .get_reference_gas_price()
        .await
        .expect("Failed to fetch reference gas price.");

    let tx_data = sui
        .transaction_builder()
        .tx_data(
            addr,
            tx_kind,
            sui::MIST_PER_SUI,
            reference_gas_price,
            vec![gas_coin.coin_object_id],
            None,
        )
        .await
        .expect("Failed to build transaction data.");

    let envelope = wallet.sign_transaction(&tx_data);

    let resp_options = sui::TransactionBlockResponseOptions::new()
        .with_events()
        .with_effects()
        .with_object_changes();
    let resp_finality = sui::ExecuteTransactionRequestType::WaitForLocalExecution;

    let response = sui
        .quorum_driver_api()
        .execute_transaction_block(envelope, resp_options, Some(resp_finality))
        .await
        .expect("Failed to execute transaction.");

    if let Some(effects) = response.effects.clone() {
        if effects.clone().into_status().is_err() {
            panic!("Transaction has erroneous effects: {package_path:?} {effects}");
        }
    }

    let lock_file = package_path.join("Move.lock");
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_file)
        .expect("Failed to create Move.lock file");

    sui_package_management::update_lock_file(
        wallet,
        sui_package_management::LockCommand::Publish,
        Some(package_path.to_path_buf()),
        Some(lock_file),
        &response,
    )
    .await
    .expect("Failed to update lock file.");

    response
}

/// Holder for the freshly published core packages plus the cache context used to build them.
pub(crate) struct CorePackages {
    root: TempDir,
    primitives_pkg_id: sui::ObjectID,
    interface_pkg_id: sui::ObjectID,
    cache_path: PathBuf,
    chain_id: String,
    primitives_hash: String,
    interface_hash: String,
}

impl CorePackages {
    pub(crate) fn primitives_pkg_id(&self) -> sui::ObjectID {
        self.primitives_pkg_id
    }

    pub(crate) fn interface_pkg_id(&self) -> sui::ObjectID {
        self.interface_pkg_id
    }

    pub(crate) fn cache_path(&self) -> &Path {
        &self.cache_path
    }

    pub(crate) fn chain_id(&self) -> &str {
        &self.chain_id
    }

    pub(crate) fn primitives_hash(&self) -> &str {
        &self.primitives_hash
    }

    pub(crate) fn interface_hash(&self) -> &str {
        &self.interface_hash
    }

    fn primitives_path(&self) -> std::path::PathBuf {
        self.root.path().join("primitives")
    }

    fn interface_path(&self) -> std::path::PathBuf {
        self.root.path().join("interface")
    }

    pub(crate) fn install_into(&self, destination: &Path) {
        copy_dir_recursive(&self.primitives_path(), &destination.join("primitives"));
        copy_dir_recursive(&self.interface_path(), &destination.join("interface"));
    }
}

/// Recursively copy a directory, preserving the structure of the source tree.
fn copy_dir_recursive(src: &Path, dest: &Path) {
    fs::create_dir_all(dest).expect("Failed to create destination directory");

    for entry in fs::read_dir(src).expect("Failed to read source directory") {
        let entry = entry.expect("Failed to read directory entry");
        let file_type = entry.file_type().expect("Failed to read entry type");
        let dest_path = dest.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path);
        } else if file_type.is_file() {
            fs::copy(entry.path(), &dest_path).expect("Failed to copy file");
        }
    }
}

/// Ensure that the wallet has a sufficient amount of gas coins.
/// It works by checking the number of gas coins in the wallet and requesting more from the faucet if necessary.
/// Or it splits the gas coins into smaller amounts and sends them to the wallet.
async fn ensure_gas_coin_reserve(
    sui: &sui::Client,
    wallet: &mut sui::WalletContext,
    faucet_url: &str,
    addr: sui::Address,
) {
    let mut coins = gas::fetch_gas_coins(sui, addr)
        .await
        .expect("Could not fetch gas coin.");

    if coins.len() >= DESIRED_GAS_COINS {
        return;
    }

    if coins.is_empty() {
        faucet::request_tokens(faucet_url, addr)
            .await
            .expect("Could not request gas tokens from the faucet.");

        coins = gas::fetch_gas_coins(sui, addr)
            .await
            .expect("Could not fetch gas coin.");

        if coins.len() >= DESIRED_GAS_COINS {
            return;
        }
    }

    coins.sort_by(|a, b| b.balance.cmp(&a.balance));

    let gas_coin = coins
        .first()
        .cloned()
        .expect("No gas coins found after faucet request.");

    let splits_needed = DESIRED_GAS_COINS.saturating_sub(coins.len());

    if splits_needed == 0 {
        return;
    }

    if gas_coin.balance <= SPLIT_RESERVE {
        faucet::request_tokens(faucet_url, addr)
            .await
            .expect("Could not request gas tokens from the faucet.");

        return;
    }

    let available_for_split = gas_coin.balance - SPLIT_RESERVE;
    let min_required = (splits_needed as u64).saturating_mul(MIN_SPLITTABLE_BALANCE);

    if available_for_split < min_required {
        faucet::request_tokens(faucet_url, addr)
            .await
            .expect("Could not request gas tokens from the faucet.");

        return;
    }

    let mut base_amount = available_for_split / splits_needed as u64;

    if base_amount < MIN_SPLITTABLE_BALANCE {
        base_amount = MIN_SPLITTABLE_BALANCE;
    }

    if base_amount * splits_needed as u64 > available_for_split {
        faucet::request_tokens(faucet_url, addr)
            .await
            .expect("Could not request gas tokens from the faucet.");

        return;
    }

    let mut amounts = vec![base_amount; splits_needed];
    let mut remaining = available_for_split - base_amount * splits_needed as u64;

    for amount in &mut amounts {
        if remaining == 0 {
            break;
        }

        *amount += 1;
        remaining -= 1;
    }

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    tx.pay_sui(vec![addr; splits_needed], amounts)
        .expect("Failed to build split transaction.");

    sign_and_execute_transaction(sui, wallet, tx.finish(), &gas_coin)
        .await
        .expect("Failed to execute split transaction.");
}

pub(crate) async fn publish_core_packages(
    wallet: &mut sui::WalletContext,
    faucet_port: u16,
) -> CorePackages {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Expected manifest directory to have two parents");
    let sui_root = project_root.join("nexus-next/sui");
    let primitives_dir = sui_root.join("primitives");
    let interface_dir = sui_root.join("interface");

    let sui = wallet.get_client().await.expect("Could not get Sui client");
    let chain_id = sui
        .read_api()
        .get_chain_identifier()
        .await
        .expect("Failed to read chain identifier");

    let primitives_hash =
        compute_package_hash(&primitives_dir).expect("Failed to hash primitives sources");
    let interface_hash =
        compute_package_hash(&interface_dir).expect("Failed to hash interface sources");

    let cache_path = project_root.join("target").join(CORE_PACKAGE_CACHE_FILE);
    let mut cache = initialize_cache(
        load_package_cache(&cache_path),
        &chain_id,
        &primitives_hash,
        &interface_hash,
    );

    let primitives_package = load_or_compile_primitives(&mut cache, &primitives_dir, &chain_id);

    let temp_dir = Builder::new()
        .prefix("nexus-core-packages-")
        .tempdir()
        .expect("Failed to create temporary directory for core Move packages");

    let primitives_path = temp_dir.path().join("primitives");
    let interface_path = temp_dir.path().join("interface");

    copy_dir_recursive(&primitives_dir, &primitives_path);
    copy_dir_recursive(&interface_dir, &interface_path);

    let addr = wallet.active_address().expect("No active address found");

    let faucet_url = format!("http://127.0.0.1:{faucet_port}/gas");

    faucet::request_tokens(&faucet_url, addr)
        .await
        .expect("Could not request gas tokens from the faucet.");

    ensure_gas_coin_reserve(&sui, wallet, &faucet_url, addr).await;

    let mut gas_coins = gas::fetch_gas_coins(&sui, addr)
        .await
        .expect("Could not fetch gas coin.")
        .into_iter();

    let gas_coin = gas_coins.next().expect("No gas coins found");
    let primitives =
        publish_compiled_package(wallet, &primitives_package, &primitives_path, gas_coin).await;

    let primitives_pkg_id = primitives
        .object_changes
        .expect("No object changes found in primitives")
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .expect("primitives_pkg_id not found");

    let interface_package = load_or_compile_interface(
        &mut cache,
        &interface_hash,
        &chain_id,
        &primitives_pkg_id,
        &interface_path,
    );

    let gas_coin = gas_coins.next().expect("No gas coins found");
    let interface =
        publish_compiled_package(wallet, &interface_package, &interface_path, gas_coin).await;

    let interface_pkg_id = interface
        .object_changes
        .expect("No object changes found in interface")
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .expect("interface_pkg_id not found");

    save_package_cache(&cache_path, &cache).expect("Failed to write core package cache");

    CorePackages {
        root: temp_dir,
        primitives_pkg_id,
        interface_pkg_id,
        cache_path,
        chain_id,
        primitives_hash,
        interface_hash,
    }
}

/// Build and deploy the Nexus Move packages into the local test Sui network,
/// returning the published object identifiers together with the generated
/// crypto capability.
pub(crate) async fn deploy_nexus_contracts(
    wallet: &mut sui::WalletContext,
    faucet_port: u16,
    namespace: &str,
    core_packages: &CorePackages,
) -> (NexusObjects, sui::types::Address) {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Expected manifest directory to have two parents");
    let sui_root = project_root.join("nexus-next/sui");

    let mut cache = initialize_cache(
        load_package_cache(core_packages.cache_path()),
        core_packages.chain_id(),
        core_packages.primitives_hash(),
        core_packages.interface_hash(),
    );

    let workflow_src_hash =
        compute_package_hash(&sui_root.join("workflow")).expect("Failed to hash workflow sources");

    let temp_dir = Builder::new()
        .prefix(&format!("nexus-move-{namespace}-"))
        .tempdir()
        .expect("Failed to create temporary directory for Move packages");

    let workflow_path = temp_dir.path().join("workflow");

    core_packages.install_into(temp_dir.path());
    copy_dir_recursive(&sui_root.join("workflow"), &workflow_path);

    let addr = wallet.active_address().expect("No active address found");
    let sui = wallet.get_client().await.expect("Could not get Sui client");

    let faucet_url = format!("http://127.0.0.1:{faucet_port}/gas");

    // Request gas coins once, then split the change locally to reach the target.
    faucet::request_tokens(&faucet_url, addr)
        .await
        .expect("Could not request gas tokens from the faucet.");

    ensure_gas_coin_reserve(&sui, wallet, &faucet_url, addr).await;

    // Build and deploy.
    let mut gas_coins = gas::fetch_gas_coins(&sui, addr)
        .await
        .expect("Could not fetch gas coin.")
        .into_iter();

    let gas_coin = gas_coins.next().expect("No gas coins found");
    let workflow_package = load_or_compile_workflow(
        &mut cache,
        &workflow_src_hash,
        core_packages.chain_id(),
        &core_packages.interface_pkg_id(),
        &workflow_path,
    );

    let workflow =
        publish_compiled_package(wallet, &workflow_package, &workflow_path, gas_coin).await;

    let workflow_changes = workflow
        .object_changes
        .expect("No object changes found in workflow");

    // Parse workflow package ID.
    let workflow_pkg_id = workflow_changes
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Published { package_id, .. } => Some(*package_id),
            _ => None,
        })
        .expect("workflow_pkg_id not found");

    save_package_cache(core_packages.cache_path(), &cache)
        .expect("Failed to update workflow cache");

    // Parse tool registry object ref.
    let tool_registry = workflow_changes
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_id,
                object_type,
                version,
                digest,
                ..
            } if object_type.module == sui::move_ident_str!("tool_registry").into()
                && object_type.name == sui::move_ident_str!("ToolRegistry").into() =>
            {
                Some(sui::ObjectRef {
                    object_id: *object_id,
                    version: *version,
                    digest: *digest,
                })
            }
            _ => None,
        })
        .expect("tool_registry not found");

    // Parse default tap object ref.
    let default_tap = workflow_changes
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_id,
                object_type,
                version,
                digest,
                ..
            } if object_type.module == sui::move_ident_str!("default_tap").into()
                && object_type.name == sui::move_ident_str!("DefaultTAP").into() =>
            {
                Some(sui::ObjectRef {
                    object_id: *object_id,
                    version: *version,
                    digest: *digest,
                })
            }
            _ => None,
        })
        .expect("default_tap not found");

    // Parse the gas service object ref.
    let gas_service = workflow_changes
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_id,
                object_type,
                version,
                digest,
                ..
            } if object_type.module == sui::move_ident_str!("gas").into()
                && object_type.name == sui::move_ident_str!("GasService").into() =>
            {
                Some(sui::ObjectRef {
                    object_id: *object_id,
                    version: *version,
                    digest: *digest,
                })
            }
            _ => None,
        })
        .expect("gas_service not found");

    // Parse the gas service object ref.
    let pre_key_vault = workflow_changes
        .iter()
        .find_map(|change| match change {
            sui::ObjectChange::Created {
                object_id,
                object_type,
                version,
                digest,
                ..
            } if object_type.module == sui::move_ident_str!("pre_key_vault").into()
                && object_type.name == sui::move_ident_str!("PreKeyVault").into() =>
            {
                Some(sui::ObjectRef {
                    object_id: *object_id,
                    version: *version,
                    digest: *digest,
                })
            }
            _ => None,
        })
        .expect("pre_key_vault not found");

    // Parse network ID.
    // let mut network_id = None;
    // let mut crypto_cap = None;

    // for event in workflow
    //     .events
    //     .expect("No events found")
    //     .data
    //     .into_iter()
    //     .filter_map(|event| event.try_into().ok() as Option<NexusEvent>)
    // {
    //     match event.data {
    //         NexusEventKind::FoundingLeaderCapCreated(FoundingLeaderCapCreatedEvent {
    //             network,
    //             ..
    //         }) => network_id = Some(network),
    //         NexusEventKind::PreKeyVaultCreated(e) => crypto_cap = Some(e.crypto_cap),
    //         _ => {}
    //     }
    // }

    // let network_id = network_id.expect("network_id not found");
    // let crypto_cap = crypto_cap.expect("crypto_cap not found");

    let network_id = sui::types::Address::from_static("0x1");
    let crypto_cap = sui::types::Address::from_static("0x2");

    let objects = nexus_sdk::types::NexusObjects {
        primitives_pkg_id: core_packages
            .primitives_pkg_id()
            .to_string()
            .parse()
            .unwrap(),
        interface_pkg_id: core_packages
            .interface_pkg_id()
            .to_string()
            .parse()
            .unwrap(),
        workflow_pkg_id: workflow_pkg_id.to_string().parse().unwrap(),
        network_id,
        tool_registry: sui::types::ObjectReference::new(
            tool_registry.object_id.to_string().parse().unwrap(),
            tool_registry.version.value(),
            tool_registry.digest.to_string().parse().unwrap(),
        ),
        default_tap: sui::types::ObjectReference::new(
            default_tap.object_id.to_string().parse().unwrap(),
            default_tap.version.value(),
            default_tap.digest.to_string().parse().unwrap(),
        ),
        gas_service: sui::types::ObjectReference::new(
            gas_service.object_id.to_string().parse().unwrap(),
            gas_service.version.value(),
            gas_service.digest.to_string().parse().unwrap(),
        ),
        pre_key_vault: sui::types::ObjectReference::new(
            pre_key_vault.object_id.to_string().parse().unwrap(),
            pre_key_vault.version.value(),
            pre_key_vault.digest.to_string().parse().unwrap(),
        ),
    };

    (objects, crypto_cap)
}

pub(crate) async fn sign_and_execute_transaction(
    sui: &sui::Client,
    wallet: &mut sui::WalletContext,
    tx: sui::ProgrammableTransaction,
    gas_coin: &sui::Coin,
) -> anyhow::Result<sui::TransactionBlockResponse> {
    let addr = wallet.active_address()?;
    sign_and_execute_transaction_keystore(sui, &wallet.config.keystore, addr, tx, gas_coin).await
}

pub(crate) async fn sign_and_execute_transaction_keystore(
    sui: &sui::Client,
    keystore: &sui::Keystore,
    sender: sui::Address,
    tx: sui::ProgrammableTransaction,
    gas_coin: &sui::Coin,
) -> anyhow::Result<sui::TransactionBlockResponse> {
    let reference_gas_price = sui.read_api().get_reference_gas_price().await?;

    let tx_data = sui::TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        tx,
        MIST_PER_SUI,
        reference_gas_price,
    );

    let sig = keystore.sign_secure(&sender, &tx_data, sui::Intent::sui_transaction())?;
    let envelope = sui::Transaction::from_data(tx_data, vec![sig]);

    let resp_options = sui::TransactionBlockResponseOptions::new()
        .with_balance_changes()
        .with_effects()
        .with_object_changes()
        .with_events();

    let resp_finality = sui::ExecuteTransactionRequestType::WaitForLocalExecution;

    let response = sui
        .quorum_driver_api()
        .execute_transaction_block(envelope, resp_options, Some(resp_finality))
        .await?;

    if !response.errors.is_empty() {
        bail!("Transaction failed with errors: {:?}", response.errors);
    }

    if let Some(sui::TransactionBlockEffects::V1(effect)) = &response.effects {
        if let sui::ExecutionStatus::Failure { error } = effect.clone().into_status() {
            bail!("Transaction effects failed: {error}");
        }
    };

    Ok(response)
}
