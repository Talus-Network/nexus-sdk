//! Live object state and package authority resolution.

use {
    crate::{
        move_bindings::{
            move_std::{option::Option as MoveOption, type_name::TypeName},
            sui_framework::object::{ID, UID},
        },
        nexus::{
            crawler::{Crawler, DynamicFieldMetadata, ObjectMetadata},
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
    serde::{de::DeserializeOwned, Deserialize},
    std::{collections::HashMap, sync::Arc, time::Duration},
    talus_sui_move::MoveStruct,
    tokio::{sync::RwLock, time::sleep},
};

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
    package_cache: Arc<RwLock<HashMap<sui::types::Address, Arc<PackageVersion>>>>,
}

impl StateResolver {
    /// Creates a resolver backed by `crawler`.
    pub fn new(crawler: Arc<Crawler>) -> Self {
        Self {
            crawler,
            package_cache: Arc::default(),
        }
    }

    /// Observes the exact anchor, witness, and inner types for `object_id`.
    ///
    /// # Errors
    ///
    /// Returns [`NexusError::InvalidObjectState`] when the anchor does not have
    /// exactly one `Inner` field and one `Witness` field, or either value is not
    /// a Move struct. Transport failures return [`NexusError::Rpc`].
    pub async fn observe(
        &self,
        object_id: sui::types::Address,
    ) -> Result<ObservedState, NexusError> {
        for attempt in 0..STATE_OBSERVATION_ATTEMPTS {
            let (object, fields) = tokio::join!(
                self.crawler.observe_object_metadata(object_id),
                self.crawler.get_dynamic_field_metadata(object_id),
            );
            let (object, fields) = match (object, fields) {
                (Ok(object), Ok(fields)) => (object, fields),
                (Err(_error), _) | (_, Err(_error)) if attempt + 1 < STATE_OBSERVATION_ATTEMPTS => {
                    sleep(STATE_OBSERVATION_RETRY_DELAY).await;
                    continue;
                }
                (Err(error), _) | (_, Err(error)) => return Err(NexusError::Rpc(error)),
            };

            match observed_state_from_metadata(object, fields) {
                Ok(observed) => return Ok(observed),
                Err(reason)
                    if state_observation_may_be_incomplete(&reason)
                        && attempt + 1 < STATE_OBSERVATION_ATTEMPTS =>
                {
                    sleep(STATE_OBSERVATION_RETRY_DELAY).await;
                }
                Err(reason) => {
                    return Err(NexusError::InvalidObjectState {
                        object: object_id,
                        reason,
                    });
                }
            }
        }

        unreachable!("the observation loop returns on its final attempt")
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
            .map_err(NexusError::Rpc)?;
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
        for root in required_roots {
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
        }
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
        for (lineage, link) in &source.linkage {
            if link.storage_id == sui::types::Address::from_static("0x1")
                || link.storage_id == sui::types::Address::from_static("0x2")
            {
                continue;
            }
            let linked = self.resolve_package(link.storage_id).await?;
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
        let observed = self
            .validate_state_pair::<A, W, V>(object_id, context)
            .await?;

        self.decode_inner::<A, V>(&observed).await
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
        let observed = self.observe(object_id).await?;
        let adapter = StateAdapter::for_observed(&observed)?;
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
                    object_id,
                    observed.witness_type().address()
                ),
            });
        }
        let expected_anchor = state_struct_tag::<A>(package)?;
        let expected_inner = state_struct_tag::<V>(package)?;
        if observed.anchor_type != expected_anchor || observed.inner.value_type != expected_inner {
            return Err(ClientUpgradeRequired::new(
                object_id,
                observed.witness.value_type,
                Some(observed.inner.value_type),
            )
            .into());
        }

        self.decode_inner::<A, V>(&observed).await
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
        let expected_anchor = crate::move_bindings::struct_tag::<A>(context);
        let expected_witness = crate::move_bindings::struct_tag::<W>(context);
        let expected_inner = crate::move_bindings::struct_tag::<V>(context);
        if observed.anchor_type != expected_anchor
            || observed.witness.value_type != expected_witness
            || observed.inner.value_type != expected_inner
        {
            return Err(ClientUpgradeRequired::new(
                object_id,
                observed.witness.value_type,
                Some(observed.inner.value_type),
            )
            .into());
        }

        Ok(observed)
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
        let observed = self.observe(object_id).await?;
        let adapter = StateAdapter::for_observed(&observed)?;
        let packages = self.resolve_package_graph(&observed).await?;
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
                observed.witness.value_type,
                Some(observed.inner.value_type),
            )
            .into());
        }

        self.decode_inner::<A, V>(&observed).await
    }

    async fn decode_inner<A, V>(
        &self,
        observed: &ObservedState,
    ) -> Result<crate::nexus::crawler::Response<V>, NexusError>
    where
        A: DeserializeOwned,
        V: DeserializeOwned,
    {
        let object_id = observed.object_id;

        let anchor = self
            .crawler
            .get_object::<A>(object_id)
            .await
            .map_err(NexusError::Rpc)?;
        let inner = self
            .crawler
            .get_dynamic_field_value_by_id::<
                crate::move_bindings::primitives::object_state::Inner,
                V,
            >(observed.inner.field_id)
            .await
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

    #[tokio::test]
    async fn observation_retries_incomplete_dynamic_field_metadata() {
        use {
            crate::test_utils::sui_mocks,
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

        let mut ledger = sui_mocks::grpc::MockLedgerService::new();
        ledger
            .expect_get_object()
            .times(2)
            .returning(move |_request| {
                let mut object = sui::grpc::Object::default();
                object.set_object_id(object_id);
                object.set_owner(sui::grpc::Owner::from(sui::types::Owner::Shared(1)));
                object.set_object_type(anchor_type.to_string());
                let mut response = sui::grpc::GetObjectResponse::default();
                response.set_object(object);
                Ok(tonic::Response::new(response))
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
                            address("0x11"),
                            witness_key.clone(),
                            witness_type.clone(),
                        ),
                        listed_state_field(address("0x12"), inner_key.clone(), inner_type.clone()),
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
        assert_eq!(observed.witness.field_id, address("0x11"));
        assert_eq!(observed.inner.field_id, address("0x12"));
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
