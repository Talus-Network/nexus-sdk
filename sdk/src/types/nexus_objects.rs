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
            if *tag.address() != defining_id
                || tag.module().as_str() != module_name
                || tag.name().as_str() != datatype_name
            {
                anyhow::bail!(
                    "{package_name} datatype '{module_name}::{datatype_name}' has inconsistent \
                     type name '{type_name}' and defining ID '{defining_id}'"
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
}
