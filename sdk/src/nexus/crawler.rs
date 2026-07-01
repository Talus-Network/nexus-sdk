//! Module defining a Sui object crawler - this struct is able to fetch object
//! and dynamic field data from Sui GRPC and deserialize them into Rust structs.

use {
    crate::{
        sui::{self, traits::FieldMaskUtil},
        types::strip_fields_owned,
    },
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

#[derive(Debug, Deserialize)]
struct DynamicFieldNameBcs<K> {
    #[allow(unused)]
    id: sui::types::Address,
    name: K,
}

fn parse_dynamic_field_name<K>(bytes: &[u8]) -> Result<K, bcs::Error>
where
    K: DeserializeOwned,
{
    bcs::from_bytes::<K>(bytes)
        .or_else(|_| bcs::from_bytes::<DynamicFieldNameBcs<K>>(bytes).map(|field| field.name))
}

/// The main crawler struct.
#[derive(Clone)]
pub struct Crawler {
    client: Arc<Mutex<sui::grpc::Client>>,
}

#[derive(Debug)]
pub struct DynamicObjectFieldReference<K> {
    pub name: K,
    pub field_id: sui::types::Address,
    pub child_id: sui::types::Address,
}

impl Crawler {
    pub fn new(client: Arc<Mutex<sui::grpc::Client>>) -> Self {
        Self { client }
    }

    async fn get_object_parsed<T>(
        &self,
        object_id: sui::types::Address,
        field_mask: sui::grpc::FieldMask,
        parse_data: fn(&Crawler, &sui::grpc::Object) -> anyhow::Result<T>,
    ) -> anyhow::Result<Response<T>>
    where
        T: DeserializeOwned,
    {
        let object = self.fetch_object(object_id, field_mask).await?;

        let (owner, digest, version, balance) = self.parse_object_metadata(object_id, &object)?;
        let data = parse_data(self, &object)?;

        Ok(Response {
            object_id,
            owner,
            version,
            data,
            digest,
            balance,
        })
    }

    async fn get_objects_parsed<T>(
        &self,
        object_ids: &[sui::types::Address],
        field_mask: sui::grpc::FieldMask,
        parse_data: fn(&Crawler, &sui::grpc::Object) -> anyhow::Result<T>,
    ) -> anyhow::Result<Vec<Response<T>>>
    where
        T: DeserializeOwned,
    {
        let objects = self.fetch_objects(object_ids, field_mask).await?;

        objects
            .into_iter()
            .map(|object| {
                let object_id = Self::parse_object_id(&object)?;
                let (owner, digest, version, balance) =
                    self.parse_object_metadata(object_id, &object)?;
                let data = parse_data(self, &object)?;

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

    fn parse_object_id(object: &sui::grpc::Object) -> anyhow::Result<sui::types::Address> {
        object
            .object_id_opt()
            .ok_or_else(|| anyhow!("Object ID missing"))?
            .parse()
            .map_err(|_| anyhow!("Could not parse object ID"))
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

        self.get_object_parsed(object_id, field_mask, Self::parse_object_content::<T>)
            .await
    }

    /// Fetch an object by its ID and deserialize its Move struct contents from BCS.
    pub async fn get_object_contents_bcs<T>(
        &self,
        object_id: sui::types::Address,
    ) -> anyhow::Result<Response<T>>
    where
        T: DeserializeOwned,
    {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
            "contents",
        ]);

        self.get_object_parsed(object_id, field_mask, Self::parse_object_contents_bcs::<T>)
            .await
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

        self.get_objects_parsed(object_ids, field_mask, Self::parse_object_content::<T>)
            .await
    }

    /// Fetch many objects by their IDs in batch and deserialize their Move struct contents from BCS.
    ///
    /// This avoids relying on Sui's JSON rendering for Move types.
    pub async fn get_objects_contents_bcs<T>(
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
            "contents",
        ]);

        self.get_objects_parsed(object_ids, field_mask, Self::parse_object_contents_bcs::<T>)
            .await
    }

    /// Fetch the connected RPC's chain identifier in the 8-hex-char form
    /// Sui's Move builder uses for `[environments]` lookups (the same value
    /// `sui client chain-identifier` prints). The gRPC service-info call
    /// returns the genesis checkpoint digest base58-encoded; we decode it
    /// and hex-encode the first four bytes to derive the short identifier.
    pub async fn get_chain_id(&self) -> anyhow::Result<String> {
        let mut client = self.client.lock().await;
        let response = client
            .ledger_client()
            .get_service_info(sui::grpc::GetServiceInfoRequest::default())
            .await
            .map_err(|e| anyhow!("failed to fetch service info from the connected RPC: {e}"))?;
        let base58 = response
            .into_inner()
            .chain_id
            .ok_or_else(|| anyhow!("connected RPC did not return a chain id in service info"))?;
        let digest = sui::types::Digest::from_base58(&base58).map_err(|e| {
            anyhow!("connected RPC returned an unparsable chain id '{base58}': {e}")
        })?;
        Ok(hex::encode(&digest.as_bytes()[..4]))
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

    /// Resolve the checkpoint sequence number of the transaction that created
    /// `object_id`. Used by event-replay flows (e.g. `WorkflowActions::
    /// inspect_execution`) so callers do not need to pass an explicit
    /// "start from this checkpoint" argument.
    ///
    /// Three RPCs are chained:
    /// 1. `GetObject` for current metadata → reads the owner's
    ///    `initial_shared_version` (the version at which the object was first
    ///    made shared — its creation version on chain).
    /// 2. `GetObject` time-travelled to that version with mask
    ///    `[previous_transaction]` → returns the digest of the transaction
    ///    that produced that version, i.e. the creation tx.
    /// 3. `BatchGetTransactions` (single digest) with mask `[checkpoint]` →
    ///    returns the checkpoint sequence number the creation tx was sealed
    ///    in.
    ///
    /// Only shared objects are supported because owned objects can be created
    /// in transactions whose initial lamport version is not deterministic
    /// without an extra owner lookup; the inspect flow this powers always
    /// receives a shared `DAGExecution`.
    pub async fn get_object_creation_checkpoint(
        &self,
        object_id: sui::types::Address,
    ) -> anyhow::Result<u64> {
        // Step 1: current metadata → initial_shared_version.
        let metadata = self.get_object_metadata(object_id).await?;
        let sui::types::Owner::Shared(initial_version) = metadata.owner else {
            bail!(
                "Object '{object_id}' is not shared; cannot resolve a creation checkpoint via \
                 initial_shared_version"
            );
        };

        // Step 2: time-travel to the initial version, read previous_transaction.
        let mut client = self.client.lock().await;
        let request = sui::grpc::GetObjectRequest::default()
            .with_object_id(object_id)
            .with_version(initial_version)
            .with_read_mask(sui::grpc::FieldMask::from_paths(["previous_transaction"]));
        let response = client
            .ledger_client()
            .get_object(request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                anyhow!(
                    "Could not fetch object '{object_id}' at version {initial_version} to derive \
                     its creation checkpoint: {e}"
                )
            })?;
        let creation_digest = response
            .object
            .and_then(|object| object.previous_transaction_opt().map(|s| s.to_string()))
            .ok_or_else(|| {
                anyhow!(
                    "Object '{object_id}' has no previous_transaction at version \
                     {initial_version}; cannot derive its creation checkpoint"
                )
            })?;

        // Step 3: read the creation tx's checkpoint via batch_get_transactions
        // (one entry) so the call shares the existing tx-fetch surface.
        let tx_request = sui::grpc::BatchGetTransactionsRequest::default()
            .with_digests(vec![creation_digest.clone()])
            .with_read_mask(sui::grpc::FieldMask::from_paths(["checkpoint"]));
        let tx_response = client
            .ledger_client()
            .batch_get_transactions(tx_request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| {
                anyhow!(
                    "Could not fetch creation transaction '{creation_digest}' for object \
                     '{object_id}': {e}"
                )
            })?;
        let checkpoint = tx_response
            .transactions
            .first()
            .and_then(|tx_result| tx_result.transaction().checkpoint_opt())
            .ok_or_else(|| {
                anyhow!(
                    "Creation transaction '{creation_digest}' for object '{object_id}' has no \
                     checkpoint field set"
                )
            })?;

        Ok(checkpoint)
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
                let object_id = Self::parse_object_id(&object)?;

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

    /// Fetch owned objects of a specific type for an owner and deserialize JSON contents.
    pub async fn get_owned_objects<T>(
        &self,
        owner: sui::types::Address,
        object_type: sui::types::StructTag,
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
        let mut results = Vec::new();
        let mut page_token = None;

        loop {
            let mut request = sui::grpc::ListOwnedObjectsRequest::default()
                .with_owner(owner)
                .with_page_size(1000)
                .with_object_type(object_type.clone())
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let mut client = self.client.lock().await;
            let response = client
                .state_client()
                .list_owned_objects(request)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| anyhow!("Could not fetch owned objects for '{owner}': {e}"))?;

            drop(client);
            page_token = response.next_page_token;

            for object in response.objects {
                let object_id = Self::parse_object_id(&object)?;
                let (owner, digest, version, balance) =
                    self.parse_object_metadata(object_id, &object)?;
                let data = Self::parse_object_content::<T>(self, &object)?;
                results.push(Response {
                    object_id,
                    owner,
                    version,
                    data,
                    digest,
                    balance,
                });
            }

            if page_token.is_none() {
                break;
            }
        }

        Ok(results)
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

        let mut name_by_field_id = HashMap::with_capacity(names_and_ids.len());
        let mut field_ids = Vec::with_capacity(names_and_ids.len());

        for (name, _child_id, field_id) in names_and_ids {
            let Some(field_id) = field_id else {
                bail!("Dynamic field ID missing for dynamic map");
            };

            if name_by_field_id.insert(field_id, name).is_some() {
                bail!("Duplicate dynamic field ID '{field_id}' for dynamic map");
            }

            field_ids.push(field_id);
        }

        // Fetch the dynamic field objects and parse only their `value` field.
        // This avoids deserializing the `name` field from JSON, which may encode
        // `u64` keys as strings.
        let field_objects = self.get_objects::<DynamicValue<V>>(&field_ids).await?;

        let mut out = HashMap::with_capacity(field_objects.len());
        for obj in field_objects {
            let name = name_by_field_id.remove(&obj.object_id).ok_or_else(|| {
                anyhow!(
                    "Unexpected dynamic field ID '{}' for dynamic map",
                    obj.object_id
                )
            })?;

            out.insert(name, obj.data.value.into_inner());
        }

        Ok(out)
    }

    /// Fetch all dynamic object fields for a given parent object id and parse them into a
    /// HashMap<K, V>, deserializing each value from Move BCS.
    pub async fn get_dynamic_fields_bcs<K, V>(
        &self,
        parent_id: sui::types::Address,
        expected_size: usize,
    ) -> anyhow::Result<HashMap<K, V>>
    where
        K: Eq + Hash + DeserializeOwned,
        V: DeserializeOwned,
    {
        #[derive(Clone, Debug, Deserialize)]
        struct DynamicFieldValueBcs<K, V> {
            #[allow(dead_code)]
            id: sui::types::Address,
            #[allow(dead_code)]
            name: K,
            value: V,
        }

        let names_and_ids = self
            .fetch_dynamic_fields::<K>(parent_id, expected_size)
            .await?;

        let mut name_by_field_id = HashMap::with_capacity(names_and_ids.len());
        let mut field_ids = Vec::with_capacity(names_and_ids.len());

        for (name, _child_id, field_id) in names_and_ids {
            let Some(field_id) = field_id else {
                bail!("Dynamic field ID missing for dynamic map");
            };

            if name_by_field_id.insert(field_id, name).is_some() {
                bail!("Duplicate dynamic field ID '{field_id}' for dynamic map");
            }

            field_ids.push(field_id);
        }

        let field_objects = self
            .get_objects_contents_bcs::<DynamicFieldValueBcs<K, V>>(&field_ids)
            .await?;

        let mut out = HashMap::with_capacity(field_objects.len());
        for obj in field_objects {
            let name = name_by_field_id.remove(&obj.object_id).ok_or_else(|| {
                anyhow!(
                    "Unexpected dynamic field ID '{}' for dynamic map",
                    obj.object_id
                )
            })?;

            out.insert(name, obj.data.value);
        }

        Ok(out)
    }

    /// Fetch one dynamic field by BCS key, returning `Ok(None)` when that key is absent.
    ///
    /// Unlike [`Crawler::get_dynamic_fields_bcs`], this skips unrelated dynamic-field key
    /// namespaces under the same parent. That is useful for Sui objects that store several
    /// unrelated dynamic-field types directly under one UID.
    pub async fn get_optional_dynamic_field_bcs<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
    ) -> anyhow::Result<Option<V>>
    where
        K: Eq + DeserializeOwned,
        V: DeserializeOwned,
    {
        self.get_optional_dynamic_field_bcs_matching_value_type(parent_id, key, &[])
            .await
    }

    /// Fetch one dynamic field by BCS key and optional value type, returning `Ok(None)` when
    /// no compatible key/value-type pair exists.
    ///
    /// The optional value-type filter mirrors Move's `dynamic_field::exists_with_type` and
    /// prevents same-key-shape fields from forcing an incompatible value decode.
    pub async fn get_optional_dynamic_field_bcs_matching_value_type<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
        expected_value_types: &[String],
    ) -> anyhow::Result<Option<V>>
    where
        K: Eq + DeserializeOwned,
        V: DeserializeOwned,
    {
        #[derive(Clone, Debug, Deserialize)]
        struct DynamicFieldValueBcs<K, V> {
            #[allow(dead_code)]
            id: sui::types::Address,
            #[allow(dead_code)]
            name: K,
            value: V,
        }

        let mut page_token = None;
        let field_mask = sui::grpc::FieldMask::from_paths(["name", "field_id", "value_type"]);

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
                let Some(name) = field.name_opt() else {
                    continue;
                };
                let Ok(parsed_name) = parse_dynamic_field_name::<K>(name.value()) else {
                    continue;
                };
                if parsed_name != key {
                    continue;
                }
                if !expected_value_types.is_empty()
                    && !field.value_type_opt().is_some_and(|value_type| {
                        expected_value_types
                            .iter()
                            .any(|expected| expected == value_type)
                    })
                {
                    continue;
                }

                let field_id = field
                    .field_id_opt()
                    .ok_or_else(|| anyhow!("Dynamic field ID missing for parent '{parent_id}'"))?
                    .parse()
                    .map_err(|_| anyhow!("Could not parse field ID for dynamic field"))?;

                let object = self
                    .get_object_contents_bcs::<DynamicFieldValueBcs<K, V>>(field_id)
                    .await?;
                return Ok(Some(object.data.value));
            }

            if page_token.is_none() {
                break;
            }
        }

        Ok(None)
    }

    /// Fetch dynamic field values from their field objects without decoding key
    /// names. This is useful for singleton dynamic fields whose on-chain name
    /// encoding is package-version dependent.
    pub async fn get_dynamic_field_object_values_bcs<K, V>(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<V>>
    where
        K: DeserializeOwned,
        V: DeserializeOwned,
    {
        #[derive(Clone, Debug, Deserialize)]
        struct DynamicFieldValueBcs<K, V> {
            #[allow(dead_code)]
            id: sui::types::Address,
            #[allow(dead_code)]
            name: K,
            value: V,
        }

        let names_and_ids = self.fetch_dynamic_fields_untyped(parent_id).await?;
        let mut field_ids = Vec::with_capacity(names_and_ids.len());
        for field_id in names_and_ids {
            field_ids.push(field_id);
        }

        Ok(self
            .get_objects_contents_bcs::<DynamicFieldValueBcs<K, V>>(&field_ids)
            .await?
            .into_iter()
            .map(|response| response.data.value)
            .collect())
    }

    /// Fetch all dynamic-field values for a parent object without attempting to decode keys.
    pub async fn get_dynamic_field_values_bcs<V>(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<(Option<String>, V)>>
    where
        V: DeserializeOwned,
    {
        let mut results = Vec::new();
        let mut page_token = None;
        let field_mask =
            sui::grpc::FieldMask::from_paths(["field_id", "kind", "value", "value_type"]);

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
                let value = field.value_opt().ok_or_else(|| {
                    anyhow!("Dynamic field value missing for parent '{parent_id}'")
                })?;
                let Some(bytes) = value.value_opt() else {
                    bail!("Dynamic field value BCS missing for parent '{parent_id}'");
                };
                let decoded = bcs::from_bytes::<V>(bytes).map_err(|e| {
                    anyhow!("Could not parse dynamic field value for parent '{parent_id}': {e}")
                })?;
                results.push((field.value_type, decoded));
            }

            if page_token.is_none() {
                break;
            }
        }

        Ok(results)
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
            .zip(child_objects)
            .collect())
    }

    /// Fetch all dynamic object fields for a parent object id without requiring
    /// a local dynamic-object-map wrapper.
    pub async fn get_dynamic_object_fields<K, V>(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<HashMap<K, Response<V>>>
    where
        K: Eq + Hash + DeserializeOwned,
        V: DeserializeOwned,
    {
        let names_and_ids = self.fetch_dynamic_fields::<K>(parent_id, 0).await?;
        let mut names = Vec::with_capacity(names_and_ids.len());
        let mut child_ids = Vec::with_capacity(names_and_ids.len());

        for (name, child_id, _) in names_and_ids {
            let Some(child_id) = child_id else {
                bail!("Dynamic object field child ID missing for parent '{parent_id}'");
            };
            names.push(name);
            child_ids.push(child_id);
        }

        let child_objects = self.get_objects::<V>(&child_ids).await?;
        Ok(names.into_iter().zip(child_objects).collect())
    }

    /// Fetch one dynamic object field child for a parent object id.
    pub async fn get_dynamic_object_field<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
    ) -> anyhow::Result<Response<V>>
    where
        K: Eq + Hash + DeserializeOwned,
        V: DeserializeOwned,
    {
        self.get_dynamic_object_fields::<K, V>(parent_id)
            .await?
            .remove(&key)
            .ok_or_else(|| anyhow!("Dynamic object field not found for parent '{parent_id}'"))
    }

    /// Fetch dynamic object fields for a parent and keep only entries whose
    /// key BCS decodes as `K`. Parents may legitimately contain several child
    /// namespaces with unrelated key types.
    pub async fn get_dynamic_object_field_refs_matching_key<K>(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<DynamicObjectFieldReference<K>>>
    where
        K: Eq + Hash + DeserializeOwned,
    {
        Ok(self
            .fetch_dynamic_fields::<K>(parent_id, 0)
            .await?
            .into_iter()
            .filter_map(|(name, child_id, field_id)| {
                Some(DynamicObjectFieldReference {
                    name,
                    field_id: field_id?,
                    child_id: child_id?,
                })
            })
            .collect())
    }

    /// Fetch all dynamic object field child IDs for a parent object.
    pub async fn get_dynamic_object_field_child_ids(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<sui::types::Address>> {
        let mut child_ids = Vec::new();
        let mut page_token = None;
        let field_mask = sui::grpc::FieldMask::from_paths(["child_id"]);

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
                let child_id = field
                    .child_id_opt()
                    .map(|id| id.parse())
                    .transpose()
                    .map_err(|_| anyhow!("Could not parse child ID for dynamic field"))?;

                if let Some(child_id) = child_id {
                    child_ids.push(child_id);
                }
            }

            if page_token.is_none() {
                break;
            }
        }

        Ok(child_ids)
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

        let mut index_by_field_id = HashMap::with_capacity(names_and_ids.len());
        let mut field_ids = Vec::with_capacity(names_and_ids.len());

        for (name, _child_id, field_id) in names_and_ids {
            let Some(field_id) = field_id else {
                bail!("Dynamic field ID missing for TableVec");
            };

            let index = usize::try_from(name).unwrap_or(usize::MAX);
            if index >= expected_size {
                bail!("TableVec index out of bounds: {index} >= {expected_size}");
            }

            if index_by_field_id.insert(field_id, index).is_some() {
                bail!("Duplicate dynamic field ID '{field_id}' for TableVec");
            }

            field_ids.push(field_id);
        }

        let field_objects = self.get_objects::<DynamicValue<T>>(&field_ids).await?;

        let mut values_by_index: Vec<Option<T>> = std::iter::repeat_with(|| None)
            .take(expected_size)
            .collect();
        for obj in field_objects {
            let index = index_by_field_id
                .get(&obj.object_id)
                .copied()
                .ok_or_else(|| {
                    anyhow!(
                        "Unexpected dynamic field ID '{}' for TableVec",
                        obj.object_id
                    )
                })?;

            if values_by_index[index].is_some() {
                bail!("Duplicate TableVec element at index {index}");
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
                let name = parse_dynamic_field_name::<K>(
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

    async fn fetch_dynamic_fields_untyped(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<sui::types::Address>> {
        let mut results = Vec::new();
        let mut page_token = None;
        let field_mask = sui::grpc::FieldMask::from_paths(["field_id"]);

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
                let field_id = field
                    .field_id_opt()
                    .ok_or_else(|| anyhow!("Dynamic field ID missing for parent '{parent_id}'"))?
                    .parse()
                    .map_err(|_| anyhow!("Could not parse field ID for dynamic field"))?;
                results.push(field_id);
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

    fn parse_object_contents_bcs<T>(&self, object: &sui::grpc::Object) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
    {
        let Some(contents) = object.contents_opt() else {
            bail!("Object contents missing");
        };

        let Some(bytes) = contents.value_opt() else {
            bail!("Object BCS contents missing");
        };

        bcs::from_bytes::<T>(bytes).map_err(|e| {
            anyhow!(
                "Could not parse object contents BCS as `{ty}` (object id `{id}`, {len} bytes): {e}",
                ty = std::any::type_name::<T>(),
                id = object
                    .object_id_opt()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "<unknown>".into()),
                len = bytes.len(),
            )
        })
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
        sui::types::ObjectReference::new(self.object_id, self.get_initial_version(), self.digest)
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
    pub fn new() -> Self {
        Self {
            contents: HashSet::new(),
        }
    }

    pub fn into_inner(self) -> HashSet<T> {
        self.contents
    }

    pub fn inner(&self) -> &HashSet<T> {
        &self.contents
    }

    pub fn inner_mut(&mut self) -> &mut HashSet<T> {
        &mut self.contents
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }
}

impl<T> Default for Set<T>
where
    T: Eq + Hash,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T> From<HashSet<T>> for Set<T>
where
    T: Eq + Hash,
{
    fn from(contents: HashSet<T>) -> Self {
        Self { contents }
    }
}

impl<T> FromIterator<T> for Set<T>
where
    T: Eq + Hash,
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            contents: HashSet::from_iter(iter),
        }
    }
}

impl<T> Serialize for Set<T>
where
    T: Eq + Hash + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct Wrapper<'a, T> {
            contents: Vec<&'a T>,
        }

        Wrapper {
            contents: self.contents.iter().collect(),
        }
        .serialize(serializer)
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

/// Wrapper around `sui::table_vec::TableVec<T>`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TableVec<T> {
    contents: IdSize,
    #[serde(skip)]
    _marker: PhantomData<T>,
}

impl<T> TableVec<T> {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            contents: IdSize { id, size },
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.contents.id
    }

    pub fn size_u64(&self) -> u64 {
        self.contents.size
    }

    pub fn size(&self) -> usize {
        self.contents.size()
    }
}

/// Wrapper around a dynamic map-like structure within parsed Sui object data.
/// These need to be fetched dynamically from Sui based on the parent object ID.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DynamicMap<K, V> {
    id: sui::types::Address,
    #[serde(
        deserialize_with = "crate::types::deserialize_sui_u64",
        serialize_with = "crate::types::serialize_sui_u64"
    )]
    size: u64,
    #[serde(skip)]
    _marker: PhantomData<(K, V)>,
}

impl<K, V> DynamicMap<K, V> {
    pub fn new(id: sui::types::Address, size: u64) -> Self {
        Self {
            id,
            size,
            _marker: PhantomData,
        }
    }

    pub fn id(&self) -> sui::types::Address {
        self.id
    }

    pub fn size(&self) -> usize {
        usize::try_from(self.size).unwrap_or(0)
    }
}

/// Wrapper around a dynamic object map-like structure within parsed Sui object
/// data. These need to be fetched dynamically from Sui based on the parent
/// object ID. And then each object needs to be fetched separately (or batched).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DynamicObjectMap<K, V> {
    id: sui::types::Address,
    #[serde(
        deserialize_with = "crate::types::deserialize_sui_u64",
        serialize_with = "crate::types::serialize_sui_u64"
    )]
    size: u64,
    #[serde(skip)]
    _marker: PhantomData<(K, V)>,
}

impl<K, V> DynamicObjectMap<K, V> {
    pub fn id(&self) -> sui::types::Address {
        self.id
    }

    pub fn size(&self) -> usize {
        usize::try_from(self.size).unwrap_or(0)
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

fn strip_dynamic_value_owned(value: serde_json::Value) -> serde_json::Value {
    let value = strip_fields_owned(value);
    let serde_json::Value::Object(mut object) = value else {
        return value;
    };

    if let Some(inner) = object.remove("value") {
        object.insert("value".to_string(), strip_fields_owned(inner));
    }

    serde_json::Value::Object(object)
}

#[derive(Clone, Debug)]
struct DynamicValue<V> {
    value: ValueOrWrapper<V>,
}

impl<'de, V> Deserialize<'de> for DynamicValue<V>
where
    V: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawDynamicValue {
            value: serde_json::Value,
        }

        if !deserializer.is_human_readable() {
            #[derive(Deserialize)]
            struct BcsDynamicValue<V> {
                value: ValueOrWrapper<V>,
            }

            return BcsDynamicValue::deserialize(deserializer).map(|raw| Self { value: raw.value });
        }

        let raw = RawDynamicValue::deserialize(deserializer)?;
        let value = strip_dynamic_value_owned(raw.value);
        let value = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self { value })
    }
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::sui_mocks,
        mockall::predicate::always,
        serde::{Deserialize, Serialize},
        serde_json::json,
    };

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestValue {
        value: u64,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    struct TestKey {
        name: String,
    }

    fn object_with_json(
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        json_value: serde_json::Value,
    ) -> sui::grpc::Object {
        let mut object = sui::grpc::Object::default();
        object.set_object_id(object_ref.object_id().to_string());
        object.set_owner(sui::grpc::Owner::from(owner));
        object.set_digest(*object_ref.digest());
        object.set_version(object_ref.version());
        object.json = Some(Box::new(json_value_to_prost_value(&json_value)));
        object
    }

    fn json_value_to_prost_value(value: &serde_json::Value) -> prost_types::Value {
        use prost_types::value::Kind;

        let kind = match value {
            serde_json::Value::Null => Kind::NullValue(0),
            serde_json::Value::Bool(value) => Kind::BoolValue(*value),
            serde_json::Value::Number(value) => {
                Kind::NumberValue(value.as_f64().unwrap_or_default())
            }
            serde_json::Value::String(value) => Kind::StringValue(value.clone()),
            serde_json::Value::Array(values) => Kind::ListValue(prost_types::ListValue {
                values: values.iter().map(json_value_to_prost_value).collect(),
            }),
            serde_json::Value::Object(values) => Kind::StructValue(prost_types::Struct {
                fields: values
                    .iter()
                    .map(|(key, value)| (key.clone(), json_value_to_prost_value(value)))
                    .collect(),
            }),
        };

        prost_types::Value { kind: Some(kind) }
    }

    #[tokio::test]
    async fn get_owned_objects_pages_and_deserializes_json() {
        let owner = sui::types::Address::from_static("0xa");
        let first_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x10"));
        let second_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x11"));
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        let first_object = object_with_json(
            first_ref.clone(),
            sui::types::Owner::Address(owner),
            json!({
                "value": 7,
                "label": "first",
                "enabled": true,
                "tags": ["a", "b"],
                "optional": null
            }),
        );
        let second_object = object_with_json(
            second_ref.clone(),
            sui::types::Owner::Address(owner),
            json!({"value": 9}),
        );
        let responses: Vec<(Vec<sui::grpc::Object>, Option<Vec<u8>>)> = vec![
            (vec![first_object], Some(Vec::from(&b"page-2"[..]))),
            (vec![second_object], None),
        ];
        let mut responses = responses.into_iter();
        state_service_mock
            .expect_list_owned_objects()
            .times(2)
            .with(always())
            .returning(move |_request| {
                let (objects, next_page_token) = responses.next().expect("owned object page");
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(objects);
                response.next_page_token = next_page_token.map(Into::into);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let objects = crawler
            .get_owned_objects::<TestValue>(
                owner,
                sui::types::StructTag::new(
                    sui::types::Address::from_static("0x1"),
                    sui::types::Identifier::from_static("test"),
                    sui::types::Identifier::from_static("TestValue"),
                    vec![],
                ),
            )
            .await
            .expect("owned objects load");

        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].object_id, *first_ref.object_id());
        assert_eq!(objects[0].data, TestValue { value: 7 });
        assert_eq!(objects[1].object_id, *second_ref.object_id());
        assert_eq!(objects[1].data, TestValue { value: 9 });
    }

    #[tokio::test]
    async fn get_dynamic_object_field_fetches_child_object_by_key() {
        let parent_id = sui::types::Address::from_static("0x40");
        let field_id = sui::types::Address::from_static("0x41");
        let child_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x42"));
        let key = TestKey {
            name: "primary".to_string(),
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();

        sui_mocks::grpc::mock_list_dynamic_object_fields(
            &mut state_service_mock,
            vec![(key.clone(), field_id, *child_ref.object_id())],
        );
        sui_mocks::grpc::mock_get_objects_json(
            &mut ledger_service_mock,
            vec![(
                child_ref.clone(),
                sui::types::Owner::Shared(child_ref.version()),
                json!({"value": 11}),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let object = crawler
            .get_dynamic_object_field::<TestKey, TestValue>(parent_id, key)
            .await
            .expect("dynamic object field loads");

        assert_eq!(object.object_id, *child_ref.object_id());
        assert_eq!(object.data, TestValue { value: 11 });
    }

    #[tokio::test]
    async fn get_dynamic_fields_accepts_fields_wrapped_values() {
        let parent =
            DynamicMap::<TestKey, TestValue>::new(sui::types::Address::from_static("0x70"), 1);
        let key = TestKey {
            name: "wanted".to_string(),
        };
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x71"));
        let json = json!({
            "id": field_ref.object_id(),
            "name": key,
            "value": {
                "fields": {
                    "value": 42
                }
            }
        });

        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(key.clone(), *field_ref.object_id())],
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_batch_get_objects()
            .times(1)
            .with(always())
            .returning(move |_| {
                let object = object_with_json(
                    field_ref.clone(),
                    sui::types::Owner::Shared(1),
                    json.clone(),
                );
                let mut result = sui::grpc::GetObjectResult::default();
                result.set_object(object);
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                response.set_objects(vec![result]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let fields = crawler
            .get_dynamic_fields(&parent)
            .await
            .expect("wrapped dynamic field value decodes");

        assert_eq!(fields.get(&key), Some(&TestValue { value: 42 }));
    }

    #[tokio::test]
    async fn get_dynamic_fields_accepts_linked_table_node_wrapped_values() {
        let parent =
            DynamicMap::<TestKey, TestValue>::new(sui::types::Address::from_static("0x72"), 1);
        let key = TestKey {
            name: "node".to_string(),
        };
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x73"));
        let json = json!({
            "id": field_ref.object_id(),
            "name": key,
            "value": {
                "type": "0x2::linked_table::Node<test::TestKey, test::TestValue>",
                "fields": {
                    "next": null,
                    "prev": null,
                    "value": {
                        "type": "test::TestValue",
                        "fields": {
                            "value": 42
                        }
                    }
                }
            }
        });

        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(key.clone(), *field_ref.object_id())],
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_batch_get_objects()
            .times(1)
            .with(always())
            .returning(move |_| {
                let object = object_with_json(
                    field_ref.clone(),
                    sui::types::Owner::Shared(1),
                    json.clone(),
                );
                let mut result = sui::grpc::GetObjectResult::default();
                result.set_object(object);
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                response.set_objects(vec![result]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let fields = crawler
            .get_dynamic_fields(&parent)
            .await
            .expect("linked-table dynamic field value decodes");

        assert_eq!(fields.get(&key), Some(&TestValue { value: 42 }));
    }

    #[tokio::test]
    async fn get_dynamic_fields_accepts_sui_linked_table_node_json() {
        let parent =
            DynamicMap::<TestKey, TestValue>::new(sui::types::Address::from_static("0x74"), 1);
        let key = TestKey {
            name: "node".to_string(),
        };
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x75"));
        let json = json!({
            "id": field_ref.object_id(),
            "name": key,
            "value": {
                "next": null,
                "prev": null,
                "value": {
                    "value": 42
                }
            }
        });

        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(key.clone(), *field_ref.object_id())],
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_batch_get_objects()
            .times(1)
            .with(always())
            .returning(move |_| {
                let object = object_with_json(
                    field_ref.clone(),
                    sui::types::Owner::Shared(1),
                    json.clone(),
                );
                let mut result = sui::grpc::GetObjectResult::default();
                result.set_object(object);
                let mut response = sui::grpc::BatchGetObjectsResponse::default();
                response.set_objects(vec![result]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let fields = crawler
            .get_dynamic_fields(&parent)
            .await
            .expect("linked-table Sui JSON dynamic field value decodes");

        assert_eq!(fields.get(&key), Some(&TestValue { value: 42 }));
    }

    #[tokio::test]
    async fn get_dynamic_field_values_bcs_pages_and_decodes_values() {
        let parent_id = sui::types::Address::from_static("0x70");
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        type DynamicFieldTestResponse = (Vec<(&'static str, TestValue)>, Option<Vec<u8>>);
        let responses: Vec<DynamicFieldTestResponse> = vec![
            (
                vec![("test::First", TestValue { value: 3 })],
                Some(Vec::from(&b"page-2"[..])),
            ),
            (vec![("test::Second", TestValue { value: 5 })], None),
        ];
        let mut responses = responses.into_iter();
        state_service_mock
            .expect_list_dynamic_fields()
            .times(2)
            .with(always())
            .returning(move |_request| {
                let (fields, next_page_token) = responses.next().expect("dynamic field page");
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                response.set_dynamic_fields(
                    fields
                        .into_iter()
                        .map(|(value_type, value)| {
                            let mut field = sui::grpc::DynamicField::default();
                            let mut bcs_value = sui::grpc::Bcs::default();
                            bcs_value.set_value(bcs::to_bytes(&value).expect("value bcs"));
                            field.set_value(bcs_value);
                            field.set_value_type(value_type);
                            field
                        })
                        .collect(),
                );
                response.next_page_token = next_page_token.map(Into::into);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let values = crawler
            .get_dynamic_field_values_bcs::<TestValue>(parent_id)
            .await
            .expect("dynamic field values load");

        assert_eq!(
            values,
            vec![
                (Some("test::First".to_string()), TestValue { value: 3 }),
                (Some("test::Second".to_string()), TestValue { value: 5 }),
            ]
        );
    }

    /// `get_object_creation_checkpoint` walks
    /// `current metadata → initial_shared_version → version-pinned previous_transaction → transaction checkpoint`.
    /// Verify the happy path returns the checkpoint mocked at the end of the
    /// chain and that the helper is wired against the right three gRPC calls.
    #[tokio::test]
    async fn get_object_creation_checkpoint_resolves_via_initial_shared_version() {
        let mut rng = rand::thread_rng();
        let object_id = sui::types::Address::generate(&mut rng);
        let object_ref =
            sui::types::ObjectReference::new(object_id, 42, sui::types::Digest::generate(&mut rng));
        let creation_tx_digest = sui::types::Digest::generate(&mut rng);
        let creation_checkpoint = 4096_u64;
        let initial_shared_version = 17_u64;

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_object_creation_checkpoint(
            &mut ledger_service_mock,
            object_ref,
            initial_shared_version,
            creation_tx_digest,
            creation_checkpoint,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let checkpoint = crawler
            .get_object_creation_checkpoint(object_id)
            .await
            .expect("creation checkpoint resolves");

        assert_eq!(checkpoint, creation_checkpoint);
    }

    /// Non-shared objects are out of scope: their creation version is not
    /// recoverable via `Owner::Shared(initial_shared_version)`. The helper
    /// must reject them with a clear error before issuing the
    /// version-pinned fetch.
    #[tokio::test]
    async fn get_object_creation_checkpoint_rejects_owned_objects() {
        let mut rng = rand::thread_rng();
        let object_id = sui::types::Address::generate(&mut rng);
        let owner_address = sui::types::Address::generate(&mut rng);
        let object_ref =
            sui::types::ObjectReference::new(object_id, 5, sui::types::Digest::generate(&mut rng));

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_metadata(
            &mut ledger_service_mock,
            object_ref,
            sui::types::Owner::Address(owner_address),
            None,
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let error = crawler
            .get_object_creation_checkpoint(object_id)
            .await
            .expect_err("owned object must not derive a creation checkpoint");

        assert!(
            error.to_string().contains("not shared"),
            "unexpected error: {error}"
        );
    }
}
