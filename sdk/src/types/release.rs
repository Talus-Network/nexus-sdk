//! Immutable package metadata for one activated Nexus protocol release.

use {
    crate::sui,
    serde::{Deserialize, Serialize},
    std::collections::BTreeMap,
};

/// Exact identity of one Move datatype within a package family.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DatatypeKey {
    pub module: String,
    pub datatype: String,
}

impl DatatypeKey {
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

/// One package family at one activated release.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageRelease {
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

impl PackageRelease {
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

/// The six packages that form one coherent Nexus release.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NexusPackages {
    pub primitives: PackageRelease,
    pub interface: PackageRelease,
    pub registry: PackageRelease,
    pub gas: PackageRelease,
    pub workflow: PackageRelease,
    pub scheduler: PackageRelease,
}

impl NexusPackages {
    pub fn first_publication(
        primitives: sui::types::Address,
        interface: sui::types::Address,
        registry: sui::types::Address,
        gas: sui::types::Address,
        workflow: sui::types::Address,
        scheduler: sui::types::Address,
    ) -> Self {
        Self {
            primitives: PackageRelease::first_publication(primitives),
            interface: PackageRelease::first_publication(interface),
            registry: PackageRelease::first_publication(registry),
            gas: PackageRelease::first_publication(gas),
            workflow: PackageRelease::first_publication(workflow),
            scheduler: PackageRelease::first_publication(scheduler),
        }
    }

    pub fn all(&self) -> [&PackageRelease; 6] {
        [
            &self.primitives,
            &self.interface,
            &self.registry,
            &self.gas,
            &self.workflow,
            &self.scheduler,
        ]
    }

    pub fn contains_package(&self, address: sui::types::Address) -> bool {
        self.all()
            .into_iter()
            .any(|package| package.contains_package(address))
    }
}
