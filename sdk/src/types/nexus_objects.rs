//! Activated Nexus protocol configuration and durable object references.

#[cfg(test)]
use crate::move_bindings::{
    interface::dag as dag_move,
    primitives::event as event_move,
    registry::agent_registry as agent_registry_move,
    scheduler::task as scheduler_task_move,
    tool::tool_payment as tool_payment_move,
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
        types::{DatatypeKey, DefaultDagExecutorTarget, NexusPackages, PackageVersion},
    },
    serde::{Deserialize, Serialize},
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

/// One validated and immutable Nexus protocol configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NexusObjects {
    /// Monotonic Nexus protocol version selected by the active configuration.
    pub protocol_version: u64,
    /// Stable onchain root that owns the active protocol configuration.
    pub protocol: sui::types::ObjectReference,
    /// Exact package versions selected by protocol package role.
    pub packages: NexusPackages,
    /// BCS digest of the canonical protocol configuration hash input.
    pub config_hash: Vec<u8>,
    /// Nexus domain identity derived from the configured `LeaderRegistry`.
    pub network_id: sui::types::Address,
    /// Canonical shared `ToolRegistry`.
    pub tool_registry: sui::types::ObjectReference,
    /// Canonical shared `NetworkAuth`.
    pub network_auth: sui::types::ObjectReference,
    /// Canonical shared `AgentRegistry`.
    pub agent_registry: sui::types::ObjectReference,
    /// Current default DAG executor derived from the configured `AgentRegistry`.
    pub default_dag_executor: DefaultDagExecutorTarget,
    /// Canonical shared `LeaderRegistry`.
    pub leader_registry: sui::types::ObjectReference,
    /// Canonical shared priority fee vault.
    pub priority_fee_vault: sui::types::ObjectReference,
    /// Optional operator authority for the configured priority fee vault.
    #[serde(default = "default_object_reference")]
    pub priority_fee_vault_owner_cap: sui::types::ObjectReference,
    /// External US token package and object configuration.
    #[serde(default)]
    pub us_token: UsTokenConfig,
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
        let (primitives, interface, tool, registry, workflow, scheduler) = tokio::try_join!(
            resolve_package_version(client, &self.packages.primitives, "primitives"),
            resolve_package_version(client, &self.packages.interface, "interface"),
            resolve_package_version(client, &self.packages.tool, "tool"),
            resolve_package_version(client, &self.packages.registry, "registry"),
            resolve_package_version(client, &self.packages.workflow, "workflow"),
            resolve_package_version(client, &self.packages.scheduler, "scheduler"),
        )?;
        self.packages = NexusPackages {
            primitives,
            interface,
            tool,
            registry,
            workflow,
            scheduler,
        };
        Ok(())
    }

    pub fn primitives_pkg_id(&self) -> sui::types::Address {
        self.packages.primitives.storage_id
    }

    pub fn interface_pkg_id(&self) -> sui::types::Address {
        self.packages.interface.storage_id
    }

    pub fn tool_pkg_id(&self) -> sui::types::Address {
        self.packages.tool.storage_id
    }

    pub fn registry_pkg_id(&self) -> sui::types::Address {
        self.packages.registry.storage_id
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

    pub fn tool_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages
            .tool
            .type_origin("tool_payment", "ToolPayment")
    }

    pub fn registry_type_origin_pkg_id(&self) -> sui::types::Address {
        self.packages
            .registry
            .type_origin("network_auth", "IdentityKey")
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

    pub fn is_tool_package(&self, address: sui::types::Address) -> bool {
        self.packages.tool.contains_package(address)
    }

    pub fn is_registry_package(&self, address: sui::types::Address) -> bool {
        self.packages.registry.contains_package(address)
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

    /// Whether a Sui event was emitted by a configured package version.
    pub fn is_active_emitter(&self, address: sui::types::Address) -> bool {
        self.packages
            .all()
            .into_iter()
            .any(|package| package.storage_id == address)
    }

    /// Returns whether `package_id` is a valid top level event source for this
    /// protocol configuration.
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
        let mut package = client
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

        let request = sui::grpc::GetObjectRequest::default()
            .with_object_id(package_id)
            .with_read_mask(sui::grpc::FieldMask::from_paths([
                "object_id",
                "object_type",
                "package",
            ]));
        let package_object = client
            .as_ref()
            .clone()
            .ledger_client()
            .get_object(request)
            .await
            .map_err(|error| {
                anyhow::anyhow!(
                    "Failed to fetch exact event source package '{package_id}': {error}"
                )
            })?
            .into_inner()
            .object
            .ok_or_else(|| {
                anyhow::anyhow!("Exact event source package '{package_id}' was not returned")
            })?;
        let object_id = parse_package_address(
            package_object.object_id.as_deref(),
            "event source",
            "object_id",
        )?;
        if object_id != package_id {
            anyhow::bail!(
                "Event source package object returned ID '{object_id}', expected '{package_id}'"
            );
        }
        if package_object.object_type.as_deref() != Some("package") {
            anyhow::bail!(
                "Event source object '{package_id}' has type '{}', expected 'package'",
                package_object.object_type.as_deref().unwrap_or("<missing>")
            );
        }
        package.linkage = package_object
            .package
            .ok_or_else(|| {
                anyhow::anyhow!("Event source package object '{package_id}' has no metadata")
            })?
            .linkage;

        self.package_uses_active_protocol(package_id, &package)
    }

    #[cfg(feature = "nexus")]
    fn package_uses_active_protocol(
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

        if self.is_tool_package(*inner_tag.address())
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
pub(crate) async fn resolve_package_version(
    client: &Arc<sui::grpc::Client>,
    expected: &PackageVersion,
    package_name: &str,
) -> anyhow::Result<PackageVersion> {
    resolve_package_version_metadata(client, expected, package_name)
        .await
        .map(|metadata| metadata.package)
}

#[cfg(feature = "nexus")]
#[derive(Clone, Debug)]
pub(crate) struct PackageLink {
    pub storage_id: sui::types::Address,
    pub version: u64,
}

#[cfg(feature = "nexus")]
#[derive(Clone, Debug)]
pub(crate) struct ResolvedPackageVersion {
    pub package: PackageVersion,
    pub linkage: BTreeMap<sui::types::Address, PackageLink>,
}

#[cfg(feature = "nexus")]
pub(crate) async fn resolve_package_version_metadata(
    client: &Arc<sui::grpc::Client>,
    expected: &PackageVersion,
    package_name: &str,
) -> anyhow::Result<ResolvedPackageVersion> {
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
    expected: &PackageVersion,
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
) -> anyhow::Result<ResolvedPackageVersion> {
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

    let mut resolved = PackageVersion::new(initial_id, storage_id, version, Default::default());
    for origin in &exact_package.type_origins {
        let (key, package_id) = parse_type_origin(origin, package_name)?;
        resolved.insert_type_origin(key, package_id)?;
    }

    // When the ABI service also supplies an origin table, require it to agree
    // with the immutable package object rather than treating it as a fallback.
    for origin in &package.type_origins {
        let (key, package_id) = parse_type_origin(origin, package_name)?;
        require_type_origin(&resolved, &key, package_id, package_name)?;
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
            require_type_origin(&resolved, &key, defining_id, package_name)?;
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

    Ok(ResolvedPackageVersion {
        package: resolved,
        linkage,
    })
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
    package: &PackageVersion,
    key: &DatatypeKey,
    expected: sui::types::Address,
    package_name: &str,
) -> anyhow::Result<()> {
    let observed = package
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

    fn package_metadata_fixture() -> (PackageVersion, sui::grpc::Package, sui::grpc::Object) {
        let initial_id = address("0xa1");
        let storage_id = address("0xa2");
        let expected = PackageVersion::new(initial_id, storage_id, 2, Default::default());
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
        expected: &PackageVersion,
        package: sui::grpc::Package,
        package_object: sui::grpc::Object,
    ) -> anyhow::Result<ResolvedPackageVersion> {
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
            protocol_version: 1,
            protocol: object_ref("0x10"),
            packages: NexusPackages::first_publication(
                address("0x11"),
                address("0x12"),
                address("0x13"),
                address("0x14"),
                address("0x15"),
                address("0x16"),
            ),
            config_hash: vec![7; 32],
            network_id: address("0x20"),
            tool_registry: object_ref("0x21"),
            network_auth: object_ref("0x23"),
            agent_registry: object_ref("0x24"),
            default_dag_executor: DefaultDagExecutorTarget {
                agent_id: address("0x25"),
                skill_id: 1,
            },
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
    fn package_version_keeps_mixed_datatype_origins() {
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

        assert_eq!(resolved.package.initial_id, address("0xa1"));
        assert_eq!(resolved.package.storage_id, address("0xa2"));
        assert_eq!(resolved.package.version, 2);
        assert_eq!(
            resolved.package.type_origin("sample", "Thing"),
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

        let (mut configured, mut package, _) = package_metadata_fixture();
        configured.initial_id = configured.storage_id;
        configured.version = 0;
        package.original_id = Some(address("0xfd").to_string());
        package.version = Some(9);
        let (_, initial_id, version) =
            validate_package_header(&configured, "sample", &package).unwrap();
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
            crate::move_bindings::struct_tag::<tool_payment_move::ToolPayment>(&objects),
        ];
        for tag in cases {
            assert!(objects.is_event_from_nexus(&wrap_event(&objects, tag)));
        }
    }

    #[test]
    fn protocol_accessors_use_storage_ids_and_exact_type_origins() {
        let mut objects = sample_objects();
        for (package, origin) in [
            (&mut objects.packages.primitives, address("0xa1")),
            (&mut objects.packages.interface, address("0xa2")),
            (&mut objects.packages.registry, address("0xa3")),
            (&mut objects.packages.tool, address("0xa4")),
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
            .tool
            .insert_type_origin(
                DatatypeKey::new("tool_payment", "ToolPayment"),
                address("0xb4"),
            )
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
        assert_eq!(objects.tool_pkg_id(), address("0xa4"));
        assert_eq!(objects.workflow_pkg_id(), address("0xa5"));
        assert_eq!(objects.scheduler_pkg_id(), address("0xa6"));
        assert_eq!(objects.primitives_type_origin_pkg_id(), address("0xb1"));
        assert_eq!(objects.interface_type_origin_pkg_id(), address("0xb2"));
        assert_eq!(objects.registry_type_origin_pkg_id(), address("0xb3"));
        assert_eq!(objects.tool_type_origin_pkg_id(), address("0xb4"));
        assert_eq!(objects.workflow_type_origin_pkg_id(), address("0xb5"));
        assert_eq!(objects.scheduler_type_origin_pkg_id(), address("0xb6"));

        assert!(objects.is_primitives_package(address("0xb1")));
        assert!(objects.is_interface_package(address("0xb2")));
        assert!(objects.is_registry_package(address("0xb3")));
        assert!(objects.is_tool_package(address("0xb4")));
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
            .package_uses_active_protocol(external_id, &active)
            .unwrap());

        let stale = event_source_package(
            external_id,
            external_id,
            &[(scheduler_origin, scheduler_origin, 1)],
        );
        assert!(!objects
            .package_uses_active_protocol(external_id, &stale)
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
            .package_uses_active_protocol(external_id, &mixed)
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
            .package_uses_active_protocol(scheduler_origin, &stale_nexus)
            .unwrap());

        let unrelated = event_source_package(address("0xc1"), address("0xc1"), &[]);
        assert!(!objects
            .package_uses_active_protocol(address("0xc1"), &unrelated)
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
    fn protocol_configuration_round_trips_through_toml() {
        let objects = sample_objects();
        let encoded = toml::to_string(&objects).unwrap();
        let decoded: NexusObjects = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded, objects);
    }

    #[test]
    fn protocol_configuration_requires_package_metadata() {
        let objects = sample_objects();
        let mut value = toml::Value::try_from(&objects).unwrap();
        value.as_table_mut().unwrap().remove("packages");
        let decoded: Result<NexusObjects, _> = value.try_into();
        let error = decoded.unwrap_err().to_string();
        assert!(error.contains("missing field `packages`"));
    }
}
