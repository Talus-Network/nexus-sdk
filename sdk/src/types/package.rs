//! Immutable package metadata for one Nexus operation.

use {
    crate::{sui, types::NexusObjects},
    serde::{Deserialize, Serialize},
    std::{collections::BTreeMap, ops::Deref, sync::Arc},
};

/// Canonical role of one Nexus Move package family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageRole {
    /// Common Nexus primitives.
    Primitives,
    /// Shared Nexus interfaces.
    Interface,
    /// Tool definitions, lifecycle, and payments.
    Tool,
    /// Canonical Nexus registries.
    Registry,
    /// Workflow execution.
    Workflow,
    /// Scheduled execution.
    Scheduler,
}

impl PackageRole {
    /// Returns the stable metadata name for this role.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primitives => "primitives",
            Self::Interface => "interface",
            Self::Tool => "tool",
            Self::Registry => "registry",
            Self::Workflow => "workflow",
            Self::Scheduler => "scheduler",
        }
    }
}

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

/// Exact immutable target selected for one linked package lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageLink {
    /// Immutable package object selected by the dependency.
    pub storage_id: sui::types::Address,
    /// Sui package version of [`Self::storage_id`].
    pub version: u64,
}

/// Immutable linkage table keyed by initial package lineage ID.
pub type PackageLinkage = BTreeMap<sui::types::Address, PackageLink>;

/// Exact immutable metadata for one package version.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PackageVersion {
    /// Stable ID of the first package in this upgrade lineage.
    pub initial_id: sui::types::Address,
    /// Immutable package object containing the selected code.
    pub storage_id: sui::types::Address,
    /// Sui package version declared by [`Self::storage_id`].
    pub version: u64,
    /// Exact package version that first defined each datatype.
    pub type_origins: TypeOrigins,
    /// Exact dependency versions recorded by the package object.
    pub linkage: PackageLinkage,
}

impl PackageVersion {
    /// Creates complete immutable package metadata.
    pub fn new(
        initial_id: sui::types::Address,
        storage_id: sui::types::Address,
        version: u64,
        type_origins: TypeOrigins,
        linkage: PackageLinkage,
    ) -> Self {
        Self {
            initial_id,
            storage_id,
            version,
            type_origins,
            linkage,
        }
    }

    /// Resolves one datatype identity.
    ///
    /// A missing origin is invalid package metadata rather than permission to
    /// guess the package lineage.
    pub fn type_origin(&self, module: &str, datatype: &str) -> anyhow::Result<sui::types::Address> {
        self.type_origins
            .get(module)
            .and_then(|types| types.get(datatype))
            .copied()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Package '{}' has no type origin for '{module}::{datatype}'",
                    self.storage_id
                )
            })
    }

    /// Inserts one exact datatype origin and rejects conflicting metadata.
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

    /// Returns whether `address` occurs in this package family metadata.
    pub fn contains_package(&self, address: sui::types::Address) -> bool {
        address == self.initial_id
            || address == self.storage_id
            || self
                .type_origins
                .values()
                .any(|types| types.values().any(|origin| *origin == address))
    }
}

/// Package dependency graph selected for one operation.
///
/// A role is [`None`] when the operation does not require that package. This
/// prevents a zero address from being mistaken for resolved package authority.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NexusPackages {
    /// Common primitives when required by the operation.
    pub primitives: Option<PackageVersion>,
    /// Shared interfaces when required by the operation.
    pub interface: Option<PackageVersion>,
    /// Tool package when required by the operation.
    pub tool: Option<PackageVersion>,
    /// Registry package when required by the operation.
    pub registry: Option<PackageVersion>,
    /// Workflow package when required by the operation.
    pub workflow: Option<PackageVersion>,
    /// Scheduler package when required by the operation.
    pub scheduler: Option<PackageVersion>,
}

impl NexusPackages {
    /// Returns the package metadata selected for `role`.
    pub const fn get(&self, role: PackageRole) -> Option<&PackageVersion> {
        match role {
            PackageRole::Primitives => self.primitives.as_ref(),
            PackageRole::Interface => self.interface.as_ref(),
            PackageRole::Tool => self.tool.as_ref(),
            PackageRole::Registry => self.registry.as_ref(),
            PackageRole::Workflow => self.workflow.as_ref(),
            PackageRole::Scheduler => self.scheduler.as_ref(),
        }
    }

    /// Inserts `package` for `role` and returns previous metadata, if any.
    pub fn insert(&mut self, role: PackageRole, package: PackageVersion) -> Option<PackageVersion> {
        match role {
            PackageRole::Primitives => self.primitives.replace(package),
            PackageRole::Interface => self.interface.replace(package),
            PackageRole::Tool => self.tool.replace(package),
            PackageRole::Registry => self.registry.replace(package),
            PackageRole::Workflow => self.workflow.replace(package),
            PackageRole::Scheduler => self.scheduler.replace(package),
        }
    }

    /// Iterates over every package present in canonical role order.
    pub fn all(&self) -> impl Iterator<Item = &PackageVersion> {
        [
            self.primitives.as_ref(),
            self.interface.as_ref(),
            self.tool.as_ref(),
            self.registry.as_ref(),
            self.workflow.as_ref(),
            self.scheduler.as_ref(),
        ]
        .into_iter()
        .flatten()
    }

    /// Returns whether `address` occurs in any selected package family.
    pub fn contains_package(&self, address: sui::types::Address) -> bool {
        self.all().any(|package| package.contains_package(address))
    }
}

/// Stable environment and exact package graph for one operation.
///
/// A [`NexusContext`] is deliberately short lived. Mutable object state is not
/// cached in it, and its [`NexusPackages`] apply only to the operation that
/// requested them.
#[derive(Clone, Debug)]
pub struct NexusContext {
    objects: Arc<NexusObjects>,
    packages: NexusPackages,
}

impl NexusContext {
    /// Creates an operation context from stable environment identity and a
    /// resolved package graph.
    pub fn new(objects: Arc<NexusObjects>, packages: NexusPackages) -> Self {
        Self { objects, packages }
    }

    /// Returns the stable environment identity.
    pub fn objects(&self) -> &NexusObjects {
        &self.objects
    }

    /// Returns the exact package graph for this operation.
    pub const fn packages(&self) -> &NexusPackages {
        &self.packages
    }

    /// Returns package metadata for `role`.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation graph does not contain `role`.
    pub fn require_package(&self, role: PackageRole) -> anyhow::Result<&PackageVersion> {
        self.packages.get(role).ok_or_else(|| {
            anyhow::anyhow!(
                "Operation package graph does not contain the '{}' role",
                role.as_str()
            )
        })
    }

    /// Returns the immutable package selected for `role`.
    ///
    /// # Errors
    ///
    /// Returns the same error as [`Self::require_package`] when the operation
    /// graph does not contain `role`.
    pub fn package_id(&self, role: PackageRole) -> anyhow::Result<sui::types::Address> {
        Ok(self.require_package(role)?.storage_id)
    }

    /// Returns the defining package for one datatype in `role`.
    ///
    /// # Errors
    ///
    /// Returns the errors reported by [`Self::require_package`] and
    /// [`PackageVersion::type_origin`].
    pub fn type_origin(
        &self,
        role: PackageRole,
        module: &str,
        datatype: &str,
    ) -> anyhow::Result<sui::types::Address> {
        self.require_package(role)?.type_origin(module, datatype)
    }
}

impl Deref for NexusContext {
    type Target = NexusObjects;

    fn deref(&self) -> &Self::Target {
        self.objects()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_roles_remain_absent() {
        let mut graph = NexusPackages::default();
        graph.insert(
            PackageRole::Registry,
            PackageVersion::new(
                address("0xa1"),
                address("0xa2"),
                2,
                TypeOrigins::new(),
                PackageLinkage::new(),
            ),
        );

        assert!(graph.registry.is_some());
        assert!(graph.workflow.is_none());
        assert_eq!(graph.all().count(), 1);
    }

    #[test]
    fn missing_type_origin_is_not_guessed() {
        let package = PackageVersion::new(
            address("0xa1"),
            address("0xa2"),
            2,
            TypeOrigins::new(),
            PackageLinkage::new(),
        );

        assert!(package.type_origin("era", "V1").is_err());
    }

    #[cfg(feature = "test_utils")]
    #[test]
    fn context_resolves_package_and_datatype_identity() {
        let objects = Arc::new(crate::test_utils::sui_mocks::mock_nexus_objects());
        let packages = crate::test_utils::sui_mocks::mock_nexus_packages();
        let expected = packages.registry.as_ref().unwrap();
        let expected_package = expected.storage_id;
        let expected_origin = expected.type_origin("leader", "LeaderRegistry").unwrap();
        let context = NexusContext::new(objects, packages);

        assert_eq!(
            context.package_id(PackageRole::Registry).unwrap(),
            expected_package
        );
        assert_eq!(
            context
                .type_origin(PackageRole::Registry, "leader", "LeaderRegistry")
                .unwrap(),
            expected_origin
        );
    }

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }
}
