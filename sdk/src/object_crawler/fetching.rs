use {
    crate::sui,
    anyhow::bail,
    serde::{de::DeserializeOwned, Deserialize, Serialize},
    std::hash::Hash,
};

/// Fetch a single object from Sui based on the provided object ID.
pub async fn fetch_one<T>(
    sui: &sui::Client,
    object_id: sui::ObjectID,
) -> anyhow::Result<Response<T>>
where
    T: DeserializeOwned,
{
    let options = sui::ObjectDataOptions::new().with_content().with_owner();

    let response = match sui
        .read_api()
        .get_object_with_options(object_id, options)
        .await
    {
        Ok(response) => response,
        Err(e) => bail!("Could not fetch object {object_id}: {e}"),
    };

    eprintln!("response: {response:#?}");
    parse_object_response(response)
}

/// Fetch a dynamic field object from Sui based on the provided key.
pub(crate) async fn dynamic_fetch_one<V>(
    sui: &sui::Client,
    object_id: sui::ObjectID,
    field_name: sui::DynamicFieldName,
) -> anyhow::Result<V>
where
    V: DeserializeOwned,
{
    let response = match sui
        .read_api()
        .get_dynamic_field_object(object_id, field_name)
        .await
    {
        Ok(response) => response,
        Err(e) => bail!("Could not fetch dynamic object {object_id} field: {e}"),
    };

    Ok(parse_object_response(response)?.data)
}

/// Batch fetch multiple objects from Sui based on the provided object IDs.
pub async fn fetch_many<T>(
    sui: &sui::Client,
    object_ids: Vec<sui::ObjectID>,
) -> anyhow::Result<Vec<Response<T>>>
where
    T: DeserializeOwned,
{
    let options = sui::ObjectDataOptions::new().with_content().with_owner();

    let response = match sui
        .read_api()
        .multi_get_object_with_options(object_ids, options)
        .await
    {
        Ok(response) => response,
        Err(e) => bail!("Could not batch fetch objects: {e}"),
    };

    response.into_iter().map(parse_object_response).collect()
}

/// Fetch all dynamic field objects for this resource.
pub(crate) async fn dynamic_fetch_many<K, V>(
    sui: &sui::Client,
    object_id: sui::ObjectID,
) -> anyhow::Result<Vec<(K, V)>>
where
    K: Eq + Hash + DeserializeOwned + Serialize,
    V: DeserializeOwned,
{
    let mut cursor = None;
    let limit = None;
    let mut objects = Vec::new();

    // Fetch all dynamic fields and their names.
    loop {
        let response = match sui
            .read_api()
            .get_dynamic_fields(object_id, cursor, limit)
            .await
        {
            Ok(response) => response,
            Err(e) => bail!("Could not fetch dynamic object {object_id} fields: {e}"),
        };

        cursor = response.next_cursor;

        objects.extend(response.data);

        if !response.has_next_page {
            break;
        }
    }

    let keys = objects
        .iter()
        .filter_map(|f| serde_json::from_value::<K>(f.name.value.clone()).ok())
        .collect::<Vec<_>>();

    let values = fetch_many::<V>(sui, objects.iter().map(|f| f.object_id).collect())
        .await?
        .into_iter()
        .map(|r| r.data)
        .collect::<Vec<_>>();

    if keys.len() != values.len() {
        bail!("Could not fetch all dynamic object {object_id} fields");
    }

    Ok(keys.into_iter().zip(values).collect())
}

/// Helper function to parse the response from Sui into a [Response] struct.
fn parse_object_response<T>(response: sui::ObjectResponse) -> anyhow::Result<Response<T>>
where
    T: DeserializeOwned,
{
    let object_id = match response.object_id() {
        Ok(object_id) => object_id,
        Err(e) => bail!("Could not get object ID from response: {e}"),
    };

    if let Some(e) = response.error {
        bail!("Could not fetch object {object_id}: {e}");
    }

    let data = match response.data {
        Some(data) => data,
        None => bail!("Could not fetch object {object_id}"),
    };

    // We expect a move object.
    let Some(sui::ParsedData::MoveObject(object)) = data.content else {
        bail!("Could not parse move object from object {object_id}")
    };

    // We expect the object owner data as we requested it.
    let Some(owner) = data.owner else {
        bail!("Could not parse owner data from object {object_id}")
    };

    // Using [serde_json::Value] as an intermediary format.
    match serde_json::to_value(&object).and_then(serde_json::from_value::<T>) {
        Ok(parsed) => Ok(Response {
            id: object_id,
            owner,
            data: parsed,
            version: data.version,
            digest: data.digest,
        }),
        Err(e) => bail!("Could not parse object {object_id}: {e}"),
    }
}

// == Response wrapper ==

/// We want to provide metadata from the response along with the object itself.
/// This struct holds this data. As we develop the Leader, more required
/// metadata can be added here.
#[derive(Clone, Debug)]
pub struct Response<T> {
    pub id: sui::ObjectID,
    pub owner: sui::Owner,
    pub version: sui::SequenceNumber,
    pub data: T,
    pub digest: sui::ObjectDigest,
}

impl<T> Response<T> {
    /// Check if the object is shared.
    pub fn is_shared(&self) -> bool {
        self.owner.is_shared()
    }

    /// Get initial shard version of the object.
    pub fn get_initial_version(&self) -> sui::SequenceNumber {
        match self.owner {
            sui::Owner::Shared {
                initial_shared_version,
            } => initial_shared_version,
            _ => self.version,
        }
    }

    // Get a Sui object ref.
    pub fn object_ref(&self) -> sui::ObjectRef {
        sui::ObjectRef {
            object_id: self.id,
            version: self.get_initial_version(),
            digest: self.digest,
        }
    }
}

// == Wrappers around various Sui SDK structures ==

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ObjectFields<T> {
    pub(crate) fields: T,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ObjectContents<T> {
    pub(crate) contents: T,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ObjectId {
    pub(crate) id: sui::UID,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ObjectKV<K, V> {
    #[serde(alias = "key", alias = "name")]
    pub(crate) key: K,
    pub(crate) value: V,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ObjectValue<T> {
    pub(crate) value: T,
}
