//! Module defining a Sui object crawler - this struct is able to fetch object
//! and dynamic field data from Sui GRPC and deserialize them into Rust structs.

use {
    crate::{
        move_bindings::{
            sui_framework::{object::ID, table_vec::TableVec, versioned::Versioned},
            VersionedAnchor,
        },
        sui::{self, traits::FieldMaskUtil},
    },
    anyhow::{anyhow, bail, Context as _},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize},
    std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        sync::Arc,
    },
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

/// BCS representation of `sui::dynamic_object_field::Wrapper<K>`.
///
/// [`DynamicFieldValue`] cannot represent this name because ordinary dynamic
/// fields store `K` directly, while dynamic object fields store this wrapper.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct DynamicObjectFieldName<K> {
    name: K,
}

fn parse_dynamic_field_name<K>(bytes: &[u8]) -> Result<K, bcs::Error>
where
    K: DeserializeOwned,
{
    bcs::from_bytes::<K>(bytes)
        .or_else(|_| bcs::from_bytes::<DynamicFieldNameBcs<K>>(bytes).map(|field| field.name))
}

fn derive_dynamic_field_id<K>(
    parent_id: sui::types::Address,
    key: &K,
    key_type: &sui::types::TypeTag,
) -> anyhow::Result<sui::types::Address>
where
    K: Serialize,
{
    let key_bytes = bcs::to_bytes(key).context("Could not encode dynamic field key")?;
    Ok(parent_id.derive_dynamic_child_id(key_type, &key_bytes))
}

fn validate_dynamic_field<K, V>(
    field_id: sui::types::Address,
    key: &K,
    field: &DynamicFieldValue<K, V>,
) -> anyhow::Result<()>
where
    K: Eq,
{
    if field.id != field_id {
        bail!("Dynamic field '{field_id}' decoded with an unexpected embedded ID");
    }
    if &field.name != key {
        bail!("Dynamic field '{field_id}' decoded with an unexpected key");
    }
    Ok(())
}

fn dynamic_object_field_wrapper_type(key_type: &sui::types::TypeTag) -> sui::types::TypeTag {
    sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
        sui::types::Address::from_static("0x2"),
        sui::types::Identifier::from_static("dynamic_object_field"),
        sui::types::Identifier::from_static("Wrapper"),
        vec![key_type.clone()],
    )))
}

/// The main crawler struct.
#[derive(Clone)]
pub struct Crawler {
    client: Arc<sui::grpc::Client>,
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

/// One RPC page of typed dynamic field values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicFieldPage<K, V> {
    data: Vec<(K, V)>,
    next_cursor: Option<Vec<u8>>,
}

impl<K, V> DynamicFieldPage<K, V> {
    /// Returns the decoded values in RPC order.
    pub fn data(&self) -> &[(K, V)] {
        &self.data
    }

    /// Returns the opaque cursor for the next RPC page.
    pub fn next_cursor(&self) -> Option<&[u8]> {
        self.next_cursor.as_deref()
    }

    /// Separates the decoded values from the opaque next page cursor.
    pub fn into_parts(self) -> (Vec<(K, V)>, Option<Vec<u8>>) {
        (self.data, self.next_cursor)
    }
}

/// One RPC page of typed objects owned by one address or object.
#[derive(Clone, Debug)]
pub struct OwnedObjectPage<T> {
    data: Vec<Response<T>>,
    next_cursor: Option<Vec<u8>>,
}

fn is_owned_by_address(owner: &sui::types::Owner, address: sui::types::Address) -> bool {
    match owner {
        sui::types::Owner::Address(owner) | sui::types::Owner::ConsensusAddress { owner, .. } => {
            *owner == address
        }
        _ => false,
    }
}

fn matches_expected_owner(observed: &sui::types::Owner, expected: &sui::types::Owner) -> bool {
    match expected {
        sui::types::Owner::Address(address) => is_owned_by_address(observed, *address),
        _ => observed == expected,
    }
}

impl<T> OwnedObjectPage<T> {
    /// Returns the decoded objects in RPC order.
    pub fn data(&self) -> &[Response<T>] {
        &self.data
    }

    /// Returns the opaque cursor for the next RPC page.
    pub fn next_cursor(&self) -> Option<&[u8]> {
        self.next_cursor.as_deref()
    }

    /// Separates the decoded objects from the opaque next page cursor.
    pub fn into_parts(self) -> (Vec<Response<T>>, Option<Vec<u8>>) {
        (self.data, self.next_cursor)
    }
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
    pub checkpoint: u64,
    pub effects: sui::types::TransactionEffectsV2,
    pub events: Vec<sui::types::Event>,
}

impl Crawler {
    pub fn new(client: Arc<sui::grpc::Client>) -> Self {
        Self { client }
    }

    pub(crate) fn grpc_client(&self) -> Arc<sui::grpc::Client> {
        Arc::clone(&self.client)
    }

    pub(crate) fn clone_grpc_client(&self) -> sui::grpc::Client {
        self.client.as_ref().clone()
    }

    /// Fetch a published Move package descriptor for ABI inspection.
    pub async fn get_package(
        &self,
        package_id: sui::types::Address,
    ) -> anyhow::Result<sui::grpc::Package> {
        let request = sui::grpc::GetPackageRequest::default().with_package_id(package_id);
        let mut client = self.clone_grpc_client();
        client
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

    async fn get_optional_objects_parsed<T>(
        &self,
        object_ids: &[sui::types::Address],
        field_mask: sui::grpc::FieldMask,
        parse_data: fn(&Crawler, &sui::grpc::Object) -> anyhow::Result<T>,
    ) -> anyhow::Result<Vec<Option<Response<T>>>>
    where
        T: DeserializeOwned,
    {
        let results = self.fetch_object_results(object_ids, field_mask).await?;
        if results.len() != object_ids.len() {
            bail!(
                "Batch object response contained {} results for {} requests",
                results.len(),
                object_ids.len()
            );
        }

        object_ids
            .iter()
            .copied()
            .zip(results)
            .map(|(requested_id, result)| {
                let object = match result.to_result() {
                    Ok(object) => object,
                    Err(status) if status.code == i32::from(tonic::Code::NotFound) => {
                        return Ok(None);
                    }
                    Err(status) => {
                        bail!(
                            "Could not fetch object '{requested_id}': {}",
                            status.message
                        );
                    }
                };
                let object_id = Self::parse_object_id(&object)?;
                if object_id != requested_id {
                    bail!("Requested object '{requested_id}', received object '{object_id}'");
                }
                let (owner, digest, version, balance) =
                    self.parse_object_metadata(object_id, &object)?;
                let data = parse_data(self, &object)?;
                Ok(Some(Response {
                    object_id,
                    owner,
                    version,
                    data,
                    digest,
                    balance,
                }))
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

    /// Fetch an object when present.
    ///
    /// # Errors
    ///
    /// Returns an error for transport and decoding failures. A missing object
    /// returns `Ok(None)`.
    pub async fn get_optional_object<T>(
        &self,
        object_id: sui::types::Address,
    ) -> anyhow::Result<Option<Response<T>>>
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
        let Some(object) = self.fetch_optional_object(object_id, field_mask).await? else {
            return Ok(None);
        };
        let returned_id = Self::parse_object_id(&object)?;
        if returned_id != object_id {
            bail!("Requested object '{object_id}', received object '{returned_id}'");
        }
        let (owner, digest, version, balance) = self.parse_object_metadata(object_id, &object)?;
        let data = self.parse_object_contents_bcs::<T>(&object)?;
        Ok(Some(Response {
            object_id,
            owner,
            version,
            data,
            digest,
            balance,
        }))
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

    /// Fetches many objects that may no longer exist.
    ///
    /// Each result corresponds to the identifier at the same position in
    /// `object_ids`. A missing object produces [`None`].
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC response is malformed, a present object
    /// has the wrong identity, or transport or decoding fails.
    pub async fn get_optional_objects<T>(
        &self,
        object_ids: &[sui::types::Address],
    ) -> anyhow::Result<Vec<Option<Response<T>>>>
    where
        T: DeserializeOwned,
    {
        if object_ids.is_empty() {
            return Ok(Vec::new());
        }

        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "version",
            "digest",
            "balance",
            "contents",
        ]);

        self.get_optional_objects_parsed(
            object_ids,
            field_mask,
            Self::parse_object_contents_bcs::<T>,
        )
        .await
    }

    /// Fetch the connected RPC's chain identifier in the 8-hex-char form
    /// Sui's Move builder uses for `[environments]` lookups (the same value
    /// `sui client chain-identifier` prints). The gRPC service-info call
    /// returns the genesis checkpoint digest base58-encoded; we decode it
    /// and hex-encode the first four bytes to derive the short identifier.
    pub async fn get_chain_id(&self) -> anyhow::Result<String> {
        let mut client = self.clone_grpc_client();
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

        let mut client = self.clone_grpc_client();
        let object = client
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
                "checkpoint",
                "effects.bcs",
                "events.events",
            ]));
        let mut client = self.clone_grpc_client();
        let transaction = client
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
        let checkpoint = transaction
            .checkpoint_opt()
            .ok_or_else(|| anyhow!("Transaction '{digest}' response has no checkpoint"))?;

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
            checkpoint,
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
        let mut client = self.clone_grpc_client();

        loop {
            let mut request = sui::grpc::ListOwnedObjectsRequest::default()
                .with_owner(owner)
                .with_page_size(1000)
                .with_object_type(object_type.clone())
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let response = client
                .state_client()
                .list_owned_objects(request)
                .await
                .map(|response| response.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch coins of type '{object_type}' owned by '{owner}': {e}")
                })?;

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
        let mut results = Vec::new();
        let mut cursor = None;

        loop {
            let page = self
                .get_owned_object_page(owner, object_type.clone(), cursor, 1000)
                .await?;
            let (data, next_cursor) = page.into_parts();
            results.extend(data);
            cursor = next_cursor;
            if cursor.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Fetch exact-type objects owned by another object and deserialize their BCS contents.
    pub async fn get_object_owned_objects<T>(
        &self,
        parent_id: sui::types::Address,
        object_type: sui::types::StructTag,
    ) -> anyhow::Result<Vec<Response<T>>>
    where
        T: DeserializeOwned,
    {
        let mut results = Vec::new();
        let mut cursor = None;

        loop {
            let page = self
                .get_object_owned_object_page(parent_id, object_type.clone(), cursor, 1000)
                .await?;
            let (data, next_cursor) = page.into_parts();
            results.extend(data);
            cursor = next_cursor;
            if cursor.is_none() {
                break;
            }
        }

        Ok(results)
    }

    /// Fetch one RPC page of objects owned by an address with an exact type.
    ///
    /// The address owner and type of every returned object are validated after
    /// the state service applies the same filters.
    ///
    /// # Errors
    ///
    /// Returns an error when `limit` is zero, the RPC request fails, the
    /// response violates its owner or type filter, or an object cannot be
    /// decoded.
    pub async fn get_owned_object_page<T>(
        &self,
        owner: sui::types::Address,
        object_type: sui::types::StructTag,
        cursor: Option<Vec<u8>>,
        limit: usize,
    ) -> anyhow::Result<OwnedObjectPage<T>>
    where
        T: DeserializeOwned,
    {
        self.get_typed_owned_object_page(
            sui::types::Owner::Address(owner),
            object_type,
            cursor,
            limit,
        )
        .await
    }

    /// Fetch one RPC page of exact-type objects owned by another object.
    ///
    /// Unlike [`Crawler::get_owned_object_page`], this requires every returned object to have the
    /// exact `Owner::Object(parent_id)` owner. Address and consensus-address ownership are rejected.
    pub async fn get_object_owned_object_page<T>(
        &self,
        parent_id: sui::types::Address,
        object_type: sui::types::StructTag,
        cursor: Option<Vec<u8>>,
        limit: usize,
    ) -> anyhow::Result<OwnedObjectPage<T>>
    where
        T: DeserializeOwned,
    {
        self.get_typed_owned_object_page(
            sui::types::Owner::Object(parent_id),
            object_type,
            cursor,
            limit,
        )
        .await
    }

    async fn get_typed_owned_object_page<T>(
        &self,
        expected_owner: sui::types::Owner,
        object_type: sui::types::StructTag,
        cursor: Option<Vec<u8>>,
        limit: usize,
    ) -> anyhow::Result<OwnedObjectPage<T>>
    where
        T: DeserializeOwned,
    {
        let request_owner = match &expected_owner {
            sui::types::Owner::Address(owner) | sui::types::Owner::Object(owner) => *owner,
            _ => bail!("typed owned-object reads require an address or object owner"),
        };
        let page_size =
            u32::try_from(limit).context("owned object page limit does not fit in u32")?;
        if page_size == 0 {
            bail!("owned object page limit must be greater than zero");
        }

        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "object_type",
            "version",
            "digest",
            "balance",
            "contents",
        ]);
        let mut request = sui::grpc::ListOwnedObjectsRequest::default()
            .with_owner(request_owner)
            .with_page_size(page_size)
            .with_object_type(object_type.clone())
            .with_read_mask(field_mask);
        if let Some(cursor) = cursor {
            request = request.with_page_token(cursor);
        }

        let mut client = self.clone_grpc_client();
        let response = client
            .state_client()
            .list_owned_objects(request)
            .await
            .map(|response| response.into_inner())
            .map_err(|error| {
                anyhow!(
                    "Could not fetch objects of type '{object_type}' owned by \
                     '{expected_owner:?}': {error}"
                )
            })?;

        let mut data = Vec::with_capacity(response.objects.len());
        for object in response.objects {
            let object_id = Self::parse_object_id(&object)?;
            let (observed_owner, digest, version, balance) =
                self.parse_object_metadata(object_id, &object)?;
            if !matches_expected_owner(&observed_owner, &expected_owner) {
                bail!(
                    "Object '{object_id}' has owner '{observed_owner:?}', expected \
                     '{expected_owner:?}'"
                );
            }
            let observed_type = object
                .object_type_opt()
                .ok_or_else(|| anyhow!("Object type missing for object '{object_id}'"))?
                .parse::<sui::types::StructTag>()
                .map_err(|error| {
                    anyhow!("Could not parse object type for object '{object_id}': {error}")
                })?;
            if observed_type != object_type {
                bail!("Object '{object_id}' has type '{observed_type}', expected '{object_type}'");
            }
            let decoded = Self::parse_object_contents_bcs::<T>(self, &object)?;
            data.push(Response {
                object_id,
                owner: observed_owner,
                version,
                data: decoded,
                digest,
                balance,
            });
        }

        Ok(OwnedObjectPage {
            data,
            next_cursor: response.next_page_token.map(|cursor| cursor.to_vec()),
        })
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

    /// Fetch the dynamic field selected by one typed key.
    ///
    /// The field object identifier is derived from `parent_id`, `key`, and
    /// `key_type`, so sibling fields are not enumerated. `key_type` must be the
    /// canonical Move [`sui::types::TypeTag`] for `K`. An absent field returns
    /// [`None`].
    ///
    /// # Errors
    ///
    /// Returns an error when the key cannot be encoded, the object request
    /// fails, or the field identity, key, or value is invalid.
    pub async fn get_dynamic_field_by_key<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
        key_type: &sui::types::TypeTag,
    ) -> anyhow::Result<Option<V>>
    where
        K: Eq + Serialize + DeserializeOwned,
        V: DeserializeOwned,
    {
        let field_id = derive_dynamic_field_id(parent_id, &key, key_type)?;
        let Some(field) = self
            .get_optional_object::<DynamicFieldValue<K, V>>(field_id)
            .await?
        else {
            return Ok(None);
        };
        validate_dynamic_field(field_id, &key, &field.data)?;
        Ok(Some(field.data.value))
    }

    /// Fetch values for the distinct dynamic field keys requested from one parent.
    ///
    /// Field identities are derived from `parent_id`, `key_type`, and each BCS
    /// encoded key. Unlike [`Crawler::get_dynamic_fields`], the amount of work
    /// is independent of unrelated fields stored under the parent.
    ///
    /// Missing fields are omitted from the returned [`HashMap`]. Duplicate keys
    /// produce one object request.
    ///
    /// # Errors
    ///
    /// Returns an error when a key cannot be encoded, an RPC response has an
    /// unexpected identity, or a present field cannot be decoded and validated.
    pub async fn get_dynamic_fields_by_keys<K, V, I>(
        &self,
        parent_id: sui::types::Address,
        keys: I,
        key_type: &sui::types::TypeTag,
    ) -> anyhow::Result<HashMap<K, V>>
    where
        K: Eq + Hash + Serialize + DeserializeOwned,
        V: DeserializeOwned,
        I: IntoIterator<Item = K>,
    {
        let mut index_by_field_id = HashMap::<sui::types::Address, usize>::new();
        let mut requests = Vec::<(sui::types::Address, K)>::new();

        for key in keys {
            let field_id = derive_dynamic_field_id(parent_id, &key, key_type)?;
            if let Some(index) = index_by_field_id.get(&field_id).copied() {
                if requests[index].1 != key {
                    bail!("Distinct dynamic field keys derived the same object ID '{field_id}'");
                }
                continue;
            }
            index_by_field_id.insert(field_id, requests.len());
            requests.push((field_id, key));
        }

        let field_ids = requests
            .iter()
            .map(|(field_id, _)| *field_id)
            .collect::<Vec<_>>();
        let responses = self
            .get_optional_objects::<DynamicFieldValue<K, V>>(&field_ids)
            .await?;
        let mut values = HashMap::with_capacity(responses.len());

        for ((field_id, key), response) in requests.into_iter().zip(responses) {
            let Some(response) = response else {
                continue;
            };
            validate_dynamic_field(field_id, &key, &response.data)?;
            values.insert(key, response.data.value);
        }

        Ok(values)
    }

    /// Fetch the payload selected by a [`Versioned`] container.
    ///
    /// The Sui framework stores each payload as a `u64` keyed dynamic field
    /// below the container UID. The container version is therefore both the
    /// schema discriminator and the dynamic field key.
    ///
    /// # Errors
    ///
    /// Returns an error when the schema is not the expected schema, when the
    /// field cannot be fetched or decoded, or when the container points at a
    /// payload that does not exist.
    pub async fn get_versioned_state<V>(
        &self,
        state: &Versioned,
        expected_schema: u64,
    ) -> anyhow::Result<V>
    where
        V: DeserializeOwned,
    {
        let parent_id = state.id.id.bytes;
        if state.version != expected_schema {
            bail!(
                "Versioned container '{parent_id}' uses unsupported state schema '{}'; expected \
                 state schema '{expected_schema}'",
                state.version,
            );
        }
        self.get_dynamic_field_by_key(parent_id, state.version, &sui::types::TypeTag::U64)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Versioned container '{parent_id}' has no payload for schema version '{}'",
                    state.version
                )
            })
    }

    /// Fetch a stable object anchor and decode its selected payload.
    ///
    /// The returned [`Response`] keeps the anchor object metadata while its
    /// `data` contains the current payload. This is the normal read path for
    /// Nexus objects backed by `sui::versioned`.
    ///
    /// # Errors
    ///
    /// Returns an error when the anchor or payload cannot be decoded, when the
    /// payload is absent, or when the embedded anchor ID differs from the
    /// requested object ID.
    pub async fn get_versioned_object<A, V>(
        &self,
        object_id: sui::types::Address,
        expected_schema: u64,
    ) -> anyhow::Result<Response<V>>
    where
        A: DeserializeOwned + VersionedAnchor,
        V: DeserializeOwned,
    {
        let anchor = self.get_object::<A>(object_id).await?;
        self.load_versioned_payload(anchor, expected_schema).await
    }

    /// Replace a fetched anchor value with its selected versioned payload.
    ///
    /// This form avoids fetching an anchor twice when it was discovered
    /// through a dynamic object field.
    pub async fn load_versioned_payload<A, V>(
        &self,
        anchor: Response<A>,
        expected_schema: u64,
    ) -> anyhow::Result<Response<V>>
    where
        A: VersionedAnchor,
        V: DeserializeOwned,
    {
        let object_id = anchor.object_id;
        let embedded_id = anchor.data.object_id();
        if embedded_id != object_id {
            bail!("Versioned anchor '{object_id}' contains embedded object ID '{embedded_id}'");
        }
        let data = self
            .get_versioned_state::<V>(anchor.data.versioned_state(), expected_schema)
            .await?;

        Ok(Response {
            object_id: anchor.object_id,
            owner: anchor.owner,
            version: anchor.version,
            data,
            digest: anchor.digest,
            balance: anchor.balance,
        })
    }

    /// Fetch one RPC page of dynamic fields matching a key and value type.
    ///
    /// Fields from other namespaces are skipped. The returned cursor is the
    /// unmodified cursor supplied by Sui for the requested page.
    ///
    /// # Errors
    ///
    /// Returns an error when `limit` is zero, the RPC request fails, or a
    /// matching field object cannot be decoded.
    pub async fn get_dynamic_field_page_matching_types<K, V>(
        &self,
        parent_id: sui::types::Address,
        cursor: Option<Vec<u8>>,
        limit: usize,
        value_type_suffix: &str,
    ) -> anyhow::Result<DynamicFieldPage<K, V>>
    where
        K: DeserializeOwned,
        V: DeserializeOwned,
    {
        let page_size =
            u32::try_from(limit).context("dynamic field page limit does not fit in u32")?;
        if page_size == 0 {
            bail!("dynamic field page limit must be greater than zero");
        }

        let field_mask = sui::grpc::FieldMask::from_paths(["name", "field_id", "value_type"]);
        let mut request = sui::grpc::ListDynamicFieldsRequest::default()
            .with_parent(parent_id)
            .with_page_size(page_size)
            .with_read_mask(field_mask);
        if let Some(cursor) = cursor {
            request = request.with_page_token(cursor);
        }

        let mut client = self.clone_grpc_client();
        let response = client
            .state_client()
            .list_dynamic_fields(request)
            .await
            .map(|response| response.into_inner())
            .map_err(|error| {
                anyhow!("Could not fetch dynamic fields for parent '{parent_id}': {error}")
            })?;

        let next_cursor = response.next_page_token.map(|cursor| cursor.to_vec());
        let mut field_ids = Vec::new();
        for field in response.dynamic_fields {
            if !field
                .value_type
                .as_deref()
                .is_some_and(|value_type| value_type.ends_with(value_type_suffix))
            {
                continue;
            }
            let Some(name) = field.name_opt() else {
                continue;
            };
            if parse_dynamic_field_name::<K>(name.value()).is_err() {
                continue;
            }
            let field_id = field
                .field_id_opt()
                .ok_or_else(|| anyhow!("Dynamic field ID missing for parent '{parent_id}'"))?
                .parse()
                .map_err(|_| anyhow!("Could not parse field ID for dynamic field"))?;
            field_ids.push(field_id);
        }

        let data = if field_ids.is_empty() {
            Vec::new()
        } else {
            self.get_objects::<DynamicFieldValue<K, V>>(&field_ids)
                .await?
                .into_iter()
                .map(|field| (field.data.name, field.data.value))
                .collect()
        };

        Ok(DynamicFieldPage { data, next_cursor })
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
        let mut client = self.clone_grpc_client();

        loop {
            let mut request = sui::grpc::ListDynamicFieldsRequest::default()
                .with_parent(parent_id)
                .with_page_size(1000)
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let response = client
                .state_client()
                .list_dynamic_fields(request)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch dynamic fields for parent '{parent_id}': {e}")
                })?;

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

    /// Fetch the dynamic object field selected by one typed key.
    ///
    /// The method derives and reads the Sui wrapper field, then fetches only
    /// the child object named by that wrapper. `key_type` must be the canonical
    /// Move [`sui::types::TypeTag`] for `K`. Unlike
    /// [`Crawler::get_dynamic_object_fields`], it does not enumerate sibling
    /// fields. A missing wrapper returns `Ok(None)`.
    ///
    /// # Errors
    ///
    /// Returns an error when the wrapper key cannot be encoded, either object
    /// request fails, the wrapper or child identity is invalid, the child is
    /// absent, or a returned value cannot be decoded and validated.
    pub async fn get_dynamic_object_field_by_key<K, V>(
        &self,
        parent_id: sui::types::Address,
        key: K,
        key_type: &sui::types::TypeTag,
    ) -> anyhow::Result<Option<Response<V>>>
    where
        K: Eq + Serialize + DeserializeOwned,
        V: DeserializeOwned,
    {
        let wrapper_key = DynamicObjectFieldName { name: key };
        let wrapper_type = dynamic_object_field_wrapper_type(key_type);
        let Some(child_id) = self
            .get_dynamic_field_by_key::<DynamicObjectFieldName<K>, ID>(
                parent_id,
                wrapper_key,
                &wrapper_type,
            )
            .await?
        else {
            return Ok(None);
        };

        let child_id = child_id.bytes;
        self.get_optional_object(child_id)
            .await?
            .map(Some)
            .ok_or_else(|| anyhow!("Dynamic object field child '{child_id}' not found"))
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
        let mut client = self.clone_grpc_client();

        loop {
            let mut request = sui::grpc::ListDynamicFieldsRequest::default()
                .with_parent(parent_id)
                .with_page_size(1000)
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let response = client
                .state_client()
                .list_dynamic_fields(request)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch dynamic fields for parent '{parent_id}': {e}")
                })?;

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

    /// Fetch every item in a [`TableVec`] by its contiguous index key.
    ///
    /// The request derives the fields for `0..parent.size_u64()` directly and
    /// returns values in index order.
    ///
    /// # Errors
    ///
    /// Returns an error when an index field cannot be fetched, validated, or
    /// decoded, or when any expected index is absent.
    pub async fn get_table_vec<T>(&self, parent: &TableVec<T>) -> anyhow::Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let indexes = 0..parent.size_u64();
        let mut values = self
            .get_dynamic_fields_by_keys(parent.id(), indexes.clone(), &sui::types::TypeTag::U64)
            .await?;

        indexes
            .map(|index| {
                values
                    .remove(&index)
                    .ok_or_else(|| anyhow!("Missing TableVec element {index}"))
            })
            .collect()
    }

    /// Helper function to fetch an object based on its ID and field mask.
    async fn fetch_object(
        &self,
        object_id: sui::types::Address,
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<sui::grpc::Object> {
        self.fetch_optional_object(object_id, field_mask)
            .await?
            .ok_or_else(|| anyhow!("Object '{object_id}' not found"))
    }

    async fn fetch_optional_object(
        &self,
        object_id: sui::types::Address,
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<Option<sui::grpc::Object>> {
        let mut client = self.clone_grpc_client();

        let request = sui::grpc::GetObjectRequest::default()
            .with_object_id(object_id)
            .with_read_mask(field_mask);

        match client.ledger_client().get_object(request).await {
            Ok(response) => Ok(response.into_inner().object),
            Err(status) if status.code() == tonic::Code::NotFound => Ok(None),
            Err(status) => Err(anyhow::Error::new(status))
                .with_context(|| format!("Could not fetch object '{object_id}'")),
        }
    }

    /// Helper function to fetch many objects based on their IDs and field mask.
    async fn fetch_objects(
        &self,
        object_ids: &[sui::types::Address],
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<Vec<sui::grpc::Object>> {
        let results = self.fetch_object_results(object_ids, field_mask).await?;

        object_ids
            .iter()
            .copied()
            .zip(results)
            .map(|(object_id, result)| {
                result.to_result().map_err(|status| {
                    anyhow!("Could not fetch object '{object_id}': {}", status.message)
                })
            })
            .collect()
    }

    async fn fetch_object_results(
        &self,
        object_ids: &[sui::types::Address],
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<Vec<sui::grpc::GetObjectResult>> {
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

        let mut client = self.clone_grpc_client();

        let response = client
            .ledger_client()
            .batch_get_objects(request)
            .await
            .map(|r| r.into_inner())
            .map_err(|e| anyhow!("Could not fetch objects: {e}"))?;

        Ok(response.objects)
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
        let mut client = self.clone_grpc_client();

        loop {
            let mut request = sui::grpc::ListDynamicFieldsRequest::default()
                .with_parent(parent_id)
                .with_page_size(1000)
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let response = client
                .state_client()
                .list_dynamic_fields(request)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch dynamic fields for parent '{parent_id}': {e}")
                })?;

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
        let mut client = self.clone_grpc_client();

        loop {
            let mut request = sui::grpc::ListDynamicFieldsRequest::default()
                .with_parent(parent_id)
                .with_page_size(1000)
                .with_read_mask(field_mask.clone());

            if let Some(token) = page_token.clone() {
                request = request.with_page_token(token);
            }

            let response = client
                .state_client()
                .list_dynamic_fields(request)
                .await
                .map(|r| r.into_inner())
                .map_err(|e| {
                    anyhow!("Could not fetch dynamic fields for parent '{parent_id}': {e}")
                })?;

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
        matches!(
            self.owner,
            sui::types::Owner::Shared(_) | sui::types::Owner::ConsensusAddress { .. }
        )
    }

    /// Check if the object is immutable.
    pub fn is_immutable(&self) -> bool {
        matches!(self.owner, sui::types::Owner::Immutable)
    }

    /// Get initial version of the object if it's shared or current version
    /// otherwise.
    pub fn get_initial_version(&self) -> sui::types::Version {
        match self.owner {
            sui::types::Owner::Shared(version) => version,
            sui::types::Owner::ConsensusAddress { start_version, .. } => start_version,
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
        std::sync::{Arc, Barrier},
    };

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestValue {
        value: u64,
    }

    #[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
    struct TestKey {
        name: String,
    }

    #[derive(Serialize)]
    struct TestDynamicObjectFieldName<K> {
        name: K,
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

    fn typed_object_with_bcs<T>(
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Owner,
        object_type: &sui::types::StructTag,
        value: &T,
    ) -> sui::grpc::Object
    where
        T: Serialize,
    {
        let mut object = object_with_bcs(object_ref, owner, value);
        object.set_object_type(object_type.to_string());
        object
    }

    #[tokio::test]
    async fn versioned_state_rejects_unknown_schema_before_rpc() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));
        let state_id = sui::types::Address::from_static("0x91");
        let state = Versioned::new(
            crate::move_bindings::sui_framework::object::UID::new(state_id),
            2,
        );

        let error = crawler
            .get_versioned_state::<TestValue>(&state, 1)
            .await
            .expect_err("unknown schema must be rejected");

        assert_eq!(
            error.to_string(),
            format!(
                "Versioned container '{state_id}' uses unsupported state schema '2'; expected \
                 state schema '1'"
            )
        );
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn inner_client_clones_allow_independent_requests_to_progress_concurrently() {
        let owner = sui::types::Address::from_static("0xa");
        let object_type = sui::types::StructTag::gas_coin();
        let metadata_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x10"));
        let coin_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x11"));
        let request_barrier = Arc::new(Barrier::new(2));
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_get_object()
            .times(1)
            .return_once({
                let request_barrier = Arc::clone(&request_barrier);
                let metadata_ref = metadata_ref.clone();
                move |_request| {
                    request_barrier.wait();
                    let object = object_with_bcs(
                        metadata_ref,
                        sui::types::Owner::Immutable,
                        &TestValue { value: 1 },
                    );
                    let mut response = sui::grpc::GetObjectResponse::default();
                    response.set_object(object);
                    Ok(tonic::Response::new(response))
                }
            });
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once({
                let request_barrier = Arc::clone(&request_barrier);
                let coin_ref = coin_ref.clone();
                let object_type = object_type.clone();
                move |_request| {
                    request_barrier.wait();
                    let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                    response.set_objects(vec![coin_object(
                        coin_ref,
                        sui::types::Owner::Address(owner),
                        50,
                        &object_type,
                    )]);
                    Ok(tonic::Response::new(response))
                }
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let (metadata, coins) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            tokio::join!(
                crawler.get_object_metadata(*metadata_ref.object_id()),
                crawler.fetch_coins_for_address_by_type(owner, object_type),
            )
        })
        .await
        .expect("independent RPCs should not wait on a shared client mutex");

        assert_eq!(
            metadata
                .expect("metadata request should succeed")
                .object_ref(),
            metadata_ref
        );
        assert_eq!(
            coins.expect("coin request should succeed"),
            vec![(coin_ref, 50)]
        );
    }

    #[tokio::test]
    async fn optional_object_batch_preserves_missing_positions() {
        let first_id = sui::types::Address::from_static("0x71");
        let missing_id = sui::types::Address::from_static("0x72");
        let last_id = sui::types::Address::from_static("0x73");
        let requested_ids = [first_id, missing_id, last_id];
        let first_ref = sui_mocks::object_ref_for_id(first_id);
        let last_ref = sui_mocks::object_ref_for_id(last_id);
        let expected_ids = requested_ids.map(|id| id.to_string());
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |request| {
                let actual_ids = request
                    .get_ref()
                    .requests
                    .iter()
                    .map(|request| request.object_id.clone().expect("object ID"))
                    .collect::<Vec<_>>();
                assert_eq!(actual_ids, expected_ids);

                let missing = sui_rpc::proto::google::rpc::Status {
                    code: tonic::Code::NotFound.into(),
                    message: "object not found".to_owned(),
                    ..Default::default()
                };
                let response = sui::grpc::BatchGetObjectsResponse::new(vec![
                    sui::grpc::GetObjectResult::new_object(object_with_bcs(
                        first_ref,
                        sui::types::Owner::Shared(1),
                        &TestValue { value: 3 },
                    )),
                    sui::grpc::GetObjectResult::new_error(missing),
                    sui::grpc::GetObjectResult::new_object(object_with_bcs(
                        last_ref,
                        sui::types::Owner::Shared(1),
                        &TestValue { value: 5 },
                    )),
                ]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));

        let objects = crawler
            .get_optional_objects::<TestValue>(&requested_ids)
            .await
            .expect("optional objects load");

        assert_eq!(
            objects[0].as_ref().map(|object| &object.data),
            Some(&TestValue { value: 3 })
        );
        assert!(objects[1].is_none());
        assert_eq!(
            objects[2].as_ref().map(|object| &object.data),
            Some(&TestValue { value: 5 })
        );
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
        let crawler = Crawler::new(Arc::new(client));

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
        let crawler = Crawler::new(Arc::new(client));

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
        let first_object = typed_object_with_bcs(
            first_ref.clone(),
            sui::types::Owner::Address(owner),
            &test_value_tag(),
            &TestValue { value: 7 },
        );
        let second_object = typed_object_with_bcs(
            second_ref.clone(),
            sui::types::Owner::Address(owner),
            &test_value_tag(),
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
        let crawler = Crawler::new(Arc::new(client));

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
    async fn owned_object_page_preserves_filters_and_both_cursors() {
        let owner = sui::types::Address::from_static("0xa");
        let object_type = test_value_tag();
        let object_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x10"));
        let request_cursor = Vec::from(&b"request-cursor"[..]);
        let response_cursor = Vec::from(&b"response-cursor"[..]);
        let expected_owner = owner.to_string();
        let expected_type = object_type.to_string();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once({
                let object_type = object_type.clone();
                let object_ref = object_ref.clone();
                let request_cursor = request_cursor.clone();
                let response_cursor = response_cursor.clone();
                move |request| {
                    let request = request.get_ref();
                    assert_eq!(request.owner.as_deref(), Some(expected_owner.as_str()));
                    assert_eq!(request.object_type.as_deref(), Some(expected_type.as_str()));
                    assert_eq!(request.page_size, Some(2));
                    assert_eq!(
                        request.page_token.as_deref(),
                        Some(request_cursor.as_slice())
                    );

                    let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                    response.set_objects(vec![typed_object_with_bcs(
                        object_ref,
                        sui::types::Owner::ConsensusAddress {
                            start_version: 5,
                            owner,
                        },
                        &object_type,
                        &TestValue { value: 42 },
                    )]);
                    response.next_page_token = Some(response_cursor.into());
                    Ok(tonic::Response::new(response))
                }
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let page = crawler
            .get_owned_object_page::<TestValue>(owner, object_type, Some(request_cursor), 2)
            .await
            .expect("owned object page loads");

        assert_eq!(page.data().len(), 1);
        assert_eq!(page.data()[0].object_id, *object_ref.object_id());
        assert_eq!(page.data()[0].data, TestValue { value: 42 });
        assert_eq!(
            page.data()[0].owner,
            sui::types::Owner::ConsensusAddress {
                start_version: 5,
                owner,
            }
        );
        assert_eq!(page.next_cursor(), Some(response_cursor.as_slice()));
    }

    #[tokio::test]
    async fn object_owned_objects_page_and_validate_the_exact_parent() {
        let parent_id = sui::types::Address::from_static("0xa");
        let object_type = test_value_tag();
        let first_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x20"));
        let second_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x21"));
        let expected_owner = parent_id.to_string();
        let expected_type = object_type.to_string();
        let responses = vec![
            (
                typed_object_with_bcs(
                    first_ref.clone(),
                    sui::types::Owner::Object(parent_id),
                    &object_type,
                    &TestValue { value: 7 },
                ),
                Some(Vec::from(&b"page-2"[..])),
            ),
            (
                typed_object_with_bcs(
                    second_ref.clone(),
                    sui::types::Owner::Object(parent_id),
                    &object_type,
                    &TestValue { value: 9 },
                ),
                None,
            ),
        ];
        let mut responses = responses.into_iter();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(2)
            .returning(move |request| {
                let request = request.get_ref();
                assert_eq!(request.owner.as_deref(), Some(expected_owner.as_str()));
                assert_eq!(request.object_type.as_deref(), Some(expected_type.as_str()));
                let (object, next_page_token) = responses.next().expect("object-owned page");
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(vec![object]);
                response.next_page_token = next_page_token.map(Into::into);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let objects = crawler
            .get_object_owned_objects::<TestValue>(parent_id, object_type)
            .await
            .expect("object-owned objects load");

        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].object_id, *first_ref.object_id());
        assert_eq!(objects[1].object_id, *second_ref.object_id());
        assert!(objects
            .iter()
            .all(|object| object.owner == sui::types::Owner::Object(parent_id)));
    }

    #[tokio::test]
    async fn object_owned_object_page_rejects_address_ownership() {
        let parent_id = sui::types::Address::from_static("0xa");
        let object_type = test_value_tag();
        let object_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x20"));
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once({
                let object_type = object_type.clone();
                move |_| {
                    let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                    response.set_objects(vec![typed_object_with_bcs(
                        object_ref,
                        sui::types::Owner::Address(parent_id),
                        &object_type,
                        &TestValue { value: 7 },
                    )]);
                    Ok(tonic::Response::new(response))
                }
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let error = crawler
            .get_object_owned_object_page::<TestValue>(parent_id, object_type, None, 1)
            .await
            .expect_err("address ownership must be rejected");
        assert!(error.to_string().contains("expected 'Object"));
    }

    #[tokio::test]
    async fn object_owned_object_page_rejects_the_wrong_type() {
        let parent_id = sui::types::Address::from_static("0xa");
        let expected_type = test_value_tag();
        let wrong_type = sui::types::StructTag::new(
            sui::types::Address::from_static("0xa1"),
            sui::types::Identifier::from_static("test"),
            sui::types::Identifier::from_static("WrongValue"),
            vec![],
        );
        let object_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x20"));
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_owned_objects()
            .times(1)
            .return_once(move |_| {
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(vec![typed_object_with_bcs(
                    object_ref,
                    sui::types::Owner::Object(parent_id),
                    &wrong_type,
                    &TestValue { value: 7 },
                )]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let error = crawler
            .get_object_owned_object_page::<TestValue>(parent_id, expected_type.clone(), None, 1)
            .await
            .expect_err("wrong type must be rejected");
        assert!(error.to_string().contains(&expected_type.to_string()));
    }

    #[tokio::test]
    async fn dynamic_object_field_by_key_fetches_only_the_wrapper_and_child() {
        let parent_id = sui::types::Address::from_static("0x40");
        let child_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x42"));
        let key = TestKey {
            name: "primary".to_string(),
        };
        let key_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0xa1"),
            sui::types::Identifier::from_static("test"),
            sui::types::Identifier::from_static("TestKey"),
            vec![],
        )));
        let wrapper_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_object_field"),
            sui::types::Identifier::from_static("Wrapper"),
            vec![key_type.clone()],
        )));
        let wrapper_key = TestDynamicObjectFieldName { name: key.clone() };
        let field_id = parent_id.derive_dynamic_child_id(
            &wrapper_type,
            &bcs::to_bytes(&wrapper_key).expect("wrapper key serializes"),
        );
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            sui_mocks::object_ref_for_id(field_id),
            sui::types::Owner::Object(parent_id),
            bcs::to_bytes(&DynamicFieldValue {
                id: field_id,
                name: wrapper_key,
                value: crate::move_bindings::sui_framework::object::ID::new(*child_ref.object_id()),
            })
            .expect("wrapper field serializes"),
        );
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            child_ref.clone(),
            sui::types::Owner::Shared(child_ref.version()),
            bcs::to_bytes(&TestValue { value: 11 }).expect("test value serializes"),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));

        let object = crawler
            .get_dynamic_object_field_by_key::<TestKey, TestValue>(parent_id, key, &key_type)
            .await
            .expect("dynamic object field request succeeds")
            .expect("dynamic object field exists");

        assert_eq!(object.object_id, *child_ref.object_id());
        assert_eq!(object.data, TestValue { value: 11 });
    }

    #[tokio::test]
    async fn dynamic_object_field_by_key_rejects_a_child_identity_mismatch() {
        let parent_id = sui::types::Address::from_static("0x40");
        let child_id = sui::types::Address::from_static("0x42");
        let returned_child_ref =
            sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x43"));
        let key = TestKey {
            name: "primary".to_string(),
        };
        let key_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0xa1"),
            sui::types::Identifier::from_static("test"),
            sui::types::Identifier::from_static("TestKey"),
            vec![],
        )));
        let wrapper_type = dynamic_object_field_wrapper_type(&key_type);
        let wrapper_key = TestDynamicObjectFieldName { name: key.clone() };
        let field_id = parent_id.derive_dynamic_child_id(
            &wrapper_type,
            &bcs::to_bytes(&wrapper_key).expect("wrapper key serializes"),
        );
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service,
            sui_mocks::object_ref_for_id(field_id),
            sui::types::Owner::Object(parent_id),
            bcs::to_bytes(&DynamicFieldValue {
                id: field_id,
                name: wrapper_key,
                value: crate::move_bindings::sui_framework::object::ID::new(child_id),
            })
            .expect("wrapper field serializes"),
        );
        ledger_service
            .expect_get_object()
            .withf(move |request| {
                request.get_ref().object_id.as_deref() == Some(child_id.to_string().as_str())
            })
            .times(1)
            .return_once(move |_| {
                let mut response = sui::grpc::GetObjectResponse::default();
                response.set_object(object_with_bcs(
                    returned_child_ref,
                    sui::types::Owner::Shared(1),
                    &TestValue { value: 11 },
                ));
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let error = crawler
            .get_dynamic_object_field_by_key::<TestKey, TestValue>(parent_id, key, &key_type)
            .await
            .expect_err("mismatched child identity must fail");

        assert!(
            error.to_string().contains("received object"),
            "unexpected error: {error}"
        );
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
        let crawler = Crawler::new(Arc::new(client));

        let fields = crawler
            .get_dynamic_fields::<TestKey, TestValue>(parent_id, 1)
            .await
            .expect("dynamic field value decodes");

        assert_eq!(fields.get(&key), Some(&TestValue { value: 42 }));
    }

    #[tokio::test]
    async fn get_dynamic_field_by_key_derives_the_field_identity() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key = 7_u64;
        let key_type = sui::types::TypeTag::U64;
        let field_id = parent_id
            .derive_dynamic_child_id(&key_type, &bcs::to_bytes(&key).expect("key serializes"));
        let field_ref = sui_mocks::object_ref_for_id(field_id);
        let field = DynamicFieldValue {
            id: field_id,
            name: key,
            value: TestValue { value: 42 },
        };
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            field_ref,
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&field).expect("dynamic field serializes"),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));

        let value = crawler
            .get_dynamic_field_by_key::<u64, TestValue>(parent_id, key, &key_type)
            .await
            .expect("dynamic field loads");

        assert_eq!(value, Some(TestValue { value: 42 }));
    }

    #[tokio::test]
    async fn dynamic_field_by_key_rejects_an_embedded_id_mismatch() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key = 7_u64;
        let key_type = sui::types::TypeTag::U64;
        let field_id = parent_id
            .derive_dynamic_child_id(&key_type, &bcs::to_bytes(&key).expect("key serializes"));
        let field_ref = sui_mocks::object_ref_for_id(field_id);
        let field = DynamicFieldValue {
            id: sui::types::Address::from_static("0x71"),
            name: key,
            value: TestValue { value: 42 },
        };
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service,
            field_ref,
            sui::types::Owner::Object(parent_id),
            bcs::to_bytes(&field).expect("dynamic field serializes"),
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let error = crawler
            .get_dynamic_field_by_key::<u64, TestValue>(parent_id, key, &key_type)
            .await
            .expect_err("mismatched embedded ID must fail");

        assert!(
            error.to_string().contains("unexpected embedded ID"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn dynamic_fields_by_keys_fetch_only_distinct_derived_ids() {
        let parent_id = sui::types::Address::from_static("0x70");
        let keys = [7_u64, 7_u64, 9_u64];
        let key_type = sui::types::TypeTag::U64;
        let requested_keys = [7_u64, 9_u64];
        let requested_ids = requested_keys.map(|key| {
            parent_id
                .derive_dynamic_child_id(&key_type, &bcs::to_bytes(&key).expect("key serializes"))
        });

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |request| {
                let actual_ids = request
                    .get_ref()
                    .requests
                    .iter()
                    .map(|request| {
                        request
                            .object_id
                            .as_deref()
                            .expect("object ID")
                            .parse()
                            .expect("valid object ID")
                    })
                    .collect::<Vec<sui::types::Address>>();
                assert_eq!(actual_ids, requested_ids);

                let objects = requested_keys
                    .into_iter()
                    .zip(requested_ids)
                    .map(|(name, object_id)| {
                        sui::grpc::GetObjectResult::new_object(object_with_bcs(
                            sui_mocks::object_ref_for_id(object_id),
                            sui::types::Owner::Object(parent_id),
                            &DynamicFieldValue {
                                id: object_id,
                                name,
                                value: TestValue { value: name },
                            },
                        ))
                    })
                    .collect();
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(objects),
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let fields = crawler
            .get_dynamic_fields_by_keys::<u64, TestValue, _>(parent_id, keys, &key_type)
            .await
            .expect("exact fields load");

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[&7].value, 7);
        assert_eq!(fields[&9].value, 9);
    }

    #[tokio::test]
    async fn dynamic_fields_by_keys_omit_missing_fields() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key_type = sui::types::TypeTag::U64;
        let keys = [7_u64, 9_u64];
        let field_ids = keys.map(|key| {
            parent_id
                .derive_dynamic_child_id(&key_type, &bcs::to_bytes(&key).expect("key serializes"))
        });
        let present_id = field_ids[0];

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |_| {
                let missing = sui_rpc::proto::google::rpc::Status {
                    code: tonic::Code::NotFound.into(),
                    message: "object not found".to_owned(),
                    ..Default::default()
                };
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(vec![
                        sui::grpc::GetObjectResult::new_object(object_with_bcs(
                            sui_mocks::object_ref_for_id(present_id),
                            sui::types::Owner::Object(parent_id),
                            &DynamicFieldValue {
                                id: present_id,
                                name: 7_u64,
                                value: TestValue { value: 7 },
                            },
                        )),
                        sui::grpc::GetObjectResult::new_error(missing),
                    ]),
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let fields = crawler
            .get_dynamic_fields_by_keys::<u64, TestValue, _>(parent_id, keys, &key_type)
            .await
            .expect("exact fields load");

        assert_eq!(fields, HashMap::from([(7, TestValue { value: 7 })]));
    }

    #[tokio::test]
    async fn dynamic_fields_by_keys_skip_rpc_for_empty_input() {
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let fields = crawler
            .get_dynamic_fields_by_keys::<u64, TestValue, _>(
                sui::types::Address::from_static("0x70"),
                [],
                &sui::types::TypeTag::U64,
            )
            .await
            .expect("empty request succeeds");

        assert!(fields.is_empty());
    }

    #[tokio::test]
    async fn dynamic_fields_by_keys_reject_a_decoded_key_mismatch() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key_type = sui::types::TypeTag::U64;
        let requested_key = 7_u64;
        let field_id = parent_id.derive_dynamic_child_id(
            &key_type,
            &bcs::to_bytes(&requested_key).expect("key serializes"),
        );

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |_| {
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(vec![
                        sui::grpc::GetObjectResult::new_object(object_with_bcs(
                            sui_mocks::object_ref_for_id(field_id),
                            sui::types::Owner::Object(parent_id),
                            &DynamicFieldValue {
                                id: field_id,
                                name: 9_u64,
                                value: TestValue { value: 9 },
                            },
                        )),
                    ]),
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let error = crawler
            .get_dynamic_fields_by_keys::<u64, TestValue, _>(parent_id, [requested_key], &key_type)
            .await
            .expect_err("mismatched key must fail");

        assert!(
            error.to_string().contains("unexpected key"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn dynamic_fields_by_keys_reject_an_embedded_id_mismatch() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key_type = sui::types::TypeTag::U64;
        let requested_key = 7_u64;
        let field_id = parent_id.derive_dynamic_child_id(
            &key_type,
            &bcs::to_bytes(&requested_key).expect("key serializes"),
        );

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |_| {
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(vec![
                        sui::grpc::GetObjectResult::new_object(object_with_bcs(
                            sui_mocks::object_ref_for_id(field_id),
                            sui::types::Owner::Object(parent_id),
                            &DynamicFieldValue {
                                id: sui::types::Address::from_static("0x71"),
                                name: requested_key,
                                value: TestValue { value: 7 },
                            },
                        )),
                    ]),
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let error = crawler
            .get_dynamic_fields_by_keys::<u64, TestValue, _>(parent_id, [requested_key], &key_type)
            .await
            .expect_err("mismatched embedded ID must fail");

        assert!(
            error.to_string().contains("unexpected embedded ID"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn table_vec_fetches_exact_index_range_without_listing_fields() {
        let parent_id = sui::types::Address::from_static("0x70");
        let table = TableVec::<TestValue>::new(parent_id, 2);
        let indexes = [0_u64, 1_u64];
        let field_ids = indexes.map(|index| {
            parent_id.derive_dynamic_child_id(
                &sui::types::TypeTag::U64,
                &bcs::to_bytes(&index).expect("index serializes"),
            )
        });

        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        ledger_service
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |request| {
                let actual_ids = request
                    .get_ref()
                    .requests
                    .iter()
                    .map(|request| {
                        request
                            .object_id
                            .as_deref()
                            .expect("object ID")
                            .parse()
                            .expect("valid object ID")
                    })
                    .collect::<Vec<sui::types::Address>>();
                assert_eq!(actual_ids, field_ids);

                let objects = indexes
                    .into_iter()
                    .zip(field_ids)
                    .map(|(index, field_id)| {
                        sui::grpc::GetObjectResult::new_object(object_with_bcs(
                            sui_mocks::object_ref_for_id(field_id),
                            sui::types::Owner::Object(parent_id),
                            &DynamicFieldValue {
                                id: field_id,
                                name: index,
                                value: TestValue { value: index + 3 },
                            },
                        ))
                    })
                    .collect();
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(objects),
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(sui::grpc::client(rpc_url).expect("mock client")));

        let values = crawler
            .get_table_vec(&table)
            .await
            .expect("table vector loads");

        assert_eq!(values, [TestValue { value: 3 }, TestValue { value: 4 }]);
    }

    #[tokio::test]
    async fn dynamic_field_page_forwards_both_cursors_unchanged() {
        let parent_id = sui::types::Address::from_static("0x70");
        let key = TestKey {
            name: "occurrence".to_owned(),
        };
        let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x71"));
        let request_cursor = Vec::from(&b"request-cursor"[..]);
        let response_cursor = Vec::from(&b"response-cursor"[..]);
        let expected_parent = parent_id.to_string();
        let listed_field_ref = field_ref.clone();
        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        state_service_mock
            .expect_list_dynamic_fields()
            .times(1)
            .return_once({
                let key = key.clone();
                let expected_parent = expected_parent.clone();
                let request_cursor = request_cursor.clone();
                let response_cursor = response_cursor.clone();
                move |request| {
                    let request = request.get_ref();
                    assert_eq!(request.parent.as_deref(), Some(expected_parent.as_str()));
                    assert_eq!(request.page_size, Some(2));
                    assert_eq!(
                        request.page_token.as_deref(),
                        Some(request_cursor.as_slice())
                    );

                    let mut field = sui::grpc::DynamicField::default();
                    field.set_field_id(*listed_field_ref.object_id());
                    let mut name = sui::grpc::Bcs::default();
                    name.set_value(bcs::to_bytes(&key).expect("key serializes"));
                    field.set_name(name);
                    field.set_value_type("0xa5::task::OccurrenceRecord");
                    let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                    response.set_dynamic_fields(vec![field]);
                    response.next_page_token = Some(response_cursor.into());
                    Ok(tonic::Response::new(response))
                }
            });
        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_dynamic_table_values_bcs(
            &mut ledger_service_mock,
            vec![(
                field_ref,
                sui::types::Owner::Shared(1),
                key.clone(),
                TestValue { value: 42 },
            )],
        );

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));

        let page = crawler
            .get_dynamic_field_page_matching_types::<TestKey, TestValue>(
                parent_id,
                Some(request_cursor),
                2,
                "::task::OccurrenceRecord",
            )
            .await
            .expect("dynamic field page loads");

        assert_eq!(page.data(), &[(key, TestValue { value: 42 })]);
        assert_eq!(page.next_cursor(), Some(response_cursor.as_slice()));
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
        let crawler = Crawler::new(Arc::new(client));

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
                transaction.set_checkpoint(1);
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
        let crawler = Crawler::new(Arc::new(client));

        let error = crawler
            .get_transaction_update(requested_digest)
            .await
            .expect_err("mismatched effects digest must fail permanently");
        let message = error.to_string();
        assert!(message.contains(&requested_digest.to_string()));
        assert!(message.contains(&effects_digest.to_string()));
        assert!(message.contains("effects transaction digest"));
    }

    #[tokio::test]
    async fn get_transaction_update_retains_checkpoint() {
        let mut rng = rand::thread_rng();
        let digest = sui::types::Digest::generate(&mut rng);
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
                transaction_digest: digest,
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
            .returning(move |_| {
                let mut grpc_effects = sui::grpc::TransactionEffects::default();
                grpc_effects.set_bcs(bcs::to_bytes(&effects).expect("effects serialize"));
                let mut grpc_events = sui::grpc::TransactionEvents::default();
                grpc_events.set_events(vec![]);
                let mut transaction = sui::grpc::ExecutedTransaction::default();
                transaction.set_digest(digest);
                transaction.set_checkpoint(42);
                transaction.set_effects(grpc_effects);
                transaction.set_events(grpc_events);
                let mut response = sui::grpc::GetTransactionResponse::default();
                response.set_transaction(transaction);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));

        let update = crawler
            .get_transaction_update(digest)
            .await
            .expect("transaction update loads");

        assert_eq!(update.checkpoint, 42);
    }
}
