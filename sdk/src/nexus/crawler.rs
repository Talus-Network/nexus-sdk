//! Module defining a Sui object crawler - this struct is able to fetch object
//! and dynamic field data from Sui GRPC and deserialize them into Rust structs.

use {
    crate::sui::{self, traits::FieldMaskUtil},
    anyhow::{anyhow, bail},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize},
    std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        marker::PhantomData,
        sync::Arc,
    },
    tokio::sync::Mutex,
};

/// The main crawler struct.
#[derive(Clone)]
pub struct Crawler {
    client: Arc<Mutex<sui::grpc::Client>>,
}

impl Crawler {
    pub fn new(client: Arc<Mutex<sui::grpc::Client>>) -> Self {
        Self { client }
    }

    /// Fetch an object by its ID and deserialize it into the specified type.
    pub async fn get_object<T>(&self, object_id: sui::types::Address) -> anyhow::Result<Response<T>>
    where
        T: DeserializeOwned,
    {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
            "json",
        ]);

        let object = self.fetch_object(object_id, field_mask).await?;
        let (owner, digest, version, balance) = self.parse_object_metadata(object_id, &object)?;
        let data = self.parse_object_content(&object)?;

        Ok(Response {
            object_id,
            owner,
            version,
            data,
            digest,
            balance,
        })
    }

    /// Fetch many objects by their IDs in batch and deserialize them into the
    /// specified type.
    pub async fn get_objects<T>(
        &self,
        object_ids: &[sui::types::Address],
    ) -> anyhow::Result<Vec<Response<T>>>
    where
        T: DeserializeOwned,
    {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
            "json",
        ]);

        let objects = self.fetch_objects(object_ids, field_mask).await?;

        objects
            .into_iter()
            .map(|object| {
                let object_id = object
                    .object_id_opt()
                    .ok_or_else(|| anyhow!("Object ID missing"))?
                    .parse()
                    .map_err(|_| anyhow!("Could not parse object ID"))?;

                let (owner, digest, version, balance) =
                    self.parse_object_metadata(object_id, &object)?;
                let data = self.parse_object_content(&object)?;

                Ok(Response {
                    object_id,
                    owner,
                    version,
                    data,
                    digest,
                    balance,
                })
            })
            .collect()
    }

    /// Fetch an object's metadata only, omitting its content.
    pub async fn get_object_metadata(
        &self,
        object_id: sui::types::Address,
    ) -> anyhow::Result<Response<()>> {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
        ]);

        let object = self.fetch_object(object_id, field_mask).await?;
        let (owner, digest, version, balance) = self.parse_object_metadata(object_id, &object)?;

        Ok(Response {
            object_id,
            owner,
            version,
            data: (),
            digest,
            balance,
        })
    }

    /// Fetch many objects' metadata only in batch, omitting their content.
    pub async fn get_objects_metadata(
        &self,
        object_ids: &[sui::types::Address],
    ) -> anyhow::Result<Vec<Response<()>>> {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
        ]);

        let objects = self.fetch_objects(object_ids, field_mask).await?;

        objects
            .into_iter()
            .map(|object| {
                let object_id = object
                    .object_id_opt()
                    .ok_or_else(|| anyhow!("Object ID missing"))?
                    .parse()
                    .map_err(|_| anyhow!("Could not parse object ID"))?;

                let (owner, digest, version, balance) =
                    self.parse_object_metadata(object_id, &object)?;

                Ok(Response {
                    object_id,
                    owner,
                    version,
                    data: (),
                    digest,
                    balance,
                })
            })
            .collect()
    }

    /// Fetch all dynamic object fields for a given parent and parse them into a
    /// HashMap<K, V>.
    pub async fn get_dynamic_fields<K, V>(
        &self,
        parent: &DynamicMap<K, V>,
    ) -> anyhow::Result<HashMap<K, V>>
    where
        K: Eq + Hash + DeserializeOwned,
        V: DeserializeOwned,
    {
        let names_and_ids = self
            .fetch_dynamic_fields::<K>(parent.id(), parent.size())
            .await?;

        // Now fetch all dynamic field objects in batch.
        let child_ids = names_and_ids
            .iter()
            .filter_map(|(_, _, id)| *id)
            .collect::<Vec<_>>();

        let child_objects = self.get_objects::<DynamicPair<K, V>>(&child_ids).await?;

        Ok(child_objects
            .into_iter()
            .map(|obj| (obj.data.name, obj.data.value.into_inner()))
            .collect())
    }

    /// Fetch all dynamic object fields for a given parent and parse them into a
    /// HashMap<K, V>. Since the values are objects, they need to be fetched
    /// first.
    pub async fn get_dynamic_field_objects<K, V>(
        &self,
        parent: &DynamicObjectMap<K, V>,
    ) -> anyhow::Result<HashMap<K, Response<V>>>
    where
        K: Eq + Hash + DeserializeOwned,
        V: DeserializeOwned,
    {
        let names_and_ids = self
            .fetch_dynamic_fields::<K>(parent.id(), parent.size())
            .await?;

        // Now fetch all dynamic field objects in batch.
        let child_ids = names_and_ids
            .iter()
            .filter_map(|(_, id, _)| *id)
            .collect::<Vec<_>>();

        let child_objects = self.get_objects::<V>(&child_ids).await?;

        Ok(names_and_ids
            .into_iter()
            .map(|(name, _, _)| name)
            .zip(child_objects.into_iter())
            .collect())
    }

    /// Fetch all items in a `TableVec<T>` and return them as a `Vec<T>`.
    pub async fn get_table_vec<T>(&self, parent: &TableVec<T>) -> anyhow::Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let expected_size = parent.size();
        if expected_size == 0 {
            return Ok(vec![]);
        }

        let names_and_ids = self
            .fetch_dynamic_fields::<u64>(parent.id(), expected_size)
            .await?;

        let field_ids = names_and_ids
            .iter()
            .filter_map(|(_, _, id)| *id)
            .collect::<Vec<_>>();

        let field_objects = self.get_objects::<DynamicPair<u64, T>>(&field_ids).await?;

        let mut values_by_index: Vec<Option<T>> = std::iter::repeat_with(|| None)
            .take(expected_size)
            .collect();
        for obj in field_objects {
            let index = usize::try_from(obj.data.name).unwrap_or(usize::MAX);
            if index >= expected_size {
                bail!("TableVec index out of bounds: {index} >= {expected_size}");
            }

            values_by_index[index] = Some(obj.data.value.into_inner());
        }

        values_by_index
            .into_iter()
            .enumerate()
            .map(|(index, value)| value.ok_or_else(|| anyhow!("Missing TableVec element {index}")))
            .collect()
    }

    /// Helper function to fetch an object based on its ID and field mask.
    async fn fetch_object(
        &self,
        object_id: sui::types::Address,
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<sui::grpc::Object> {
        let mut client = self.client.lock().await;

        let request = sui::grpc::GetObjectRequest::default()
            .with_object_id(object_id)
            .with_read_mask(field_mask);

        let response = client
            .ledger_client()
            .get_object(request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| anyhow!("Could not fetch object: {e}"))?;

        let Some(object) = response.object else {
            bail!("Object '{object_id}' not found");
        };

        Ok(object)
    }

    /// Helper function to fetch many objects based on their IDs and field mask.
    async fn fetch_objects(
        &self,
        object_ids: &[sui::types::Address],
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<Vec<sui::grpc::Object>> {
        let request = {
            let mut req = sui::grpc::BatchGetObjectsRequest::default();

            req.set_requests(
                object_ids
                    .iter()
                    .map(|&id| {
                        sui::grpc::GetObjectRequest::default()
                            .with_object_id(id)
                            .with_read_mask(field_mask.clone())
                    })
                    .collect(),
            );

            req.set_read_mask(field_mask);

            req
        };

        let mut client = self.client.lock().await;

        let response = client
            .ledger_client()
            .batch_get_objects(request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| anyhow!("Could not fetch objects: {e}"))?;

        let mut objects = Vec::with_capacity(object_ids.len());

        for result in response.objects {
            let object = result
                .object_opt()
                .ok_or_else(|| anyhow!("Object not found"))?;

            objects.push(object.clone());
        }

        Ok(objects)
    }

    /// Helper function to fetch all dynamic fields for a given parent object.
    /// Optionally stopping at `stop_at` if we're only interested in a singular
    /// item.
    async fn fetch_dynamic_fields<K>(
        &self,
        parent_id: sui::types::Address,
        expected_size: usize,
    ) -> anyhow::Result<Vec<(K, Option<sui::types::Address>, Option<sui::types::Address>)>>
    where
        K: Eq + Hash + DeserializeOwned,
    {
        let mut results = Vec::with_capacity(expected_size);
        let mut page_token = None;
        let field_mask = sui::grpc::FieldMask::from_paths(["name", "child_id", "field_id"]);

        loop {
            let mut request = sui::grpc::ListDynamicFieldsRequest::default()
                .with_parent(parent_id)
                .with_page_size(1000)
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let mut client = self.client.lock().await;
            let response = client
                .state_client()
                .list_dynamic_fields(request)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch dynamic fields for parent '{parent_id}': {e}")
                })?;

            drop(client);

            page_token = response.next_page_token;

            for field in response.dynamic_fields {
                // Parse the dynamic field name as K.
                let name = bcs::from_bytes::<K>(
                    field
                        .name_opt()
                        .ok_or_else(|| {
                            anyhow!("Dynamic field name missing for parent '{parent_id}'")
                        })?
                        .value(),
                )
                .map_err(|e| {
                    anyhow!("Could not parse dynamic field name for parent '{parent_id}': {e}")
                })?;

                let field_id = field
                    .field_id_opt()
                    .map(|id| id.parse())
                    .transpose()
                    .map_err(|_| anyhow!("Could not parse field ID for dynamic field"))?;

                let child_id = field
                    .child_id_opt()
                    .map(|id| id.parse())
                    .transpose()
                    .map_err(|_| anyhow!("Could not parse child ID for dynamic field"))?;

                results.push((name, child_id, field_id));
            }

            if page_token.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Helper function to parse metadata from an object.
    fn parse_object_metadata(
        &self,
        object_id: sui::types::Address,
        object: &sui::grpc::Object,
    ) -> anyhow::Result<(
        sui::types::Owner,
        sui::types::Digest,
        sui::types::Version,
        Option<u64>,
    )> {
        let owner = object
            .owner_opt()
            .ok_or_else(|| anyhow!("Owner missing for object '{object_id}'"))?
            .try_into()
            .map_err(|_| anyhow!("Could not parse owner for object '{object_id}'"))?;

        let digest = object
            .digest_opt()
            .ok_or_else(|| anyhow!("Digest missing for object '{object_id}'"))?
            .parse()
            .map_err(|_| anyhow!("Could not parse digest for object '{object_id}'"))?;

        let version = object
            .version_opt()
            .ok_or_else(|| anyhow!("Version missing for object '{object_id}'"))?;

        let balance = object.balance_opt();

        Ok((owner, digest, version, balance))
    }

    /// Helper function to turn returned object data in prost format into T.
    fn parse_object_content<T>(&self, object: &sui::grpc::Object) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
    {
        let Some(json) = object.json_opt() else {
            bail!("Object content missing");
        };

        prost_value_to_json_value(json)
            .and_then(|v| serde_json::from_value::<T>(v).map_err(anyhow::Error::new))
    }
}

/// Wrapper for `sui::bag::Bag` projection.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bag {
    #[serde(flatten)]
    inner: IdSize,
}

impl Bag {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            inner: IdSize { id, size },
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.inner.id
    }

    pub fn size_u64(&self) -> u64 {
        self.inner.size
    }

    pub fn size(&self) -> usize {
        self.inner.size()
    }
}

/// Wrapper for `sui::object_bag::ObjectBag` projection.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectBag {
    #[serde(flatten)]
    inner: IdSize,
}

impl ObjectBag {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            inner: IdSize { id, size },
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.inner.id
    }

    pub fn size_u64(&self) -> u64 {
        self.inner.size
    }

    pub fn size(&self) -> usize {
        self.inner.size()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdSize {
    pub id: sui::types::Address,
    #[serde(
        deserialize_with = "crate::types::deserialize_sui_u64",
        serialize_with = "crate::types::serialize_sui_u64"
    )]
    pub size: u64,
}

impl IdSize {
    pub fn size(&self) -> usize {
        usize::try_from(self.size).unwrap_or(0)
    }
}

/// A generic response wrapper for fetched objects. Contains metadata such as
/// the object ID version and owner.
#[derive(Clone, Debug)]
pub struct Response<T> {
    pub object_id: sui::types::Address,
    pub owner: sui::types::Owner,
    pub version: sui::types::Version,
    pub data: T,
    pub digest: sui::types::Digest,
    /// If the object is `0x2::coin::Coin`, contains the balance.
    pub balance: Option<u64>,
}

impl<T> Response<T> {
    /// Check if the object is shared.
    pub fn is_shared(&self) -> bool {
        matches!(self.owner, sui::types::Owner::Shared(_))
    }

    /// Check if the object is immutable.
    pub fn is_immutable(&self) -> bool {
        matches!(self.owner, sui::types::Owner::Immutable)
    }

    /// Get initial version of the object if it's shared or current version
    /// otherwise.
    pub fn get_initial_version(&self) -> sui::types::Version {
        match self.owner {
            sui::types::Owner::Shared(v) => v,
            _ => self.version,
        }
    }

    // Get a Sui object ref.
    pub fn object_ref(&self) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(self.object_id, self.version, self.digest)
    }
}

// == Wrappers ==

/// Wrapper around any vec-like structure within parsed Sui object data. These
/// are always wrapped in a struct with a single `contents` field.
#[derive(Clone, Debug, Deserialize)]
pub struct Set<T>
where
    T: Eq + Hash,
{
    contents: HashSet<T>,
}

impl<T> Set<T>
where
    T: Eq + Hash,
{
    pub fn into_inner(self) -> HashSet<T> {
        self.contents
    }

    pub fn inner(&self) -> &HashSet<T> {
        &self.contents
    }

    pub fn inner_mut(&mut self) -> &mut HashSet<T> {
        &mut self.contents
    }
}

/// Wrapper around any map-like structure within parsed Sui object data. These
/// are always wrapped in a struct with a single `contents` field. The contents
/// is a vec of key-value pairs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map<K, V>
where
    K: Eq + Hash,
{
    contents: HashMap<K, V>,
}

impl<K, V> Map<K, V>
where
    K: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            contents: HashMap::new(),
        }
    }

    pub fn from_map(contents: HashMap<K, V>) -> Self {
        Self { contents }
    }

    pub fn into_inner(self) -> HashMap<K, V> {
        self.contents
    }

    pub fn into_map(self) -> HashMap<K, V> {
        self.into_inner()
    }

    pub fn inner(&self) -> &HashMap<K, V> {
        &self.contents
    }

    pub fn inner_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.contents
    }
}

impl<K, V> Default for Map<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> From<HashMap<K, V>> for Map<K, V>
where
    K: Eq + Hash,
{
    fn from(contents: HashMap<K, V>) -> Self {
        Self { contents }
    }
}

impl<K, V> FromIterator<(K, V)> for Map<K, V>
where
    K: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        Self {
            contents: HashMap::from_iter(iter),
        }
    }
}

impl<K, V> Serialize for Map<K, V>
where
    K: Eq + Hash + Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct PairRef<'a, K, V> {
            key: &'a K,
            value: &'a V,
        }

        #[derive(Serialize)]
        struct Wrapper<'a, K, V> {
            contents: Vec<PairRef<'a, K, V>>,
        }

        let contents = self
            .contents
            .iter()
            .map(|(key, value)| PairRef { key, value })
            .collect();

        Wrapper { contents }.serialize(serializer)
    }
}

impl<'de, K, V> Deserialize<'de> for Map<K, V>
where
    K: Eq + Hash + DeserializeOwned,
    V: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Wrapper<K, V>
        where
            K: Eq + Hash,
        {
            Pairs { contents: Vec<Pair<K, V>> },
            Map { contents: HashMap<K, V> },
        }

        match Wrapper::<K, V>::deserialize(deserializer)? {
            Wrapper::Pairs { contents } => Ok(Self {
                contents: contents
                    .into_iter()
                    .map(|Pair { key, value }| (key, value))
                    .collect(),
            }),
            Wrapper::Map { contents } => Ok(Self { contents }),
        }
    }
}

/// Wrapper around `sui::table::Table<K, V>`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Table<K, V> {
    #[serde(flatten)]
    inner: IdSize,
    #[serde(skip)]
    _marker: PhantomData<(K, V)>,
}

impl<K, V> Table<K, V> {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            inner: IdSize { id, size },
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.inner.id
    }

    pub fn size_u64(&self) -> u64 {
        self.inner.size
    }

    pub fn size(&self) -> usize {
        self.inner.size()
    }
}

/// Wrapper around `sui::table_vec::TableVec<T>`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableVec<T> {
    contents: Table<u64, T>,
}

impl<T> TableVec<T> {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            contents: Table::new(id, size),
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.contents.id()
    }

    pub fn size_u64(&self) -> u64 {
        self.contents.size_u64()
    }

    pub fn size(&self) -> usize {
        self.contents.size()
    }
}

/// Wrapper around a dynamic map-like structure within parsed Sui object data.
/// These need to be fetched dynamically from Sui based on the parent object ID.
#[derive(Clone, Debug, Deserialize)]
pub struct DynamicMap<K, V> {
    id: sui::types::Address,
    size: String,
    #[serde(default)]
    _marker: PhantomData<(K, V)>,
}

impl<K, V> DynamicMap<K, V> {
    pub fn id(&self) -> sui::types::Address {
        self.id
    }

    pub fn size(&self) -> usize {
        self.size.parse().unwrap_or(0)
    }
}

/// Wrapper around a dynamic object map-like structure within parsed Sui object
/// data. These need to be fetched dynamically from Sui based on the parent
/// object ID. And then each object needs to be fetched separately (or batched).
#[derive(Clone, Debug, Deserialize)]
pub struct DynamicObjectMap<K, V> {
    id: sui::types::Address,
    size: String,
    #[serde(default)]
    _marker: PhantomData<(K, V)>,
}

impl<K, V> DynamicObjectMap<K, V> {
    pub fn id(&self) -> sui::types::Address {
        self.id
    }

    pub fn size(&self) -> usize {
        self.size.parse().unwrap_or(0)
    }
}

/// Internal wrapper around a key-value pair within a map-like structure.
#[derive(Clone, Debug, Deserialize)]
struct Pair<K, V> {
    #[serde(alias = "key", alias = "name")]
    key: K,
    value: V,
}

/// Internal wrapper around dynamic map fields.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum ValueOrWrapper<V> {
    Value(V),
    Wrapper { value: V },
}

impl<V> ValueOrWrapper<V> {
    fn into_inner(self) -> V {
        match self {
            ValueOrWrapper::Value(v) => v,
            ValueOrWrapper::Wrapper { value } => value,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct DynamicPair<K, V> {
    name: K,
    value: ValueOrWrapper<V>,
}

// == Helper functions ==

/// Helper function to transform [`prost_types::Value`] which is returned by the
/// GRPC into a [`serde_json::Value`]. This can then be easily deserialized into
/// any Rust struct.
pub fn prost_value_to_json_value(value: &prost_types::Value) -> anyhow::Result<serde_json::Value> {
    use {
        prost_types::value::Kind,
        serde_json::{Map, Number, Value},
    };

    let kind = value
        .kind
        .as_ref()
        .ok_or_else(|| anyhow!("Missing kind in prost_types::Value"))?;

    match kind {
        Kind::NullValue(_) => Ok(Value::Null),
        Kind::NumberValue(n) => match (n.fract() == 0.0, *n < 0.0) {
            (true, false) => Ok(Value::Number(Number::from_u128(*n as u128).ok_or_else(
                || anyhow!("Could not convert number value '{n}' to JSON number"),
            )?)),
            (true, true) => Ok(Value::Number(Number::from_i128(*n as i128).ok_or_else(
                || anyhow!("Could not convert number value '{n}' to JSON number"),
            )?)),
            (false, _) => Ok(Value::Number(Number::from_f64(*n).ok_or_else(|| {
                anyhow!("Could not convert number value '{n}' to JSON number")
            })?)),
        },
        Kind::StringValue(s) => Ok(Value::String(s.clone())),
        Kind::BoolValue(b) => Ok(Value::Bool(*b)),
        Kind::StructValue(sv) => {
            let mut map = Map::with_capacity(sv.fields.len());

            for (k, v) in &sv.fields {
                map.insert(k.clone(), prost_value_to_json_value(v)?);
            }

            Ok(Value::Object(map))
        }
        Kind::ListValue(lv) => {
            let mut vec = Vec::with_capacity(lv.values.len());

            for v in &lv.values {
                vec.push(prost_value_to_json_value(v)?);
            }

            Ok(Value::Array(vec))
        }
    }
}
