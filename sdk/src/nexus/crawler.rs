//! Module defining a Sui object crawler - this struct is able to fetch object
//! and dynamic field data from Sui GRPC and deserialize them into Rust structs.

use {
    crate::{
        move_bindings::sui_framework::{object::ID, table_vec::TableVec},
        nexus::error::NexusError,
        sui::{self, traits::FieldMaskUtil},
    },
    anyhow::{anyhow, bail, ensure, Context as _},
    serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize},
    std::{
        collections::{HashMap, HashSet},
        hash::Hash,
        sync::Arc,
    },
};

lazy_static::lazy_static! {
    static ref DYNAMIC_FIELD_LIST_REQUESTS: prometheus::CounterVec =
        prometheus::register_counter_vec!(
            "nexus_dynamic_field_list_requests_total",
            "ListDynamicFields requests issued by each typed crawler operation",
            &["operation"],
        )
        .unwrap();
}

fn observe_dynamic_field_list(operation: &'static str) {
    DYNAMIC_FIELD_LIST_REQUESTS
        .with_label_values(&[operation])
        .inc();
}

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

pub(crate) fn derive_dynamic_field_id<K>(
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

enum FinalizedObjectChange {
    Unchanged,
    Written(Box<sui::grpc::Object>),
    Removed,
}

fn validate_executed_transaction(
    executed: &sui::grpc::ExecutedTransaction,
    transaction: sui::types::Digest,
) -> anyhow::Result<()> {
    let observed = executed
        .digest_opt()
        .ok_or_else(|| anyhow!("Transaction '{transaction}' response has no digest"))?
        .parse::<sui::types::Digest>()
        .map_err(|error| {
            anyhow!("Transaction '{transaction}' response has an invalid digest: {error}")
        })?;
    if observed != transaction {
        bail!("Requested transaction '{transaction}', received transaction '{observed}'");
    }
    Ok(())
}

fn finalized_transaction_object_change(
    objects: &[sui::grpc::Object],
    transaction: sui::types::Digest,
    object_id: sui::types::Address,
) -> anyhow::Result<FinalizedObjectChange> {
    let mut mentioned = false;
    let mut output = None;
    for object in objects {
        if Crawler::parse_object_id(object)? != object_id {
            continue;
        }
        mentioned = true;
        let previous_transaction = object
            .previous_transaction_opt()
            .ok_or_else(|| {
                anyhow!(
                    "Finalized transaction '{transaction}' returned object '{object_id}' without \
                     its previous transaction"
                )
            })?
            .parse::<sui::types::Digest>()
            .map_err(|error| {
                anyhow!(
                    "Finalized transaction '{transaction}' returned object '{object_id}' with an \
                     invalid previous transaction: {error}"
                )
            })?;
        if previous_transaction == transaction && output.replace(object.clone()).is_some() {
            bail!(
                "Finalized transaction '{transaction}' returned output object '{object_id}' \
                 more than once"
            );
        }
    }

    match (mentioned, output) {
        (_, Some(output)) => Ok(FinalizedObjectChange::Written(Box::new(output))),
        (true, None) => Ok(FinalizedObjectChange::Removed),
        (false, None) => Ok(FinalizedObjectChange::Unchanged),
    }
}

fn reject_removed_state_object(
    transaction: sui::types::Digest,
    object_id: sui::types::Address,
    role: &str,
) -> ObjectStateSnapshotError {
    ObjectStateSnapshotError::Invalid(format!(
        "Finalized transaction '{transaction}' removed the {role} object '{object_id}'"
    ))
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
    state_catalog: Arc<sui::grpc::Client>,
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

/// Immutable type metadata for one ordinary dynamic field.
///
/// The key and value types come from the complete
/// `0x2::dynamic_field::Field<K, V>` object type. The value bytes are never
/// decoded while this metadata is collected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicFieldMetadata {
    /// Object ID of the dynamic field wrapper.
    pub field_id: sui::types::Address,
    /// Exact Move type of the field key.
    pub key_type: sui::types::TypeTag,
    /// Exact Move type of the field value.
    pub value_type: sui::types::TypeTag,
}

/// Identity, owner, and Move type of one live object.
///
/// Mutable version and digest data are deliberately absent because neither is
/// package authority or stable object identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectMetadata {
    /// Stable Sui object ID.
    pub object_id: sui::types::Address,
    /// Current object owner.
    pub owner: sui::types::Owner,
    /// Exact Move type of the object anchor.
    pub object_type: sui::types::StructTag,
}

/// One live anchor and its state fields returned by one Sui batch read.
///
/// The snapshot retains object bytes only until the caller validates the
/// complete anchor, witness, and inner type identity. This lets typed state
/// reads decode the value without a second RPC.
#[derive(Clone, Debug)]
pub(crate) struct ObjectStateSnapshot {
    pub(crate) object: ObjectMetadata,
    pub(crate) witness: DynamicFieldMetadata,
    pub(crate) inner: DynamicFieldMetadata,
    anchor_object: sui::grpc::Object,
    inner_object: sui::grpc::Object,
}

impl ObjectStateSnapshot {
    pub(crate) fn inner_object_reference(&self) -> anyhow::Result<sui::types::ObjectReference> {
        let object_id = Crawler::parse_object_id(&self.inner_object)?;
        ensure!(
            object_id == self.inner.field_id,
            "State inner object '{}' does not match expected field '{}'",
            object_id,
            self.inner.field_id,
        );
        let version = self
            .inner_object
            .version_opt()
            .ok_or_else(|| anyhow!("Version missing for state inner object '{object_id}'"))?;
        let digest = self
            .inner_object
            .digest_opt()
            .ok_or_else(|| anyhow!("Digest missing for state inner object '{object_id}'"))?
            .parse()
            .map_err(|_| anyhow!("Invalid digest for state inner object '{object_id}'"))?;

        Ok(sui::types::ObjectReference::new(object_id, version, digest))
    }
}

/// Failure while reading a typed state snapshot.
#[derive(Debug)]
pub(crate) enum ObjectStateSnapshotError {
    /// Sui transport or object availability failed.
    Rpc(anyhow::Error),
    /// Returned object identity, ownership, or type metadata was invalid.
    Invalid(String),
}

#[derive(Debug)]
struct ObjectStateObjects {
    anchor: sui::grpc::Object,
    witness: sui::grpc::Object,
    inner: sui::grpc::Object,
}

fn parse_dynamic_field_metadata(
    parent_id: sui::types::Address,
    field: &sui::grpc::DynamicField,
) -> anyhow::Result<Option<DynamicFieldMetadata>> {
    match field.kind {
        Some(kind) if kind == sui::grpc::dynamic_field::DynamicFieldKind::Object as i32 => {
            return Ok(None);
        }
        Some(kind) if kind != sui::grpc::dynamic_field::DynamicFieldKind::Field as i32 => {
            bail!("Dynamic field for parent '{parent_id}' has unknown kind '{kind}'");
        }
        _ => {}
    }

    let field_id = field
        .field_id_opt()
        .ok_or_else(|| anyhow!("Dynamic field ID missing for parent '{parent_id}'"))?
        .parse()
        .map_err(|error| anyhow!("Could not parse dynamic field ID: {error}"))?;
    let object_type = field
        .field_object_opt()
        .and_then(|object| object.object_type_opt())
        .ok_or_else(|| anyhow!("Dynamic field '{field_id}' has no field object type"))?;
    let (key_type, value_type) = parse_dynamic_field_type(field_id, object_type)?;
    if let Some(reported_value_type) = field.value_type_opt() {
        let reported_value_type =
            reported_value_type
                .parse::<sui::types::TypeTag>()
                .map_err(|error| {
                    anyhow!("Dynamic field '{field_id}' has invalid reported value type: {error}")
                })?;
        if reported_value_type != value_type {
            bail!(
                "Dynamic field '{field_id}' reports value type '{reported_value_type}', but its \
                 field object contains '{value_type}'"
            );
        }
    }

    Ok(Some(DynamicFieldMetadata {
        field_id,
        key_type,
        value_type,
    }))
}

fn parse_dynamic_field_type(
    field_id: sui::types::Address,
    object_type: &str,
) -> anyhow::Result<(sui::types::TypeTag, sui::types::TypeTag)> {
    let field_type = object_type
        .parse::<sui::types::StructTag>()
        .map_err(|error| anyhow!("Dynamic field '{field_id}' has invalid type: {error}"))?;
    if *field_type.address() != sui::types::Address::from_static("0x2")
        || field_type.module().as_str() != "dynamic_field"
        || field_type.name().as_str() != "Field"
        || field_type.type_params().len() != 2
    {
        bail!(
            "Dynamic field '{field_id}' has object type '{field_type}', expected \
             '0x2::dynamic_field::Field<K, V>'"
        );
    }

    let key_type = field_type.type_params()[0].clone();
    let value_type = field_type.type_params()[1].clone();
    Ok((key_type, value_type))
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
        Self {
            state_catalog: Arc::clone(&client),
            client,
        }
    }

    /// Uses `state_catalog` for indexed dynamic field discovery.
    ///
    /// Exact objects, transactions, and packages continue to come from
    /// `client`. Collection identities can therefore be discovered without
    /// making an indexed service the authority for current object state.
    pub fn with_state_catalog(
        client: Arc<sui::grpc::Client>,
        state_catalog: Arc<sui::grpc::Client>,
    ) -> Self {
        Self {
            client,
            state_catalog,
        }
    }

    pub(crate) fn grpc_client(&self) -> Arc<sui::grpc::Client> {
        Arc::clone(&self.client)
    }

    pub(crate) fn clone_grpc_client(&self) -> sui::grpc::Client {
        self.client.as_ref().clone()
    }

    fn clone_state_catalog_client(&self) -> sui::grpc::Client {
        self.state_catalog.as_ref().clone()
    }

    /// Fetches stable identity, owner, and exact Move type for `object_id`.
    ///
    /// Object contents are not requested or decoded.
    ///
    /// # Errors
    ///
    /// Returns an error when the object cannot be fetched or its identity,
    /// owner, or Move type is absent or invalid.
    pub async fn observe_object_metadata(
        &self,
        object_id: sui::types::Address,
    ) -> anyhow::Result<ObjectMetadata> {
        let object = self
            .fetch_object(
                object_id,
                sui::grpc::FieldMask::from_paths(["object_id", "owner", "object_type"]),
            )
            .await?;
        Self::parse_observed_object_metadata(object_id, &object)
    }

    fn parse_observed_object_metadata(
        object_id: sui::types::Address,
        object: &sui::grpc::Object,
    ) -> anyhow::Result<ObjectMetadata> {
        let returned_id = Self::parse_object_id(object)?;
        if returned_id != object_id {
            bail!("Requested object '{object_id}', received object '{returned_id}'");
        }
        let owner = object
            .owner_opt()
            .ok_or_else(|| anyhow!("Owner missing for object '{object_id}'"))?
            .try_into()
            .map_err(|_| anyhow!("Could not parse owner for object '{object_id}'"))?;
        let object_type = object
            .object_type_opt()
            .ok_or_else(|| anyhow!("Object type missing for object '{object_id}'"))?
            .parse()
            .map_err(|error| anyhow!("Could not parse type for object '{object_id}': {error}"))?;

        Ok(ObjectMetadata {
            object_id,
            owner,
            object_type,
        })
    }

    /// Observes one anchor and two known dynamic field wrappers in one batch.
    ///
    /// The field IDs must already have been derived from their exact typed
    /// keys. A missing wrapper returns [`None`] so the caller can discover a
    /// different key lineage. All present identities, owners, and complete
    /// `0x2::dynamic_field::Field<K, V>` types are validated.
    pub(crate) async fn observe_object_state_fields(
        &self,
        object_id: sui::types::Address,
        witness_field_id: sui::types::Address,
        inner_field_id: sui::types::Address,
    ) -> anyhow::Result<Option<(ObjectMetadata, Vec<DynamicFieldMetadata>)>> {
        let field_mask = sui::grpc::FieldMask::from_paths(["object_id", "owner", "object_type"]);
        let Some(objects) = self
            .fetch_object_state_objects(object_id, witness_field_id, inner_field_id, field_mask)
            .await?
        else {
            return Ok(None);
        };

        let object = Self::parse_observed_object_metadata(object_id, &objects.anchor)?;
        let witness =
            Self::parse_state_field_metadata(object_id, witness_field_id, &objects.witness)?;
        let inner = Self::parse_state_field_metadata(object_id, inner_field_id, &objects.inner)?;

        Ok(Some((object, vec![witness, inner])))
    }

    /// Reads one anchor and both known state fields in one batch.
    ///
    /// The returned bytes remain opaque until the caller validates the exact
    /// state pair. Missing derived fields return [`None`] so an unknown key
    /// lineage can be discovered through the metadata path.
    pub(crate) async fn object_state_snapshot(
        &self,
        object_id: sui::types::Address,
        witness_field_id: sui::types::Address,
        inner_field_id: sui::types::Address,
    ) -> Result<Option<ObjectStateSnapshot>, ObjectStateSnapshotError> {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "object_type",
            "version",
            "digest",
            "balance",
            "contents",
        ]);
        let Some(objects) = self
            .fetch_object_state_objects(object_id, witness_field_id, inner_field_id, field_mask)
            .await
            .map_err(ObjectStateSnapshotError::Rpc)?
        else {
            return Ok(None);
        };
        let invalid = |error: anyhow::Error| ObjectStateSnapshotError::Invalid(error.to_string());
        let object =
            Self::parse_observed_object_metadata(object_id, &objects.anchor).map_err(invalid)?;
        let witness =
            Self::parse_state_field_metadata(object_id, witness_field_id, &objects.witness)
                .map_err(invalid)?;
        let inner = Self::parse_state_field_metadata(object_id, inner_field_id, &objects.inner)
            .map_err(invalid)?;

        Ok(Some(ObjectStateSnapshot {
            object,
            witness,
            inner,
            anchor_object: objects.anchor,
            inner_object: objects.inner,
        }))
    }

    /// Build one state snapshot from a verified causal transaction view.
    ///
    /// The view may contain objects inherited from finalized ancestors, so an
    /// object's `previous_transaction` need not equal `transaction`. Exact
    /// object identity, ownership, field derivation, and Move types are still
    /// validated before any bytes can become application authority.
    pub(crate) fn object_state_snapshot_from_transaction_view(
        &self,
        object_id: sui::types::Address,
        witness_field_id: sui::types::Address,
        inner_field_id: sui::types::Address,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
    ) -> Result<Option<ObjectStateSnapshot>, ObjectStateSnapshotError> {
        let invalid = |error: anyhow::Error| ObjectStateSnapshotError::Invalid(error.to_string());
        validate_executed_transaction(executed, transaction).map_err(invalid)?;

        let find =
            |id| -> anyhow::Result<Option<sui::grpc::Object>> {
                let mut matches = executed.objects().objects.iter().filter_map(|object| {
                    match Self::parse_object_id(object) {
                        Ok(returned) if returned == id => Some(Ok(object.clone())),
                        Ok(_) => None,
                        Err(error) => Some(Err(error)),
                    }
                });
                let Some(object) = matches.next().transpose()? else {
                    return Ok(None);
                };
                ensure!(
                    matches.next().transpose()?.is_none(),
                    "Causal transaction view contains object '{id}' more than once"
                );
                Ok(Some(object))
            };

        let Some(anchor) = find(object_id).map_err(invalid)? else {
            return Ok(None);
        };
        let Some(witness_object) = find(witness_field_id).map_err(invalid)? else {
            return Ok(None);
        };
        let Some(inner_object) = find(inner_field_id).map_err(invalid)? else {
            return Ok(None);
        };

        let object = Self::parse_observed_object_metadata(object_id, &anchor).map_err(invalid)?;
        let witness =
            Self::parse_state_field_metadata(object_id, witness_field_id, &witness_object)
                .map_err(invalid)?;
        let inner = Self::parse_state_field_metadata(object_id, inner_field_id, &inner_object)
            .map_err(invalid)?;

        Ok(Some(ObjectStateSnapshot {
            object,
            witness,
            inner,
            anchor_object: anchor,
            inner_object,
        }))
    }

    async fn fetch_object_state_objects(
        &self,
        object_id: sui::types::Address,
        witness_field_id: sui::types::Address,
        inner_field_id: sui::types::Address,
        field_mask: sui::grpc::FieldMask,
    ) -> anyhow::Result<Option<ObjectStateObjects>> {
        let object_ids = [object_id, witness_field_id, inner_field_id];
        let results = self.fetch_object_results(&object_ids, field_mask).await?;
        if results.len() != object_ids.len() {
            bail!(
                "Batch object response contained {} results for {} state objects",
                results.len(),
                object_ids.len()
            );
        }

        let mut results = results.into_iter();
        let anchor = results
            .next()
            .expect("response length was validated")
            .to_result()
            .map_err(|status| {
                if status.code == i32::from(tonic::Code::NotFound) {
                    anyhow::Error::new(NexusError::ObjectNotFound { object: object_id })
                } else {
                    anyhow!("Could not fetch object '{object_id}': {}", status.message)
                }
            })?;
        let field = |field_id, result: sui::grpc::GetObjectResult| match result.to_result() {
            Ok(field) => Ok(Some(field)),
            Err(status) if status.code == i32::from(tonic::Code::NotFound) => Ok(None),
            Err(status) => Err(anyhow!(
                "Could not fetch dynamic field '{field_id}': {}",
                status.message
            )),
        };
        let Some(witness) = field(
            witness_field_id,
            results.next().expect("response length was validated"),
        )?
        else {
            return Ok(None);
        };
        let Some(inner) = field(
            inner_field_id,
            results.next().expect("response length was validated"),
        )?
        else {
            return Ok(None);
        };

        Ok(Some(ObjectStateObjects {
            anchor,
            witness,
            inner,
        }))
    }

    fn parse_state_field_metadata(
        parent_id: sui::types::Address,
        field_id: sui::types::Address,
        field: &sui::grpc::Object,
    ) -> anyhow::Result<DynamicFieldMetadata> {
        let returned_id = Self::parse_object_id(field)?;
        if returned_id != field_id {
            bail!("Requested dynamic field '{field_id}', received '{returned_id}'");
        }
        let owner: sui::types::Owner = field
            .owner_opt()
            .ok_or_else(|| anyhow!("Owner missing for dynamic field '{field_id}'"))?
            .try_into()
            .map_err(|_| anyhow!("Could not parse owner for dynamic field '{field_id}'"))?;
        if owner != sui::types::Owner::Object(parent_id) {
            bail!(
                "Dynamic field '{field_id}' has owner '{owner:?}', expected object '{parent_id}'"
            );
        }
        let object_type = field
            .object_type_opt()
            .ok_or_else(|| anyhow!("Object type missing for dynamic field '{field_id}'"))?;
        let (key_type, value_type) = parse_dynamic_field_type(field_id, object_type)?;
        Ok(DynamicFieldMetadata {
            field_id,
            key_type,
            value_type,
        })
    }

    /// Decodes a validated object state snapshot without another RPC.
    pub(crate) fn decode_object_state_snapshot<A, K, V>(
        &self,
        snapshot: &ObjectStateSnapshot,
        key: &K,
    ) -> anyhow::Result<(Response<A>, V)>
    where
        A: DeserializeOwned,
        K: DeserializeOwned + Eq,
        V: DeserializeOwned,
    {
        let object_id = snapshot.object.object_id;
        let (owner, digest, version, balance) =
            self.parse_object_metadata(object_id, &snapshot.anchor_object)?;
        let data = self.parse_object_contents_bcs::<A>(&snapshot.anchor_object)?;
        let field =
            self.parse_object_contents_bcs::<DynamicFieldValue<K, V>>(&snapshot.inner_object)?;
        validate_dynamic_field(snapshot.inner.field_id, key, &field)?;
        Ok((
            Response {
                object_id,
                owner,
                version,
                data,
                digest,
                balance,
            },
            field.value,
        ))
    }

    /// Lists exact key and value types for every dynamic field below `parent_id`.
    ///
    /// This method reads only field identities and type metadata. In particular,
    /// it does not decode field names or values, so an unknown value layout can
    /// still be reported to compatibility logic.
    ///
    /// # Errors
    ///
    /// Returns an error when the RPC request fails or a returned field does not
    /// have a valid `0x2::dynamic_field::Field<K, V>` object type.
    pub async fn get_dynamic_field_metadata(
        &self,
        parent_id: sui::types::Address,
    ) -> anyhow::Result<Vec<DynamicFieldMetadata>> {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "kind",
            "field_id",
            "field_object.object_type",
            "value_type",
        ]);
        let mut metadata = Vec::new();
        let mut page_token = None;
        let mut client = self.clone_state_catalog_client();

        loop {
            observe_dynamic_field_list("state_metadata");
            let mut request = sui::grpc::ListDynamicFieldsRequest::default()
                .with_parent(parent_id)
                .with_page_size(1000)
                .with_read_mask(field_mask.clone());
            if let Some(token) = page_token {
                request = request.with_page_token(token);
            }

            let response = client
                .state_client()
                .list_dynamic_fields(request)
                .await
                .map(|response| response.into_inner())
                .map_err(anyhow::Error::new)
                .with_context(|| {
                    format!("Could not fetch dynamic fields for parent '{parent_id}'")
                })?;
            page_token = response.next_page_token;
            metadata.extend(
                response
                    .dynamic_fields
                    .iter()
                    .map(|field| parse_dynamic_field_metadata(parent_id, field))
                    .collect::<anyhow::Result<Vec<_>>>()?
                    .into_iter()
                    .flatten(),
            );

            if page_token.is_none() {
                break;
            }
        }

        Ok(metadata)
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

    /// Fetches one object and one of its ordinary dynamic fields in one batch.
    ///
    /// The object and field identities, current owner, anchor type, field value
    /// type, embedded field ID, and field key are validated before values are
    /// returned.
    pub(crate) async fn get_object_with_dynamic_field<A, K, V>(
        &self,
        object_id: sui::types::Address,
        expected_object_type: &sui::types::StructTag,
        field_id: sui::types::Address,
        expected_value_type: &sui::types::TypeTag,
        key: &K,
    ) -> anyhow::Result<(Response<A>, V)>
    where
        A: DeserializeOwned,
        K: DeserializeOwned + Eq,
        V: DeserializeOwned,
    {
        let field_mask = sui::grpc::FieldMask::from_paths([
            "object_id",
            "owner",
            "object_type",
            "version",
            "digest",
            "balance",
            "contents",
        ]);
        let requested = [object_id, field_id];
        let results = self.fetch_object_results(&requested, field_mask).await?;
        if results.len() != requested.len() {
            bail!(
                "Batch object response contained {} results for {} requests",
                results.len(),
                requested.len()
            );
        }
        let mut results = results.into_iter();
        let object = results
            .next()
            .expect("response length was validated")
            .to_result()
            .map_err(|status| {
                anyhow!("Could not fetch object '{object_id}': {}", status.message)
            })?;
        let field = results
            .next()
            .expect("response length was validated")
            .to_result()
            .map_err(|status| {
                anyhow!(
                    "Could not fetch dynamic field '{field_id}': {}",
                    status.message
                )
            })?;

        let returned_object_id = Self::parse_object_id(&object)?;
        if returned_object_id != object_id {
            bail!("Requested object '{object_id}', received object '{returned_object_id}'");
        }
        let object_type = object
            .object_type_opt()
            .ok_or_else(|| anyhow!("Object type missing for object '{object_id}'"))?
            .parse::<sui::types::StructTag>()
            .map_err(|error| anyhow!("Could not parse type for object '{object_id}': {error}"))?;
        if &object_type != expected_object_type {
            bail!(
                "Object '{object_id}' has type '{object_type}', expected '{expected_object_type}'"
            );
        }

        let returned_field_id = Self::parse_object_id(&field)?;
        if returned_field_id != field_id {
            bail!("Requested dynamic field '{field_id}', received '{returned_field_id}'");
        }
        let field_owner: sui::types::Owner = field
            .owner_opt()
            .ok_or_else(|| anyhow!("Owner missing for dynamic field '{field_id}'"))?
            .try_into()
            .map_err(|_| anyhow!("Could not parse owner for dynamic field '{field_id}'"))?;
        if field_owner != sui::types::Owner::Object(object_id) {
            bail!(
                "Dynamic field '{field_id}' has owner '{field_owner:?}', expected object \
                 '{object_id}'"
            );
        }
        let field_type = field
            .object_type_opt()
            .ok_or_else(|| anyhow!("Object type missing for dynamic field '{field_id}'"))?;
        let (_, value_type) = parse_dynamic_field_type(field_id, field_type)?;
        if &value_type != expected_value_type {
            bail!(
                "Dynamic field '{field_id}' has value type '{value_type}', expected \
                 '{expected_value_type}'"
            );
        }

        let (owner, digest, version, balance) = self.parse_object_metadata(object_id, &object)?;
        let data = self.parse_object_contents_bcs::<A>(&object)?;
        let field = self.parse_object_contents_bcs::<DynamicFieldValue<K, V>>(&field)?;
        validate_dynamic_field(field_id, key, &field)?;
        Ok((
            Response::<A> {
                object_id,
                owner,
                version,
                data,
                digest,
                balance,
            },
            field.value,
        ))
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

    /// Fetches the short chain identifier used by Move environments.
    ///
    /// This is the first four bytes of [`Self::get_chain_digest`] encoded as
    /// hexadecimal, matching `sui client chain-identifier`. It is an
    /// environment selector, not the complete chain identity used for replay
    /// protection.
    pub async fn get_chain_id(&self) -> anyhow::Result<String> {
        let digest = self.get_chain_digest().await?;
        Ok(hex::encode(&digest.as_bytes()[..4]))
    }

    /// Fetches the complete genesis checkpoint digest that identifies the chain.
    ///
    /// Use this value for durable environment identity and transaction replay
    /// protection. Use [`Self::get_chain_id`] only when selecting a Move build
    /// environment.
    pub async fn get_chain_digest(&self) -> anyhow::Result<sui::types::Digest> {
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
        sui::types::Digest::from_base58(&base58)
            .map_err(|e| anyhow!("connected RPC returned an unparsable chain id '{base58}': {e}"))
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

    /// Fetches the object set returned for one exact transaction.
    ///
    /// The response can contain both input and output versions. Consumers must
    /// select outputs by checking `previous_transaction` against `transaction`.
    ///
    /// # Errors
    ///
    /// Returns an error when the transaction is absent or the response names a
    /// different transaction.
    pub async fn get_transaction_objects(
        &self,
        transaction: sui::types::Digest,
    ) -> anyhow::Result<sui::grpc::ExecutedTransaction> {
        let request = sui::grpc::GetTransactionRequest::default()
            .with_digest(transaction.to_string())
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "digest",
                "objects.objects.object_id",
                "objects.objects.owner",
                "objects.objects.version",
                "objects.objects.digest",
                "objects.objects.balance",
                "objects.objects.object_type",
                "objects.objects.contents",
                "objects.objects.previous_transaction",
            ]));
        let mut client = self.clone_grpc_client();
        let executed = client
            .ledger_client()
            .get_transaction(request)
            .await
            .map(|response| response.into_inner().transaction)
            .with_context(|| format!("Could not fetch transaction '{transaction}'"))?
            .ok_or_else(|| anyhow!("Transaction '{transaction}' not found"))?;
        validate_executed_transaction(&executed, transaction)?;
        Ok(executed)
    }

    /// Decodes one object version written by an exact transaction response.
    ///
    /// Sui can return both input and output versions. This method accepts only
    /// the version whose `previous_transaction` is `transaction`.
    ///
    /// # Errors
    ///
    /// Returns an error when the response identity is inconsistent, the output
    /// object is absent or repeated, or its value cannot be decoded.
    pub fn transaction_output_object<T>(
        &self,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
        object_id: sui::types::Address,
    ) -> anyhow::Result<Response<T>>
    where
        T: DeserializeOwned,
    {
        validate_executed_transaction(executed, transaction)?;

        let mut output = executed.objects().objects.iter().filter(|object| {
            Self::parse_object_id(object).ok() == Some(object_id)
                && object
                    .previous_transaction_opt()
                    .and_then(|digest| digest.parse::<sui::types::Digest>().ok())
                    == Some(transaction)
        });
        let object = output.next().ok_or_else(|| {
            anyhow!("Transaction '{transaction}' did not write object '{object_id}'")
        })?;
        if output.next().is_some() {
            bail!("Transaction '{transaction}' returned object '{object_id}' more than once");
        }
        let (owner, digest, version, balance) = self.parse_object_metadata(object_id, object)?;
        let data = self.parse_object_contents_bcs::<T>(object)?;

        Ok(Response {
            object_id,
            owner,
            version,
            data,
            digest,
            balance,
        })
    }

    /// Returns the initial version of one shared object named by an exact
    /// [`sui::grpc::ExecutedTransaction`].
    ///
    /// A transaction object set can contain both the input and output version
    /// of the same object. Every matching version must retain the expected
    /// type and the same shared start version. This permits a consumer to
    /// construct a causal shared-object input without consulting a lagging
    /// latest-object surface.
    ///
    /// # Errors
    ///
    /// Returns an error when the response identity is inconsistent, the object
    /// is absent, its type differs from `expected_type`, it is not shared, or
    /// matching versions disagree about the shared start version.
    pub fn transaction_shared_initial_version(
        &self,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
        object_id: sui::types::Address,
        expected_type: &sui::types::StructTag,
    ) -> anyhow::Result<sui::types::Version> {
        validate_executed_transaction(executed, transaction)?;

        let mut initial_version = None;
        for object in executed
            .objects()
            .objects
            .iter()
            .filter(|object| Self::parse_object_id(object).ok() == Some(object_id))
        {
            let object_type = object
                .object_type_opt()
                .ok_or_else(|| anyhow!("Object type missing for transaction object '{object_id}'"))?
                .parse::<sui::types::StructTag>()
                .map_err(|error| {
                    anyhow!("Transaction object '{object_id}' has an invalid type: {error}")
                })?;
            ensure!(
                &object_type == expected_type,
                "Transaction object '{object_id}' has type '{object_type}', expected '{expected_type}'"
            );
            let (owner, _, _, _) = self.parse_object_metadata(object_id, object)?;
            let observed = match owner {
                sui::types::Owner::Shared(version) => version,
                sui::types::Owner::ConsensusAddress { start_version, .. } => start_version,
                owner => bail!(
                    "Transaction object '{object_id}' is not shared; observed owner {owner:?}"
                ),
            };
            match initial_version {
                Some(expected) => ensure!(
                    observed == expected,
                    "Transaction object '{object_id}' has inconsistent shared start versions {expected} and {observed}"
                ),
                None => initial_version = Some(observed),
            }
        }

        initial_version.ok_or_else(|| {
            anyhow!("Transaction '{transaction}' does not name shared object '{object_id}'")
        })
    }

    /// Fetches and decodes one object version written by an exact transaction.
    ///
    /// # Errors
    ///
    /// Returns the errors from [`Self::get_transaction_objects`] or
    /// [`Self::transaction_output_object`].
    pub async fn get_transaction_output_object<T>(
        &self,
        transaction: sui::types::Digest,
        object_id: sui::types::Address,
    ) -> anyhow::Result<Response<T>>
    where
        T: DeserializeOwned,
    {
        let executed = self.get_transaction_objects(transaction).await?;
        self.transaction_output_object(&executed, transaction, object_id)
    }

    pub(crate) fn transaction_dynamic_field_outputs<K, V>(
        &self,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
        key: &K,
        expected_key_type: &sui::types::TypeTag,
        expected_value_type: &sui::types::TypeTag,
    ) -> anyhow::Result<Vec<TransactionStateOutput<V>>>
    where
        K: DeserializeOwned + Eq + Serialize,
        V: DeserializeOwned,
    {
        validate_executed_transaction(executed, transaction)?;
        let mut outputs = Vec::new();
        for object in &executed.objects().objects {
            let object_id = Self::parse_object_id(object)?;
            let previous_transaction = object
                .previous_transaction_opt()
                .ok_or_else(|| {
                    anyhow!(
                        "Transaction '{transaction}' returned object '{object_id}' without its \
                         previous transaction"
                    )
                })?
                .parse::<sui::types::Digest>()
                .map_err(|error| {
                    anyhow!(
                        "Transaction '{transaction}' returned object '{object_id}' with an \
                         invalid previous transaction: {error}"
                    )
                })?;
            if previous_transaction != transaction {
                continue;
            }
            if let Some(output) = self.decode_transaction_dynamic_field(
                object,
                object_id,
                key,
                expected_key_type,
                expected_value_type,
            )? {
                outputs.push(output);
            }
        }
        Ok(outputs)
    }

    /// Decodes matching state fields from one retained causal view.
    ///
    /// Unlike [`Self::transaction_dynamic_field_outputs`], values written by
    /// ancestors are included. A direct execution response can also contain
    /// the input and output versions of an object written by `transaction`.
    /// In that representation the root output is the visible value. Any other
    /// repeated object history is ambiguous and rejected.
    pub(crate) fn causal_dynamic_field_values<K, V>(
        &self,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
        key: &K,
        expected_key_type: &sui::types::TypeTag,
        expected_value_type: &sui::types::TypeTag,
    ) -> anyhow::Result<Vec<TransactionStateOutput<V>>>
    where
        K: DeserializeOwned + Eq + Serialize,
        V: DeserializeOwned,
    {
        validate_executed_transaction(executed, transaction)?;
        let mut positions = HashMap::new();
        let mut paired = HashSet::new();
        let mut visible = Vec::new();
        for object in &executed.objects().objects {
            let object_id = Self::parse_object_id(object)?;
            let previous_transaction = object
                .previous_transaction_opt()
                .ok_or_else(|| {
                    anyhow!(
                        "Causal transaction view returned object '{object_id}' without its \
                         previous transaction"
                    )
                })?
                .parse::<sui::types::Digest>()
                .map_err(|error| {
                    anyhow!(
                        "Causal transaction view returned object '{object_id}' with an invalid \
                         previous transaction: {error}"
                    )
                })?;
            let root_output = previous_transaction == transaction;

            let Some(index) = positions.get(&object_id).copied() else {
                positions.insert(object_id, visible.len());
                visible.push((object, root_output));
                continue;
            };

            ensure!(
                paired.insert(object_id),
                "Causal transaction view contains more than an input and output version of \
                 object '{object_id}'"
            );
            let (selected, selected_is_root_output) = visible[index];
            ensure!(
                selected_is_root_output != root_output,
                "Causal transaction view contains ambiguous versions of object '{object_id}'"
            );
            let (input, output) = if root_output {
                (selected, object)
            } else {
                (object, selected)
            };
            let input_version = input
                .version_opt()
                .ok_or_else(|| anyhow!("Version missing for causal input object '{object_id}'"))?;
            let output_version = output
                .version_opt()
                .ok_or_else(|| anyhow!("Version missing for causal output object '{object_id}'"))?;
            ensure!(
                output_version > input_version,
                "Causal output object '{object_id}' has version {output_version}, not newer than \
                 input version {input_version}"
            );
            if root_output {
                visible[index] = (object, true);
            }
        }

        let mut values = Vec::new();
        for (object, _) in visible {
            let object_id = Self::parse_object_id(object)?;
            if let Some(value) = self.decode_transaction_dynamic_field(
                object,
                object_id,
                key,
                expected_key_type,
                expected_value_type,
            )? {
                values.push(value);
            }
        }
        Ok(values)
    }

    fn decode_transaction_dynamic_field<K, V>(
        &self,
        object: &sui::grpc::Object,
        object_id: sui::types::Address,
        key: &K,
        expected_key_type: &sui::types::TypeTag,
        expected_value_type: &sui::types::TypeTag,
    ) -> anyhow::Result<Option<TransactionStateOutput<V>>>
    where
        K: DeserializeOwned + Eq + Serialize,
        V: DeserializeOwned,
    {
        let object_type = object
            .object_type_opt()
            .ok_or_else(|| anyhow!("Object type missing for transaction object '{object_id}'"))?
            .parse::<sui::types::StructTag>()
            .map_err(|error| {
                anyhow!("Transaction object '{object_id}' has an invalid type: {error}")
            })?;
        if *object_type.address() != sui::types::Address::from_static("0x2")
            || object_type.module().as_str() != "dynamic_field"
            || object_type.name().as_str() != "Field"
        {
            return Ok(None);
        }
        if object_type.type_params().len() != 2 {
            bail!("Dynamic field '{object_id}' does not have two type arguments");
        }
        if &object_type.type_params()[0] != expected_key_type
            || &object_type.type_params()[1] != expected_value_type
        {
            return Ok(None);
        }

        let (owner, digest, version, _) = self.parse_object_metadata(object_id, object)?;
        let sui::types::Owner::Object(parent_id) = owner else {
            bail!("Dynamic field '{object_id}' is not owned by an object");
        };
        let expected_field_id = derive_dynamic_field_id(parent_id, key, expected_key_type)?;
        if object_id != expected_field_id {
            bail!("Dynamic field '{object_id}' does not derive from parent '{parent_id}'");
        }
        let field = self.parse_object_contents_bcs::<DynamicFieldValue<K, V>>(object)?;
        validate_dynamic_field(object_id, key, &field)?;
        Ok(Some(TransactionStateOutput {
            object_id: parent_id,
            field_ref: sui::types::ObjectReference::new(object_id, version, digest),
            data: field.value,
        }))
    }

    /// Advances one validated object state snapshot from a finalized response.
    ///
    /// Sui returns only objects named by transaction effects: input and output
    /// versions of changed objects. The existing snapshot therefore supplies
    /// unchanged anchor and witness data, while the response must supply the
    /// inner value written by `transaction`. A mentioned object with no output
    /// was removed and cannot be reused. No Sui request is performed.
    pub(crate) fn advance_object_state_snapshot(
        &self,
        basis: &ObjectStateSnapshot,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
    ) -> Result<Option<ObjectStateSnapshot>, ObjectStateSnapshotError> {
        let invalid = |error: anyhow::Error| ObjectStateSnapshotError::Invalid(error.to_string());
        let observed_transaction = executed
            .digest_opt()
            .ok_or_else(|| anyhow!("Finalized transaction response has no digest"))
            .and_then(|digest| {
                digest.parse::<sui::types::Digest>().map_err(|error| {
                    anyhow!("Finalized transaction response has an invalid digest: {error}")
                })
            })
            .map_err(invalid)?;
        if observed_transaction != transaction {
            return Err(ObjectStateSnapshotError::Invalid(format!(
                "Finalized transaction response names '{observed_transaction}', expected \
                 '{transaction}'"
            )));
        }
        let objects = &executed.objects().objects;
        let object_id = basis.object.object_id;
        let witness_field_id = basis.witness.field_id;
        let inner_field_id = basis.inner.field_id;
        let change =
            |id| finalized_transaction_object_change(objects, transaction, id).map_err(invalid);

        let inner_object = match change(inner_field_id)? {
            FinalizedObjectChange::Written(object) => *object,
            FinalizedObjectChange::Unchanged => return Ok(None),
            FinalizedObjectChange::Removed => {
                return Err(reject_removed_state_object(
                    transaction,
                    inner_field_id,
                    "inner field",
                ));
            }
        };
        let inner = Self::parse_state_field_metadata(object_id, inner_field_id, &inner_object)
            .map_err(invalid)?;

        let (object, anchor_object) = match change(object_id)? {
            FinalizedObjectChange::Written(anchor) => {
                let metadata =
                    Self::parse_observed_object_metadata(object_id, &anchor).map_err(invalid)?;
                (metadata, *anchor)
            }
            FinalizedObjectChange::Unchanged => (basis.object.clone(), basis.anchor_object.clone()),
            FinalizedObjectChange::Removed => {
                return Err(reject_removed_state_object(
                    transaction,
                    object_id,
                    "anchor",
                ));
            }
        };
        let witness = match change(witness_field_id)? {
            FinalizedObjectChange::Written(witness) => {
                Self::parse_state_field_metadata(object_id, witness_field_id, &witness)
                    .map_err(invalid)?
            }
            FinalizedObjectChange::Unchanged => basis.witness.clone(),
            FinalizedObjectChange::Removed => {
                return Err(reject_removed_state_object(
                    transaction,
                    witness_field_id,
                    "witness field",
                ));
            }
        };

        Ok(Some(ObjectStateSnapshot {
            object,
            witness,
            inner,
            anchor_object,
            inner_object,
        }))
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

        observe_dynamic_field_list("typed_page");
        let mut client = self.clone_state_catalog_client();
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
        let mut client = self.clone_state_catalog_client();

        loop {
            observe_dynamic_field_list("values");
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
        let mut client = self.clone_state_catalog_client();

        loop {
            observe_dynamic_field_list("object_children");
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
            .ok_or_else(|| anyhow::Error::new(NexusError::ObjectNotFound { object: object_id }))
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
        let mut client = self.clone_state_catalog_client();

        loop {
            observe_dynamic_field_list("typed_values");
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
        let mut client = self.clone_state_catalog_client();

        loop {
            observe_dynamic_field_list("field_ids");
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

/// One typed object state value written by an exact transaction.
///
/// `object_id` is the stable state object which owns the written `Inner`
/// field. `field_ref` identifies the field version written by the transaction.
#[derive(Clone, Debug)]
pub struct TransactionStateOutput<T> {
    pub object_id: sui::types::Address,
    pub field_ref: sui::types::ObjectReference,
    pub data: T,
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

    #[test]
    fn dynamic_field_metadata_reports_unknown_value_type_without_bcs_decode() {
        let parent_id = sui::types::Address::from_static("0x91");
        let field_id = sui::types::Address::from_static("0x92");
        let key_type = "0xa1::object_state::Inner"
            .parse::<sui::types::TypeTag>()
            .unwrap();
        let value_type = "0xf0::future::UnknownInner"
            .parse::<sui::types::TypeTag>()
            .unwrap();
        let field_type = sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![key_type.clone(), value_type.clone()],
        );
        let mut field_object = sui::grpc::Object::default();
        field_object.set_object_type(field_type.to_string());
        let mut invalid_value = sui::grpc::Bcs::default();
        invalid_value.set_value(vec![0xff]);
        let mut field = sui::grpc::DynamicField::default();
        field.set_field_id(field_id.to_string());
        field.set_field_object(field_object);
        field.set_value_type(value_type.to_string());
        field.set_value(invalid_value);

        let metadata = parse_dynamic_field_metadata(parent_id, &field)
            .unwrap()
            .unwrap();

        assert_eq!(metadata.field_id, field_id);
        assert_eq!(metadata.key_type, key_type);
        assert_eq!(metadata.value_type, value_type);
    }

    #[test]
    fn dynamic_field_metadata_rejects_an_incomplete_field_shape() {
        let parent_id = sui::types::Address::from_static("0x91");
        let field_id = sui::types::Address::from_static("0x92");
        let mut field_object = sui::grpc::Object::default();
        field_object.set_object_type("0x2::dynamic_field::Field<u64>".to_owned());
        let mut field = sui::grpc::DynamicField::default();
        field.set_field_id(field_id.to_string());
        field.set_field_object(field_object);

        let error = parse_dynamic_field_metadata(parent_id, &field)
            .unwrap_err()
            .to_string();

        assert!(error.contains("Field<K, V>"));
    }

    #[test]
    fn dynamic_object_field_metadata_is_excluded_from_ordinary_state_metadata() {
        let parent_id = sui::types::Address::from_static("0x91");
        let field_id = sui::types::Address::from_static("0x92");
        let key_type = "0xa1::task::AuthorizationKey"
            .parse::<sui::types::TypeTag>()
            .unwrap();
        let wrapper_type = dynamic_object_field_wrapper_type(&key_type);
        let child_type = "0xb1::authorization::AgentSkillAuthorization"
            .parse::<sui::types::TypeTag>()
            .unwrap();
        let field_type = sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![
                wrapper_type,
                sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                    sui::types::Address::from_static("0x2"),
                    sui::types::Identifier::from_static("object"),
                    sui::types::Identifier::from_static("ID"),
                    vec![],
                ))),
            ],
        );
        let mut field_object = sui::grpc::Object::default();
        field_object.set_object_type(field_type.to_string());
        let mut field = sui::grpc::DynamicField::default();
        field.set_kind(sui::grpc::dynamic_field::DynamicFieldKind::Object);
        field.set_field_id(field_id.to_string());
        field.set_field_object(field_object);
        field.set_value_type(child_type.to_string());

        let metadata = parse_dynamic_field_metadata(parent_id, &field).unwrap();

        assert!(metadata.is_none());
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

        let catalog_rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let live_rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let live = Arc::new(sui::grpc::client(live_rpc_url).expect("live mock client"));
        let catalog = Arc::new(sui::grpc::client(catalog_rpc_url).expect("catalog mock client"));
        let crawler = Crawler::with_state_catalog(live, catalog);

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

    #[tokio::test]
    async fn transaction_output_object_selects_the_written_version() {
        let mut rng = rand::thread_rng();
        let transaction = sui::types::Digest::generate(&mut rng);
        let earlier_transaction = sui::types::Digest::generate(&mut rng);
        let object_id = sui::types::Address::generate(&mut rng);
        let owner = sui::types::Owner::Object(sui::types::Address::generate(&mut rng));
        let input_ref =
            sui::types::ObjectReference::new(object_id, 1, sui::types::Digest::generate(&mut rng));
        let output_ref =
            sui::types::ObjectReference::new(object_id, 2, sui::types::Digest::generate(&mut rng));
        let mut input = object_with_bcs(input_ref, owner, &TestValue { value: 3 });
        input.set_previous_transaction(earlier_transaction);
        let mut output = object_with_bcs(output_ref.clone(), owner, &TestValue { value: 5 });
        output.set_previous_transaction(transaction);

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        ledger_service_mock
            .expect_get_transaction()
            .withf(move |request| {
                request.get_ref().digest_opt() == Some(transaction.to_string().as_str())
                    && request.get_ref().read_mask.as_ref().is_some_and(|mask| {
                        mask.paths
                            .iter()
                            .any(|path| path == "objects.objects.previous_transaction")
                    })
            })
            .times(1)
            .returning(move |_| {
                let mut objects = sui::grpc::ObjectSet::default();
                objects.set_objects(vec![input.clone(), output.clone()]);
                let mut executed = sui::grpc::ExecutedTransaction::default();
                executed.set_digest(transaction);
                executed.set_objects(objects);
                let mut response = sui::grpc::GetTransactionResponse::default();
                response.set_transaction(executed);
                Ok(tonic::Response::new(response))
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::client(rpc_url).expect("mock client");
        let crawler = Crawler::new(Arc::new(client));

        let observed = crawler
            .get_transaction_output_object::<TestValue>(transaction, object_id)
            .await
            .expect("transaction output object loads");

        assert_eq!(observed.object_ref(), output_ref);
        assert_eq!(observed.owner, owner);
        assert_eq!(observed.data, TestValue { value: 5 });
    }

    #[tokio::test]
    async fn transaction_shared_initial_version_requires_exact_type_and_owner() {
        let mut rng = rand::thread_rng();
        let transaction = sui::types::Digest::generate(&mut rng);
        let earlier_transaction = sui::types::Digest::generate(&mut rng);
        let object_id = sui::types::Address::generate(&mut rng);
        let object_type = test_value_tag();
        let input_ref =
            sui::types::ObjectReference::new(object_id, 9, sui::types::Digest::generate(&mut rng));
        let output_ref =
            sui::types::ObjectReference::new(object_id, 10, sui::types::Digest::generate(&mut rng));
        let owner = sui::types::Owner::Shared(7);
        let mut input =
            typed_object_with_bcs(input_ref, owner, &object_type, &TestValue { value: 3 });
        input.set_previous_transaction(earlier_transaction);
        let mut output =
            typed_object_with_bcs(output_ref, owner, &object_type, &TestValue { value: 5 });
        output.set_previous_transaction(transaction);

        let mut executed = sui::grpc::ExecutedTransaction::default();
        executed.set_digest(transaction);
        executed.set_objects(
            sui::grpc::ObjectSet::default().with_objects(vec![input.clone(), output.clone()]),
        );
        let crawler = Crawler::new(Arc::new(
            sui::grpc::client(sui_mocks::grpc::mock_server(Default::default()))
                .expect("mock client builds"),
        ));

        assert_eq!(
            crawler
                .transaction_shared_initial_version(
                    &executed,
                    transaction,
                    object_id,
                    &object_type,
                )
                .expect("causal shared input validates"),
            7,
        );

        output.set_owner(sui::grpc::Owner::from(sui::types::Owner::Address(
            sui::types::Address::generate(&mut rng),
        )));
        executed
            .set_objects(sui::grpc::ObjectSet::default().with_objects(vec![input.clone(), output]));
        assert!(crawler
            .transaction_shared_initial_version(&executed, transaction, object_id, &object_type,)
            .expect_err("address-owned output must be rejected")
            .to_string()
            .contains("is not shared"));

        let wrong_type = sui::types::StructTag::gas_coin();
        assert!(crawler
            .transaction_shared_initial_version(&executed, transaction, object_id, &wrong_type,)
            .expect_err("wrong expected type must be rejected")
            .to_string()
            .contains("expected"));
    }

    #[tokio::test]
    async fn transaction_dynamic_field_outputs_and_causal_values_select_root_output() {
        let mut rng = rand::thread_rng();
        let transaction = sui::types::Digest::generate(&mut rng);
        let earlier_transaction = sui::types::Digest::generate(&mut rng);
        let parent_id = sui::types::Address::generate(&mut rng);
        let wrong_parent = sui::types::Address::generate(&mut rng);
        let key = TestKey {
            name: "inner".to_owned(),
        };
        let key_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0x1"),
            sui::types::Identifier::from_static("state"),
            sui::types::Identifier::from_static("Inner"),
            vec![],
        )));
        let value_type = sui::types::TypeTag::Struct(Box::new(test_value_tag()));
        let field_type = sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![key_type.clone(), value_type.clone()],
        );
        let field_id = derive_dynamic_field_id(parent_id, &key, &key_type).unwrap();
        let input_ref =
            sui::types::ObjectReference::new(field_id, 1, sui::types::Digest::generate(&mut rng));
        let output_ref =
            sui::types::ObjectReference::new(field_id, 2, sui::types::Digest::generate(&mut rng));
        let mut input = typed_object_with_bcs(
            input_ref,
            sui::types::Owner::Object(parent_id),
            &field_type,
            &DynamicFieldValue {
                id: field_id,
                name: key.clone(),
                value: TestValue { value: 3 },
            },
        );
        input.set_previous_transaction(earlier_transaction);
        let inherited_input = input.clone();
        let mut output = typed_object_with_bcs(
            output_ref.clone(),
            sui::types::Owner::Object(parent_id),
            &field_type,
            &DynamicFieldValue {
                id: field_id,
                name: key.clone(),
                value: TestValue { value: 5 },
            },
        );
        output.set_previous_transaction(transaction);

        let mut executed = sui::grpc::ExecutedTransaction::default();
        executed.set_digest(transaction);
        let mut objects = sui::grpc::ObjectSet::default();
        objects.set_objects(vec![input, output.clone()]);
        executed.set_objects(objects);
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let crawler = Crawler::new(Arc::new(
            sui::grpc::client(rpc_url).expect("mock client builds"),
        ));

        let observed = crawler
            .transaction_dynamic_field_outputs::<TestKey, TestValue>(
                &executed,
                transaction,
                &key,
                &key_type,
                &value_type,
            )
            .expect("exact output decodes");
        assert_eq!(observed.len(), 1);
        assert_eq!(observed[0].object_id, parent_id);
        assert_eq!(observed[0].field_ref, output_ref);
        assert_eq!(observed[0].data, TestValue { value: 5 });

        let causal = crawler
            .causal_dynamic_field_values::<TestKey, TestValue>(
                &executed,
                transaction,
                &key,
                &key_type,
                &value_type,
            )
            .expect("the root output is the visible causal value");
        assert_eq!(causal.len(), 1);
        assert_eq!(causal[0].object_id, parent_id);
        assert_eq!(causal[0].field_ref, output_ref);
        assert_eq!(causal[0].data, TestValue { value: 5 });

        let mut inherited_output = output.clone();
        inherited_output.set_previous_transaction(earlier_transaction);
        executed.set_objects(
            sui::grpc::ObjectSet::default().with_objects(vec![inherited_input, inherited_output]),
        );
        let error = crawler
            .causal_dynamic_field_values::<TestKey, TestValue>(
                &executed,
                transaction,
                &key,
                &key_type,
                &value_type,
            )
            .expect_err("two inherited versions have no unique visible value");
        assert!(error.to_string().contains("ambiguous versions"));

        output.set_owner(sui::grpc::Owner::from(sui::types::Owner::Object(
            wrong_parent,
        )));
        let mut objects = sui::grpc::ObjectSet::default();
        objects.set_objects(vec![output]);
        executed.set_objects(objects);
        let error = crawler
            .transaction_dynamic_field_outputs::<TestKey, TestValue>(
                &executed,
                transaction,
                &key,
                &key_type,
                &value_type,
            )
            .expect_err("wrong parent cannot validate the deterministic field ID");
        assert!(error.to_string().contains("does not derive from parent"));
    }

    #[tokio::test]
    async fn causal_dynamic_field_values_include_inherited_state() {
        let mut rng = rand::thread_rng();
        let transaction = sui::types::Digest::generate(&mut rng);
        let ancestor = sui::types::Digest::generate(&mut rng);
        let parents = [
            sui::types::Address::generate(&mut rng),
            sui::types::Address::generate(&mut rng),
        ];
        let key = TestKey {
            name: "inner".to_owned(),
        };
        let key_type = sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
            sui::types::Address::from_static("0x1"),
            sui::types::Identifier::from_static("state"),
            sui::types::Identifier::from_static("Inner"),
            vec![],
        )));
        let value_type = sui::types::TypeTag::Struct(Box::new(test_value_tag()));
        let field_type = sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![key_type.clone(), value_type.clone()],
        );
        let objects = parents
            .iter()
            .copied()
            .zip([ancestor, transaction])
            .zip([3, 5])
            .map(|((parent, previous), value)| {
                let field_id = derive_dynamic_field_id(parent, &key, &key_type).unwrap();
                let object_ref = sui::types::ObjectReference::new(
                    field_id,
                    1,
                    sui::types::Digest::generate(&mut rng),
                );
                let mut object = typed_object_with_bcs(
                    object_ref,
                    sui::types::Owner::Object(parent),
                    &field_type,
                    &DynamicFieldValue {
                        id: field_id,
                        name: key.clone(),
                        value: TestValue { value },
                    },
                );
                object.set_previous_transaction(previous);
                object
            })
            .collect::<Vec<_>>();
        let mut executed = sui::grpc::ExecutedTransaction::default();
        executed.set_digest(transaction);
        executed.set_objects(sui::grpc::ObjectSet::default().with_objects(objects));
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let crawler = Crawler::new(Arc::new(
            sui::grpc::client(rpc_url).expect("mock client builds"),
        ));

        let values = crawler
            .causal_dynamic_field_values::<TestKey, TestValue>(
                &executed,
                transaction,
                &key,
                &key_type,
                &value_type,
            )
            .expect("complete causal view decodes");
        assert_eq!(
            values
                .iter()
                .map(|value| (value.object_id, value.data.value))
                .collect::<Vec<_>>(),
            vec![(parents[0], 3), (parents[1], 5)]
        );
    }
}
