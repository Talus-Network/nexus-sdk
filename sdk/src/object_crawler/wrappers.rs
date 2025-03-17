//! This module defines wrapper around Move structures so that they can be
//! correctly deserialized into these structures.

use {
    crate::{object_crawler::fetching::*, sui},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize},
    std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        marker::PhantomData,
        str::FromStr,
    },
};

/// Move's `ObjectTable` wrapper. This wraps objects that are not stored
/// directly on the fetched object - one must fetch them separately.
///
/// If one value is fetched based on its name then the result is `Response<V>`
/// where `V` is the generic of `ObjectTable`.
///
/// If multiple values are fetched, then the result is `HashMap<K, V>`.
#[derive(Clone, Debug)]
pub struct ObjectTable<K, V> {
    /// The ID of the dynamic object.
    id: sui::UID,
    /// Fetching an ObjectTable automatically gives us the key type.
    tag: sui::MoveTypeTag,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> ObjectTable<K, V>
where
    K: Eq + Hash + DeserializeOwned + Serialize,
    V: DeserializeOwned,
{
    /// Fetch a single object from the table by its key.
    pub async fn fetch_one(&self, sui: &sui::Client, key: K) -> anyhow::Result<V> {
        let field_name = sui::DynamicFieldName {
            type_: self.tag.clone(),
            value: serde_json::to_value(key).map_err(anyhow::Error::new)?,
        };

        dynamic_fetch_one::<V>(sui, *self.id.object_id(), field_name).await
    }

    /// Fetch all objects from the table.
    pub async fn fetch_all(&self, sui: &sui::Client) -> anyhow::Result<HashMap<K, V>> {
        let response = dynamic_fetch_many::<K, V>(sui, *self.id.object_id()).await?;

        Ok(response.into_iter().collect())
    }
}

impl<'de, K, V> Deserialize<'de> for ObjectTable<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(rename = "type")]
            type_: String,
            fields: ObjectId,
        }

        let Wrapper { type_, fields } = Wrapper::deserialize(deserializer)?;

        let struct_tag = sui::MoveStructTag::from_str(type_.as_str()).map_err(|e| {
            serde::de::Error::custom(format!(
                "Could not parse sui::MoveStructTag from String: {e}"
            ))
        })?;

        let Some(tag) = struct_tag.type_params.into_iter().next() else {
            return Err(serde::de::Error::custom(
                "Could not get type parameter from `type_`",
            ));
        };

        Ok(Self {
            tag,
            id: fields.id,
            _marker: PhantomData,
        })
    }
}

/// Move's `Table` wrapper. This wraps objects that are not stored directly on
/// the fetched object - one must fetch them separately.
///
/// The structure of the dynamically fetched object is different from the
/// `ObjectTable` structure. `Table` has the key on the response, while
/// `ObjectTable` does not.
///
/// If one value is fetched based on its name then the result is `Response<V>`
/// where `V` is the generic of `Table`.
///
/// If multiple values are fetched, then the result is `HashMap<K, V>`.
#[derive(Clone, Debug)]
pub struct Table<K, V> {
    /// The ID of the dynamic object.
    id: sui::UID,
    /// Fetching an Table automatically gives us the key type.
    tag: sui::MoveTypeTag,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> Table<K, V>
where
    K: Eq + Hash + DeserializeOwned + Serialize,
    V: DeserializeOwned,
{
    /// Fetch a single object from the table by its key.
    pub async fn fetch_one(&self, sui: &sui::Client, key: K) -> anyhow::Result<V> {
        let field_name = sui::DynamicFieldName {
            type_: self.tag.clone(),
            value: serde_json::to_value(key).map_err(anyhow::Error::new)?,
        };

        Ok(
            dynamic_fetch_one::<ObjectFields<ObjectValue<V>>>(
                sui,
                *self.id.object_id(),
                field_name,
            )
            .await?
            .fields
            .value,
        )
    }

    /// Fetch all objects from the table.
    pub async fn fetch_all(&self, sui: &sui::Client) -> anyhow::Result<HashMap<K, V>> {
        let response =
            dynamic_fetch_many::<K, ObjectFields<ObjectValue<V>>>(sui, *self.id.object_id())
                .await?;

        Ok(response
            .into_iter()
            .map(|(key, ObjectFields { fields })| (key, fields.value))
            .collect())
    }
}

impl<'de, K, V> Deserialize<'de> for Table<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(rename = "type")]
            type_: String,
            fields: ObjectId,
        }

        let Wrapper { type_, fields } = Wrapper::deserialize(deserializer)?;

        let struct_tag = sui::MoveStructTag::from_str(type_.as_str()).map_err(|e| {
            serde::de::Error::custom(format!(
                "Could not parse sui::MoveStructTag from String: {e}"
            ))
        })?;

        let Some(tag) = struct_tag.type_params.into_iter().next() else {
            return Err(serde::de::Error::custom(
                "Could not get type parameter from `type_`",
            ));
        };

        Ok(Self {
            tag,
            id: fields.id,
            _marker: PhantomData,
        })
    }
}

/// Move's `ObjectBag` wrapper. This wraps objects that are not stored directly
/// on the fetched object - one must fetch them separately.
///
/// If one value is fetched based on its name then the result is `Response<V>`
/// where `V` is the generic of `ObjectBag`.
///
/// `ObjectBag` doesn't provide the key type because in Move, `ObjectBag`s are
/// heterogeneous. This means that when fetching one, one must provide the key
/// type. When fetching multiple, one must provide an enumeration of all
/// possible key types.
///
/// If multiple values are fetched, then the result is `HashMap<K, V>`.
#[derive(Clone, Debug)]
pub struct ObjectBag<K, V> {
    /// The ID of the dynamic object.
    id: sui::UID,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> ObjectBag<K, V>
where
    K: Eq + Hash + DeserializeOwned + Serialize,
    V: DeserializeOwned,
{
    /// Fetch a single object from the bag by its key.
    pub async fn fetch_one(
        &self,
        sui: &sui::Client,
        key: K,
        tag: sui::MoveTypeTag,
    ) -> anyhow::Result<V> {
        let field_name = sui::DynamicFieldName {
            type_: tag,
            value: serde_json::to_value(key).map_err(anyhow::Error::new)?,
        };

        dynamic_fetch_one::<V>(sui, *self.id.object_id(), field_name).await
    }

    /// Fetch all objects from the bag.
    pub async fn fetch_all(&self, sui: &sui::Client) -> anyhow::Result<HashMap<K, V>> {
        let response = dynamic_fetch_many::<K, V>(sui, *self.id.object_id()).await?;

        Ok(response.into_iter().collect())
    }
}

impl<'de, K, V> Deserialize<'de> for ObjectBag<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper {
            fields: ObjectId,
        }

        let Wrapper { fields } = Wrapper::deserialize(deserializer)?;

        Ok(Self {
            id: fields.id,
            _marker: PhantomData,
        })
    }
}

/// Move's `Bag` wrapper. This wraps objects that are not stored directly on the
/// fetched object - one must fetch them separately.
///
/// The structure of the dynamically fetched object is different from the
/// `ObjectBag` structure. `Bag` has the key on the response, while `ObjectBag`
/// does not.
///
/// If one value is fetched based on its name then the result is `Response<V>`
/// where `V` is the generic of `Bag`.
///
/// `Bag` doesn't provide the key type because in Move, `Bag`s are heterogeneous.
/// This means that when fetching one, one must provide the key type. When
/// fetching multiple, one must provide an enumeration of all possible key types.
///
/// If multiple values are fetched, then the result is `HashMap<K, V>`.
#[derive(Clone, Debug)]
pub struct Bag<K, V> {
    /// The ID of the dynamic object.
    id: sui::UID,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> Bag<K, V>
where
    K: Eq + Hash + DeserializeOwned + Serialize,
    V: DeserializeOwned,
{
    /// Fetch a single object from the bag by its key.
    pub async fn fetch_one(
        &self,
        sui: &sui::Client,
        key: K,
        tag: sui::MoveTypeTag,
    ) -> anyhow::Result<V> {
        let field_name = sui::DynamicFieldName {
            type_: tag,
            value: serde_json::to_value(key).map_err(anyhow::Error::new)?,
        };

        Ok(
            dynamic_fetch_one::<ObjectFields<ObjectValue<V>>>(
                sui,
                *self.id.object_id(),
                field_name,
            )
            .await?
            .fields
            .value,
        )
    }

    /// Fetch all objects from the bag.
    pub async fn fetch_all(&self, sui: &sui::Client) -> anyhow::Result<HashMap<K, V>> {
        let response =
            dynamic_fetch_many::<K, ObjectFields<ObjectValue<V>>>(sui, *self.id.object_id())
                .await?;

        Ok(response
            .into_iter()
            .map(|(key, ObjectFields { fields })| (key, fields.value))
            .collect())
    }
}

impl<'de, K, V> Deserialize<'de> for Bag<K, V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper {
            fields: ObjectId,
        }

        let Wrapper { fields } = Wrapper::deserialize(deserializer)?;

        Ok(Self {
            id: fields.id,
            _marker: PhantomData,
        })
    }
}

/// Move's `VecMap` wrapper. This type is stored directly on the fetched object.
#[derive(Clone, Debug)]
pub struct VecMap<K, V> {
    values: HashMap<K, V>,
}

impl<K, V> VecMap<K, V> {
    pub fn into_inner(self) -> HashMap<K, V> {
        self.values
    }

    pub fn inner(&self) -> &HashMap<K, V> {
        &self.values
    }

    pub fn inner_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.values
    }
}

impl<'de, K, V> Deserialize<'de> for VecMap<K, V>
where
    K: Eq + Hash + DeserializeOwned,
    V: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper<K, V> {
            fields: ObjectContents<Vec<ObjectFields<ObjectKV<K, V>>>>,
        }

        let Wrapper { fields } = Wrapper::<K, V>::deserialize(deserializer)?;

        Ok(Self {
            values: fields
                .contents
                .into_iter()
                .map(|ObjectFields { fields }| (fields.key, fields.value))
                .collect(),
        })
    }
}

/// Move's `VecSet` wrapper. This type is stored directly on the fetched object.
#[derive(Clone, Debug)]
pub struct VecSet<T> {
    values: HashSet<T>,
}

impl<T> VecSet<T> {
    pub fn into_inner(self) -> HashSet<T> {
        self.values
    }

    pub fn inner(&self) -> &HashSet<T> {
        &self.values
    }

    pub fn inner_mut(&mut self) -> &mut HashSet<T> {
        &mut self.values
    }
}

impl<'de, T> Deserialize<'de> for VecSet<T>
where
    T: Eq + Hash + DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper<T>
        where
            T: Eq + Hash,
        {
            fields: ObjectContents<HashSet<T>>,
        }

        let Wrapper { fields } = Wrapper::<T>::deserialize(deserializer)?;

        Ok(Self {
            values: fields.contents,
        })
    }
}

/// Move's `struct` type wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Structure<T> {
    fields: T,
}

impl<T> Structure<T> {
    pub fn into_inner(self) -> T {
        self.fields
    }

    pub fn inner(&self) -> &T {
        &self.fields
    }

    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.fields
    }
}
