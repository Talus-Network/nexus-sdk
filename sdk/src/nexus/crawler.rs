//! Module defining a Sui object crawler - this struct is able to fetch object
//! and dynamic field data from Sui GRPC and deserialize them into Rust structs.

use {
    crate::sui,
    anyhow::bail,
    serde::{de::DeserializeOwned, Deserialize, Deserializer},
    std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        marker::PhantomData,
        sync::Arc,
    },
    sui_rpc::field::FieldMaskUtil,
    tokio::sync::Mutex,
};

/// The main crawler struct.
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
        let field_mask =
            sui::grpc::FieldMask::from_paths(&["object_id", "owner", "version", "digest", "json"]);

        let object = self.fetch_object(object_id, field_mask).await?;
        let (owner, digest, version) = self.parse_object_metadata(object_id, &object)?;
        let data = self.parse_object_content(&object)?;

        Ok(Response {
            object_id,
            owner,
            version,
            data,
            digest,
        })
    }

    /// Fetch an object's metadata only, omitting its content.
    pub async fn get_object_metadata(
        &self,
        object_id: sui::types::Address,
    ) -> anyhow::Result<Response<()>> {
        let field_mask =
            sui::grpc::FieldMask::from_paths(&["object_id", "owner", "version", "digest"]);

        let object = self.fetch_object(object_id, field_mask).await?;
        let (owner, digest, version) = self.parse_object_metadata(object_id, &object)?;

        Ok(Response {
            object_id,
            owner,
            version,
            data: (),
            digest,
        })
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
            .map_err(|e| anyhow::anyhow!("Could not fetch object: {e}"))?;

        let Some(object) = response.object else {
            bail!("Object '{object_id}' not found");
        };

        Ok(object)
    }

    /// Helper function to parse metadata from an object.
    fn parse_object_metadata(
        &self,
        object_id: sui::types::Address,
        object: &sui::grpc::Object,
    ) -> anyhow::Result<(sui::types::Owner, sui::types::Digest, sui::types::Version)> {
        let owner = object
            .owner_opt()
            .ok_or_else(|| anyhow::anyhow!("Owner missing for object '{object_id}'"))?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Could not parse owner for object '{object_id}'"))?;

        let digest = object
            .digest_opt()
            .ok_or_else(|| anyhow::anyhow!("Digest missing for object '{object_id}'"))?
            .parse()
            .map_err(|_| anyhow::anyhow!("Could not parse digest for object '{object_id}'"))?;

        let version = object
            .version_opt()
            .ok_or_else(|| anyhow::anyhow!("Version missing for object '{object_id}'"))?;

        Ok((owner, digest, version))
    }

    /// Helper function to turn returned object data in prost format into T.
    fn parse_object_content<T>(&self, object: &sui::grpc::Object) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
    {
        let Some(json) = object.json_opt() else {
            bail!("Object content missing");
        };

        prost_value_to_json_value(&json)
            .and_then(|v| serde_json::from_value::<T>(v).map_err(anyhow::Error::new))
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
}

impl<T> Response<T> {
    /// Check if the object is shared.
    pub fn is_shared(&self) -> bool {
        matches!(self.owner, sui::types::Owner::Shared(_))
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
#[derive(Clone, Debug)]
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
    pub fn into_inner(self) -> HashMap<K, V> {
        self.contents
    }

    pub fn inner(&self) -> &HashMap<K, V> {
        &self.contents
    }

    pub fn inner_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.contents
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
        struct Wrapper<K, V>
        where
            K: Eq + Hash,
        {
            contents: Vec<ObjectKV<K, V>>,
        }

        let Wrapper { contents } = Wrapper::<K, V>::deserialize(deserializer)?;

        Ok(Self {
            contents: contents
                .into_iter()
                .map(|ObjectKV { key, value }| (key, value))
                .collect(),
        })
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

    // TODO: fetching.
}

/// Internal wrapper around a key-value pair within a map-like structure.
#[derive(Clone, Debug, Deserialize)]
struct ObjectKV<K, V> {
    key: K,
    value: V,
}

// == Helper functions ==

/// Helper function to transform [`prost_types::Value`] which is returned by the
/// GRPC into a [`serde_json::Value`]. This can then be easily deserialized into
/// any Rust struct.
fn prost_value_to_json_value(value: &prost_types::Value) -> anyhow::Result<serde_json::Value> {
    use {
        prost_types::value::Kind,
        serde_json::{Map, Number, Value},
    };

    let kind = value
        .kind
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing kind in prost_types::Value"))?;

    match kind {
        Kind::NullValue(_) => Ok(Value::Null),
        Kind::NumberValue(n) => match (n.fract() == 0.0, *n < 0.0) {
            (true, false) => Ok(Value::Number(Number::from_u128(*n as u128).ok_or_else(
                || anyhow::anyhow!("Could not convert number value '{n}' to JSON number"),
            )?)),
            (true, true) => Ok(Value::Number(Number::from_i128(*n as i128).ok_or_else(
                || anyhow::anyhow!("Could not convert number value '{n}' to JSON number"),
            )?)),
            (false, _) => Ok(Value::Number(Number::from_f64(*n).ok_or_else(|| {
                anyhow::anyhow!("Could not convert number value '{n}' to JSON number")
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
