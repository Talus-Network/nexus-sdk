//! Live object state and package authority resolution.

use {
    crate::{
        move_bindings::{
            move_std::{option::Option as MoveOption, type_name::TypeName},
            sui_framework::object::{ID, UID},
        },
        nexus::{
            crawler::{
                derive_dynamic_field_id,
                Crawler,
                DynamicFieldMetadata,
                ObjectMetadata,
                ObjectStateSnapshot,
                ObjectStateSnapshotError,
                TransactionStateOutput,
            },
            error::{ClientUpgradeRequired, NexusError},
        },
        sui::{self, traits::FieldMaskUtil as _},
        types::{
            NexusContext,
            NexusObjects,
            NexusPackages,
            PackageLink,
            PackageLinkage,
            PackageRole,
            PackageVersion,
            SharedRoot,
            TypeOrigins,
        },
    },
    futures::future::try_join_all,
    serde::{de::DeserializeOwned, Deserialize},
    std::{collections::HashMap, sync::Arc, time::Duration},
    talus_sui_move::MoveStruct,
    tokio::{
        sync::{Mutex, RwLock},
        time::sleep,
    },
};

lazy_static::lazy_static! {
    static ref STATE_OBSERVATIONS: prometheus::CounterVec = prometheus::register_counter_vec!(
        "nexus_state_observations_total",
        "State observations resolved through a known key lineage or field discovery",
        &["source"],
    )
    .unwrap();
    static ref STATE_OBJECT_OBSERVATIONS: prometheus::CounterVec =
        prometheus::register_counter_vec!(
            "nexus_state_object_observations_total",
            "Successful state observations grouped by read shape and anchor type",
            &["mode", "module", "name"],
        )
        .unwrap();
}

fn observe_state_object(mode: &'static str, state: &ObservedState) {
    STATE_OBJECT_OBSERVATIONS
        .with_label_values(&[
            mode,
            state.anchor_type.module().as_str(),
            state.anchor_type.name().as_str(),
        ])
        .inc();
}

const STATE_OBSERVATION_ATTEMPTS: usize = 20;
const STATE_OBSERVATION_RETRY_DELAY: Duration = Duration::from_millis(50);

#[derive(Deserialize)]
struct RuntimeAuthorityState {
    id: UID,
    scheduler_upgrade_cap: MoveOption<ID>,
    current_runtime: MoveOption<TypeName>,
    current_runtime_package: MoveOption<ID>,
    paused: bool,
}

/// One typed dynamic field selected from an object anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedStateField {
    /// Object ID of the dynamic field wrapper.
    pub field_id: sui::types::Address,
    /// Exact Move type stored as the field value.
    pub value_type: sui::types::StructTag,
}

/// Package witness and stored layout observed on one live object.
///
/// Observation records exact type identity without decoding either field
/// value. A later compatibility adapter decides whether this SDK understands
/// the pair.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedState {
    /// Stable Sui object ID.
    pub object_id: sui::types::Address,
    /// Current object owner.
    pub owner: sui::types::Owner,
    /// Exact Move type of the stable anchor.
    pub anchor_type: sui::types::StructTag,
    /// Field containing the package witness.
    pub witness: ObservedStateField,
    /// Field containing the stored layout.
    pub inner: ObservedStateField,
}

/// One exact state observation with its anchor and inner bytes.
///
/// The bytes come from the same [`sui::grpc::BatchGetObjectsRequest`] as the
/// type metadata. They are decoded only after the complete state pair is
/// validated against a [`NexusContext`].
#[derive(Clone, Debug)]
pub struct ObservedStateSnapshot {
    observed: ObservedState,
    snapshot: ObjectStateSnapshot,
}

impl ObservedStateSnapshot {
    /// Returns the exact type and ownership observation for this snapshot.
    pub const fn observed(&self) -> &ObservedState {
        &self.observed
    }

    /// Returns the exact object reference for the mutable inner state value.
    ///
    /// A transaction derived from this snapshot reads the inner dynamic field.
    /// The returned reference therefore identifies the concrete state version
    /// that a connected Sui node must expose before it can evaluate a causal
    /// transaction against the snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidObjectState`] when the snapshot is missing
    /// the inner object version or digest, or its object identity changed.
    pub fn inner_object_reference(&self) -> Result<sui::types::ObjectReference, NexusError> {
        self.snapshot
            .inner_object_reference()
            .map_err(|error| NexusError::InvalidObjectState {
                object: self.observed.object_id,
                reason: error.to_string(),
            })
    }
}

impl ObservedState {
    /// Returns the exact package witness type.
    pub const fn witness_type(&self) -> &sui::types::StructTag {
        &self.witness.value_type
    }

    /// Returns the exact stored inner type.
    pub const fn inner_type(&self) -> &sui::types::StructTag {
        &self.inner.value_type
    }
}

/// Resolves live object state without caching mutable object data.
#[derive(Clone)]
pub struct StateResolver {
    crawler: Arc<Crawler>,
    catalog: Arc<Crawler>,
    package_cache: Arc<RwLock<HashMap<sui::types::Address, Arc<PackageVersion>>>>,
    state_field_keys: Arc<RwLock<Vec<StateFieldKeyTypes>>>,
    state_field_discovery: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StateFieldKeyTypes {
    witness: sui::types::TypeTag,
    inner: sui::types::TypeTag,
}

impl StateResolver {
    /// Creates a resolver backed by `crawler`.
    pub fn new(crawler: Arc<Crawler>) -> Self {
        Self::with_catalog(Arc::clone(&crawler), crawler)
    }

    /// Creates a resolver that reads current objects from `crawler` and uses
    /// `catalog` only to discover dynamic field key types.
    ///
    /// Sui nodes without indexes can serve exact objects, simulation, and
    /// submission but cannot enumerate dynamic fields. Once a key type is
    /// discovered, the field ID is derived and every state observation comes
    /// from `crawler`.
    pub fn with_catalog(crawler: Arc<Crawler>, catalog: Arc<Crawler>) -> Self {
        Self {
            crawler,
            catalog,
            package_cache: Arc::default(),
            state_field_keys: Arc::default(),
            state_field_discovery: Arc::default(),
        }
    }

    /// Observes the exact anchor, witness, and inner types for `object_id`.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidObjectState`] when the anchor does not have
    /// exactly one `Inner` field and one `Witness` field, or either value is not
    /// a Move struct. A definitive absence returns
    /// [`NexusError::ObjectNotFound`]; other transport failures return
    /// [`NexusError::Rpc`].
    pub async fn observe(
        &self,
        object_id: sui::types::Address,
    ) -> Result<ObservedState, NexusError> {
        for attempt in 0..STATE_OBSERVATION_ATTEMPTS {
            match self.observe_once(object_id).await {
                Ok(observed) => {
                    observe_state_object("metadata", &observed);
                    return Ok(observed);
                }
                Err(NexusError::Rpc(_)) if attempt + 1 < STATE_OBSERVATION_ATTEMPTS => {
                    sleep(STATE_OBSERVATION_RETRY_DELAY).await;
                }
                Err(NexusError::InvalidObjectState { reason, .. })
                    if state_observation_may_be_incomplete(&reason)
                        && attempt + 1 < STATE_OBSERVATION_ATTEMPTS =>
                {
                    sleep(STATE_OBSERVATION_RETRY_DELAY).await;
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("the observation loop returns on its final attempt")
    }

    /// Observes one object and retains its current anchor and inner bytes.
    ///
    /// Known state field IDs are derived and read together with the anchor in
    /// one batch. An unknown key lineage is enumerated once, then retried by
    /// exact field ID. No mutable state survives outside this resolver.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::observe`]. Missing or incoherent
    /// state fields are retried before an error is returned.
    pub async fn observe_snapshot(
        &self,
        object_id: sui::types::Address,
    ) -> Result<ObservedStateSnapshot, NexusError> {
        for attempt in 0..STATE_OBSERVATION_ATTEMPTS {
            match self.observe_snapshot_once(object_id).await {
                Ok(snapshot) => {
                    observe_state_object("snapshot", snapshot.observed());
                    return Ok(snapshot);
                }
                Err(NexusError::Rpc(_)) if attempt + 1 < STATE_OBSERVATION_ATTEMPTS => {
                    sleep(STATE_OBSERVATION_RETRY_DELAY).await;
                }
                Err(NexusError::InvalidObjectState { reason, .. })
                    if state_observation_may_be_incomplete(&reason)
                        && attempt + 1 < STATE_OBSERVATION_ATTEMPTS =>
                {
                    sleep(STATE_OBSERVATION_RETRY_DELAY).await;
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("the snapshot observation loop returns on its final attempt")
    }

    /// Advances a validated state snapshot from one finalized transaction.
    ///
    /// The existing snapshot supplies the exact state field lineage already
    /// validated before submission. The finalized response must contain an
    /// inner value written by `transaction`; unchanged anchor and witness data
    /// remain valid because Sui omits objects not named by transaction effects.
    /// The method returns [`None`] when the transaction did not write that
    /// inner field, allowing callers to use the ordinary Sui read path. No Sui
    /// request is performed.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidObjectState`] when response objects have
    /// inconsistent identities, ownership, or field types.
    pub fn observe_finalized_snapshot(
        &self,
        basis: &ObservedStateSnapshot,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
    ) -> Result<Option<ObservedStateSnapshot>, NexusError> {
        let object_id = basis.observed.object_id;
        let Some(snapshot) = self
            .crawler
            .advance_object_state_snapshot(&basis.snapshot, executed, transaction)
            .map_err(|error| match error {
                ObjectStateSnapshotError::Rpc(error) => NexusError::from_rpc(error),
                ObjectStateSnapshotError::Invalid(reason) => NexusError::InvalidObjectState {
                    object: object_id,
                    reason,
                },
            })?
        else {
            return Ok(None);
        };
        if snapshot.witness != basis.snapshot.witness || snapshot.inner != basis.snapshot.inner {
            return Err(NexusError::InvalidObjectState {
                object: object_id,
                reason: "finalized state fields changed their validated identity or type"
                    .to_owned(),
            });
        }
        let observed = observed_state_from_metadata(
            snapshot.object.clone(),
            vec![snapshot.witness.clone(), snapshot.inner.clone()],
        )
        .map_err(|reason| NexusError::InvalidObjectState {
            object: object_id,
            reason,
        })?;
        if observed.anchor_type != basis.observed.anchor_type
            || observed.witness.value_type != basis.observed.witness.value_type
            || observed.inner.value_type != basis.observed.inner.value_type
        {
            return Err(NexusError::InvalidObjectState {
                object: object_id,
                reason: "finalized state changed its validated anchor or value type".to_owned(),
            });
        }
        observe_state_object("finalized", &observed);
        Ok(Some(ObservedStateSnapshot { observed, snapshot }))
    }

    async fn observe_snapshot_once(
        &self,
        object_id: sui::types::Address,
    ) -> Result<ObservedStateSnapshot, NexusError> {
        if let Some(snapshot) = self.snapshot_with_known_keys(object_id).await? {
            STATE_OBSERVATIONS
                .with_label_values(&["known_snapshot"])
                .inc();
            return Ok(snapshot);
        }

        // Discovering metadata records the exact key lineage. The following
        // read uses only derived IDs and is the sole extra request paid by a
        // previously unseen lineage.
        self.observe_once(object_id).await?;
        self.snapshot_with_known_keys(object_id)
            .await?
            .ok_or_else(|| NexusError::InvalidObjectState {
                object: object_id,
                reason: "missing object_state::Witness or Inner field after discovery".to_owned(),
            })
    }

    async fn observe_once(
        &self,
        object_id: sui::types::Address,
    ) -> Result<ObservedState, NexusError> {
        if let Some(observed) = self.observe_with_known_keys(object_id).await? {
            STATE_OBSERVATIONS.with_label_values(&["known"]).inc();
            return Ok(observed);
        }

        // Only an unknown key lineage enters this boundary. Once one caller
        // discovers it, waiting callers retry exact derived IDs without
        // repeating an enumeration.
        let _discovery = self.state_field_discovery.lock().await;
        if let Some(observed) = self.observe_with_known_keys(object_id).await? {
            STATE_OBSERVATIONS.with_label_values(&["known"]).inc();
            return Ok(observed);
        }

        STATE_OBSERVATIONS.with_label_values(&["discovery"]).inc();

        let (object, fields) = tokio::join!(
            self.crawler.observe_object_metadata(object_id),
            self.catalog.get_dynamic_field_metadata(object_id),
        );
        let fields = match (object, fields) {
            (Err(error), _) => return Err(NexusError::from_rpc(error)),
            (_, Err(error)) => return Err(NexusError::from_rpc(error)),
            (Ok(_), Ok(fields)) => fields,
        };
        let key_types =
            state_field_key_types(&fields).map_err(|reason| NexusError::InvalidObjectState {
                object: object_id,
                reason,
            })?;
        let mut known = self.state_field_keys.write().await;
        if !known.contains(&key_types) {
            known.push(key_types);
        }
        drop(known);

        self.observe_with_known_keys(object_id)
            .await?
            .ok_or_else(|| NexusError::InvalidObjectState {
                object: object_id,
                reason: "missing object_state::Witness or Inner field after discovery".to_owned(),
            })
    }

    async fn observe_with_known_keys(
        &self,
        object_id: sui::types::Address,
    ) -> Result<Option<ObservedState>, NexusError> {
        use crate::move_bindings::primitives::object_state::{Inner, Witness};

        let known = self.state_field_keys.read().await.clone();
        for keys in known {
            let witness_id =
                derive_dynamic_field_id(object_id, &Witness::new(false), &keys.witness)
                    .map_err(NexusError::Rpc)?;
            let inner_id = derive_dynamic_field_id(object_id, &Inner::new(false), &keys.inner)
                .map_err(NexusError::Rpc)?;
            let Some((object, fields)) = self
                .crawler
                .observe_object_state_fields(object_id, witness_id, inner_id)
                .await
                .map_err(NexusError::from_rpc)?
            else {
                continue;
            };
            if fields[0].key_type != keys.witness || fields[1].key_type != keys.inner {
                return Err(NexusError::InvalidObjectState {
                    object: object_id,
                    reason: "derived state field returned a different key type".to_owned(),
                });
            }
            let observed = observed_state_from_metadata(object, fields).map_err(|reason| {
                NexusError::InvalidObjectState {
                    object: object_id,
                    reason,
                }
            })?;
            return Ok(Some(observed));
        }
        Ok(None)
    }

    async fn snapshot_with_known_keys(
        &self,
        object_id: sui::types::Address,
    ) -> Result<Option<ObservedStateSnapshot>, NexusError> {
        use crate::move_bindings::primitives::object_state::{Inner, Witness};

        let known = self.state_field_keys.read().await.clone();
        for keys in known {
            let witness_id =
                derive_dynamic_field_id(object_id, &Witness::new(false), &keys.witness)
                    .map_err(NexusError::Rpc)?;
            let inner_id = derive_dynamic_field_id(object_id, &Inner::new(false), &keys.inner)
                .map_err(NexusError::Rpc)?;
            let Some(snapshot) = self
                .crawler
                .object_state_snapshot(object_id, witness_id, inner_id)
                .await
                .map_err(|error| match error {
                    ObjectStateSnapshotError::Rpc(error) => NexusError::from_rpc(error),
                    ObjectStateSnapshotError::Invalid(reason) => NexusError::InvalidObjectState {
                        object: object_id,
                        reason,
                    },
                })?
            else {
                continue;
            };
            if snapshot.witness.key_type != keys.witness || snapshot.inner.key_type != keys.inner {
                return Err(NexusError::InvalidObjectState {
                    object: object_id,
                    reason: "derived state field returned a different key type".to_owned(),
                });
            }
            let observed = observed_state_from_metadata(
                snapshot.object.clone(),
                vec![snapshot.witness.clone(), snapshot.inner.clone()],
            )
            .map_err(|reason| NexusError::InvalidObjectState {
                object: object_id,
                reason,
            })?;
            return Ok(Some(ObservedStateSnapshot { observed, snapshot }));
        }
        Ok(None)
    }

    /// Resolves one operation context from the state stored at `object_id`.
    ///
    /// The returned [`ObservedState`] is the exact observation that selected
    /// the package graph. Keeping both values together lets callers report the
    /// selected witness and inner types without observing the object again.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::observe`] and
    /// [`Self::resolve_package_graph`].
    pub async fn resolve_context(
        &self,
        objects: Arc<NexusObjects>,
        object_id: sui::types::Address,
    ) -> Result<(ObservedState, NexusContext), NexusError> {
        let state = self.observe(object_id).await?;
        let packages = self.resolve_package_graph(&state).await?;
        Ok((state, NexusContext::new(objects, packages)))
    }

    /// Resolves one operation context and retains the source object's bytes.
    ///
    /// This is the read shape for a reconciler that must both select package
    /// authority and inspect the source value. The common path performs one
    /// object batch request rather than observing and loading in two rounds.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::observe_snapshot`] and
    /// [`Self::resolve_package_graph`].
    pub async fn resolve_context_snapshot(
        &self,
        objects: Arc<NexusObjects>,
        object_id: sui::types::Address,
    ) -> Result<(ObservedStateSnapshot, NexusContext), NexusError> {
        let snapshot = self.observe_snapshot(object_id).await?;
        let packages = self.resolve_package_graph(snapshot.observed()).await?;
        Ok((snapshot, NexusContext::new(objects, packages)))
    }

    /// Resolve object authority from one verified causal transaction view.
    ///
    /// Unlike a canonical object read, this path can represent a newly created
    /// object or several finalized transitions which have not reached a
    /// checkpoint. The caller must obtain `executed` from an acknowledged
    /// causal finality stream. Missing state key lineage returns `None` so the
    /// complete checkpoint feed can recover without weakening validation.
    pub async fn resolve_transaction_view_context_snapshot(
        &self,
        objects: Arc<NexusObjects>,
        object_id: sui::types::Address,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
    ) -> Result<Option<(ObservedStateSnapshot, NexusContext)>, NexusError> {
        use crate::move_bindings::primitives::object_state::{Inner, Witness};

        let known = self.state_field_keys.read().await.clone();
        for keys in known {
            let witness_id =
                derive_dynamic_field_id(object_id, &Witness::new(false), &keys.witness)
                    .map_err(NexusError::Rpc)?;
            let inner_id = derive_dynamic_field_id(object_id, &Inner::new(false), &keys.inner)
                .map_err(NexusError::Rpc)?;
            let Some(snapshot) = self
                .crawler
                .object_state_snapshot_from_transaction_view(
                    object_id,
                    witness_id,
                    inner_id,
                    executed,
                    transaction,
                )
                .map_err(|error| match error {
                    ObjectStateSnapshotError::Rpc(error) => NexusError::from_rpc(error),
                    ObjectStateSnapshotError::Invalid(reason) => NexusError::InvalidObjectState {
                        object: object_id,
                        reason,
                    },
                })?
            else {
                continue;
            };
            if snapshot.witness.key_type != keys.witness || snapshot.inner.key_type != keys.inner {
                return Err(NexusError::InvalidObjectState {
                    object: object_id,
                    reason: "causal state fields use an unexpected key type".to_owned(),
                });
            }
            let observed = observed_state_from_metadata(
                snapshot.object.clone(),
                vec![snapshot.witness.clone(), snapshot.inner.clone()],
            )
            .map_err(|reason| NexusError::InvalidObjectState {
                object: object_id,
                reason,
            })?;
            let snapshot = ObservedStateSnapshot { observed, snapshot };
            let packages = self.resolve_package_graph(snapshot.observed()).await?;
            observe_state_object("transaction_view", snapshot.observed());
            return Ok(Some((snapshot, NexusContext::new(objects, packages))));
        }
        Ok(None)
    }

    /// Resolves one source object and validates every canonical root used by
    /// the same operation.
    ///
    /// The source witness selects the operation package graph. Each required
    /// root must independently select the same package version for its role.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::IncompatiblePackage`] when a root selects a
    /// package outside the source graph. Other observation and resolver errors
    /// are returned unchanged.
    pub async fn resolve_context_with_roots(
        &self,
        objects: Arc<NexusObjects>,
        object_id: sui::types::Address,
        required_roots: &[SharedRoot],
    ) -> Result<(ObservedState, NexusContext), NexusError> {
        let state = self.observe(object_id).await?;
        let packages = self.resolve_package_graph(&state).await?;
        self.validate_required_roots(&packages, object_id, required_roots)
            .await?;
        Ok((state, NexusContext::new(objects, packages)))
    }

    /// Resolves one object context, validates every required root, and decodes
    /// the object's inner value from the same observation.
    ///
    /// Root validation and value decoding run concurrently because neither
    /// operation depends on the other. The anchor, witness, and inner types
    /// must still match `A`, `W`, and `V` before value bytes are decoded.
    ///
    /// # Errors
    ///
    /// Returns compatibility, object state, or RPC errors from context
    /// resolution, required root validation, or inner value decoding.
    pub async fn resolve_inner_with_roots<A, W, V>(
        &self,
        objects: Arc<NexusObjects>,
        object_id: sui::types::Address,
        required_roots: &[SharedRoot],
    ) -> Result<(NexusContext, crate::nexus::crawler::Response<V>), NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        W: MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        let snapshot = self.observe_snapshot(object_id).await?;
        let packages = self.resolve_package_graph(snapshot.observed()).await?;
        let context = NexusContext::new(objects, packages);
        Self::validate_observation::<A, W, V>(snapshot.observed(), &context)?;
        let object = self.decode_snapshot::<A, V>(&snapshot)?;
        self.validate_required_roots(context.packages(), object_id, required_roots)
            .await?;
        Ok((context, object))
    }

    /// Resolves the immutable package dependency graph selected by `state`.
    ///
    /// The complete witness and inner pair must match a supported adapter. The
    /// witness package and every Nexus package in its immutable linkage table
    /// are then validated and cached by exact storage ID.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] for an unknown state pair,
    /// [`NexusError::IncompatiblePackage`] for inconsistent package metadata,
    /// and [`NexusError::Rpc`] for transport failures.
    pub async fn resolve_package_graph(
        &self,
        state: &ObservedState,
    ) -> Result<NexusPackages, NexusError> {
        let adapter = StateAdapter::for_observed(state)?;
        let witness_package_id = *state.witness_type().address();
        let source = self.resolve_package(witness_package_id).await?;
        let source_role = package_role(&source).ok_or_else(|| NexusError::IncompatiblePackage {
            package: witness_package_id,
            reason: "package does not have a supported Nexus ABI".to_owned(),
        })?;
        if source_role != adapter.role {
            return Err(NexusError::IncompatiblePackage {
                package: witness_package_id,
                reason: format!(
                    "object anchor selects role '{}', but the witness package has role '{}'",
                    adapter.role.as_str(),
                    source_role.as_str()
                ),
            });
        }
        validate_state_type_origins(state, &source)?;

        self.resolve_package_graph_from_source(Some(source_role), source)
            .await
    }

    /// Resolves the immutable dependency graph for an explicit package.
    ///
    /// `role` declares the required Nexus package family. The package is
    /// rejected when its complete supported ABI identifies another role.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::IncompatiblePackage`] when the package ABI,
    /// lineage, version, or linkage is inconsistent. Transport failures return
    /// [`NexusError::Rpc`].
    pub async fn resolve_explicit_package_graph(
        &self,
        storage_id: sui::types::Address,
        role: PackageRole,
    ) -> Result<NexusPackages, NexusError> {
        let source = self.resolve_package(storage_id).await?;
        let observed_role =
            package_role(&source).ok_or_else(|| NexusError::IncompatiblePackage {
                package: storage_id,
                reason: "package does not have a supported Nexus ABI".to_owned(),
            })?;
        if observed_role != role {
            return Err(NexusError::IncompatiblePackage {
                package: storage_id,
                reason: format!(
                    "package has role '{}', expected '{}'",
                    observed_role.as_str(),
                    role.as_str()
                ),
            });
        }

        self.resolve_package_graph_from_source(Some(role), source)
            .await
    }

    /// Resolves the Nexus dependency graph recorded by an event emitter.
    ///
    /// A Nexus package contributes itself to the graph. An application package
    /// contributes only the Nexus packages in its immutable linkage table.
    /// This permits historical events to use the dependencies selected by the
    /// code that emitted them without granting that code live object authority.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::IncompatiblePackage`] when the emitter has no
    /// supported Nexus dependency graph or its package metadata is
    /// inconsistent. Transport failures return [`NexusError::Rpc`].
    pub async fn resolve_emitter_context(
        &self,
        objects: Arc<NexusObjects>,
        emitter_package: sui::types::Address,
    ) -> Result<NexusContext, NexusError> {
        let source = self.resolve_package(emitter_package).await?;
        let source_role = package_role(&source);
        let packages = self
            .resolve_package_graph_from_source(source_role, Arc::clone(&source))
            .await?;
        if packages.all().next().is_none() {
            return Err(NexusError::IncompatiblePackage {
                package: emitter_package,
                reason: "event emitter has no supported Nexus dependency".to_owned(),
            });
        }

        Ok(NexusContext::new(objects, packages))
    }

    /// Resolves an explicit creator package and validates every root it uses.
    ///
    /// Each root witness independently selects accepted package authority. The
    /// creator linkage must select that exact package version for every root
    /// role before a transaction can be constructed.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::IncompatiblePackage`] when the creator graph and a
    /// root witness select different package versions. Other failures are
    /// returned by [`Self::observe`] and [`Self::resolve_explicit_package_graph`].
    pub async fn resolve_creator_context(
        &self,
        objects: Arc<NexusObjects>,
        creator_package: sui::types::Address,
        creator_role: PackageRole,
        required_roots: &[SharedRoot],
    ) -> Result<NexusContext, NexusError> {
        let packages = self
            .resolve_explicit_package_graph(creator_package, creator_role)
            .await?;
        self.validate_required_roots(&packages, creator_package, required_roots)
            .await?;

        Ok(NexusContext::new(objects, packages))
    }

    /// Resolves the preferred package graph selected by the fixed runtime authority.
    ///
    /// Routing remains available while the runtime is paused because proposal
    /// construction does not grant protocol effect authority. Onchain runtime
    /// authorization remains the only authority check.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidObjectState`] when the configured root is
    /// malformed or unbound. Package and root compatibility errors are
    /// returned by the normal package resolver.
    pub async fn resolve_routing_context(
        &self,
        objects: Arc<NexusObjects>,
        required_roots: &[SharedRoot],
    ) -> Result<NexusContext, NexusError> {
        self.resolve_bound_runtime_context(objects, required_roots, false)
            .await
    }

    /// Resolves the package graph selected by the fixed runtime authority.
    ///
    /// Effect builders must use this context instead of deriving authority
    /// from the Task, execution, or leader that happens to be an input. The
    /// root is the sole mutable selector for code allowed to produce protocol
    /// effects.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidObjectState`] when the configured root is
    /// malformed, unbound, paused, or does not have its configured stable
    /// identity. Package and root compatibility errors are returned by the
    /// normal package resolver.
    pub async fn resolve_runtime_context(
        &self,
        objects: Arc<NexusObjects>,
        required_roots: &[SharedRoot],
    ) -> Result<NexusContext, NexusError> {
        self.resolve_bound_runtime_context(objects, required_roots, true)
            .await
    }

    async fn resolve_bound_runtime_context(
        &self,
        objects: Arc<NexusObjects>,
        required_roots: &[SharedRoot],
        require_active: bool,
    ) -> Result<NexusContext, NexusError> {
        let root = objects.runtime_authority;
        let observed = self
            .crawler
            .get_object::<RuntimeAuthorityState>(root.object_id())
            .await
            .map_err(NexusError::from_rpc)?;
        let invalid = |reason: String| NexusError::InvalidObjectState {
            object: root.object_id(),
            reason,
        };

        if observed.data.id.address() != root.object_id() {
            return Err(invalid(
                "RuntimeAuthority UID does not match its object ID".to_owned(),
            ));
        }
        if observed.get_initial_version() != root.initial_shared_version {
            return Err(invalid(format!(
                "RuntimeAuthority was first shared at version {}, expected {}",
                observed.get_initial_version(),
                root.initial_shared_version
            )));
        }
        if observed.data.scheduler_upgrade_cap.as_option().is_none() {
            return Err(invalid(
                "RuntimeAuthority is not bound to a Scheduler lineage".to_owned(),
            ));
        }
        if observed.data.current_runtime.as_option().is_none() {
            return Err(invalid(
                "RuntimeAuthority has no current runtime type".to_owned(),
            ));
        }
        let runtime_package = observed
            .data
            .current_runtime_package
            .as_option()
            .map(ID::address)
            .ok_or_else(|| invalid("RuntimeAuthority has no current runtime package".to_owned()))?;
        if require_active && observed.data.paused {
            return Err(invalid(format!(
                "runtime package '{runtime_package}' is paused"
            )));
        }

        let packages = self
            .resolve_explicit_package_graph(runtime_package, PackageRole::Scheduler)
            .await?;
        self.validate_required_roots(&packages, runtime_package, required_roots)
            .await?;
        Ok(NexusContext::new(objects, packages))
    }

    async fn validate_required_roots(
        &self,
        packages: &NexusPackages,
        source: sui::types::Address,
        required_roots: &[SharedRoot],
    ) -> Result<(), NexusError> {
        try_join_all(required_roots.iter().map(|root| async move {
            let state = self.observe(root.object_id()).await?;
            let adapter = StateAdapter::for_observed(&state)?;
            let accepted_graph = self.resolve_package_graph(&state).await?;
            let accepted = accepted_graph.get(adapter.role).ok_or_else(|| {
                NexusError::IncompatiblePackage {
                    package: *state.witness_type().address(),
                    reason: format!(
                        "root '{}' resolved without its '{}' package role",
                        root.object_id(),
                        adapter.role.as_str()
                    ),
                }
            })?;
            let Some(linked) = packages.get(adapter.role) else {
                return Err(NexusError::IncompatiblePackage {
                    package: source,
                    reason: format!(
                        "source graph does not contain the '{}' role required by root '{}'",
                        adapter.role.as_str(),
                        root.object_id()
                    ),
                });
            };
            if linked.storage_id != accepted.storage_id || linked.version != accepted.version {
                return Err(NexusError::IncompatiblePackage {
                    package: source,
                    reason: format!(
                        "source graph selects the '{}' role as '{}' at version {}, but root '{}' \
                         accepts '{}' at version {}",
                        adapter.role.as_str(),
                        linked.storage_id,
                        linked.version,
                        root.object_id(),
                        accepted.storage_id,
                        accepted.version
                    ),
                });
            }
            Ok(())
        }))
        .await?;
        Ok(())
    }

    async fn resolve_package_graph_from_source(
        &self,
        source_role: Option<PackageRole>,
        source: Arc<PackageVersion>,
    ) -> Result<NexusPackages, NexusError> {
        let mut graph = NexusPackages::default();
        if let Some(source_role) = source_role {
            graph.insert(source_role, source.as_ref().clone());
        }
        let linked_packages = try_join_all(source.linkage.iter().filter_map(|(lineage, link)| {
            if link.storage_id == sui::types::Address::from_static("0x1")
                || link.storage_id == sui::types::Address::from_static("0x2")
            {
                return None;
            }
            Some(async move {
                let linked = self.resolve_package(link.storage_id).await?;
                Ok::<_, NexusError>((lineage, link, linked))
            })
        }))
        .await?;
        for (lineage, link, linked) in linked_packages {
            if linked.initial_id != *lineage || linked.version != link.version {
                return Err(NexusError::IncompatiblePackage {
                    package: source.storage_id,
                    reason: format!(
                        "linkage for lineage '{lineage}' selects '{}' at version {}, but the \
                         package object reports lineage '{}' at version {}",
                        link.storage_id, link.version, linked.initial_id, linked.version
                    ),
                });
            }
            let Some(role) = package_role(&linked) else {
                continue;
            };
            if let Some(previous) = graph.insert(role, linked.as_ref().clone()) {
                if previous.storage_id != linked.storage_id {
                    return Err(NexusError::IncompatiblePackage {
                        package: source.storage_id,
                        reason: format!(
                            "linkage contains two packages for the '{}' role",
                            role.as_str()
                        ),
                    });
                }
            }
        }
        validate_graph_linkage(&graph).map_err(|reason| NexusError::IncompatiblePackage {
            package: source.storage_id,
            reason,
        })?;

        Ok(graph)
    }

    /// Fetches and validates immutable metadata for one exact package object.
    pub async fn resolve_package(
        &self,
        storage_id: sui::types::Address,
    ) -> Result<Arc<PackageVersion>, NexusError> {
        if let Some(package) = self.package_cache.read().await.get(&storage_id).cloned() {
            return Ok(package);
        }

        let package = Arc::new(
            fetch_package_metadata(&self.crawler, storage_id)
                .await
                .map_err(NexusError::Rpc)?,
        );
        let mut cache = self.package_cache.write().await;
        Ok(cache
            .entry(storage_id)
            .or_insert_with(|| Arc::clone(&package))
            .clone())
    }

    /// Decodes the inner value after validating its complete supported state
    /// pair against `context`.
    ///
    /// `A` is the anchor, `W` is its package witness, and `V` is its stored
    /// inner layout. Unknown types fail before any inner value bytes are
    /// decoded.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] when the observed types do
    /// not exactly match `A`, `W`, and `V`. RPC and BCS failures return
    /// [`NexusError::Rpc`].
    pub async fn load_inner<A, W, V>(
        &self,
        object_id: sui::types::Address,
        context: &crate::types::NexusContext,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        W: MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        let snapshot = self.observe_snapshot(object_id).await?;
        Self::validate_observation::<A, W, V>(snapshot.observed(), context)?;
        self.decode_snapshot::<A, V>(&snapshot)
    }

    /// Decodes an inner value from an existing one-round state snapshot.
    ///
    /// The complete state pair is validated before locally retained bytes are
    /// decoded. This method performs no Sui request.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] when the snapshot does not
    /// match `A`, `W`, and `V`. BCS failures return [`NexusError::Rpc`].
    pub fn load_inner_from_snapshot<A, W, V>(
        &self,
        snapshot: &ObservedStateSnapshot,
        context: &NexusContext,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        W: MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        Self::validate_observation::<A, W, V>(snapshot.observed(), context)?;
        self.decode_snapshot::<A, V>(snapshot)
    }

    /// Decodes every `Inner<V>` value written by an exact transaction.
    ///
    /// Each result is tied to its stable parent object through the dynamic
    /// field owner and deterministic field identity. Input versions and values
    /// of another package type are ignored. No Sui request is performed.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidTransactionOutput`] when the response
    /// identity, field ownership, type, key, or contents are inconsistent.
    pub fn finalized_inner_outputs<V>(
        &self,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
        context: &NexusContext,
    ) -> Result<Vec<TransactionStateOutput<V>>, NexusError>
    where
        V: DeserializeOwned + MoveStruct,
    {
        use crate::move_bindings::primitives::object_state::Inner;

        let key = Inner::new(false);
        let key_type = crate::move_bindings::type_tag::<Inner>(context);
        let value_type = crate::move_bindings::type_tag::<V>(context);
        self.crawler
            .transaction_dynamic_field_outputs(executed, transaction, &key, &key_type, &value_type)
            .map_err(|error| NexusError::InvalidTransactionOutput {
                transaction,
                reason: error.to_string(),
            })
    }

    /// Decodes every `Inner<V>` value visible to one causal transaction.
    ///
    /// The retained view can include values written by finalized ancestors.
    /// Each value is still tied to its stable parent through the dynamic field
    /// owner and deterministic field identity. This method performs no Sui
    /// request.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidTransactionOutput`] when the view has an
    /// inconsistent transaction identity, ambiguous object history, field
    /// ownership, type, key, or contents.
    pub fn causal_inner_values<V>(
        &self,
        executed: &sui::grpc::ExecutedTransaction,
        transaction: sui::types::Digest,
        context: &NexusContext,
    ) -> Result<Vec<TransactionStateOutput<V>>, NexusError>
    where
        V: DeserializeOwned + MoveStruct,
    {
        use crate::move_bindings::primitives::object_state::Inner;

        let key = Inner::new(false);
        let key_type = crate::move_bindings::type_tag::<Inner>(context);
        let value_type = crate::move_bindings::type_tag::<V>(context);
        self.crawler
            .causal_dynamic_field_values(executed, transaction, &key, &key_type, &value_type)
            .map_err(|error| NexusError::InvalidTransactionOutput {
                transaction,
                reason: error.to_string(),
            })
    }

    /// Decodes an inner value from an existing exact object observation.
    ///
    /// Reusing the observation avoids reading mutable metadata twice within
    /// one operation. The complete state pair is validated against `context`
    /// before any value bytes are decoded.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] when the observation does
    /// not match `A`, `W`, and `V`. RPC and BCS failures return
    /// [`NexusError::Rpc`].
    pub async fn load_inner_from_observation<A, W, V>(
        &self,
        observed: &ObservedState,
        context: &NexusContext,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        W: MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        Self::validate_observation::<A, W, V>(observed, context)?;
        self.decode_inner::<A, V>(observed).await
    }

    /// Decodes an inner value whose observed witness has a supported adapter.
    ///
    /// This permits an execution change to introduce a new package witness
    /// while retaining the exact stored inner type. The adapter still checks
    /// the complete observed pair before any value bytes are decoded.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] when the observed pair has
    /// no supported adapter. A context from another package graph returns
    /// [`NexusError::IncompatiblePackage`]. RPC and BCS failures return
    /// [`NexusError::Rpc`].
    pub(crate) async fn load_inner_for_supported_witness<A, V>(
        &self,
        object_id: sui::types::Address,
        context: &NexusContext,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        let snapshot = self.observe_snapshot(object_id).await?;
        self.load_inner_for_supported_witness_from_snapshot::<A, V>(&snapshot, context)
    }

    /// Decodes an inner value with a supported witness from an existing snapshot.
    ///
    /// This is the snapshot counterpart to
    /// [`Self::load_inner_for_supported_witness`]. It validates the complete
    /// observed state pair and performs no Sui request.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] when the observed pair has
    /// no supported adapter. A context from another package graph returns
    /// [`NexusError::IncompatiblePackage`]. BCS failures return
    /// [`NexusError::Rpc`].
    pub(crate) fn load_inner_for_supported_witness_from_snapshot<A, V>(
        &self,
        snapshot: &ObservedStateSnapshot,
        context: &NexusContext,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        let observed = snapshot.observed();
        let adapter = StateAdapter::for_observed(observed)?;
        let package = context.require_package(adapter.role).map_err(|error| {
            NexusError::IncompatiblePackage {
                package: *observed.witness_type().address(),
                reason: error.to_string(),
            }
        })?;
        if package.storage_id != *observed.witness_type().address() {
            return Err(NexusError::IncompatiblePackage {
                package: package.storage_id,
                reason: format!(
                    "context selects package '{}', but object '{}' accepts '{}'",
                    package.storage_id,
                    observed.object_id,
                    observed.witness_type().address()
                ),
            });
        }
        let expected_anchor = state_struct_tag::<A>(package)?;
        let expected_inner = state_struct_tag::<V>(package)?;
        if observed.anchor_type != expected_anchor || observed.inner.value_type != expected_inner {
            return Err(ClientUpgradeRequired::new(
                observed.object_id,
                observed.witness.value_type.clone(),
                Some(observed.inner.value_type.clone()),
            )
            .into());
        }

        self.decode_snapshot::<A, V>(snapshot)
    }

    /// Validates a supported state pair without decoding its inner value.
    ///
    /// This is useful when an object participates in an operation but its
    /// stored data is not otherwise needed. The complete anchor, witness, and
    /// inner identity must match the operation [`NexusContext`].
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::ClientUpgradeRequired`] when the observed state
    /// does not exactly match `A`, `W`, and `V`. Observation failures are
    /// returned unchanged.
    pub async fn validate_state_pair<A, W, V>(
        &self,
        object_id: sui::types::Address,
        context: &NexusContext,
    ) -> Result<ObservedState, NexusError>
    where
        A: MoveStruct,
        W: MoveStruct,
        V: MoveStruct,
    {
        let observed = self.observe(object_id).await?;
        Self::validate_observation::<A, W, V>(&observed, context)?;
        Ok(observed)
    }

    fn validate_observation<A, W, V>(
        observed: &ObservedState,
        context: &NexusContext,
    ) -> Result<(), NexusError>
    where
        A: MoveStruct,
        W: MoveStruct,
        V: MoveStruct,
    {
        let expected_anchor = crate::move_bindings::struct_tag::<A>(context);
        let expected_witness = crate::move_bindings::struct_tag::<W>(context);
        let expected_inner = crate::move_bindings::struct_tag::<V>(context);
        if observed.anchor_type != expected_anchor
            || observed.witness.value_type != expected_witness
            || observed.inner.value_type != expected_inner
        {
            return Err(ClientUpgradeRequired::new(
                observed.object_id,
                observed.witness.value_type.clone(),
                Some(observed.inner.value_type.clone()),
            )
            .into());
        }
        Ok(())
    }

    /// Resolves one object package graph and decodes a supported inner value.
    ///
    /// Unlike [`Self::load_inner`], this method does not require stable Nexus
    /// environment identity. It exists for standalone readers whose only
    /// authority source is the object being read.
    ///
    /// # Errors
    ///
    /// Returns compatibility and package errors from
    /// [`Self::resolve_package_graph`]. RPC and BCS failures return
    /// [`NexusError::Rpc`].
    pub async fn resolve_and_load_inner<A, W, V>(
        &self,
        object_id: sui::types::Address,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned + MoveStruct,
        W: MoveStruct,
        V: DeserializeOwned + MoveStruct,
    {
        let snapshot = self.observe_snapshot(object_id).await?;
        let observed = snapshot.observed();
        let adapter = StateAdapter::for_observed(observed)?;
        let packages = self.resolve_package_graph(observed).await?;
        let package =
            packages
                .get(adapter.role)
                .ok_or_else(|| NexusError::IncompatiblePackage {
                    package: *observed.witness_type().address(),
                    reason: format!(
                        "resolved graph does not contain the '{}' source role",
                        adapter.role.as_str()
                    ),
                })?;
        let expected_anchor = state_struct_tag::<A>(package)?;
        let expected_witness = state_struct_tag::<W>(package)?;
        let expected_inner = state_struct_tag::<V>(package)?;
        if observed.anchor_type != expected_anchor
            || observed.witness.value_type != expected_witness
            || observed.inner.value_type != expected_inner
        {
            return Err(ClientUpgradeRequired::new(
                object_id,
                observed.witness.value_type.clone(),
                Some(observed.inner.value_type.clone()),
            )
            .into());
        }

        self.decode_snapshot::<A, V>(&snapshot)
    }

    fn decode_snapshot<A, V>(
        &self,
        snapshot: &ObservedStateSnapshot,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned,
        V: DeserializeOwned,
    {
        let key = crate::move_bindings::primitives::object_state::Inner::new(false);
        let (anchor, inner) = self
            .crawler
            .decode_object_state_snapshot::<A, _, V>(&snapshot.snapshot, &key)
            .map_err(NexusError::Rpc)?;

        Ok(crate::nexus::crawler::Response {
            object_id: anchor.object_id,
            owner: anchor.owner,
            version: anchor.version,
            data: inner,
            digest: anchor.digest,
            balance: anchor.balance,
        })
    }

    async fn decode_inner<A, V>(
        &self,
        observed: &ObservedState,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned,
        V: DeserializeOwned,
    {
        let key = crate::move_bindings::primitives::object_state::Inner::new(false);
        let (anchor, inner) = self
            .crawler
            .get_object_with_dynamic_field::<A, _, V>(
                observed.object_id,
                &observed.anchor_type,
                observed.inner.field_id,
                &sui::types::TypeTag::Struct(Box::new(observed.inner.value_type.clone())),
                &key,
            )
            .await
            .map_err(NexusError::from_rpc)?;

        Ok(crate::nexus::crawler::Response {
            object_id: anchor.object_id,
            owner: anchor.owner,
            version: anchor.version,
            data: inner,
            digest: anchor.digest,
            balance: anchor.balance,
        })
    }
}

fn state_observation_may_be_incomplete(reason: &str) -> bool {
    reason.starts_with("missing object_state::")
}

fn state_struct_tag<T>(package: &PackageVersion) -> Result<sui::types::StructTag, NexusError>
where
    T: MoveStruct,
{
    let shape = T::struct_tag_static();
    if !shape.type_params().is_empty() {
        return Err(NexusError::IncompatiblePackage {
            package: package.storage_id,
            reason: format!(
                "state type '{}::{}' unexpectedly has type parameters",
                shape.module(),
                shape.name()
            ),
        });
    }
    let origin = package
        .type_origin(shape.module().as_str(), shape.name().as_str())
        .map_err(|error| NexusError::IncompatiblePackage {
            package: package.storage_id,
            reason: error.to_string(),
        })?;

    Ok(sui::types::StructTag::new(
        origin,
        shape.module().clone(),
        shape.name().clone(),
        vec![],
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StateAdapter {
    role: PackageRole,
    anchor_module: &'static str,
    anchor_name: &'static str,
    inner_module: &'static str,
    inner_name: &'static str,
}

impl StateAdapter {
    fn for_observed(state: &ObservedState) -> Result<Self, ClientUpgradeRequired> {
        let adapter = STATE_ADAPTERS.iter().copied().find(|adapter| {
            state.anchor_type.module().as_str() == adapter.anchor_module
                && state.anchor_type.name().as_str() == adapter.anchor_name
                && state.anchor_type.type_params().is_empty()
        });
        let Some(adapter) = adapter else {
            return Err(ClientUpgradeRequired::new(
                state.object_id,
                state.witness_type().clone(),
                Some(state.inner_type().clone()),
            ));
        };
        let witness = state.witness_type();
        let inner = state.inner_type();
        let version_one_witness = witness.module().as_str() == "era"
            && witness.name().as_str() == "V1"
            && witness.type_params().is_empty();
        #[cfg(feature = "upgrade_test")]
        let supported_witness = version_one_witness
            || (witness.module().as_str() == "era"
                && witness.name().as_str() == "V2"
                && witness.type_params().is_empty());
        #[cfg(not(feature = "upgrade_test"))]
        let supported_witness = version_one_witness;

        let version_one_inner = inner.module().as_str() == adapter.inner_module
            && inner.name().as_str() == adapter.inner_name
            && inner.type_params().is_empty();
        #[cfg(feature = "upgrade_test")]
        let supported_inner = version_one_inner
            || (adapter.anchor_module == "agent_registry"
                && adapter.anchor_name == "AgentRegistry"
                && inner.module().as_str() == "upgrade_fixture"
                && inner.name().as_str() == "AgentRegistryInnerV2"
                && inner.type_params().is_empty());
        #[cfg(not(feature = "upgrade_test"))]
        let supported_inner = version_one_inner;

        if !supported_witness || !supported_inner || !witness.type_params().is_empty() {
            return Err(ClientUpgradeRequired::new(
                state.object_id,
                witness.clone(),
                Some(inner.clone()),
            ));
        }
        Ok(adapter)
    }
}

const STATE_ADAPTERS: &[StateAdapter] = &[
    StateAdapter {
        role: PackageRole::Tool,
        anchor_module: "tool_registry",
        anchor_name: "ToolRegistry",
        inner_module: "tool_registry",
        inner_name: "ToolRegistryInnerV1",
    },
    StateAdapter {
        role: PackageRole::Tool,
        anchor_module: "tool_registry",
        anchor_name: "Tool",
        inner_module: "tool_registry",
        inner_name: "ToolInnerV1",
    },
    StateAdapter {
        role: PackageRole::Tool,
        anchor_module: "tool_cashier",
        anchor_name: "ToolCashier",
        inner_module: "tool_cashier",
        inner_name: "ToolCashierInnerV1",
    },
    StateAdapter {
        role: PackageRole::Registry,
        anchor_module: "network_auth",
        anchor_name: "NetworkAuth",
        inner_module: "network_auth",
        inner_name: "NetworkAuthInnerV1",
    },
    StateAdapter {
        role: PackageRole::Registry,
        anchor_module: "network_auth",
        anchor_name: "KeyBinding",
        inner_module: "network_auth",
        inner_name: "KeyBindingInnerV1",
    },
    StateAdapter {
        role: PackageRole::Registry,
        anchor_module: "agent_registry",
        anchor_name: "AgentRegistry",
        inner_module: "agent_registry",
        inner_name: "AgentRegistryInnerV1",
    },
    StateAdapter {
        role: PackageRole::Registry,
        anchor_module: "leader",
        anchor_name: "LeaderRegistry",
        inner_module: "leader",
        inner_name: "LeaderRegistryInnerV1",
    },
    StateAdapter {
        role: PackageRole::Registry,
        anchor_module: "priority_fee_vault",
        anchor_name: "PriorityFeeVault",
        inner_module: "priority_fee_vault",
        inner_name: "PriorityFeeVaultInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "agent",
        anchor_name: "Agent",
        inner_module: "agent",
        inner_name: "AgentInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "agent",
        anchor_name: "AgentPaymentVault",
        inner_module: "agent",
        inner_name: "AgentPaymentVaultInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "authorization",
        anchor_name: "AgentSkillAuthorization",
        inner_module: "authorization",
        inner_name: "AgentSkillAuthorizationInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "dag",
        anchor_name: "DAG",
        inner_module: "dag",
        inner_name: "DAGInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "graph",
        anchor_name: "VertexEvaluations",
        inner_module: "graph",
        inner_name: "VertexEvaluationsInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "onchain_tool_result",
        anchor_name: "OnchainToolResult",
        inner_module: "onchain_tool_result",
        inner_name: "OnchainToolResultInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "payment",
        anchor_name: "ExecutionPayment",
        inner_module: "payment",
        inner_name: "ExecutionPaymentInnerV1",
    },
    StateAdapter {
        role: PackageRole::Interface,
        anchor_module: "payment",
        anchor_name: "TaskPaymentReserve",
        inner_module: "payment",
        inner_name: "TaskPaymentReserveInnerV1",
    },
    StateAdapter {
        role: PackageRole::Workflow,
        anchor_module: "execution",
        anchor_name: "DAGExecution",
        inner_module: "execution",
        inner_name: "DAGExecutionInnerV1",
    },
    StateAdapter {
        role: PackageRole::Scheduler,
        anchor_module: "task",
        anchor_name: "Task",
        inner_module: "task",
        inner_name: "TaskInnerV1",
    },
];

fn package_role(package: &PackageVersion) -> Option<PackageRole> {
    let has = |module: &str, datatype: &str| {
        package
            .type_origins
            .get(module)
            .is_some_and(|types| types.contains_key(datatype))
    };
    [
        (
            PackageRole::Primitives,
            has("object_state", "Inner")
                && has("object_state", "Witness")
                && has("event", "EventWrapper"),
        ),
        (
            PackageRole::Interface,
            has("agent", "Agent") && has("dag", "DAG") && has("graph", "Vertex"),
        ),
        (
            PackageRole::Tool,
            has("tool_registry", "ToolRegistry")
                && has("tool_registry", "Tool")
                && has("tool_cashier", "ToolCashier"),
        ),
        (
            PackageRole::Registry,
            has("agent_registry", "AgentRegistry")
                && has("leader", "LeaderRegistry")
                && has("network_auth", "NetworkAuth")
                && has("priority_fee_vault", "PriorityFeeVault"),
        ),
        (
            PackageRole::Workflow,
            has("execution", "DAGExecution") && has("execution", "DAGExecutionInnerV1"),
        ),
        (
            PackageRole::Scheduler,
            has("task", "Task") && has("task", "TaskInnerV1"),
        ),
    ]
    .into_iter()
    .find_map(|(role, matches)| matches.then_some(role))
}

fn validate_state_type_origins(
    state: &ObservedState,
    package: &PackageVersion,
) -> Result<(), NexusError> {
    for (label, tag) in [
        ("anchor", &state.anchor_type),
        ("witness", state.witness_type()),
        ("inner", state.inner_type()),
    ] {
        let origin = package
            .type_origin(tag.module().as_str(), tag.name().as_str())
            .map_err(|error| NexusError::IncompatiblePackage {
                package: package.storage_id,
                reason: error.to_string(),
            })?;
        if origin != *tag.address() {
            return Err(NexusError::IncompatiblePackage {
                package: package.storage_id,
                reason: format!(
                    "{label} type '{tag}' has package origin '{origin}' in immutable metadata"
                ),
            });
        }
    }
    Ok(())
}

fn validate_graph_linkage(graph: &NexusPackages) -> Result<(), String> {
    for package in graph.all() {
        for linked in graph.all() {
            if package.storage_id == linked.storage_id {
                continue;
            }
            let Some(actual) = package.linkage.get(&linked.initial_id) else {
                continue;
            };
            if actual.storage_id != linked.storage_id || actual.version != linked.version {
                return Err(format!(
                    "package '{}' links lineage '{}' to '{}' at version {}, expected '{}' at \
                     version {}",
                    package.storage_id,
                    linked.initial_id,
                    actual.storage_id,
                    actual.version,
                    linked.storage_id,
                    linked.version
                ));
            }
        }
    }
    Ok(())
}

async fn fetch_package_metadata(
    crawler: &Crawler,
    storage_id: sui::types::Address,
) -> anyhow::Result<PackageVersion> {
    let package = crawler.get_package(storage_id).await?;
    let observed_storage = parse_package_address(package.storage_id.as_deref(), "storage_id")?;
    if observed_storage != storage_id {
        anyhow::bail!(
            "Package request for '{storage_id}' returned storage ID '{observed_storage}'"
        );
    }
    let initial_id = parse_package_address(package.original_id.as_deref(), "original_id")?;
    let version = package
        .version
        .ok_or_else(|| anyhow::anyhow!("Package '{storage_id}' has no version"))?;

    let request = sui::grpc::GetObjectRequest::default()
        .with_object_id(storage_id)
        .with_read_mask(sui::grpc::FieldMask::from_paths([
            "object_id",
            "version",
            "object_type",
            "package",
        ]));
    let object = crawler
        .grpc_client()
        .as_ref()
        .clone()
        .ledger_client()
        .get_object(request)
        .await
        .map_err(|error| anyhow::anyhow!("Could not fetch package object '{storage_id}': {error}"))?
        .into_inner()
        .object
        .ok_or_else(|| anyhow::anyhow!("Package object '{storage_id}' was not returned"))?;
    if object.object_id_opt().and_then(|id| id.parse().ok()) != Some(storage_id)
        || object.version_opt() != Some(version)
        || object.object_type_opt() != Some("package")
    {
        anyhow::bail!("Package object '{storage_id}' has inconsistent immutable identity");
    }
    let exact = object
        .package
        .ok_or_else(|| anyhow::anyhow!("Package object '{storage_id}' has no package metadata"))?;
    let mut type_origins = TypeOrigins::new();
    for origin in exact.type_origins {
        let module = origin.module_name.ok_or_else(|| {
            anyhow::anyhow!("Package '{storage_id}' has an origin without module")
        })?;
        let datatype = origin.datatype_name.ok_or_else(|| {
            anyhow::anyhow!("Package '{storage_id}' has an origin without datatype")
        })?;
        let package_id = parse_package_address(origin.package_id.as_deref(), "type origin")?;
        let previous = type_origins
            .entry(module.clone())
            .or_default()
            .insert(datatype.clone(), package_id);
        if previous.is_some_and(|previous| previous != package_id) {
            anyhow::bail!(
                "Package '{storage_id}' has conflicting origins for '{module}::{datatype}'"
            );
        }
    }
    validate_abi_type_origins(storage_id, &package, &type_origins)?;

    let mut linkage = PackageLinkage::new();
    for link in exact.linkage {
        let original_id = parse_package_address(link.original_id.as_deref(), "linkage lineage")?;
        let storage_id = parse_package_address(link.upgraded_id.as_deref(), "linkage storage")?;
        let version = link
            .upgraded_version
            .ok_or_else(|| anyhow::anyhow!("Linkage for '{original_id}' has no version"))?;
        if linkage
            .insert(
                original_id,
                PackageLink {
                    storage_id,
                    version,
                },
            )
            .is_some()
        {
            anyhow::bail!("Package '{observed_storage}' has duplicate linkage for '{original_id}'");
        }
    }

    Ok(PackageVersion::new(
        initial_id,
        storage_id,
        version,
        type_origins,
        linkage,
    ))
}

fn validate_abi_type_origins(
    storage_id: sui::types::Address,
    package: &sui::grpc::Package,
    origins: &TypeOrigins,
) -> anyhow::Result<()> {
    for module in &package.modules {
        let module_name = module
            .name
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Package '{storage_id}' has an unnamed module"))?;
        for datatype in &module.datatypes {
            let datatype_name = datatype.name.as_deref().ok_or_else(|| {
                anyhow::anyhow!("Package '{storage_id}' module '{module_name}' has an unnamed type")
            })?;
            let defining_id =
                parse_package_address(datatype.defining_id.as_deref(), "ABI defining ID")?;
            let exact = origins
                .get(module_name)
                .and_then(|types| types.get(datatype_name))
                .copied();
            if exact != Some(defining_id) {
                anyhow::bail!(
                    "Package '{storage_id}' ABI origin for '{module_name}::{datatype_name}' does \
                     not match immutable metadata"
                );
            }
        }
    }
    Ok(())
}

fn parse_package_address(value: Option<&str>, field: &str) -> anyhow::Result<sui::types::Address> {
    value
        .ok_or_else(|| anyhow::anyhow!("Package {field} is missing"))?
        .parse()
        .map_err(|error| anyhow::anyhow!("Package {field} is invalid: {error}"))
}

fn observed_state_from_metadata(
    object: ObjectMetadata,
    fields: Vec<DynamicFieldMetadata>,
) -> Result<ObservedState, String> {
    let witness = select_state_field(&fields, "Witness")?;
    let inner = select_state_field(&fields, "Inner")?;

    Ok(ObservedState {
        object_id: object.object_id,
        owner: object.owner,
        anchor_type: object.object_type,
        witness,
        inner,
    })
}

fn state_field_key_types(fields: &[DynamicFieldMetadata]) -> Result<StateFieldKeyTypes, String> {
    let witness = select_state_field(fields, "Witness")?;
    let inner = select_state_field(fields, "Inner")?;
    let key_type = |field_id| {
        fields
            .iter()
            .find(|field| field.field_id == field_id)
            .map(|field| field.key_type.clone())
            .ok_or_else(|| "selected object state field metadata is missing".to_owned())
    };
    Ok(StateFieldKeyTypes {
        witness: key_type(witness.field_id)?,
        inner: key_type(inner.field_id)?,
    })
}

fn select_state_field(
    fields: &[DynamicFieldMetadata],
    key_name: &str,
) -> Result<ObservedStateField, String> {
    let mut matches = fields.iter().filter(|field| {
        let sui::types::TypeTag::Struct(key) = &field.key_type else {
            return false;
        };
        key.module().as_str() == "object_state"
            && key.name().as_str() == key_name
            && key.type_params().is_empty()
    });
    let field = matches
        .next()
        .ok_or_else(|| format!("missing object_state::{key_name} field"))?;
    if matches.next().is_some() {
        return Err(format!("multiple object_state::{key_name} fields"));
    }
    let sui::types::TypeTag::Struct(value_type) = &field.value_type else {
        return Err(format!(
            "object_state::{key_name} field '{}' stores non struct type '{}'",
            field.field_id, field.value_type
        ));
    };

    Ok(ObservedStateField {
        field_id: field.field_id,
        value_type: value_type.as_ref().clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize)]
    struct StateFieldValue<K, V> {
        id: sui::types::Address,
        name: K,
        value: V,
    }

    fn state_object_result(
        id: sui::types::Address,
        owner: sui::types::Owner,
        object_type: sui::types::StructTag,
        contents: Option<Vec<u8>>,
    ) -> sui::grpc::GetObjectResult {
        let object_ref = crate::test_utils::sui_mocks::object_ref_for_id(id);
        let mut object = sui::grpc::Object::default();
        object.set_object_id(id);
        object.set_owner(sui::grpc::Owner::from(owner));
        object.set_object_type(object_type.to_string());
        object.set_version(object_ref.version());
        object.set_digest(*object_ref.digest());
        if let Some(contents) = contents {
            let mut bcs = sui::grpc::Bcs::default();
            bcs.set_name(object_type.to_string());
            bcs.set_value(contents);
            object.set_contents(bcs);
        }
        sui::grpc::GetObjectResult::new_object(object)
    }

    fn dynamic_field_type(
        key: sui::types::TypeTag,
        value: sui::types::TypeTag,
    ) -> sui::types::StructTag {
        sui::types::StructTag::new(
            sui::types::Address::from_static("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![key, value],
        )
    }

    #[tokio::test]
    async fn typed_state_load_uses_one_known_lineage_batch() {
        use crate::{
            move_bindings::{
                primitives::object_state::{Inner, Witness},
                registry::{
                    era::V1 as RegistryWitnessV1,
                    leader::{LeaderRegistry, LeaderRegistryInnerV1},
                },
                sui_framework::object::UID,
            },
            test_utils::sui_mocks,
        };

        let context = sui_mocks::mock_nexus_context();
        let object_id = context.leader_registry.object_id();
        let owner = sui::types::Owner::Shared(context.leader_registry.initial_shared_version);
        let witness_key = Witness::new(false);
        let inner_key = Inner::new(false);
        let witness_key_type = crate::move_bindings::type_tag::<Witness>(&context);
        let inner_key_type = crate::move_bindings::type_tag::<Inner>(&context);
        let witness_id = derive_dynamic_field_id(object_id, &witness_key, &witness_key_type)
            .expect("Witness field ID derives");
        let inner_id = derive_dynamic_field_id(object_id, &inner_key, &inner_key_type)
            .expect("Inner field ID derives");
        let anchor = LeaderRegistry::new(UID::new(object_id));
        let inner = LeaderRegistryInnerV1::new_for_test(object_id, context.network_id);
        let field = StateFieldValue {
            id: inner_id,
            name: inner_key,
            value: inner.clone(),
        };
        let objects = vec![
            state_object_result(
                object_id,
                owner,
                crate::move_bindings::struct_tag::<LeaderRegistry>(&context),
                Some(bcs::to_bytes(&anchor).expect("anchor serializes")),
            ),
            state_object_result(
                witness_id,
                sui::types::Owner::Object(object_id),
                dynamic_field_type(
                    witness_key_type.clone(),
                    crate::move_bindings::type_tag::<RegistryWitnessV1>(&context),
                ),
                None,
            ),
            state_object_result(
                inner_id,
                sui::types::Owner::Object(object_id),
                dynamic_field_type(
                    inner_key_type.clone(),
                    crate::move_bindings::type_tag::<LeaderRegistryInnerV1>(&context),
                ),
                Some(bcs::to_bytes(&field).expect("inner field serializes")),
            ),
        ];
        let expected_ids = [object_id, witness_id, inner_id].map(|id| id.to_string());
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        ledger
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |request| {
                let request = request.get_ref();
                assert_eq!(
                    request
                        .requests
                        .iter()
                        .map(|request| request.object_id.clone().expect("object ID"))
                        .collect::<Vec<_>>(),
                    expected_ids
                );
                assert!(request
                    .read_mask
                    .as_ref()
                    .is_some_and(|mask| mask.paths.iter().any(|path| path == "contents")));
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(objects),
                ))
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let resolver = StateResolver::new(Arc::new(Crawler::new(Arc::new(
            sui::grpc::client(rpc_url).expect("mock Sui client builds"),
        ))));
        resolver
            .state_field_keys
            .write()
            .await
            .push(StateFieldKeyTypes {
                witness: witness_key_type,
                inner: inner_key_type,
            });

        let loaded = resolver
            .load_inner::<LeaderRegistry, RegistryWitnessV1, LeaderRegistryInnerV1>(
                object_id, &context,
            )
            .await
            .expect("typed state loads");

        assert_eq!(loaded.object_id, object_id);
        assert_eq!(
            loaded.data.max_transaction_budget(),
            inner.max_transaction_budget()
        );
    }

    #[tokio::test]
    async fn finalized_snapshot_reuses_unchanged_structure_and_rejects_removal() {
        use crate::{
            move_bindings::{
                primitives::object_state::{Inner, Witness},
                registry::{
                    era::V1 as RegistryWitnessV1,
                    leader::{LeaderRegistry, LeaderRegistryInnerV1},
                },
                sui_framework::object::UID,
            },
            test_utils::sui_mocks,
        };

        let context = sui_mocks::mock_nexus_context();
        let object_id = context.leader_registry.object_id();
        let owner = sui::types::Owner::Shared(context.leader_registry.initial_shared_version);
        let witness_key = Witness::new(false);
        let inner_key = Inner::new(false);
        let witness_key_type = crate::move_bindings::type_tag::<Witness>(&context);
        let inner_key_type = crate::move_bindings::type_tag::<Inner>(&context);
        let witness_id = derive_dynamic_field_id(object_id, &witness_key, &witness_key_type)
            .expect("Witness field ID derives");
        let inner_id = derive_dynamic_field_id(object_id, &inner_key, &inner_key_type)
            .expect("Inner field ID derives");
        let anchor = LeaderRegistry::new(UID::new(object_id));
        let old_inner = LeaderRegistryInnerV1::new_for_test(object_id, address("0x41"));
        let current_inner = LeaderRegistryInnerV1::new_for_test(object_id, address("0x42"));
        let inner_field = |value| StateFieldValue {
            id: inner_id,
            name: inner_key,
            value,
        };
        let object = |result: sui::grpc::GetObjectResult| {
            result
                .to_result()
                .expect("state object result contains an object")
        };
        let mut anchor = object(state_object_result(
            object_id,
            owner,
            crate::move_bindings::struct_tag::<LeaderRegistry>(&context),
            Some(bcs::to_bytes(&anchor).expect("anchor serializes")),
        ));
        anchor.set_previous_transaction(sui::types::Digest::new([1; 32]));
        let mut witness = object(state_object_result(
            witness_id,
            sui::types::Owner::Object(object_id),
            dynamic_field_type(
                witness_key_type.clone(),
                crate::move_bindings::type_tag::<RegistryWitnessV1>(&context),
            ),
            None,
        ));
        witness.set_previous_transaction(sui::types::Digest::new([2; 32]));
        let mut old = object(state_object_result(
            inner_id,
            sui::types::Owner::Object(object_id),
            dynamic_field_type(
                inner_key_type.clone(),
                crate::move_bindings::type_tag::<LeaderRegistryInnerV1>(&context),
            ),
            Some(bcs::to_bytes(&inner_field(old_inner)).expect("old inner serializes")),
        ));
        old.set_previous_transaction(sui::types::Digest::new([3; 32]));
        let transaction = sui::types::Digest::new([4; 32]);
        let mut output = object(state_object_result(
            inner_id,
            sui::types::Owner::Object(object_id),
            dynamic_field_type(
                inner_key_type.clone(),
                crate::move_bindings::type_tag::<LeaderRegistryInnerV1>(&context),
            ),
            Some(bcs::to_bytes(&inner_field(current_inner)).expect("current inner serializes")),
        ));
        output.set_version(2);
        output.set_digest(sui::types::Digest::new([5; 32]));
        output.set_previous_transaction(transaction);

        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let basis_objects = [anchor.clone(), witness.clone(), old.clone()]
            .into_iter()
            .map(sui::grpc::GetObjectResult::new_object)
            .collect();
        ledger
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |_| {
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(basis_objects),
                ))
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let resolver = StateResolver::new(Arc::new(Crawler::new(Arc::new(
            sui::grpc::client(rpc_url).expect("mock Sui client builds"),
        ))));
        resolver
            .state_field_keys
            .write()
            .await
            .push(StateFieldKeyTypes {
                witness: witness_key_type,
                inner: inner_key_type,
            });
        let basis = resolver
            .observe_snapshot(object_id)
            .await
            .expect("basis state is valid");
        let mut executed = sui::grpc::ExecutedTransaction::default();
        executed.set_digest(transaction);
        executed.set_objects(
            sui::grpc::ObjectSet::default().with_objects(vec![old.clone(), output.clone()]),
        );

        let snapshot = resolver
            .observe_finalized_snapshot(&basis, &executed, transaction)
            .expect("finalized objects are valid")
            .expect("finalized objects contain a causal snapshot");
        let loaded = resolver
            .load_inner_from_snapshot::<LeaderRegistry, RegistryWitnessV1, LeaderRegistryInnerV1>(
                &snapshot, &context,
            )
            .expect("finalized snapshot decodes");

        assert_eq!(loaded.data.network_id(), address("0x42"));
        assert_eq!(
            snapshot
                .inner_object_reference()
                .expect("finalized inner reference is complete"),
            sui::types::ObjectReference::new(inner_id, 2, sui::types::Digest::new([5; 32]),)
        );

        let mut removed_anchor = sui::grpc::ExecutedTransaction::default();
        removed_anchor.set_digest(transaction);
        removed_anchor
            .set_objects(sui::grpc::ObjectSet::default().with_objects(vec![anchor, old, output]));
        let error = resolver
            .observe_finalized_snapshot(&basis, &removed_anchor, transaction)
            .expect_err("an input anchor without an output was removed");
        assert!(
            error.to_string().contains("removed the anchor"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn snapshot_rejects_wrong_field_owner_before_decoding() {
        use crate::{
            move_bindings::primitives::object_state::{Inner, Witness},
            test_utils::sui_mocks,
        };

        let object_id = address("0x20");
        let witness_key_type =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa1", "object_state", "Witness")));
        let inner_key_type =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa1", "object_state", "Inner")));
        let witness_id =
            derive_dynamic_field_id(object_id, &Witness::new(false), &witness_key_type).unwrap();
        let inner_id =
            derive_dynamic_field_id(object_id, &Inner::new(false), &inner_key_type).unwrap();
        let objects = vec![
            state_object_result(
                object_id,
                sui::types::Owner::Shared(1),
                struct_tag("0xa6", "task", "Task"),
                Some(vec![]),
            ),
            state_object_result(
                witness_id,
                sui::types::Owner::Object(object_id),
                dynamic_field_type(
                    witness_key_type.clone(),
                    sui::types::TypeTag::Struct(Box::new(struct_tag("0xa6", "era", "V1"))),
                ),
                None,
            ),
            state_object_result(
                inner_id,
                sui::types::Owner::Address(object_id),
                dynamic_field_type(
                    inner_key_type.clone(),
                    sui::types::TypeTag::Struct(Box::new(struct_tag(
                        "0xa6",
                        "task",
                        "TaskInnerV1",
                    ))),
                ),
                Some(vec![]),
            ),
        ];
        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        ledger
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |_| {
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(objects),
                ))
            });
        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            ..Default::default()
        });
        let resolver = StateResolver::new(Arc::new(Crawler::new(Arc::new(
            sui::grpc::client(rpc_url).expect("mock Sui client builds"),
        ))));
        resolver
            .state_field_keys
            .write()
            .await
            .push(StateFieldKeyTypes {
                witness: witness_key_type,
                inner: inner_key_type,
            });

        let error = resolver
            .observe_snapshot(object_id)
            .await
            .expect_err("wrong field owner must fail");

        assert!(
            error.to_string().contains("expected object"),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn observation_separates_absence_from_an_unreachable_node() {
        use crate::test_utils::sui_mocks;

        let object_id = address("0x10");

        let observe = |status: tonic::Status| async move {
            let mut ledger = sui_mocks::grpc::MockLedgerService::new();
            ledger
                .expect_get_object()
                .returning(move |_request| Err(status.clone()));

            let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
                ledger_service_mock: Some(ledger),
                ..Default::default()
            });
            let crawler = Crawler::new(Arc::new(
                sui::grpc::client(&rpc_url).expect("mock Sui client builds"),
            ));

            StateResolver::new(Arc::new(crawler))
                .observe(object_id)
                .await
                .expect_err("the mocked node always fails")
        };

        assert!(matches!(
            observe(tonic::Status::not_found("missing")).await,
            NexusError::ObjectNotFound { object } if object == object_id
        ));
        assert!(matches!(
            observe(tonic::Status::unavailable("connection refused")).await,
            NexusError::Rpc(_)
        ));
    }

    #[tokio::test]
    async fn observation_retries_incomplete_dynamic_field_metadata() {
        use {
            crate::{
                move_bindings::primitives::object_state::{Inner, Witness},
                test_utils::sui_mocks,
            },
            std::sync::atomic::{AtomicUsize, Ordering},
        };

        let object_id = address("0x10");
        let anchor_type = struct_tag("0xa6", "task", "Task");
        let witness_key =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa1", "object_state", "Witness")));
        let inner_key =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa1", "object_state", "Inner")));
        let witness_type = sui::types::TypeTag::Struct(Box::new(struct_tag("0xa6", "era", "V1")));
        let inner_type =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa6", "task", "TaskInnerV1")));
        let witness_field_id = object_id.derive_dynamic_child_id(
            &witness_key,
            &bcs::to_bytes(&Witness::new(false)).expect("Witness key serializes"),
        );
        let inner_field_id = object_id.derive_dynamic_child_id(
            &inner_key,
            &bcs::to_bytes(&Inner::new(false)).expect("Inner key serializes"),
        );

        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let metadata_anchor_type = anchor_type.clone();
        ledger
            .expect_get_object()
            .times(2)
            .returning(move |request| {
                assert_eq!(
                    request.get_ref().object_id_opt(),
                    Some(object_id.to_string().as_str())
                );
                let mut object = sui::grpc::Object::default();
                object.set_object_id(object_id);
                object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Shared(1)));
                object.set_object_type(metadata_anchor_type.to_string());
                Ok(tonic::Response::new(
                    sui::grpc::GetObjectResponse::default().with_object(object),
                ))
            });
        let batch_witness_key = witness_key.clone();
        let batch_inner_key = inner_key.clone();
        let batch_witness_type = witness_type.clone();
        let batch_inner_type = inner_type.clone();
        ledger
            .expect_batch_get_objects()
            .times(1)
            .return_once(move |request| {
                let ids = request
                    .get_ref()
                    .requests
                    .iter()
                    .map(|request| request.object_id_opt().expect("object ID").parse().unwrap())
                    .collect::<Vec<sui::types::Address>>();
                assert_eq!(ids, [object_id, witness_field_id, inner_field_id]);
                let field_type = |key, value| {
                    sui::types::StructTag::new(
                        address("0x2"),
                        sui::types::Identifier::from_static("dynamic_field"),
                        sui::types::Identifier::from_static("Field"),
                        vec![key, value],
                    )
                };
                let object = |id, owner, object_type: sui::types::StructTag| {
                    let mut object = sui::grpc::Object::default();
                    object.set_object_id(id);
                    object.set_owner(sui::grpc::Owner::from(owner));
                    object.set_object_type(object_type.to_string());
                    sui::grpc::GetObjectResult::new_object(object)
                };
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(vec![
                        object(object_id, sui::types::Owner::Shared(1), anchor_type),
                        object(
                            witness_field_id,
                            sui::types::Owner::Object(object_id),
                            field_type(batch_witness_key, batch_witness_type),
                        ),
                        object(
                            inner_field_id,
                            sui::types::Owner::Object(object_id),
                            field_type(batch_inner_key, batch_inner_type),
                        ),
                    ]),
                ))
            });

        let observations = Arc::new(AtomicUsize::new(0));
        let mut state = sui_mocks::grpc::MockStateService::new();
        state
            .expect_list_dynamic_fields()
            .times(2)
            .returning(move |_request| {
                let observation = observations.fetch_add(1, Ordering::SeqCst);
                let fields = if observation == 0 {
                    let mut incomplete = sui::grpc::DynamicField::default();
                    incomplete.set_field_id(address("0x11"));
                    vec![incomplete]
                } else {
                    vec![
                        listed_state_field(
                            witness_field_id,
                            witness_key.clone(),
                            witness_type.clone(),
                        ),
                        listed_state_field(inner_field_id, inner_key.clone(), inner_type.clone()),
                    ]
                };
                let mut response = sui::grpc::ListDynamicFieldsResponse::default();
                response.set_dynamic_fields(fields);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            state_service_mock: Some(state),
            ..Default::default()
        });
        let crawler = Crawler::new(Arc::new(
            sui::grpc::client(&rpc_url).expect("mock Sui client builds"),
        ));
        let observed = StateResolver::new(Arc::new(crawler))
            .observe(object_id)
            .await
            .expect("the coherent retry should be observed");

        assert_eq!(observed.object_id, object_id);
        assert_eq!(observed.witness.field_id, witness_field_id);
        assert_eq!(observed.inner.field_id, inner_field_id);
    }

    #[tokio::test]
    async fn observation_derives_cached_fields_and_reads_current_types() {
        use crate::{
            move_bindings::primitives::object_state::{Inner, Witness},
            test_utils::sui_mocks,
        };

        let object_id = address("0x20");
        let anchor_type = struct_tag("0xa6", "task", "Task");
        let witness_key_type =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa1", "object_state", "Witness")));
        let inner_key_type =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa1", "object_state", "Inner")));
        let witness_field_id = object_id.derive_dynamic_child_id(
            &witness_key_type,
            &bcs::to_bytes(&Witness::new(false)).expect("Witness key serializes"),
        );
        let inner_field_id = object_id.derive_dynamic_child_id(
            &inner_key_type,
            &bcs::to_bytes(&Inner::new(false)).expect("Inner key serializes"),
        );
        let witness_v1 = sui::types::TypeTag::Struct(Box::new(struct_tag("0xa6", "era", "V1")));
        let inner_v1 =
            sui::types::TypeTag::Struct(Box::new(struct_tag("0xa6", "task", "TaskInnerV1")));
        let witness_v2 = struct_tag("0xa7", "era", "V2");
        let inner_v2 = struct_tag("0xa7", "task", "TaskInnerV2");

        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        let listed_anchor_type = anchor_type.clone();
        ledger
            .expect_get_object()
            .times(1)
            .return_once(move |_request| {
                let mut object = sui::grpc::Object::default();
                object.set_object_id(object_id);
                object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Shared(1)));
                object.set_object_type(listed_anchor_type.to_string());
                Ok(tonic::Response::new(
                    sui::grpc::GetObjectResponse::default().with_object(object),
                ))
            });
        let batch_anchor_type = anchor_type.clone();
        let batch_witness_key = witness_key_type.clone();
        let batch_inner_key = inner_key_type.clone();
        let batch_witness_v2 = witness_v2.clone();
        let batch_inner_v2 = inner_v2.clone();
        ledger
            .expect_batch_get_objects()
            .times(2)
            .returning(move |request| {
                let ids = request
                    .get_ref()
                    .requests
                    .iter()
                    .map(|request| request.object_id_opt().expect("object ID").parse().unwrap())
                    .collect::<Vec<sui::types::Address>>();
                assert_eq!(ids, [object_id, witness_field_id, inner_field_id]);
                let field_type = |key, value| {
                    sui::types::StructTag::new(
                        address("0x2"),
                        sui::types::Identifier::from_static("dynamic_field"),
                        sui::types::Identifier::from_static("Field"),
                        vec![key, sui::types::TypeTag::Struct(Box::new(value))],
                    )
                };
                let object = |id, owner, object_type: sui::types::StructTag| {
                    let mut object = sui::grpc::Object::default();
                    object.set_object_id(id);
                    object.set_owner(sui::grpc::Owner::from(owner));
                    object.set_object_type(object_type.to_string());
                    sui::grpc::GetObjectResult::new_object(object)
                };
                Ok(tonic::Response::new(
                    sui::grpc::BatchGetObjectsResponse::new(vec![
                        object(
                            object_id,
                            sui::types::Owner::Shared(1),
                            batch_anchor_type.clone(),
                        ),
                        object(
                            witness_field_id,
                            sui::types::Owner::Object(object_id),
                            field_type(batch_witness_key.clone(), batch_witness_v2.clone()),
                        ),
                        object(
                            inner_field_id,
                            sui::types::Owner::Object(object_id),
                            field_type(batch_inner_key.clone(), batch_inner_v2.clone()),
                        ),
                    ]),
                ))
            });

        let mut state = sui_mocks::grpc::MockStateService::new();
        state
            .expect_list_dynamic_fields()
            .times(1)
            .return_once(move |_request| {
                Ok(tonic::Response::new(
                    sui::grpc::ListDynamicFieldsResponse::default().with_dynamic_fields(vec![
                        listed_state_field(witness_field_id, witness_key_type, witness_v1),
                        listed_state_field(inner_field_id, inner_key_type, inner_v1),
                    ]),
                ))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger),
            state_service_mock: Some(state),
            ..Default::default()
        });
        let resolver = StateResolver::new(Arc::new(Crawler::new(Arc::new(
            sui::grpc::client(&rpc_url).expect("mock Sui client builds"),
        ))));

        let first = resolver
            .observe(object_id)
            .await
            .expect("first observation");
        assert_eq!(first.witness_type(), &witness_v2);
        assert_eq!(first.inner_type(), &inner_v2);
        let second = resolver
            .observe(object_id)
            .await
            .expect("cached observation");
        assert_eq!(second.witness_type(), &witness_v2);
        assert_eq!(second.inner_type(), &inner_v2);
    }

    #[test]
    fn only_missing_state_markers_are_reobserved() {
        assert!(state_observation_may_be_incomplete(
            "missing object_state::Witness field"
        ));
        assert!(state_observation_may_be_incomplete(
            "missing object_state::Inner field"
        ));
        assert!(!state_observation_may_be_incomplete(
            "multiple object_state::Witness fields"
        ));
        assert!(!state_observation_may_be_incomplete(
            "object_state::Inner field stores non struct type"
        ));
    }

    #[test]
    fn observation_keeps_unknown_types_without_decoding_values() {
        let object_id = address("0x10");
        let witness_type = struct_tag("0xa2", "era", "V2");
        let inner_type = struct_tag("0xa2", "agent_registry", "AgentRegistryInnerV2");
        let observed = observed_state_from_metadata(
            ObjectMetadata {
                object_id,
                owner: sui::types::Owner::Shared(3),
                object_type: struct_tag("0xa1", "agent_registry", "AgentRegistry"),
            },
            vec![
                state_field("0x11", "Witness", witness_type.clone()),
                state_field("0x12", "Inner", inner_type.clone()),
            ],
        )
        .unwrap();

        assert_eq!(observed.object_id, object_id);
        assert_eq!(observed.witness_type(), &witness_type);
        assert_eq!(observed.inner_type(), &inner_type);
    }

    #[test]
    fn observation_requires_exactly_one_field_for_each_state_key() {
        let object = ObjectMetadata {
            object_id: address("0x10"),
            owner: sui::types::Owner::Shared(3),
            object_type: struct_tag("0xa1", "agent_registry", "AgentRegistry"),
        };
        let fields = vec![
            state_field("0x11", "Witness", struct_tag("0xa2", "era", "V1")),
            state_field("0x12", "Witness", struct_tag("0xa3", "era", "V2")),
            state_field(
                "0x13",
                "Inner",
                struct_tag("0xa2", "agent_registry", "AgentRegistryInnerV1"),
            ),
        ];

        assert_eq!(
            observed_state_from_metadata(object, fields).unwrap_err(),
            "multiple object_state::Witness fields"
        );
    }

    #[test]
    fn version_one_era_selects_the_supported_registry_adapter() {
        let state = ObservedState {
            object_id: address("0x10"),
            owner: sui::types::Owner::Shared(3),
            anchor_type: struct_tag("0xa2", "leader", "LeaderRegistry"),
            witness: ObservedStateField {
                field_id: address("0x11"),
                value_type: struct_tag("0xa2", "era", "V1"),
            },
            inner: ObservedStateField {
                field_id: address("0x12"),
                value_type: struct_tag("0xa2", "leader", "LeaderRegistryInnerV1"),
            },
        };

        let adapter =
            StateAdapter::for_observed(&state).expect("the published V1 era must be supported");

        assert_eq!(adapter.role, PackageRole::Registry);
    }

    #[cfg(feature = "upgrade_test")]
    #[test]
    fn upgrade_fixture_accepts_new_witness_with_retained_inner_origin() {
        let state = ObservedState {
            object_id: address("0x10"),
            owner: sui::types::Owner::Shared(3),
            anchor_type: struct_tag("0xa1", "tool_registry", "ToolRegistry"),
            witness: ObservedStateField {
                field_id: address("0x11"),
                value_type: struct_tag("0xa2", "era", "V2"),
            },
            inner: ObservedStateField {
                field_id: address("0x12"),
                value_type: struct_tag("0xa1", "tool_registry", "ToolRegistryInnerV1"),
            },
        };

        let adapter = StateAdapter::for_observed(&state).expect("fixture pair is supported");
        assert_eq!(adapter.role, PackageRole::Tool);
    }

    #[cfg(feature = "upgrade_test")]
    #[test]
    fn upgrade_fixture_accepts_declared_agent_registry_layout_change() {
        let state = ObservedState {
            object_id: address("0x20"),
            owner: sui::types::Owner::Shared(3),
            anchor_type: struct_tag("0xb1", "agent_registry", "AgentRegistry"),
            witness: ObservedStateField {
                field_id: address("0x21"),
                value_type: struct_tag("0xb2", "era", "V2"),
            },
            inner: ObservedStateField {
                field_id: address("0x22"),
                value_type: struct_tag("0xb2", "upgrade_fixture", "AgentRegistryInnerV2"),
            },
        };

        let adapter = StateAdapter::for_observed(&state).expect("fixture pair is supported");
        assert_eq!(adapter.role, PackageRole::Registry);
    }

    fn state_field(
        field_id: &'static str,
        key_name: &'static str,
        value_type: sui::types::StructTag,
    ) -> DynamicFieldMetadata {
        DynamicFieldMetadata {
            field_id: address(field_id),
            key_type: sui::types::TypeTag::Struct(Box::new(struct_tag(
                "0xa0",
                "object_state",
                key_name,
            ))),
            value_type: sui::types::TypeTag::Struct(Box::new(value_type)),
        }
    }

    fn listed_state_field(
        field_id: sui::types::Address,
        key_type: sui::types::TypeTag,
        value_type: sui::types::TypeTag,
    ) -> sui::grpc::DynamicField {
        let field_type = sui::types::StructTag::new(
            address("0x2"),
            sui::types::Identifier::from_static("dynamic_field"),
            sui::types::Identifier::from_static("Field"),
            vec![key_type, value_type.clone()],
        );
        let mut object = sui::grpc::Object::default();
        object.set_object_type(field_type.to_string());
        let mut field = sui::grpc::DynamicField::default();
        field.set_field_id(field_id);
        field.set_field_object(object);
        field.set_value_type(value_type.to_string());
        field
    }

    fn struct_tag(
        package: &'static str,
        module: &'static str,
        name: &'static str,
    ) -> sui::types::StructTag {
        sui::types::StructTag::new(
            address(package),
            sui::types::Identifier::from_static(module),
            sui::types::Identifier::from_static(name),
            vec![],
        )
    }

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }
}
