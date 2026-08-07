//! Immutable package metadata for one activated Nexus protocol configuration.

use {
    crate::sui,
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// Exact identity of one Move datatype within a package family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DatatypeKey {
    /// Move module that declares the datatype.
    pub module: String,
    /// Move datatype name within [`Self::module`].
    pub datatype: String,
}

impl DatatypeKey {
    /// Creates one module and datatype lookup key.
    pub fn new(module: impl Into<String>, datatype: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            datatype: datatype.into(),
        }
    }
}

/// Datatype origins grouped by module and datatype name.
///
/// This shape is shared with generated Move binding package scopes.
pub type TypeOrigins = BTreeMap<String, BTreeMap<String, sui::types::Address>>;

/// One selected version of a package family.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageVersion {
    /// Stable ID of the first package in this upgrade lineage.
    pub initial_id: sui::types::Address,
    /// Immutable package object containing the code used for calls.
    pub storage_id: sui::types::Address,
    /// Sui package version declared by `storage_id`.
    pub version: u64,
    /// Exact package version that first defined each datatype.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub type_origins: TypeOrigins,
}

impl PackageVersion {
    pub fn new(
        initial_id: sui::types::Address,
        storage_id: sui::types::Address,
        version: u64,
        type_origins: TypeOrigins,
    ) -> Self {
        Self {
            initial_id,
            storage_id,
            version,
            type_origins,
        }
    }

    /// Bootstrap metadata for a first publication.
    ///
    /// [`crate::types::NexusObjects::resolve_package_metadata`] replaces the
    /// empty origin map with authoritative Sui package metadata before live use.
    pub fn first_publication(package_id: sui::types::Address) -> Self {
        Self::new(package_id, package_id, 1, TypeOrigins::new())
    }

    /// Resolve one datatype identity, falling back to the stable initial ID.
    pub fn type_origin(&self, module: &str, datatype: &str) -> sui::types::Address {
        self.type_origins
            .get(module)
            .and_then(|types| types.get(datatype))
            .copied()
            .unwrap_or(self.initial_id)
    }

    /// Insert one exact datatype origin and reject conflicting metadata.
    pub fn insert_type_origin(
        &mut self,
        key: DatatypeKey,
        package_id: sui::types::Address,
    ) -> anyhow::Result<()> {
        let previous = self
            .type_origins
            .entry(key.module.clone())
            .or_default()
            .insert(key.datatype.clone(), package_id);
        if previous.is_some_and(|previous| previous != package_id) {
            anyhow::bail!(
                "Datatype '{}::{}' has conflicting package origins",
                key.module,
                key.datatype
            );
        }
        Ok(())
    }

    /// Whether an address belongs to this package family.
    pub fn contains_package(&self, address: sui::types::Address) -> bool {
        address == self.initial_id
            || address == self.storage_id
            || self
                .type_origins
                .values()
                .any(|types| types.values().any(|origin| *origin == address))
    }
}

/// The six package versions selected by one protocol configuration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NexusPackages {
    /// Package version that owns protocol state and common primitives.
    pub primitives: PackageVersion,
    /// Package version that defines shared Nexus interfaces.
    pub interface: PackageVersion,
    /// Package version that owns Tool registration and payment state.
    pub tool: PackageVersion,
    /// Package version that owns Nexus registries.
    pub registry: PackageVersion,
    /// Package version that implements workflow execution.
    pub workflow: PackageVersion,
    /// Package version that implements scheduled execution.
    pub scheduler: PackageVersion,
}

impl NexusPackages {
    /// Creates package metadata for the first publication of every family.
    pub fn first_publication(
        primitives: sui::types::Address,
        interface: sui::types::Address,
        tool: sui::types::Address,
        registry: sui::types::Address,
        workflow: sui::types::Address,
        scheduler: sui::types::Address,
    ) -> Self {
        Self {
            primitives: PackageVersion::first_publication(primitives),
            interface: PackageVersion::first_publication(interface),
            tool: PackageVersion::first_publication(tool),
            registry: PackageVersion::first_publication(registry),
            workflow: PackageVersion::first_publication(workflow),
            scheduler: PackageVersion::first_publication(scheduler),
        }
    }

    /// Returns all package families in canonical protocol role order.
    pub fn all(&self) -> [&PackageVersion; 6] {
        [
            &self.primitives,
            &self.interface,
            &self.tool,
            &self.registry,
            &self.workflow,
            &self.scheduler,
        ]
    }

    /// Whether an address belongs to any configured package family.
    pub fn contains_package(&self, address: sui::types::Address) -> bool {
        self.all()
            .into_iter()
            .any(|package| package.contains_package(address))
    }
}
