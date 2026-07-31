//! Activated Nexus release and durable object references.

#[cfg(test)]
use crate::move_bindings::{
    gas::gas as gas_move,
    interface::dag as dag_move,
    primitives::event as event_move,
    registry::agent_registry as agent_registry_move,
    scheduler::task as scheduler_task_move,
    workflow::execution as execution_move,
};
#[cfg(feature = "nexus")]
use {
    crate::sui::traits::FieldMaskUtil as _,
    std::{collections::BTreeMap, sync::Arc},
};
use {
    crate::{
        move_bindings::{
            interface::{
                agent as agent_move,
                authorization as authorization_move,
                dag as dag_move_common,
                payment as payment_move,
                version as version_move,
            },
            sui_framework::coin::Coin as MoveCoin,
            talus::us::US,
        },
        sui,
        types::{DatatypeKey, DefaultDagExecutorTarget, NexusPackages, PackageRelease},
    },
    serde::{de::Error as _, Deserialize, Deserializer, Serialize},
    sui_move::{MoveStruct, MoveType},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsTokenConfig {
    pub package_id: sui::types::Address,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protected_treasury: Option<sui::types::Address>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<sui::types::Address>,
}

impl Default for UsTokenConfig {
    fn default() -> Self {
        Self::new(sui::types::Address::ZERO)
    }
}

impl UsTokenConfig {
    pub fn new(package_id: sui::types::Address) -> Self {
        Self {
            package_id,
            protected_treasury: None,
            metadata: None,
        }
    }

    pub fn type_tag(&self) -> sui::types::TypeTag {
        crate::move_bindings::talus::with_packages(
            self.package_id,
            self.package_id,
            US::type_tag_static,
        )
    }

    pub fn coin_type_tag(&self) -> sui::types::StructTag {
        crate::move_bindings::sui_framework::with_packages(
            sui::types::Address::from_static("0x2"),
            sui::types::Address::from_static("0x2"),
            || {
                crate::move_bindings::talus::with_packages(
                    self.package_id,
                    self.package_id,
                    MoveCoin::<US>::struct_tag_static,
                )
            },
        )
    }

    pub fn qualified_type(&self) -> String {
        let tag = crate::move_bindings::talus::with_packages(
            self.package_id,
            self.package_id,
            US::struct_tag_static,
        );
        format!("{}::{}::{}", tag.address(), tag.module(), tag.name())
    }
}

/// One validated and immutable Nexus release snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NexusObjects {
    pub release: u64,
    pub protocol: sui::types::ObjectReference,
    pub packages: NexusPackages,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manifest_hash: Vec<u8>,
    pub network_id: sui::types::Address,
    pub tool_registry: sui::types::ObjectReference,
    pub verifier_registry: sui::types::ObjectReference,
    pub network_auth: sui::types::ObjectReference,
    pub agent_registry: sui::types::ObjectReference,
    pub default_dag_executor: DefaultDagExecutorTarget,
    pub gas_service: sui::types::ObjectReference,
    pub leader_registry: sui::types::ObjectReference,
    pub priority_fee_vault: sui::types::ObjectReference,
    #[serde(default = "default_object_reference")]
    pub priority_fee_vault_owner_cap: sui::types::ObjectReference,
    #[serde(default)]
    pub us_token: UsTokenConfig,
}

#[derive(Deserialize)]
struct NexusObjectsWire {
    #[serde(default)]
    release: Option<u64>,
    #[serde(default)]
    active_release: Option<u64>,
    #[serde(default = "default_object_reference")]
    protocol: sui::types::ObjectReference,
    #[serde(default)]
    packages: Option<NexusPackages>,
    #[serde(default)]
    manifest_hash: Vec<u8>,
    network_id: sui::types::Address,
    tool_registry: sui::types::ObjectReference,
    verifier_registry: sui::types::ObjectReference,
    network_auth: sui::types::ObjectReference,
    agent_registry: sui::types::ObjectReference,
    default_dag_executor: DefaultDagExecutorTarget,
    gas_service: sui::types::ObjectReference,
    leader_registry: sui::types::ObjectReference,
    priority_fee_vault: sui::types::ObjectReference,
    #[serde(default = "default_object_reference")]
    priority_fee_vault_owner_cap: sui::types::ObjectReference,
    #[serde(default)]
    us_token: UsTokenConfig,

    #[serde(default)]
    primitives_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    interface_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    registry_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    gas_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    workflow_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    scheduler_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    primitives_original_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    interface_original_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    registry_original_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    gas_original_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    workflow_original_pkg_id: Option<sui::types::Address>,
    #[serde(default)]
    scheduler_original_pkg_id: Option<sui::types::Address>,
}

impl<'de> Deserialize<'de> for NexusObjects {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = NexusObjectsWire::deserialize(deserializer)?;
        let packages = match wire.packages {
            Some(packages) => packages,
            None => NexusPackages {
                primitives: legacy_package(
                    "primitives_pkg_id",
                    wire.primitives_pkg_id,
                    wire.primitives_original_pkg_id,
                )
                .map_err(D::Error::custom)?,
                interface: legacy_package(
                    "interface_pkg_id",
                    wire.interface_pkg_id,
                    wire.interface_original_pkg_id,
                )
                .map_err(D::Error::custom)?,
                registry: legacy_package(
                    "registry_pkg_id",
                    wire.registry_pkg_id,
                    wire.registry_original_pkg_id,
                )
                .map_err(D::Error::custom)?,
                gas: legacy_package("gas_pkg_id", wire.gas_pkg_id, wire.gas_original_pkg_id)
                    .map_err(D::Error::custom)?,
                workflow: legacy_package(
                    "workflow_pkg_id",
                    wire.workflow_pkg_id,
                    wire.workflow_original_pkg_id,
                )
                .map_err(D::Error::custom)?,
                scheduler: legacy_package(
                    "scheduler_pkg_id",
                    wire.scheduler_pkg_id,
                    wire.scheduler_original_pkg_id,
                )
                .map_err(D::Error::custom)?,
            },
        };

        Ok(Self {
            release: wire.release.or(wire.active_release).unwrap_or_default(),
            protocol: wire.protocol,
            packages,
            manifest_hash: wire.manifest_hash,
            network_id: wire.network_id,
            tool_registry: wire.tool_registry,
            verifier_registry: wire.verifier_registry,
            network_auth: wire.network_auth,
            agent_registry: wire.agent_registry,
            default_dag_executor: wire.default_dag_executor,
            gas_service: wire.gas_service,
            leader_registry: wire.leader_registry,
            priority_fee_vault: wire.priority_fee_vault,
            priority_fee_vault_owner_cap: wire.priority_fee_vault_owner_cap,
            us_token: wire.us_token,
        })
    }
}

fn legacy_package(
    field: &'static str,
    storage_id: Option<sui::types::Address>,
    initial_id: Option<sui::types::Address>,
) -> Result<PackageRelease, String> {
    let storage_id = storage_id.ok_or_else(|| format!("missing field `{field}`"))?;
    Ok(PackageRelease::new(
        initial_id.unwrap_or(storage_id),
        storage_id,
        0,
        Default::default(),
    ))
}

pub(crate) fn default_object_reference() -> sui::types::ObjectReference {
    sui::types::ObjectReference::new(sui::types::Address::ZERO, 1, sui::types::Digest::ZERO)
}

impl NexusObjects {
    /// Resolve authoritative storage IDs, lineage, versions, and exact type
    /// origins for all six configured package targets.
    #[cfg(feature = "nexus")]
    pub async fn resolve_package_metadata(
        &mut self,
        client: &Arc<sui::grpc::Client>,
    ) -> anyhow::Result<()> {
        let (primitives, interface, registry, gas, workflow, scheduler) = tokio::try_join!(
            resolve_package_release(client, &self.packages.primitives, "primitives"),
            resolve_package_release(client, &self.packages.interface, "interface"),
            resolve_package_release(client, &self.packages.registry, "registry"),
            resolve_package_release(client, &self.packages.gas, "gas"),
            resolve_package_release(client, &self.packages.workflow, "workflow"),
            resolve_package_release(client, &self.packages.scheduler, "scheduler"),
        )?;
        self.packages = NexusPackages {
            primitives,
            interface,
            registry,
            gas,
            workflow,
            scheduler,
        };
        Ok(())
    }

    /// Compatibility alias for callers that previously resolved one origin per
    /// package. The method now resolves every datatype origin.
    #[cfg(feature = "nexus")]
    pub async fn resolve_original_pkg_ids(
        &mut self,
        client: &Arc<sui::grpc::Client>,
    ) -> anyhow::Result<()> {
        self.resolve_package_metadata(client).await
    }

    pub fn primitives_pkg_id(&self) -> sui::types::Address {
        self.packages.primitives.storage_id
    }

    pub fn interface_pkg_id(&self) -> sui::types::Address {
        self.packages.interface.storage_id
    }

    pub fn registry_pkg_id(&self) -> sui::types::Address {
        self.packages.registry.storage_id
    }

    pub fn gas_pkg_id(&self) -> sui::types::Address {
        self.packages.gas.storage_id
    }

    pub fn workflow_pkg_id(&self) -> sui::types::Address {
        self.packages.workflow.storage_id
    }

    pub fn scheduler_pkg_id(&self) -> sui::types::Address {
        self.packages.scheduler.storage_id
    }

    pub fn primitives_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages
            .primitives
            .type_origin("event", "EventWrapper")
    }

    pub fn interface_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages.interface.type_origin("graph", "Vertex")
    }

    pub fn registry_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages
            .registry
            .type_origin("network_auth", "IdentityKey")
    }

    pub fn gas_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages.gas.type_origin("gas", "ToolGas")
    }

    pub fn workflow_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages
            .workflow
            .type_origin("execution", "DAGExecution")
    }

    pub fn scheduler_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages.scheduler.type_origin("task", "Task")
    }

    pub fn is_primitives_package(&self, address: sui::types::Address) -> bool {
        self.packages.primitives.contains_package(address)
    }

    pub fn is_interface_package(&self, address: sui::types::Address) -> bool {
        self.packages.interface.contains_package(address)
    }

    pub fn is_registry_package(&self, address: sui::types::Address) -> bool {
        self.packages.registry.contains_package(address)
    }

    pub fn is_gas_package(&self, address: sui::types::Address) -> bool {
        self.packages.gas.contains_package(address)
    }

    pub fn is_workflow_package(&self, address: sui::types::Address) -> bool {
        self.packages.workflow.contains_package(address)
    }

    pub fn is_scheduler_package(&self, address: sui::types::Address) -> bool {
        self.packages.scheduler.contains_package(address)
    }

    pub fn is_nexus_package(&self, address: sui::types::Address) -> bool {
        self.packages.contains_package(address)
    }

    /// Whether a Sui event was emitted by code in this activated release.
    pub fn is_active_emitter(&self, address: sui::types::Address) -> bool {
        self.packages
            .all()
            .into_iter()
            .any(|package| package.storage_id == address)
    }

    /// Returns whether `package_id` is a valid top level event source for this
    /// release.
    ///
    /// Sui records the package containing the top level Move call as an
    /// event's source. A direct Nexus call is valid only from one exact active
    /// storage package. A composed call from an external package is valid only
    /// when every Nexus lineage in its transitive linkage table resolves to the
    /// exact package ID and version in [`Self::packages`].
    #[cfg(feature = "nexus")]
    pub async fn is_compatible_event_source(
        &self,
        client: &Arc<sui::grpc::Client>,
        package_id: sui::types::Address,
    ) -> anyhow::Result<bool> {
        if self.is_active_emitter(package_id) {
            return Ok(true);
        }

        let request = sui::grpc::GetPackageRequest::default().with_package_id(package_id);
        let package = client
            .as_ref()
            .clone()
            .package_client()
            .get_package(request)
            .await
            .map_err(|error| {
                anyhow::anyhow!("Failed to fetch event source package '{package_id}': {error}")
            })?
            .into_inner()
            .package
            .ok_or_else(|| {
                anyhow::anyhow!("Event source package '{package_id}' was not returned")
            })?;

        self.package_uses_active_release(package_id, &package)
    }

    #[cfg(feature = "nexus")]
    fn package_uses_active_release(
        &self,
        package_id: sui::types::Address,
        package: &sui::grpc::Package,
    ) -> anyhow::Result<bool> {
        let storage_id =
            parse_package_address(package.storage_id.as_deref(), "event source", "storage_id")?;
        if storage_id != package_id {
            anyhow::bail!(
                "Event source package returned storage ID '{storage_id}', expected '{package_id}'"
            );
        }

        let original_id = parse_package_address(
            package.original_id.as_deref(),
            "event source",
            "original_id",
        )?;
        if self
            .packages
            .all()
            .into_iter()
            .any(|active| active.initial_id == original_id)
        {
            return Ok(false);
        }

        let active_by_origin = self
            .packages
            .all()
            .into_iter()
            .map(|active| (active.initial_id, active))
            .collect::<BTreeMap<_, _>>();
        let mut nexus_link_found = false;
        let mut seen_origins = BTreeMap::new();

        for link in &package.linkage {
            let linked_original = parse_package_address(
                link.original_id.as_deref(),
                "event source",
                "linkage original_id",
            )?;
            let linked_storage = parse_package_address(
                link.upgraded_id.as_deref(),
                "event source",
                "linkage upgraded_id",
            )?;
            let linked_version = link.upgraded_version.ok_or_else(|| {
                anyhow::anyhow!(
                    "Event source package linkage for '{linked_original}' has no upgraded version"
                )
            })?;
            if seen_origins
                .insert(linked_original, (linked_storage, linked_version))
                .is_some()
            {
                anyhow::bail!(
                    "Event source package contains duplicate linkage for '{linked_original}'"
                );
            }

            let Some(active) = active_by_origin.get(&linked_original) else {
                continue;
            };
            nexus_link_found = true;
            if linked_storage != active.storage_id || linked_version != active.version {
                return Ok(false);
            }
        }

        Ok(nexus_link_found)
    }

    /// Returns true when the wrapped event datatype belongs to this Nexus
    /// package family and has a recognized interface shape.
    pub fn is_event_from_nexus(&self, event: &sui::types::Event) -> bool {
        let Some(sui::types::TypeTag::Struct(inner_tag)) = event.type_.type_params().first() else {
            return false;
        };

        if self.is_gas_package(*inner_tag.address())
            || self.is_workflow_package(*inner_tag.address())
            || self.is_scheduler_package(*inner_tag.address())
            || self.is_registry_package(*inner_tag.address())
        {
            return true;
        }

        self.is_interface_package(*inner_tag.address())
            && (self.interface_module_matches::<agent_move::Agent>(inner_tag.module())
                || self.interface_module_matches::<authorization_move::AgentVertexAuthorization>(
                    inner_tag.module(),
                )
                || self
                    .interface_module_matches::<payment_move::ExecutionPayment>(inner_tag.module())
                || self
                    .interface_module_matches::<version_move::InterfaceVersion>(inner_tag.module())
                || self.interface_module_matches::<dag_move_common::DAG>(inner_tag.module()))
    }

    fn interface_module_matches<T>(&self, module: &sui::types::Identifier) -> bool
    where
        T: MoveStruct,
    {
        let tag = crate::move_bindings::struct_tag::<T>(self);
        module == tag.module()
    }
}

#[cfg(feature = "nexus")]
pub(crate) async fn resolve_package_release(
    client: &Arc<sui::grpc::Client>,
    expected: &PackageRelease,
    package_name: &str,
) -> anyhow::Result<PackageRelease> {
    resolve_package_release_metadata(client, expected, package_name)
        .await
        .map(|metadata| metadata.release)
}

#[cfg(feature = "nexus")]
#[derive(Clone, Debug)]
pub(crate) struct PackageLink {
    pub storage_id: sui::types::Address,
    pub version: u64,
}

#[cfg(feature = "nexus")]
#[derive(Clone, Debug)]
pub(crate) struct ResolvedPackageRelease {
    pub release: PackageRelease,
    pub linkage: BTreeMap<sui::types::Address, PackageLink>,
}

#[cfg(feature = "nexus")]
pub(crate) async fn resolve_package_release_metadata(
    client: &Arc<sui::grpc::Client>,
    expected: &PackageRelease,
    package_name: &str,
) -> anyhow::Result<ResolvedPackageRelease> {
    let request = sui::grpc::GetPackageRequest::default().with_package_id(expected.storage_id);
    let package = client
        .as_ref()
        .clone()
        .package_client()
        .get_package(request)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to fetch {package_name} package: {error}"))?
        .into_inner()
        .package
        .ok_or_else(|| anyhow::anyhow!("{package_name} package was not returned"))?;

    let (storage_id, initial_id, version) =
        validate_package_header(expected, package_name, &package)?;

    // The ABI service does not guarantee that it returns package linkage and
    // type origins. Read those immutable tables from the package object itself.
    let request = sui::grpc::GetObjectRequest::default()
        .with_object_id(storage_id)
        .with_read_mask(sui::grpc::FieldMask::from_paths([
            "object_id",
            "version",
            "object_type",
            "package",
        ]));
    let package_object = client
        .as_ref()
        .clone()
        .ledger_client()
        .get_object(request)
        .await
        .map_err(|error| anyhow::anyhow!("Failed to fetch {package_name} package object: {error}"))?
        .into_inner()
        .object
        .ok_or_else(|| anyhow::anyhow!("{package_name} package object was not returned"))?;

    validate_package_object(
        package,
        package_object,
        package_name,
        storage_id,
        initial_id,
        version,
    )
}

#[cfg(feature = "nexus")]
fn validate_package_header(
    expected: &PackageRelease,
    package_name: &str,
    package: &sui::grpc::Package,
) -> anyhow::Result<(sui::types::Address, sui::types::Address, u64)> {
    let storage_id =
        parse_package_address(package.storage_id.as_deref(), package_name, "storage_id")?;
    if storage_id != expected.storage_id {
        anyhow::bail!(
            "{package_name} package returned storage ID '{storage_id}', expected '{}'",
            expected.storage_id
        );
    }
    let initial_id =
        parse_package_address(package.original_id.as_deref(), package_name, "original_id")?;
    if expected.initial_id != sui::types::Address::ZERO
        && (expected.version != 0 || expected.initial_id != expected.storage_id)
        && initial_id != expected.initial_id
    {
        anyhow::bail!(
            "{package_name} belongs to lineage '{initial_id}', expected '{}'",
            expected.initial_id
        );
    }
    let version = package
        .version
        .ok_or_else(|| anyhow::anyhow!("{package_name} package version is missing"))?;
    if expected.version != 0 && version != expected.version {
        anyhow::bail!(
            "{package_name} package version is '{version}', expected '{}'",
            expected.version
        );
    }
    Ok((storage_id, initial_id, version))
}

#[cfg(feature = "nexus")]
fn validate_package_object(
    package: sui::grpc::Package,
    package_object: sui::grpc::Object,
    package_name: &str,
    storage_id: sui::types::Address,
    initial_id: sui::types::Address,
    version: u64,
) -> anyhow::Result<ResolvedPackageRelease> {
    let object_id = parse_package_address(
        package_object.object_id.as_deref(),
        package_name,
        "object_id",
    )?;
    if object_id != storage_id {
        anyhow::bail!(
            "{package_name} package object returned ID '{object_id}', expected '{storage_id}'"
        );
    }
    let object_version = package_object
        .version
        .ok_or_else(|| anyhow::anyhow!("{package_name} package object version is missing"))?;
    if object_version != version {
        anyhow::bail!(
            "{package_name} package object has version '{object_version}', \
             but its ABI reports '{version}'"
        );
    }
    if package_object.object_type.as_deref() != Some("package") {
        anyhow::bail!(
            "{package_name} object '{storage_id}' has type '{}', expected 'package'",
            package_object.object_type.as_deref().unwrap_or("<missing>")
        );
    }
    let exact_package = package_object
        .package
        .ok_or_else(|| anyhow::anyhow!("{package_name} package object metadata is missing"))?;

    let mut release = PackageRelease::new(initial_id, storage_id, version, Default::default());
    for origin in &exact_package.type_origins {
        let (key, package_id) = parse_type_origin(origin, package_name)?;
        release.insert_type_origin(key, package_id)?;
    }

    // When the ABI service also supplies an origin table, require it to agree
    // with the immutable package object rather than treating it as a fallback.
    for origin in &package.type_origins {
        let (key, package_id) = parse_type_origin(origin, package_name)?;
        require_type_origin(&release, &key, package_id, package_name)?;
    }

    for module in &package.modules {
        let module_name = module
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("{package_name} contains a module without a name"))?;
        for datatype in &module.datatypes {
            let datatype_name = datatype.name.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "{package_name} module '{module_name}' contains an unnamed datatype"
                )
            })?;
            if datatype.module.as_deref() != Some(module_name.as_str()) {
                anyhow::bail!(
                    "{package_name} datatype '{module_name}::{datatype_name}' declares module '{}'",
                    datatype.module.as_deref().unwrap_or("<missing>")
                );
            }
            let defining_id = parse_package_address(
                datatype.defining_id.as_deref(),
                package_name,
                "datatype defining_id",
            )?;
            let type_name = datatype.type_name.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "{package_name} datatype '{module_name}::{datatype_name}' has no type name"
                )
            })?;
            let tag: sui::types::StructTag = type_name.parse().map_err(|error| {
                anyhow::anyhow!(
                    "{package_name} datatype '{module_name}::{datatype_name}' has invalid type \
                     name '{type_name}': {error}"
                )
            })?;
            if *tag.address() != storage_id
                || tag.module().as_str() != module_name
                || tag.name().as_str() != datatype_name
            {
                anyhow::bail!(
                    "{package_name} datatype '{module_name}::{datatype_name}' has inconsistent \
                     runtime type name '{type_name}' for storage ID '{storage_id}'"
                );
            }
            let key = DatatypeKey::new(module_name.clone(), datatype_name.clone());
            require_type_origin(&release, &key, defining_id, package_name)?;
        }
    }

    let mut linkage = BTreeMap::new();
    for link in exact_package.linkage {
        let original_id = parse_package_address(
            link.original_id.as_deref(),
            package_name,
            "linkage original_id",
        )?;
        let storage_id = parse_package_address(
            link.upgraded_id.as_deref(),
            package_name,
            "linkage upgraded_id",
        )?;
        let version = link.upgraded_version.ok_or_else(|| {
            anyhow::anyhow!("{package_name} linkage for '{original_id}' has no upgraded version")
        })?;
        if let Some(previous) = linkage.insert(
            original_id,
            PackageLink {
                storage_id,
                version,
            },
        ) {
            anyhow::bail!(
                "{package_name} contains duplicate linkage for '{original_id}' \
                 ('{}' and '{storage_id}')",
                previous.storage_id
            );
        }
    }

    Ok(ResolvedPackageRelease { release, linkage })
}

#[cfg(feature = "nexus")]
fn parse_package_address(
    value: Option<&str>,
    package_name: &str,
    field: &str,
) -> anyhow::Result<sui::types::Address> {
    value
        .ok_or_else(|| anyhow::anyhow!("{package_name} package {field} is missing"))?
        .parse()
        .map_err(|error| anyhow::anyhow!("{package_name} package {field} is invalid: {error}"))
}

#[cfg(feature = "nexus")]
fn parse_type_origin(
    origin: &sui::grpc::TypeOrigin,
    package_name: &str,
) -> anyhow::Result<(DatatypeKey, sui::types::Address)> {
    let module = origin
        .module_name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("{package_name} contains a type origin without a module"))?;
    let datatype = origin.datatype_name.as_ref().ok_or_else(|| {
        anyhow::anyhow!("{package_name} contains a type origin without a datatype")
    })?;
    let package_id = parse_package_address(
        origin.package_id.as_deref(),
        package_name,
        "type origin package_id",
    )?;
    Ok((
        DatatypeKey::new(module.clone(), datatype.clone()),
        package_id,
    ))
}

#[cfg(feature = "nexus")]
fn require_type_origin(
    release: &PackageRelease,
    key: &DatatypeKey,
    expected: sui::types::Address,
    package_name: &str,
) -> anyhow::Result<()> {
    let observed = release
        .type_origins
        .get(&key.module)
        .and_then(|types| types.get(&key.datatype))
        .copied();
    if observed != Some(expected) {
        anyhow::bail!(
            "{package_name} datatype '{}::{}' has origin '{}', expected '{expected}'",
            key.module,
            key.datatype,
            observed
                .map(|origin| origin.to_string())
                .unwrap_or_else(|| "<missing>".to_owned())
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn object_ref(value: &'static str) -> sui::types::ObjectReference {
        sui::types::ObjectReference::new(address(value), 1, sui::types::Digest::ZERO)
    }

    fn type_origin(
        module: &str,
        datatype: &str,
        package_id: sui::types::Address,
    ) -> sui::grpc::TypeOrigin {
        let mut origin = sui::grpc::TypeOrigin::default();
        origin.module_name = Some(module.to_owned());
        origin.datatype_name = Some(datatype.to_owned());
        origin.package_id = Some(package_id.to_string());
        origin
    }

    fn package_metadata_fixture() -> (PackageRelease, sui::grpc::Package, sui::grpc::Object) {
        let initial_id = address("0xa1");
        let storage_id = address("0xa2");
        let expected = PackageRelease::new(initial_id, storage_id, 2, Default::default());
        let origin = type_origin("sample", "Thing", initial_id);

        let mut datatype = sui::grpc::DatatypeDescriptor::default();
        datatype.type_name = Some(format!("{storage_id}::sample::Thing"));
        datatype.defining_id = Some(initial_id.to_string());
        datatype.module = Some("sample".to_owned());
        datatype.name = Some("Thing".to_owned());
        let mut module = sui::grpc::Module::default();
        module.name = Some("sample".to_owned());
        module.datatypes = vec![datatype];

        let mut package = sui::grpc::Package::default();
        package.storage_id = Some(storage_id.to_string());
        package.original_id = Some(initial_id.to_string());
        package.version = Some(2);
        package.modules = vec![module];
        package.type_origins = vec![origin.clone()];

        let mut linkage = sui::grpc::Linkage::default();
        linkage.original_id = Some(address("0xb1").to_string());
        linkage.upgraded_id = Some(address("0xb2").to_string());
        linkage.upgraded_version = Some(3);
        let mut exact_package = sui::grpc::Package::default();
        exact_package.type_origins = vec![origin];
        exact_package.linkage = vec![linkage];

        let mut package_object = sui::grpc::Object::default();
        package_object.object_id = Some(storage_id.to_string());
        package_object.version = Some(2);
        package_object.object_type = Some("package".to_owned());
        package_object.package = Some(exact_package);

        (expected, package, package_object)
    }

    fn validate_package_fixture(
        expected: &PackageRelease,
        package: sui::grpc::Package,
        package_object: sui::grpc::Object,
    ) -> anyhow::Result<ResolvedPackageRelease> {
        let (storage_id, initial_id, version) =
            validate_package_header(expected, "sample", &package)?;
        validate_package_object(
            package,
            package_object,
            "sample",
            storage_id,
            initial_id,
            version,
        )
    }

    fn package_header_error(modify: impl FnOnce(&mut sui::grpc::Package)) -> String {
        let (expected, mut package, _) = package_metadata_fixture();
        modify(&mut package);
        validate_package_header(&expected, "sample", &package)
            .unwrap_err()
            .to_string()
    }

    fn package_object_error(
        modify: impl FnOnce(&mut sui::grpc::Package, &mut sui::grpc::Object),
    ) -> String {
        let (expected, mut package, mut package_object) = package_metadata_fixture();
        modify(&mut package, &mut package_object);
        validate_package_fixture(&expected, package, package_object)
            .unwrap_err()
            .to_string()
    }

    fn sample_objects() -> NexusObjects {
        NexusObjects {
            release: 1,
            protocol: object_ref("0x10"),
            packages: NexusPackages::first_publication(
                address("0x11"),
                address("0x12"),
                address("0x13"),
                address("0x14"),
                address("0x15"),
                address("0x16"),
            ),
            manifest_hash: vec![7; 32],
            network_id: address("0x20"),
            tool_registry: object_ref("0x21"),
            verifier_registry: object_ref("0x22"),
            network_auth: object_ref("0x23"),
            agent_registry: object_ref("0x24"),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: address("0x25"),
                skill_id: 1,
            },
            gas_service: object_ref("0x26"),
            leader_registry: object_ref("0x27"),
            priority_fee_vault: object_ref("0x28"),
            priority_fee_vault_owner_cap: object_ref("0x29"),
            us_token: UsTokenConfig::new(address("0x30")),
        }
    }

    fn wrap_event(objects: &NexusObjects, inner: sui::types::StructTag) -> sui::types::Event {
        let wrapper = crate::move_bindings::struct_tag::<
            event_move::EventWrapper<agent_move::AgentCreatedEvent>,
        >(objects);
        sui::types::Event {
            package_id: objects.registry_pkg_id(),
            module: wrapper.module().clone(),
            sender: address("0x99"),
            type_: sui::types::StructTag::new(
                *wrapper.address(),
                wrapper.module().clone(),
                wrapper.name().clone(),
                vec![sui::types::TypeTag::Struct(Box::new(inner))],
            ),
            contents: vec![],
        }
    }

    #[test]
    fn package_release_keeps_mixed_datatype_origins() {
        let mut objects = sample_objects();
        objects
            .packages
            .interface
            .insert_type_origin(DatatypeKey::new("agent", "Agent"), address("0xa1"))
            .unwrap();
        objects
            .packages
            .interface
            .insert_type_origin(DatatypeKey::new("agent", "AgentStateV2"), address("0xa2"))
            .unwrap();

        assert_eq!(
            objects.packages.interface.type_origin("agent", "Agent"),
            address("0xa1")
        );
        assert_eq!(
            objects
                .packages
                .interface
                .type_origin("agent", "AgentStateV2"),
            address("0xa2")
        );
    }

    #[test]
    fn package_metadata_accepts_one_exact_upgrade_snapshot() {
        let (expected, package, package_object) = package_metadata_fixture();
        let resolved = validate_package_fixture(&expected, package, package_object).unwrap();

        assert_eq!(resolved.release.initial_id, address("0xa1"));
        assert_eq!(resolved.release.storage_id, address("0xa2"));
        assert_eq!(resolved.release.version, 2);
        assert_eq!(
            resolved.release.type_origin("sample", "Thing"),
            address("0xa1")
        );
        let link = resolved.linkage.get(&address("0xb1")).unwrap();
        assert_eq!(link.storage_id, address("0xb2"));
        assert_eq!(link.version, 3);
    }

    #[test]
    fn package_header_rejects_incomplete_or_inconsistent_identity() {
        assert!(package_header_error(|package| package.storage_id = None).contains("is missing"));
        assert!(
            package_header_error(|package| package.storage_id = Some("not-an-id".to_owned()))
                .contains("is invalid")
        );
        assert!(package_header_error(
            |package| package.storage_id = Some(address("0xff").to_string())
        )
        .contains("expected"));
        assert!(package_header_error(|package| package.original_id = None).contains("is missing"));
        assert!(
            package_header_error(|package| package.original_id = Some("not-an-id".to_owned()))
                .contains("is invalid")
        );
        assert!(package_header_error(
            |package| package.original_id = Some(address("0xfe").to_string())
        )
        .contains("belongs to lineage"));
        assert!(package_header_error(|package| package.version = None).contains("is missing"));
        assert!(package_header_error(|package| package.version = Some(3)).contains("expected '2'"));

        let (mut legacy, mut package, _) = package_metadata_fixture();
        legacy.initial_id = legacy.storage_id;
        legacy.version = 0;
        package.original_id = Some(address("0xfd").to_string());
        package.version = Some(9);
        let (_, initial_id, version) =
            validate_package_header(&legacy, "sample", &package).unwrap();
        assert_eq!(initial_id, address("0xfd"));
        assert_eq!(version, 9);
    }

    #[test]
    fn package_object_rejects_inconsistent_object_metadata() {
        assert!(package_object_error(|_, object| object.object_id = None).contains("is missing"));
        assert!(package_object_error(
            |_, object| object.object_id = Some(address("0xff").to_string())
        )
        .contains("expected"));
        assert!(package_object_error(|_, object| object.version = None).contains("is missing"));
        assert!(package_object_error(|_, object| object.version = Some(3)).contains("ABI reports"));
        assert!(package_object_error(|_, object| object.object_type = None)
            .contains("expected 'package'"));
        assert!(
            package_object_error(|_, object| object.package = None).contains("metadata is missing")
        );
    }

    #[test]
    fn package_object_rejects_untrusted_type_metadata() {
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().type_origins[0].module_name = None;
        })
        .contains("without a module"));
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().type_origins[0].datatype_name = None;
        })
        .contains("without a datatype"));
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().type_origins[0].package_id = None;
        })
        .contains("is missing"));
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().type_origins[0].package_id =
                Some("not-an-id".to_owned());
        })
        .contains("is invalid"));
        assert!(package_object_error(|_, object| {
            object
                .package
                .as_mut()
                .unwrap()
                .type_origins
                .push(type_origin("sample", "Thing", address("0xff")));
        })
        .contains("conflicting package origins"));
        assert!(package_object_error(|package, _| {
            package.type_origins[0].package_id = Some(address("0xff").to_string());
        })
        .contains("expected"));
        assert!(package_object_error(|package, _| {
            package.modules[0].name = None;
        })
        .contains("module without a name"));
        assert!(package_object_error(|package, _| {
            package.modules[0].datatypes[0].name = None;
        })
        .contains("unnamed datatype"));
        assert!(package_object_error(|package, _| {
            package.modules[0].datatypes[0].module = Some("other".to_owned());
        })
        .contains("declares module"));
        assert!(package_object_error(|package, _| {
            package.modules[0].datatypes[0].defining_id = None;
        })
        .contains("is missing"));
        assert!(package_object_error(|package, _| {
            package.modules[0].datatypes[0].type_name = None;
        })
        .contains("has no type name"));
        assert!(package_object_error(|package, _| {
            package.modules[0].datatypes[0].type_name = Some("invalid".to_owned());
        })
        .contains("invalid type name"));
        assert!(package_object_error(|package, _| {
            package.modules[0].datatypes[0].type_name =
                Some(format!("{}::other::Thing", address("0xa2")));
        })
        .contains("inconsistent runtime type name"));
    }

    #[test]
    fn package_object_rejects_incomplete_or_duplicate_linkage() {
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().linkage[0].original_id = None;
        })
        .contains("is missing"));
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().linkage[0].upgraded_id = None;
        })
        .contains("is missing"));
        assert!(package_object_error(|_, object| {
            object.package.as_mut().unwrap().linkage[0].upgraded_version = None;
        })
        .contains("no upgraded version"));
        assert!(package_object_error(|_, object| {
            let duplicate = object.package.as_ref().unwrap().linkage[0].clone();
            object.package.as_mut().unwrap().linkage.push(duplicate);
        })
        .contains("duplicate linkage"));
    }

    #[test]
    fn package_scope_recognizes_each_nexus_family() {
        let objects = sample_objects();
        let cases = [
            crate::move_bindings::struct_tag::<execution_move::DAGExecution>(&objects),
            crate::move_bindings::struct_tag::<scheduler_task_move::Task>(&objects),
            crate::move_bindings::struct_tag::<agent_registry_move::SkillRegisteredEvent>(&objects),
            crate::move_bindings::struct_tag::<dag_move::DAG>(&objects),
            crate::move_bindings::struct_tag::<gas_move::ToolGas>(&objects),
        ];
        for tag in cases {
            assert!(objects.is_event_from_nexus(&wrap_event(&objects, tag)));
        }
    }

    #[test]
    fn release_accessors_use_storage_ids_and_exact_type_origins() {
        let mut objects = sample_objects();
        for (package, origin) in [
            (&mut objects.packages.primitives, address("0xa1")),
            (&mut objects.packages.interface, address("0xa2")),
            (&mut objects.packages.registry, address("0xa3")),
            (&mut objects.packages.gas, address("0xa4")),
            (&mut objects.packages.workflow, address("0xa5")),
            (&mut objects.packages.scheduler, address("0xa6")),
        ] {
            package.storage_id = origin;
        }
        objects
            .packages
            .primitives
            .insert_type_origin(DatatypeKey::new("event", "EventWrapper"), address("0xb1"))
            .unwrap();
        objects
            .packages
            .interface
            .insert_type_origin(DatatypeKey::new("graph", "Vertex"), address("0xb2"))
            .unwrap();
        objects
            .packages
            .registry
            .insert_type_origin(
                DatatypeKey::new("network_auth", "IdentityKey"),
                address("0xb3"),
            )
            .unwrap();
        objects
            .packages
            .gas
            .insert_type_origin(DatatypeKey::new("gas", "ToolGas"), address("0xb4"))
            .unwrap();
        objects
            .packages
            .workflow
            .insert_type_origin(
                DatatypeKey::new("execution", "DAGExecution"),
                address("0xb5"),
            )
            .unwrap();
        objects
            .packages
            .scheduler
            .insert_type_origin(DatatypeKey::new("task", "Task"), address("0xb6"))
            .unwrap();

        assert_eq!(objects.primitives_pkg_id(), address("0xa1"));
        assert_eq!(objects.interface_pkg_id(), address("0xa2"));
        assert_eq!(objects.registry_pkg_id(), address("0xa3"));
        assert_eq!(objects.gas_pkg_id(), address("0xa4"));
        assert_eq!(objects.workflow_pkg_id(), address("0xa5"));
        assert_eq!(objects.scheduler_pkg_id(), address("0xa6"));
        assert_eq!(objects.primitives_type_origin_pkg_id(), address("0xb1"));
        assert_eq!(objects.interface_type_origin_pkg_id(), address("0xb2"));
        assert_eq!(objects.registry_type_origin_pkg_id(), address("0xb3"));
        assert_eq!(objects.gas_type_origin_pkg_id(), address("0xb4"));
        assert_eq!(objects.workflow_type_origin_pkg_id(), address("0xb5"));
        assert_eq!(objects.scheduler_type_origin_pkg_id(), address("0xb6"));

        assert!(objects.is_primitives_package(address("0xb1")));
        assert!(objects.is_interface_package(address("0xb2")));
        assert!(objects.is_registry_package(address("0xb3")));
        assert!(objects.is_gas_package(address("0xb4")));
        assert!(objects.is_workflow_package(address("0xb5")));
        assert!(objects.is_scheduler_package(address("0xb6")));
        assert!(objects.is_nexus_package(address("0xb6")));
        assert!(objects.is_active_emitter(address("0xa6")));
        assert!(!objects.is_active_emitter(address("0xb6")));
        assert!(!objects.is_nexus_package(address("0xff")));
    }

    #[cfg(feature = "nexus")]
    fn event_source_package(
        storage_id: sui::types::Address,
        original_id: sui::types::Address,
        links: &[(sui::types::Address, sui::types::Address, u64)],
    ) -> sui::grpc::Package {
        let mut package = sui::grpc::Package::default();
        package.storage_id = Some(storage_id.to_string());
        package.original_id = Some(original_id.to_string());
        package.version = Some(1);
        package.linkage = links
            .iter()
            .map(|(original_id, upgraded_id, upgraded_version)| {
                let mut linkage = sui::grpc::Linkage::default();
                linkage.original_id = Some(original_id.to_string());
                linkage.upgraded_id = Some(upgraded_id.to_string());
                linkage.upgraded_version = Some(*upgraded_version);
                linkage
            })
            .collect();
        package
    }

    #[cfg(feature = "nexus")]
    #[test]
    fn composed_event_sources_require_exact_active_nexus_linkage() {
        let mut objects = sample_objects();
        objects.packages.scheduler.storage_id = address("0xa6");
        objects.packages.scheduler.version = 2;
        let external_id = address("0xc1");
        let scheduler_origin = objects.packages.scheduler.initial_id;

        let active = event_source_package(
            external_id,
            external_id,
            &[(scheduler_origin, address("0xa6"), 2)],
        );
        assert!(objects
            .package_uses_active_release(external_id, &active)
            .unwrap());

        let stale = event_source_package(
            external_id,
            external_id,
            &[(scheduler_origin, scheduler_origin, 1)],
        );
        assert!(!objects
            .package_uses_active_release(external_id, &stale)
            .unwrap());

        let mixed = event_source_package(
            external_id,
            external_id,
            &[
                (scheduler_origin, address("0xa6"), 2),
                (objects.packages.workflow.initial_id, address("0xff"), 1),
            ],
        );
        assert!(!objects
            .package_uses_active_release(external_id, &mixed)
            .unwrap());
    }

    #[cfg(feature = "nexus")]
    #[test]
    fn event_sources_reject_inactive_nexus_versions_and_unrelated_packages() {
        let mut objects = sample_objects();
        let scheduler_origin = objects.packages.scheduler.initial_id;
        objects.packages.scheduler.storage_id = address("0xa6");
        objects.packages.scheduler.version = 2;

        let stale_nexus = event_source_package(
            scheduler_origin,
            scheduler_origin,
            &[(scheduler_origin, address("0xa6"), 2)],
        );
        assert!(!objects
            .package_uses_active_release(scheduler_origin, &stale_nexus)
            .unwrap());

        let unrelated = event_source_package(address("0xc1"), address("0xc1"), &[]);
        assert!(!objects
            .package_uses_active_release(address("0xc1"), &unrelated)
            .unwrap());
    }

    #[test]
    fn token_type_helpers_use_the_configured_package() {
        let token = UsTokenConfig::new(address("0xc1"));
        let type_tag = token.type_tag();
        let sui::types::TypeTag::Struct(type_tag) = type_tag else {
            panic!("expected US struct tag");
        };
        assert_eq!(*type_tag.address(), address("0xc1"));
        assert_eq!(
            token.coin_type_tag().type_params().first(),
            Some(&sui::types::TypeTag::Struct(type_tag))
        );
        assert!(token
            .qualified_type()
            .starts_with(&address("0xc1").to_string()));
        assert_eq!(
            UsTokenConfig::default().package_id,
            sui::types::Address::ZERO
        );
    }

    #[test]
    fn new_release_shape_round_trips_through_toml() {
        let objects = sample_objects();
        let encoded = toml::to_string(&objects).unwrap();
        let decoded: NexusObjects = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, objects);
    }

    #[test]
    fn legacy_flat_package_ids_remain_bootstrap_compatible() {
        let objects = sample_objects();
        let mut value = toml::Value::try_from(&objects).unwrap();
        let table = value.as_table_mut().unwrap();
        table.remove("packages");
        for (name, package) in [
            ("primitives", &objects.packages.primitives),
            ("interface", &objects.packages.interface),
            ("registry", &objects.packages.registry),
            ("gas", &objects.packages.gas),
            ("workflow", &objects.packages.workflow),
            ("scheduler", &objects.packages.scheduler),
        ] {
            table.insert(
                format!("{name}_pkg_id"),
                toml::Value::String(package.storage_id.to_string()),
            );
            table.insert(
                format!("{name}_original_pkg_id"),
                toml::Value::String(package.initial_id.to_string()),
            );
        }
        let decoded: NexusObjects = toml::from_str(&toml::to_string(&value).unwrap()).unwrap();

        assert_eq!(
            decoded.packages.primitives.storage_id,
            objects.packages.primitives.storage_id
        );
        assert_eq!(decoded.packages.primitives.version, 0);
        assert!(decoded.packages.primitives.type_origins.is_empty());
    }

    #[test]
    fn legacy_shape_defaults_release_and_rejects_missing_package_ids() {
        let objects = sample_objects();
        let mut value = toml::Value::try_from(&objects).unwrap();
        let table = value.as_table_mut().unwrap();
        table.remove("packages");
        table.remove("release");
        table.remove("protocol");
        table.insert(
            "active_release".to_owned(),
            toml::Value::Integer(objects.release as i64),
        );
        for (name, package) in [
            ("primitives", &objects.packages.primitives),
            ("interface", &objects.packages.interface),
            ("registry", &objects.packages.registry),
            ("gas", &objects.packages.gas),
            ("workflow", &objects.packages.workflow),
            ("scheduler", &objects.packages.scheduler),
        ] {
            table.insert(
                format!("{name}_pkg_id"),
                toml::Value::String(package.storage_id.to_string()),
            );
        }
        let decoded: NexusObjects = value.clone().try_into().unwrap();
        assert_eq!(decoded.release, objects.release);
        assert_eq!(*decoded.protocol.object_id(), sui::types::Address::ZERO);

        value.as_table_mut().unwrap().remove("scheduler_pkg_id");
        let decoded: Result<NexusObjects, _> = value.try_into();
        let error = decoded.unwrap_err().to_string();
        assert!(error.contains("missing field `scheduler_pkg_id`"));
    }
}
