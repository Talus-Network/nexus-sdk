//! Module defining a Sui object crawler - this struct is able to fetch object
//! and dynamic field data from Sui GRPC and deserialize them into Rust structs.

use {
    crate::{
        move_bindings::sui_framework::table_vec::TableVec,
        sui::{self, traits::FieldMaskUtil},
    },
    anyhow::{anyhow, bail, Context as _},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize},
    std::{
        collections::{HashMap, HashSet},
        hash::Hash,
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

#[derive(Debug, Deserialize, Serialize)]
struct DynamicFieldValue<K, V> {
    #[allow(unused)]
    id: sui::types::Address,
    #[allow(unused)]
    name: K,
    value: V,
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

#[derive(Clone, Debug)]
pub struct DynamicFieldReference<K> {
    pub name: K,
    pub field_id: sui::types::Address,
}

/// The on-chain reference that identifies the transaction which produced one
/// version of an object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectUpdateReference {
    pub owner: sui::types::Owner,
    pub object_type: sui::types::StructTag,
    pub version: sui::types::Version,
    pub digest: sui::types::Digest,
    pub previous_transaction: sui::types::Digest,
}

/// Effects and events fetched for one transaction that updated an object.
#[derive(Clone, Debug)]
pub struct TransactionUpdate {
    pub digest: sui::types::Digest,
    pub effects: sui::types::TransactionEffectsV2,
    pub events: Vec<sui::types::Event>,
}

impl Crawler {
    pub fn new(client: Arc<Mutex<sui::grpc::Client>>) -> Self {
        Self { client }
    }

    /// Fetch a published Move package descriptor for ABI inspection.
    pub async fn get_package(
        &self,
        package_id: sui::types::Address,
    ) -> anyhow::Result<sui::grpc::Package> {
        let request = sui::grpc::GetPackageRequest::default().with_package_id(package_id);
        self.client
            .lock()
            .await
            .package_client()
            .get_package(request)
            .await
            .map_err(|e| anyhow!("Could not fetch package '{package_id}': {e}"))?
            .into_inner()
            .package
            .ok_or_else(|| anyhow!("Package '{package_id}' was not returned"))
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

    /// Fetch an object by its ID and deserialize its Move struct contents from BCS.
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
            "contents",
        ]);

        self.get_object_parsed(object_id, field_mask, Self::parse_object_contents_bcs::<T>)
            .await
    }

    /// Fetch many objects by their IDs in batch and deserialize Move struct contents from BCS.
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

    /// Fetch the update reference for the latest or a historical object
    /// version. Unlike [`Self::get_object_metadata`], this includes the
    /// transaction digest that produced the requested version.
    pub async fn get_object_update_reference(
        &self,
        object_id: sui::types::Address,
        version: Option<sui::types::Version>,
    ) -> anyhow::Result<ObjectUpdateReference> {
        let mut request = sui::grpc::GetObjectRequest::default()
            .with_object_id(object_id)
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "object_id",
                "owner",
                "object_type",
                "version",
                "digest",
                "previous_transaction",
            ]));
        if let Some(version) = version {
            request = request.with_version(version);
        }

        let object = self
            .client
            .lock()
            .await
            .ledger_client()
            .get_object(request)
            .await
            .map(|response| response.into_inner().object)
            .with_context(|| {
                let version = version
                    .map(|version| format!(" at version {version}"))
                    .unwrap_or_default();
                format!("Could not fetch object '{object_id}'{version}")
            })?
            .ok_or_else(|| {
                let version = version
                    .map(|version| format!(" at version {version}"))
                    .unwrap_or_default();
                anyhow!("Object '{object_id}'{version} not found")
            })?;

        let (owner, digest, observed_version, _) =
            self.parse_object_metadata(object_id, &object)?;
        let object_type = object
            .object_type_opt()
            .ok_or_else(|| anyhow!("Object type missing for object '{object_id}'"))?
            .parse()
            .map_err(|e| anyhow!("Could not parse object type for object '{object_id}': {e}"))?;
        if version.is_some_and(|requested| requested != observed_version) {
            bail!(
                "Requested object '{object_id}' at version {}, received version {observed_version}",
                version.expect("checked as some")
            );
        }
        let previous_transaction = object
            .previous_transaction_opt()
            .ok_or_else(|| {
                anyhow!(
                    "Object '{object_id}' at version {observed_version} has no previous_transaction"
                )
            })?
            .parse()
            .map_err(|e| {
                anyhow!(
                    "Object '{object_id}' at version {observed_version} has an invalid previous_transaction: {e}"
                )
            })?;

        Ok(ObjectUpdateReference {
            owner,
            object_type,
            version: observed_version,
            digest,
            previous_transaction,
        })
    }

    /// Fetch and decode the effects and events for one transaction digest
    pub async fn get_transaction_update(
        &self,
        digest: sui::types::Digest,
    ) -> anyhow::Result<TransactionUpdate> {
        let request = sui::grpc::GetTransactionRequest::default()
            .with_digest(digest.to_string())
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "digest",
                "effects.bcs",
                "events.events",
            ]));
        let transaction = self
            .client
            .lock()
            .await
            .ledger_client()
            .get_transaction(request)
            .await
            .map(|response| response.into_inner().transaction)
            .with_context(|| format!("Could not fetch transaction '{digest}'"))?
            .ok_or_else(|| anyhow!("Transaction '{digest}' not found"))?;

        let observed_digest = transaction
            .digest_opt()
            .ok_or_else(|| anyhow!("Failed to get Executed Transaction for digest '{digest}'"))?
            .parse()
            .map_err(|e| anyhow!("Transaction '{digest}' response has an invalid digest: {e}"))?;
        if observed_digest != digest {
            bail!("Requested transaction '{digest}', received transaction '{observed_digest}'");
        }

        let effects = match sui::types::TransactionEffects::try_from(transaction.effects())
            .map_err(|e| anyhow!("Could not decode effects for transaction '{digest}': {e}"))?
        {
            sui::types::TransactionEffects::V2(effects) => *effects,
            sui::types::TransactionEffects::V1(_) => {
                bail!("Transaction '{digest}' returned unsupported V1 effects")
            }
        };
        if effects.transaction_digest != observed_digest {
            bail!(
                "Transaction '{observed_digest}' response contains effects for transaction '{}'; expected the effects transaction digest to match the requested transaction '{digest}'",
                effects.transaction_digest
            );
        }
        let events = sui::types::TransactionEvents::try_from(transaction.events())
            .map_err(|e| anyhow!("Could not decode events for transaction '{digest}': {e}"))?
            .0;

        Ok(TransactionUpdate {
            digest: observed_digest,
            effects,
            events,
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

    /// Fetch every coin owned by `owner` with the exact requested Move struct tag.
    ///
    /// The state service applies owner and type filters. Returned objects are validated again so
    /// callers never receive a reference whose address owner or type differs from the request.
    pub async fn fetch_coins_for_address_by_type(
        &self,
        owner: sui::types::Address,
        object_type: sui::types::StructTag,
    ) -> anyhow::Result<Vec<(sui::types::ObjectReference, u64)>> {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
            "object_type",
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
                .map(|response| response.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch coins of type '{object_type}' owned by '{owner}': {e}")
                })?;

            drop(client);
            page_token = response.next_page_token;
            results.extend(response.objects.iter().filter_map(|object| {
                Self::parse_owned_coin_with_type(object, owner, &object_type)
            }));

            if page_token.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Fetch owned objects of a specific type for an owner and deserialize BCS contents.
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
            "contents",
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
                let data = Self::parse_object_contents_bcs::<T>(self, &object)?;
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

    /// Fetch all dynamic fields for a given parent table object and parse them into a
    /// `HashMap<K, V>`.
    pub async fn get_dynamic_fields<K, V>(
        &self,
        parent_id: sui::types::Address,
        expected_size: usize,
    ) -> anyhow::Result<HashMap<K, V>>
    where
        K: Eq + Hash + DeserializeOwned,
        V: DeserializeOwned,
    {
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
            .get_objects::<DynamicFieldValue<K, V>>(&field_ids)
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

    /// Fetch dynamic field references whose BCS names decode as `K`.
    ///
    /// This is the right shape for heterogeneous dynamic field parents: inspect
    /// names first, then fetch the selected field object with the expected value type.
    pub async fn get_dynamic_field_refs_matching_key<K>(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<DynamicFieldReference<K>>>
    where
        K: Eq + Hash + DeserializeOwned,
    {
        Ok(self
            .fetch_dynamic_fields::<K>(parent_id, 0)
            .await?
            .into_iter()
            .filter_map(|(name, _child_id, field_id)| {
                Some(DynamicFieldReference {
                    name,
                    field_id: field_id?,
                })
            })
            .collect())
    }

    /// Fetch a dynamic field object by field ID and decode only its value.
    pub async fn get_dynamic_field_value_by_id<K, V>(
        &self,
        field_id: sui::types::Address,
    ) -> anyhow::Result<V>
    where
        K: DeserializeOwned,
        V: DeserializeOwned,
    {
        Ok(self
            .get_object::<DynamicFieldValue<K, V>>(field_id)
            .await?
            .data
            .value)
    }

    /// Fetch one dynamic field by BCS key, returning `Ok(None)` when that key is absent.
    ///
    /// Unlike [`Crawler::get_dynamic_fields`], this skips unrelated dynamic field key
    /// namespaces under the same parent. That is useful for Sui objects that store several
    /// unrelated dynamic field types directly under one UID.
    pub async fn get_optional_dynamic_field<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
    ) -> anyhow::Result<Option<V>>
    where
        K: Eq + DeserializeOwned,
        V: DeserializeOwned,
    {
        self.get_optional_dynamic_field_matching_value_type(parent_id, key, None)
            .await
    }

    /// Fetch one dynamic field by BCS key and optional value-type suffix.
    ///
    /// Sui dynamic fields can use keys with the same BCS shape under one parent. The value type
    /// lets callers disambiguate those namespaces before fetching and decoding the field object.
    pub async fn get_optional_dynamic_field_matching_value_type<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
        value_type_suffix: Option<&str>,
    ) -> anyhow::Result<Option<V>>
    where
        K: Eq + DeserializeOwned,
        V: DeserializeOwned,
    {
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
                if let (Some(expected_suffix), Some(value_type)) =
                    (value_type_suffix, field.value_type.as_deref())
                {
                    if !value_type.ends_with(expected_suffix) {
                        continue;
                    }
                }

                let Some(name) = field.name_opt() else {
                    continue;
                };
                let Ok(parsed_name) = parse_dynamic_field_name::<K>(name.value()) else {
                    continue;
                };
                if parsed_name != key {
                    continue;
                }

                let field_id = field
                    .field_id_opt()
                    .ok_or_else(|| anyhow!("Dynamic field ID missing for parent '{parent_id}'"))?
                    .parse()
                    .map_err(|_| anyhow!("Could not parse field ID for dynamic field"))?;

                let object = self.get_object::<DynamicFieldValue<K, V>>(field_id).await?;
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
    pub async fn get_dynamic_field_object_values<K, V>(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<V>>
    where
        K: DeserializeOwned,
        V: DeserializeOwned,
    {
        let names_and_ids = self.fetch_dynamic_fields_untyped(parent_id).await?;
        let mut field_ids = Vec::with_capacity(names_and_ids.len());
        for field_id in names_and_ids {
            field_ids.push(field_id);
        }

        Ok(self
            .get_objects::<DynamicFieldValue<K, V>>(&field_ids)
            .await?
            .into_iter()
            .map(|response| response.data.value)
            .collect())
    }

    /// Fetch all dynamic-field values for a parent object without attempting to decode keys.
    pub async fn get_dynamic_field_values<V>(
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

        let field_objects = self
            .get_objects::<DynamicFieldValue<u64, T>>(&field_ids)
            .await?;

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

            values_by_index[index] = Some(obj.data.value);
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

    fn parse_owned_coin_with_type(
        object: &sui::grpc::Object,
        expected_owner: sui::types::Address,
        expected_type: &sui::types::StructTag,
    ) -> Option<(sui::types::ObjectReference, u64)> {
        let owner: sui::types::Owner = object.owner_opt()?.try_into().ok()?;
        if owner != sui::types::Owner::Address(expected_owner) {
            return None;
        }

        let object_type = object
            .object_type_opt()?
            .parse::<sui::types::StructTag>()
            .ok()?;
        if object_type != *expected_type {
            return None;
        }

        Some((
            sui::types::ObjectReference::new(
                object.object_id_opt()?.parse().ok()?,
                object.version_opt()?,
                object.digest_opt()?.parse().ok()?,
            ),
            object.balance_opt().unwrap_or(0),
        ))
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

/// Internal wrapper around a key-value pair within a map-like structure.
#[derive(Clone, Debug, Deserialize)]
struct Pair<K, V> {
    #[serde(alias = "key", alias = "name")]
    key: K,
    value: V,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::test_utils::sui_mocks,
        mockall::predicate::always,
        serde::{Deserialize, Serialize},
    };

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestValue {
        value: u64,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    struct TestKey {
        name: String,
    }

    fn test_value_tag() -> sui::types::StructTag {
        sui::types::StructTag::new(
            sui::types::Address::from_static("0x1"),
            sui::types::Identifier::from_static("test"),
            sui::types::Identifier::from_static("TestValue"),
            vec![],
        )
    }

    fn object_with_bcs<T>(
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        value: &T,
    ) -> sui::grpc::Object
    where
        T: Serialize,
    {
        let mut object = sui::grpc::Object::default();
        object.set_object_id(object_ref.object_id().to_string());
        object.set_owner(sui::grpc::Owner::from(owner));
        object.set_digest(*object_ref.digest());
        object.set_version(object_ref.version());
        let mut contents = sui::grpc::Bcs::default();
        contents.set_value(bcs::to_bytes(value).expect("object value serializes"));
        object.contents = Some(contents);
        object
    }

    fn coin_object(
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        balance: u64,
        object_type: &sui::types::StructTag,
    ) -> sui::grpc::Object {
        let mut object = sui::grpc::Object::default();
        object.set_object_id(*object_ref.object_id());
        object.set_owner(sui::grpc::Owner::from(owner));
        object.set_digest(*object_ref.digest());
        object.set_version(object_ref.version());
        object.set_balance(balance);
        object.set_object_type(object_type.to_string());
        object
    }

    #[tokio::test]
    async fn fetch_coins_for_address_by_type_reads_every_page() {
        let owner = sui::types::Address::from_static("0xa");
        let object_type = sui::types::StructTag::gas_coin();
        let first_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x10"));
        let second_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x11"));
        let responses = vec![
            (
                vec![coin_object(
                    first_ref.clone(),
                    sui::types::Owner::Address(owner),
                    70,
                    &object_type,
                )],
                Some(Vec::from(&b"page-2"[..])),
            ),
            (
                vec![coin_object(
                    second_ref.clone(),
                    sui::types::Owner::Address(owner),
                    30,
                    &object_type,
                )],
                None,
            ),
        ];
        let mut responses = responses.into_iter();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(2)
            .with(always())
            .returning(move |_request| {
                let (objects, next_page_token) = responses.next().expect("typed coin page");
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

        let coins = crawler
            .fetch_coins_for_address_by_type(owner, object_type)
            .await
            .expect("typed coins load");

        assert_eq!(coins, vec![(first_ref, 70), (second_ref, 30)]);
    }

    #[tokio::test]
    async fn fetch_coins_for_address_by_type_excludes_wrong_owner_and_type() {
        let owner = sui::types::Address::from_static("0xa");
        let other_owner = sui::types::Address::from_static("0xb");
        let object_type = sui::types::StructTag::gas_coin();
        let valid_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x20"));
        let wrong_owner_ref =
            sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x21"));
        let wrong_type_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x22"));
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .with(always())
            .return_once({
                let valid_ref = valid_ref.clone();
                let object_type = object_type.clone();
                move |_request| {
                    let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                    response.set_objects(vec![
                        coin_object(
                            valid_ref,
                            sui::types::Owner::Address(owner),
                            50,
                            &object_type,
                        ),
                        coin_object(
                            wrong_owner_ref,
                            sui::types::Owner::Address(other_owner),
                            60,
                            &object_type,
                        ),
                        coin_object(
                            wrong_type_ref,
                            sui::types::Owner::Address(owner),
                            70,
                            &test_value_tag(),
                        ),
                    ]);
                    Ok(tonic::Response::new(response))
                }
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let coins = crawler
            .fetch_coins_for_address_by_type(owner, object_type)
            .await
            .expect("typed coins load");

        assert_eq!(coins, vec![(valid_ref, 50)]);
    }

    #[tokio::test]
    async fn get_owned_objects_pages_and_deserializes_bcs() {
        let owner = sui::types::Address::from_static("0xa");
        let first_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x10"));
        let second_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x11"));
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        let first_object = object_with_bcs(
            first_ref.clone(),
            sui::types::Owner::Address(owner),
            &TestValue { value: 7 },
        );
        let second_object = object_with_bcs(
            second_ref.clone(),
            sui::types::Owner::Address(owner),
            &TestValue { value: 9 },
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
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let objects = crawler
            .get_owned_objects::<TestValue>(owner, test_value_tag())
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
        sui_mocks::grpc::mock_get_objects_bcs(
            &mut ledger_service_mock,
            vec![(
                child_ref.clone(),
                sui::types::Owner::Shared(child_ref.version()),
                bcs::to_bytes(&TestValue { value: 11 }).expect("test value bcs"),
                test_value_tag(),
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let object = crawler
            .get_dynamic_object_field::<TestKey, TestValue>(parent_id, key)
            .await
            .expect("dynamic object field loads");

        assert_eq!(object.object_id, *child_ref.object_id());
        assert_eq!(object.data, TestValue { value: 11 });
    }

    #[tokio::test]
    async fn get_dynamic_fields_decodes_field_object_bcs() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key = TestKey {
            name: "wanted".to_string(),
        };
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x71"));
        let field = DynamicFieldValue {
            id: *field_ref.object_id(),
            name: key.clone(),
            value: TestValue { value: 42 },
        };
        let field_type = sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![],
        );

        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        sui_mocks::grpc::mock_list_dynamic_fields(
            &mut state_service_mock,
            vec![(key.clone(), *field_ref.object_id())],
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_objects_bcs(
            &mut ledger_service_mock,
            vec![(
                field_ref.clone(),
                sui::types::Owner::Shared(1),
                bcs::to_bytes(&field).expect("dynamic field bcs"),
                field_type,
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let fields = crawler
            .get_dynamic_fields::<TestKey, TestValue>(parent_id, 1)
            .await
            .expect("dynamic field value decodes");

        assert_eq!(fields.get(&key), Some(&TestValue { value: 42 }));
    }

    #[tokio::test]
    async fn get_dynamic_field_values_pages_and_decodes_values() {
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
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let values = crawler
            .get_dynamic_field_values::<TestValue>(parent_id)
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

    #[tokio::test]
    async fn get_transaction_update_rejects_mismatched_effects_digest() {
        let mut rng = rand::thread_rng();
        let requested_digest = sui::types::Digest::generate(&mut rng);
        let effects_digest = sui::types::Digest::generate(&mut rng);
        let effects =
            sui::types::TransactionEffects::V2(Box::new(sui::types::TransactionEffectsV2 {
                status: sui::types::ExecutionStatus::Success,
                epoch: 1,
                gas_used: sui::types::GasCostSummary {
                    computation_cost: 0,
                    storage_cost: 0,
                    storage_rebate: 0,
                    non_refundable_storage_fee: 0,
                },
                transaction_digest: effects_digest,
                gas_object_index: None,
                events_digest: None,
                dependencies: vec![],
                lamport_version: 1,
                changed_objects: vec![],
                unchanged_consensus_objects: vec![],
                auxiliary_data_digest: None,
            }));
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_get_transaction()
            .times(1)
            .returning(move |request| {
                assert_eq!(
                    request.get_ref().digest_opt(),
                    Some(requested_digest.to_string().as_str())
                );
                let mut grpc_effects = sui::grpc::TransactionEffects::default();
                grpc_effects.set_bcs(bcs::to_bytes(&effects).expect("effects serialize"));
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(requested_digest);
                transaction.set_effects(grpc_effects);
                let mut response = sui::grpc::GetTransactionResponse::default();
                response.set_transaction(transaction);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));

        let error = crawler
            .get_transaction_update(requested_digest)
            .await
            .expect_err("mismatched effects digest must fail permanently");
        let message = error.to_string();
        assert!(message.contains(&requested_digest.to_string()));
        assert!(message.contains(&effects_digest.to_string()));
        assert!(message.contains("effects transaction digest"));
    }
}
