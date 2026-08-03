//! Canonical Nexus protocol configuration resolution.

use {
    crate::{
        move_bindings::{
            primitives::protocol::{
                PackageInfo,
                Protocol,
                ProtocolConfigHashInputV1,
                ProtocolConfigV1,
                ProtocolStateV1,
                ProtocolVersionActivatedV1,
                SharedObjectInfo,
            },
            registry::leader::{LeaderRegistry, LeaderRegistryStateV1},
        },
        nexus::{
            crawler::{Crawler, Response},
            error::NexusError,
            registry::extract_network_id_from_leader_registry,
            tap,
        },
        sui,
        types::{
            nexus_objects::{
                default_object_reference,
                resolve_package_version_metadata,
                ResolvedPackageVersion,
            },
            NexusObjects,
            NexusPackages,
            PackageVersion,
            UsTokenConfig,
        },
    },
    anyhow::{anyhow, Context as _},
    futures::future::try_join_all,
    std::{
        collections::{BTreeMap, HashMap},
        sync::Arc,
    },
};

/// Newest Nexus protocol version whose behavior this SDK understands.
pub const MAX_SUPPORTED_PROTOCOL_VERSION: u64 = 2;

const PACKAGE_ROLES: [(u8, &str); 6] = [
    (0, "primitives"),
    (1, "interface"),
    (2, "registry"),
    (3, "gas"),
    (4, "workflow"),
    (5, "scheduler"),
];

const SHARED_OBJECT_ROLES: [(u8, &str); 7] = [
    (0, "ToolRegistry"),
    (1, "VerifierRegistry"),
    (2, "NetworkAuth"),
    (3, "AgentRegistry"),
    (4, "GasService"),
    (5, "LeaderRegistry"),
    (6, "PriorityFeeVault"),
];

/// Runtime configuration that is intentionally outside [`ProtocolConfigV1`].
///
/// The default DAG executor is refreshed from [`crate::types::AgentRegistrySnapshot`].
/// The US token is an external dependency and the priority fee capability is
/// optional operator authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolExtras {
    /// Optional authority used to administer the configured priority fee vault.
    pub priority_fee_vault_owner_cap: sui::types::ObjectReference,
    /// External US token package and object configuration.
    pub us_token: UsTokenConfig,
}

impl Default for ProtocolExtras {
    fn default() -> Self {
        Self {
            priority_fee_vault_owner_cap: default_object_reference(),
            us_token: UsTokenConfig::default(),
        }
    }
}

impl From<&NexusObjects> for ProtocolExtras {
    fn from(objects: &NexusObjects) -> Self {
        Self {
            priority_fee_vault_owner_cap: objects.priority_fee_vault_owner_cap.clone(),
            us_token: objects.us_token.clone(),
        }
    }
}

/// Resolves and validates the configuration selected by one stable [`Protocol`].
#[derive(Clone)]
pub struct ProtocolResolver {
    protocol: sui::types::ObjectReference,
    client: Arc<sui::grpc::Client>,
    extras: ProtocolExtras,
}

impl ProtocolResolver {
    /// Creates a resolver for one stable protocol root and Sui client.
    pub fn new(protocol: sui::types::ObjectReference, client: Arc<sui::grpc::Client>) -> Self {
        Self {
            protocol,
            client,
            extras: ProtocolExtras::default(),
        }
    }

    /// Supplies runtime configuration that is outside the onchain protocol config.
    pub fn with_extras(mut self, extras: ProtocolExtras) -> Self {
        self.extras = extras;
        self
    }

    /// Returns the configured stable protocol root.
    pub fn protocol(&self) -> &sui::types::ObjectReference {
        &self.protocol
    }

    /// Resolve the configuration selected by the canonical protocol root.
    pub async fn resolve_active(&self) -> Result<NexusObjects, NexusError> {
        let (protocol, state) = self.protocol_state().await?;
        let config = active_config(state)?;
        self.resolve_config_inner(protocol, &config).await
    }

    /// Resolve the active configuration only when its identity changed.
    pub async fn resolve_active_if_changed(
        &self,
        current: &NexusObjects,
    ) -> Result<Option<NexusObjects>, NexusError> {
        let (protocol, state) = self.protocol_state().await?;
        let config = active_config(state)?;
        validate_config(&config)?;
        if config.protocol_version > MAX_SUPPORTED_PROTOCOL_VERSION {
            return Err(NexusError::UnsupportedProtocolVersion {
                protocol_version: config.protocol_version,
                maximum: MAX_SUPPORTED_PROTOCOL_VERSION,
            });
        }
        if !active_configuration_changed(current.protocol_version, &current.config_hash, &config)? {
            return Ok(None);
        }

        self.resolve_config_inner(protocol, &config).await.map(Some)
    }

    /// Resolve an activation after confirming it still names the active configuration.
    pub async fn resolve_activation(
        &self,
        activation: &ProtocolVersionActivatedV1,
    ) -> Result<NexusObjects, NexusError> {
        let (protocol, state) = self.protocol_state().await?;
        let active = active_config(state)?;
        validate_config(&active)?;
        if activation.protocol_id.bytes != *protocol.object_ref().object_id()
            || activation.protocol_version != active.protocol_version
            || activation.config_hash != active.config_hash
        {
            return Err(protocol_error(format!(
                "Protocol activation version '{}' with configuration hash '{}' does not match \
                 active version '{}' with configuration hash '{}'",
                activation.protocol_version,
                hex::encode(&activation.config_hash),
                active.protocol_version,
                hex::encode(&active.config_hash),
            )));
        }
        self.resolve_config_inner(protocol, &active).await
    }

    async fn protocol_state(&self) -> Result<(Response<Protocol>, ProtocolStateV1), NexusError> {
        let crawler = Crawler::new(Arc::clone(&self.client));
        let protocol_id = *self.protocol.object_id();
        let protocol = crawler
            .get_object::<Protocol>(protocol_id)
            .await
            .map_err(NexusError::Rpc)?;
        validate_protocol_root(protocol_id, &protocol)?;
        let state = crawler
            .get_versioned_state::<ProtocolStateV1>(&protocol.data.state)
            .await
            .map_err(NexusError::Rpc)?;
        if state.active.vec.len() > 1 {
            return Err(protocol_error(format!(
                "Protocol active option contains '{}' configurations",
                state.active.vec.len()
            )));
        }
        Ok((protocol, state))
    }

    async fn resolve_config_inner(
        &self,
        protocol: Response<Protocol>,
        config: &ProtocolConfigV1,
    ) -> Result<NexusObjects, NexusError> {
        let crawler = Crawler::new(Arc::clone(&self.client));
        validate_config(config)?;
        let protocol_version = config.protocol_version;

        if protocol_version > MAX_SUPPORTED_PROTOCOL_VERSION {
            return Err(NexusError::UnsupportedProtocolVersion {
                protocol_version,
                maximum: MAX_SUPPORTED_PROTOCOL_VERSION,
            });
        }

        let packages = self.resolve_packages(config).await?;
        validate_type_origin_lineages(&self.client, &packages).await?;
        let mut refs = resolve_shared_objects(&crawler, config).await?;
        let network_id = resolve_network_id(
            &crawler,
            &mut refs,
            *protocol.object_ref().object_id(),
            protocol_version,
            &config.shared_objects.contents[5].value,
        )
        .await?;
        let default_dag_executor =
            tap::fetch_default_dag_executor(&crawler, *refs.agent_registry.object_id())
                .await
                .map_err(NexusError::Rpc)?
                .ok_or_else(|| {
                    protocol_error("Configured AgentRegistry has no default DAG executor")
                })?
                .target();
        let priority_fee_vault_owner_cap =
            refresh_optional_authority(&crawler, &self.extras.priority_fee_vault_owner_cap).await?;

        Ok(NexusObjects {
            protocol_version,
            protocol: protocol.object_ref(),
            packages,
            config_hash: config.config_hash.clone(),
            network_id,
            tool_registry: refs.tool_registry,
            verifier_registry: refs.verifier_registry,
            network_auth: refs.network_auth,
            agent_registry: refs.agent_registry,
            default_dag_executor,
            gas_service: refs.gas_service,
            leader_registry: refs.leader_registry,
            priority_fee_vault: refs.priority_fee_vault,
            priority_fee_vault_owner_cap,
            us_token: self.extras.us_token.clone(),
        })
    }

    async fn resolve_packages(
        &self,
        config: &ProtocolConfigV1,
    ) -> Result<NexusPackages, NexusError> {
        let bindings = &config.packages.contents;
        let primitives = package_version(&bindings[0].value);
        let interface = package_version(&bindings[1].value);
        let registry = package_version(&bindings[2].value);
        let gas = package_version(&bindings[3].value);
        let workflow = package_version(&bindings[4].value);
        let scheduler = package_version(&bindings[5].value);
        let (primitives, interface, registry, gas, workflow, scheduler) = tokio::try_join!(
            resolve_package_version_metadata(&self.client, &primitives, "primitives"),
            resolve_package_version_metadata(&self.client, &interface, "interface"),
            resolve_package_version_metadata(&self.client, &registry, "registry"),
            resolve_package_version_metadata(&self.client, &gas, "gas"),
            resolve_package_version_metadata(&self.client, &workflow, "workflow"),
            resolve_package_version_metadata(&self.client, &scheduler, "scheduler"),
        )
        .map_err(NexusError::ProtocolValidation)?;

        let families = [
            ("primitives", &primitives.package),
            ("interface", &interface.package),
            ("registry", &registry.package),
            ("gas", &gas.package),
            ("workflow", &workflow.package),
            ("scheduler", &scheduler.package),
        ];
        validate_package_linkage("primitives", &primitives, &[], &families)?;
        validate_package_linkage(
            "interface",
            &interface,
            &[("primitives", &primitives.package)],
            &families,
        )?;
        validate_package_linkage(
            "registry",
            &registry,
            &[
                ("primitives", &primitives.package),
                ("interface", &interface.package),
            ],
            &families,
        )?;
        validate_package_linkage(
            "gas",
            &gas,
            &[
                ("primitives", &primitives.package),
                ("interface", &interface.package),
                ("registry", &registry.package),
            ],
            &families,
        )?;
        validate_package_linkage(
            "workflow",
            &workflow,
            &[
                ("primitives", &primitives.package),
                ("interface", &interface.package),
                ("registry", &registry.package),
                ("gas", &gas.package),
            ],
            &families,
        )?;
        validate_package_linkage(
            "scheduler",
            &scheduler,
            &[
                ("primitives", &primitives.package),
                ("interface", &interface.package),
                ("registry", &registry.package),
                ("workflow", &workflow.package),
            ],
            &families,
        )?;

        Ok(NexusPackages {
            primitives: primitives.package,
            interface: interface.package,
            registry: registry.package,
            gas: gas.package,
            workflow: workflow.package,
            scheduler: scheduler.package,
        })
    }
}

fn validate_protocol_root(
    protocol_id: sui::types::Address,
    protocol: &Response<Protocol>,
) -> Result<(), NexusError> {
    if protocol.data.id.id.bytes != protocol_id {
        return Err(protocol_error(format!(
            "Protocol '{protocol_id}' contains embedded identity '{}'",
            protocol.data.id.id.bytes
        )));
    }
    if protocol.data.state.version != 1 {
        return Err(protocol_error(format!(
            "Protocol '{protocol_id}' uses unsupported state schema '{}'",
            protocol.data.state.version
        )));
    }
    if !protocol.is_shared() {
        return Err(protocol_error(format!(
            "Protocol '{protocol_id}' is not shared"
        )));
    }
    Ok(())
}

fn validate_package_linkage(
    package_name: &str,
    package: &ResolvedPackageVersion,
    required: &[(&str, &PackageVersion)],
    families: &[(&str, &PackageVersion)],
) -> Result<(), NexusError> {
    for (dependency_name, dependency) in required {
        let link = package.linkage.get(&dependency.initial_id).ok_or_else(|| {
            protocol_error(format!(
                "{package_name} has no linkage for required {dependency_name} \
                     lineage '{}'",
                dependency.initial_id
            ))
        })?;
        validate_link(package_name, dependency_name, link, dependency)?;
    }

    for (dependency_name, dependency) in families {
        if let Some(link) = package.linkage.get(&dependency.initial_id) {
            validate_link(package_name, dependency_name, link, dependency)?;
        }
    }
    Ok(())
}

fn validate_link(
    package_name: &str,
    dependency_name: &str,
    link: &crate::types::nexus_objects::PackageLink,
    expected: &PackageVersion,
) -> Result<(), NexusError> {
    if link.storage_id != expected.storage_id || link.version != expected.version {
        return Err(protocol_error(format!(
            "{package_name} links {dependency_name} lineage '{}' to version '{}' \
             at '{}', expected version '{}' at '{}'",
            expected.initial_id,
            link.version,
            link.storage_id,
            expected.version,
            expected.storage_id
        )));
    }
    Ok(())
}

fn package_version(info: &PackageInfo) -> PackageVersion {
    PackageVersion::new(
        info.initial_id.bytes,
        info.storage_id.bytes,
        info.version,
        Default::default(),
    )
}

fn active_config(state: ProtocolStateV1) -> Result<ProtocolConfigV1, NexusError> {
    match state.active.vec.as_slice() {
        [] => Err(protocol_error("Protocol has no active configuration")),
        [config] => Ok(config.clone()),
        configs => Err(protocol_error(format!(
            "Protocol active option contains '{}' configurations",
            configs.len()
        ))),
    }
}

fn active_configuration_changed(
    current_version: u64,
    current_hash: &[u8],
    active: &ProtocolConfigV1,
) -> Result<bool, NexusError> {
    match active.protocol_version.cmp(&current_version) {
        std::cmp::Ordering::Less => Err(protocol_error(format!(
            "Refusing protocol downgrade from version '{current_version}' to '{}'",
            active.protocol_version,
        ))),
        std::cmp::Ordering::Equal if active.config_hash != current_hash => {
            Err(protocol_error(format!(
                "Protocol version '{}' changed configuration hash",
                active.protocol_version,
            )))
        }
        std::cmp::Ordering::Equal => Ok(false),
        std::cmp::Ordering::Greater => Ok(true),
    }
}

fn validate_config(config: &ProtocolConfigV1) -> Result<(), NexusError> {
    if config.protocol_version == 0 {
        return Err(protocol_error("Protocol version zero is invalid"));
    }
    validate_package_bindings(config)?;
    validate_shared_object_bindings(config)?;
    if config.config_hash.len() != sui::types::Digest::LENGTH {
        return Err(protocol_error(format!(
            "Protocol version '{}' configuration hash is {} bytes, expected 32",
            config.protocol_version,
            config.config_hash.len()
        )));
    }
    let input = ProtocolConfigHashInputV1::new(
        config.protocol_version,
        config.packages.clone(),
        config.shared_objects.clone(),
    );
    let bytes = bcs::to_bytes(&input)
        .context("Could not encode protocol configuration hash input")
        .map_err(NexusError::ProtocolValidation)?;
    let digest = sui::types::hash::Hasher::digest(bytes);
    if config.config_hash.as_slice() != digest.as_bytes() {
        return Err(protocol_error(format!(
            "Protocol version '{}' configuration hash does not match its contents",
            config.protocol_version
        )));
    }
    Ok(())
}

fn validate_package_bindings(config: &ProtocolConfigV1) -> Result<(), NexusError> {
    if config.packages.contents.len() != PACKAGE_ROLES.len() {
        return Err(protocol_error(format!(
            "Protocol package bindings contain '{}' entries, expected '{}'",
            config.packages.contents.len(),
            PACKAGE_ROLES.len()
        )));
    }

    let mut initial_ids = HashMap::new();
    let mut storage_ids = HashMap::new();
    for (entry, (expected_role, name)) in config.packages.contents.iter().zip(PACKAGE_ROLES) {
        if entry.key != expected_role {
            return Err(protocol_error(format!(
                "Protocol package binding for {name} has role '{}', expected '{expected_role}'",
                entry.key
            )));
        }
        let package = &entry.value;
        if package.version == 0 {
            return Err(protocol_error(format!(
                "Protocol package binding for {name} has version zero"
            )));
        }
        if package.version == 1 && package.initial_id != package.storage_id {
            return Err(protocol_error(format!(
                "Protocol package binding for {name} version one has different initial and \
                 storage identities"
            )));
        }
        reject_duplicate_id(
            &mut initial_ids,
            package.initial_id.bytes,
            name,
            "initial package",
        )?;
        reject_duplicate_id(
            &mut storage_ids,
            package.storage_id.bytes,
            name,
            "storage package",
        )?;
    }
    Ok(())
}

fn validate_shared_object_bindings(config: &ProtocolConfigV1) -> Result<(), NexusError> {
    if config.shared_objects.contents.len() != SHARED_OBJECT_ROLES.len() {
        return Err(protocol_error(format!(
            "Protocol shared object bindings contain '{}' entries, expected '{}'",
            config.shared_objects.contents.len(),
            SHARED_OBJECT_ROLES.len()
        )));
    }

    let mut ids = HashMap::new();
    for (entry, (expected_role, name)) in config
        .shared_objects
        .contents
        .iter()
        .zip(SHARED_OBJECT_ROLES)
    {
        if entry.key != expected_role {
            return Err(protocol_error(format!(
                "Protocol shared object binding for {name} has role '{}', expected \
                 '{expected_role}'",
                entry.key
            )));
        }
        if entry.value.initial_shared_version == 0 {
            return Err(protocol_error(format!(
                "Protocol shared object binding for {name} has initial version zero"
            )));
        }
        reject_duplicate_id(&mut ids, entry.value.id.bytes, name, "shared object")?;
    }
    Ok(())
}

fn reject_duplicate_id<'a>(
    ids: &mut HashMap<sui::types::Address, &'a str>,
    id: sui::types::Address,
    name: &'a str,
    kind: &str,
) -> Result<(), NexusError> {
    if let Some(previous) = ids.insert(id, name) {
        return Err(protocol_error(format!(
            "Protocol {kind} identity '{id}' is bound to both {previous} and {name}"
        )));
    }
    Ok(())
}

async fn validate_type_origin_lineages(
    client: &Arc<sui::grpc::Client>,
    packages: &NexusPackages,
) -> Result<(), NexusError> {
    let origins = collect_type_origin_lineages(packages)?;
    try_join_all(origins.into_iter().map(|(storage_id, initial_id)| {
        validate_origin_lineage(Arc::clone(client), storage_id, initial_id)
    }))
    .await?;
    Ok(())
}

fn collect_type_origin_lineages(
    packages: &NexusPackages,
) -> Result<BTreeMap<sui::types::Address, sui::types::Address>, NexusError> {
    let mut origins = BTreeMap::new();
    for package in packages.all() {
        for origin in package
            .type_origins
            .values()
            .flat_map(|types| types.values())
            .copied()
            .chain([package.initial_id, package.storage_id])
        {
            if let Some(previous) = origins.insert(origin, package.initial_id) {
                if previous != package.initial_id {
                    return Err(protocol_error(format!(
                        "Package '{origin}' is claimed by lineages '{previous}' and '{}'",
                        package.initial_id
                    )));
                }
            }
        }
    }
    Ok(origins)
}

async fn validate_origin_lineage(
    client: Arc<sui::grpc::Client>,
    storage_id: sui::types::Address,
    expected_initial_id: sui::types::Address,
) -> Result<(), NexusError> {
    let request = sui::grpc::GetPackageRequest::default().with_package_id(storage_id);
    let package = client
        .as_ref()
        .clone()
        .package_client()
        .get_package(request)
        .await
        .map_err(|error| {
            NexusError::Rpc(anyhow!(
                "Failed to fetch datatype origin package '{storage_id}': {error}"
            ))
        })?
        .into_inner()
        .package
        .ok_or_else(|| {
            protocol_error(format!(
                "Datatype origin package '{storage_id}' was not returned"
            ))
        })?;
    validate_origin_package(&package, storage_id, expected_initial_id)
}

fn validate_origin_package(
    package: &sui::grpc::Package,
    storage_id: sui::types::Address,
    expected_initial_id: sui::types::Address,
) -> Result<(), NexusError> {
    let observed_storage: sui::types::Address = package
        .storage_id
        .as_deref()
        .ok_or_else(|| {
            protocol_error(format!(
                "Datatype origin package '{storage_id}' has no storage ID"
            ))
        })?
        .parse()
        .map_err(|error| {
            protocol_error(format!(
                "Datatype origin package '{storage_id}' has invalid storage ID: {error}"
            ))
        })?;
    let observed_initial: sui::types::Address = package
        .original_id
        .as_deref()
        .ok_or_else(|| {
            protocol_error(format!(
                "Datatype origin package '{storage_id}' has no original ID"
            ))
        })?
        .parse()
        .map_err(|error| {
            protocol_error(format!(
                "Datatype origin package '{storage_id}' has invalid original ID: {error}"
            ))
        })?;
    if observed_storage != storage_id || observed_initial != expected_initial_id {
        return Err(protocol_error(format!(
            "Datatype origin package '{storage_id}' belongs to lineage \
             '{observed_initial}', expected '{expected_initial_id}'"
        )));
    }
    Ok(())
}

struct SharedReferences {
    tool_registry: sui::types::ObjectReference,
    verifier_registry: sui::types::ObjectReference,
    network_auth: sui::types::ObjectReference,
    agent_registry: sui::types::ObjectReference,
    gas_service: sui::types::ObjectReference,
    leader_registry: sui::types::ObjectReference,
    priority_fee_vault: sui::types::ObjectReference,
}

async fn resolve_shared_objects(
    crawler: &Crawler,
    config: &ProtocolConfigV1,
) -> Result<SharedReferences, NexusError> {
    let bindings = &config.shared_objects.contents;
    let entries = [
        ("ToolRegistry", &bindings[0].value),
        ("VerifierRegistry", &bindings[1].value),
        ("NetworkAuth", &bindings[2].value),
        ("AgentRegistry", &bindings[3].value),
        ("GasService", &bindings[4].value),
        ("LeaderRegistry", &bindings[5].value),
        ("PriorityFeeVault", &bindings[6].value),
    ];
    let ids = entries
        .iter()
        .map(|(_, info)| info.id.bytes)
        .collect::<Vec<_>>();
    let metadata = crawler
        .get_objects_metadata(&ids)
        .await
        .map_err(NexusError::Rpc)?;
    let by_id = metadata
        .into_iter()
        .map(|response| (response.object_id, response))
        .collect::<HashMap<_, _>>();

    Ok(SharedReferences {
        tool_registry: resolve_shared_object(&by_id, "ToolRegistry", &bindings[0].value)?,
        verifier_registry: resolve_shared_object(&by_id, "VerifierRegistry", &bindings[1].value)?,
        network_auth: resolve_shared_object(&by_id, "NetworkAuth", &bindings[2].value)?,
        agent_registry: resolve_shared_object(&by_id, "AgentRegistry", &bindings[3].value)?,
        gas_service: resolve_shared_object(&by_id, "GasService", &bindings[4].value)?,
        leader_registry: resolve_shared_object(&by_id, "LeaderRegistry", &bindings[5].value)?,
        priority_fee_vault: resolve_shared_object(&by_id, "PriorityFeeVault", &bindings[6].value)?,
    })
}

async fn resolve_network_id(
    crawler: &Crawler,
    refs: &mut SharedReferences,
    protocol_id: sui::types::Address,
    protocol_version: u64,
    expected: &SharedObjectInfo,
) -> Result<sui::types::Address, NexusError> {
    let registry = crawler
        .get_object::<LeaderRegistry>(expected.id.bytes)
        .await
        .map_err(NexusError::Rpc)?;
    if registry.data.id.id.bytes != expected.id.bytes {
        return Err(protocol_error(format!(
            "LeaderRegistry '{}' contains embedded identity '{}'",
            expected.id.bytes, registry.data.id.id.bytes
        )));
    }
    if !registry.is_shared() || registry.get_initial_version() != expected.initial_shared_version {
        return Err(protocol_error(format!(
            "LeaderRegistry '{}' does not match its protocol binding",
            expected.id.bytes
        )));
    }
    if registry.data.state.version != 1 {
        return Err(protocol_error(format!(
            "LeaderRegistry '{}' uses unsupported state schema '{}'",
            expected.id.bytes, registry.data.state.version
        )));
    }
    let state = crawler
        .get_versioned_state::<LeaderRegistryStateV1>(&registry.data.state)
        .await
        .map_err(NexusError::Rpc)?;
    if state.protocol_id.bytes != protocol_id {
        return Err(protocol_error(format!(
            "LeaderRegistry '{}' belongs to protocol '{}', expected '{protocol_id}'",
            expected.id.bytes, state.protocol_id.bytes
        )));
    }
    if state.minimum_protocol_version > protocol_version {
        return Err(protocol_error(format!(
            "LeaderRegistry '{}' requires protocol version '{}', active version is \
             '{protocol_version}'",
            expected.id.bytes, state.minimum_protocol_version
        )));
    }
    refs.leader_registry = registry.object_ref();
    Ok(extract_network_id_from_leader_registry(&state))
}

fn resolve_shared_object(
    by_id: &HashMap<sui::types::Address, Response<()>>,
    name: &str,
    info: &SharedObjectInfo,
) -> Result<sui::types::ObjectReference, NexusError> {
    let response = by_id.get(&info.id.bytes).ok_or_else(|| {
        protocol_error(format!(
            "{name} object '{}' was not returned",
            info.id.bytes
        ))
    })?;
    if !response.is_shared() {
        return Err(protocol_error(format!(
            "{name} object '{}' is not shared",
            info.id.bytes
        )));
    }
    if response.get_initial_version() != info.initial_shared_version {
        return Err(protocol_error(format!(
            "{name} object '{}' has initial version '{}', expected '{}'",
            info.id.bytes,
            response.get_initial_version(),
            info.initial_shared_version
        )));
    }
    Ok(response.object_ref())
}

async fn refresh_optional_authority(
    crawler: &Crawler,
    configured: &sui::types::ObjectReference,
) -> Result<sui::types::ObjectReference, NexusError> {
    if *configured.object_id() == sui::types::Address::ZERO {
        return Ok(configured.clone());
    }
    crawler
        .get_object_metadata(*configured.object_id())
        .await
        .map(|response| response.object_ref())
        .map_err(NexusError::Rpc)
}

fn protocol_error(error: impl std::fmt::Display) -> NexusError {
    NexusError::ProtocolValidation(anyhow!("{error}"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                move_std::option::Option as MoveOption,
                sui_framework::{
                    object::{ID, UID},
                    vec_map::{Entry, VecMap},
                    versioned::Versioned,
                },
            },
            types::nexus_objects::PackageLink,
        },
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn package(value: &'static str) -> PackageInfo {
        let id = ID::new(address(value));
        PackageInfo::new(id, id, 1)
    }

    fn shared(value: &'static str) -> SharedObjectInfo {
        SharedObjectInfo::new(ID::new(address(value)), 1)
    }

    fn config(protocol_version: u64) -> ProtocolConfigV1 {
        let packages = VecMap::new(
            ["0x11", "0x12", "0x13", "0x14", "0x15", "0x16"]
                .into_iter()
                .zip(PACKAGE_ROLES)
                .map(|(id, (role, _))| Entry::new(role, package(id)))
                .collect(),
        );
        let shared_objects = VecMap::new(
            ["0x21", "0x22", "0x23", "0x24", "0x25", "0x26", "0x27"]
                .into_iter()
                .zip(SHARED_OBJECT_ROLES)
                .map(|(id, (role, _))| Entry::new(role, shared(id)))
                .collect(),
        );
        let mut config = ProtocolConfigV1::new(protocol_version, packages, shared_objects, vec![]);
        set_config_hash(&mut config);
        config
    }

    fn set_config_hash(config: &mut ProtocolConfigV1) {
        let input = ProtocolConfigHashInputV1::new(
            config.protocol_version,
            config.packages.clone(),
            config.shared_objects.clone(),
        );
        config.config_hash = sui::types::hash::Hasher::digest(bcs::to_bytes(&input).unwrap())
            .as_bytes()
            .to_vec();
    }

    fn protocol_response(
        protocol_id: sui::types::Address,
        owner: sui::types::Owner,
        state_schema: u64,
    ) -> Response<Protocol> {
        Response {
            object_id: protocol_id,
            owner,
            version: 9,
            data: Protocol::new(
                UID::new(protocol_id),
                Versioned::new(UID::new(address("0x31")), state_schema),
            ),
            digest: sui::types::Digest::ZERO,
            balance: None,
        }
    }

    fn protocol_state(configs: Vec<ProtocolConfigV1>) -> ProtocolStateV1 {
        let mut active = MoveOption::from_option(None::<ProtocolConfigV1>);
        active.vec = configs;
        ProtocolStateV1::new(active)
    }

    fn origin_package(storage_id: Option<&str>, original_id: Option<&str>) -> sui::grpc::Package {
        let mut package = sui::grpc::Package::default();
        package.storage_id = storage_id.map(ToOwned::to_owned);
        package.original_id = original_id.map(ToOwned::to_owned);
        package
    }

    fn shared_response(object_id: sui::types::Address, owner: sui::types::Owner) -> Response<()> {
        Response {
            object_id,
            owner,
            version: 9,
            data: (),
            digest: sui::types::Digest::ZERO,
            balance: None,
        }
    }

    #[test]
    fn config_hash_covers_every_protocol_binding() {
        let config = config(1);
        validate_config(&config).unwrap();

        let mut tampered = config;
        tampered.packages.contents[5].value.version += 1;
        let error = validate_config(&tampered).unwrap_err();
        assert!(error.to_string().contains("configuration hash"));
    }

    #[test]
    fn config_and_active_state_reject_invalid_shapes() {
        let mut version_zero = config(1);
        version_zero.protocol_version = 0;
        assert!(validate_config(&version_zero)
            .unwrap_err()
            .to_string()
            .contains("version zero"));

        let mut short_hash = config(1);
        short_hash.config_hash.pop();
        assert!(validate_config(&short_hash)
            .unwrap_err()
            .to_string()
            .contains("expected 32"));

        assert!(active_config(protocol_state(vec![]))
            .unwrap_err()
            .to_string()
            .contains("no active configuration"));
        assert_eq!(
            active_config(protocol_state(vec![config(1)]))
                .unwrap()
                .protocol_version,
            1
        );
        assert!(active_config(protocol_state(vec![config(1), config(2)]))
            .unwrap_err()
            .to_string()
            .contains("contains '2' configurations"));
    }

    #[test]
    fn active_configuration_change_is_monotonic_and_immutable() {
        let current = config(2);
        let unchanged = current.clone();
        let upgrade = config(3);
        let downgrade = config(1);
        let mut conflict = current.clone();
        conflict.config_hash = vec![9; sui::types::Digest::LENGTH];

        assert!(!active_configuration_changed(
            current.protocol_version,
            &current.config_hash,
            &unchanged,
        )
        .unwrap());
        assert!(active_configuration_changed(
            current.protocol_version,
            &current.config_hash,
            &upgrade,
        )
        .unwrap());
        assert!(active_configuration_changed(
            current.protocol_version,
            &current.config_hash,
            &downgrade,
        )
        .unwrap_err()
        .to_string()
        .contains("downgrade"));
        assert!(active_configuration_changed(
            current.protocol_version,
            &current.config_hash,
            &conflict,
        )
        .unwrap_err()
        .to_string()
        .contains("changed configuration hash"));
    }

    #[test]
    fn role_bindings_require_canonical_order_and_unique_identities() {
        let mut wrong_role = config(1);
        wrong_role.packages.contents.swap(0, 1);
        set_config_hash(&mut wrong_role);
        assert!(validate_config(&wrong_role)
            .unwrap_err()
            .to_string()
            .contains("expected '0'"));

        let mut duplicate_package = config(1);
        duplicate_package.packages.contents[1].value.initial_id =
            duplicate_package.packages.contents[0].value.initial_id;
        duplicate_package.packages.contents[1].value.storage_id =
            duplicate_package.packages.contents[0].value.storage_id;
        set_config_hash(&mut duplicate_package);
        assert!(validate_config(&duplicate_package)
            .unwrap_err()
            .to_string()
            .contains("bound to both"));

        let mut duplicate_object = config(1);
        duplicate_object.shared_objects.contents[1].value.id =
            duplicate_object.shared_objects.contents[0].value.id;
        set_config_hash(&mut duplicate_object);
        assert!(validate_config(&duplicate_object)
            .unwrap_err()
            .to_string()
            .contains("bound to both"));
    }

    #[test]
    fn protocol_root_requires_embedded_identity_schema_and_shared_ownership() {
        let protocol_id = address("0x30");
        let valid = protocol_response(protocol_id, sui::types::Owner::Shared(1), 1);
        validate_protocol_root(protocol_id, &valid).unwrap();

        let wrong_identity = protocol_response(address("0x32"), sui::types::Owner::Shared(1), 1);
        assert!(validate_protocol_root(protocol_id, &wrong_identity)
            .unwrap_err()
            .to_string()
            .contains("embedded identity"));
        let wrong_schema = protocol_response(protocol_id, sui::types::Owner::Shared(1), 2);
        assert!(validate_protocol_root(protocol_id, &wrong_schema)
            .unwrap_err()
            .to_string()
            .contains("unsupported state schema"));
        let owned = protocol_response(protocol_id, sui::types::Owner::Address(address("0x33")), 1);
        assert!(validate_protocol_root(protocol_id, &owned)
            .unwrap_err()
            .to_string()
            .contains("not shared"));
    }

    #[test]
    fn datatype_origin_package_must_match_the_claimed_lineage() {
        let storage_id = address("0x41");
        let initial_id = address("0x42");
        validate_origin_package(
            &origin_package(Some(&storage_id.to_string()), Some(&initial_id.to_string())),
            storage_id,
            initial_id,
        )
        .unwrap();

        for (package, expected) in [
            (origin_package(None, Some("0x42")), "no storage ID"),
            (
                origin_package(Some("not-an-id"), Some("0x42")),
                "invalid storage ID",
            ),
            (origin_package(Some("0x41"), None), "no original ID"),
            (
                origin_package(Some("0x41"), Some("not-an-id")),
                "invalid original ID",
            ),
            (
                origin_package(Some("0x43"), Some("0x42")),
                "belongs to lineage",
            ),
            (
                origin_package(Some("0x41"), Some("0x44")),
                "belongs to lineage",
            ),
        ] {
            assert!(validate_origin_package(&package, storage_id, initial_id)
                .unwrap_err()
                .to_string()
                .contains(expected));
        }
    }

    #[test]
    fn datatype_origins_cannot_be_claimed_by_two_package_families() {
        let mut packages = NexusPackages::first_publication(
            address("0x51"),
            address("0x52"),
            address("0x53"),
            address("0x54"),
            address("0x55"),
            address("0x56"),
        );
        let origins = collect_type_origin_lineages(&packages).unwrap();
        assert_eq!(origins.len(), 6);

        let primitives_initial_id = packages.primitives.initial_id;
        packages
            .interface
            .insert_type_origin(
                crate::types::DatatypeKey::new("graph", "Vertex"),
                primitives_initial_id,
            )
            .unwrap();
        assert!(collect_type_origin_lineages(&packages)
            .unwrap_err()
            .to_string()
            .contains("claimed by lineages"));
    }

    #[test]
    fn configured_shared_objects_require_exact_initial_versions() {
        let object_id = address("0x61");
        let info = SharedObjectInfo::new(ID::new(object_id), 3);
        let empty = HashMap::new();
        assert!(resolve_shared_object(&empty, "Registry", &info)
            .unwrap_err()
            .to_string()
            .contains("was not returned"));

        let owned = HashMap::from([(
            object_id,
            shared_response(object_id, sui::types::Owner::Address(address("0x62"))),
        )]);
        assert!(resolve_shared_object(&owned, "Registry", &info)
            .unwrap_err()
            .to_string()
            .contains("not shared"));

        let wrong_version = HashMap::from([(
            object_id,
            shared_response(object_id, sui::types::Owner::Shared(2)),
        )]);
        assert!(resolve_shared_object(&wrong_version, "Registry", &info)
            .unwrap_err()
            .to_string()
            .contains("expected '3'"));

        let valid = HashMap::from([(
            object_id,
            shared_response(object_id, sui::types::Owner::Shared(3)),
        )]);
        let object = resolve_shared_object(&valid, "Registry", &info).unwrap();
        assert_eq!(*object.object_id(), object_id);
        assert_eq!(object.version(), 3);
    }

    #[tokio::test]
    async fn resolver_rejects_unsupported_protocol_version_before_network_resolution() {
        let protocol_id = address("0x70");
        let protocol = protocol_response(protocol_id, sui::types::Owner::Shared(1), 1);
        let client = Arc::new(sui::grpc::client("http://127.0.0.1:1").unwrap());
        let protocol_ref = protocol.object_ref();
        let extras = ProtocolExtras {
            priority_fee_vault_owner_cap: sui::types::ObjectReference::new(
                address("0x71"),
                1,
                sui::types::Digest::ZERO,
            ),
            us_token: UsTokenConfig::new(address("0x72")),
        };
        let resolver = ProtocolResolver::new(protocol_ref.clone(), client).with_extras(extras);
        assert_eq!(resolver.protocol(), &protocol_ref);

        let unsupported = config(MAX_SUPPORTED_PROTOCOL_VERSION + 1);

        let error = resolver
            .resolve_config_inner(protocol, &unsupported)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            NexusError::UnsupportedProtocolVersion {
                protocol_version: 3,
                maximum: 2,
            }
        ));
    }

    #[test]
    fn dependency_linkage_must_select_the_exact_storage_version() {
        let dependency =
            PackageVersion::new(address("0x11"), address("0x21"), 2, Default::default());
        let mut linkage = BTreeMap::new();
        linkage.insert(
            dependency.initial_id,
            PackageLink {
                storage_id: dependency.storage_id,
                version: dependency.version,
            },
        );
        let resolved_package = ResolvedPackageVersion {
            package: PackageVersion::first_publication(address("0x30")),
            linkage,
        };
        validate_package_linkage(
            "interface",
            &resolved_package,
            &[("primitives", &dependency)],
            &[("primitives", &dependency)],
        )
        .unwrap();

        let mut stale = resolved_package;
        stale
            .linkage
            .get_mut(&dependency.initial_id)
            .unwrap()
            .version = 1;
        let error = validate_package_linkage(
            "interface",
            &stale,
            &[("primitives", &dependency)],
            &[("primitives", &dependency)],
        )
        .unwrap_err();
        assert!(error.to_string().contains("expected version '2'"));

        let missing = ResolvedPackageVersion {
            package: PackageVersion::first_publication(address("0x31")),
            linkage: BTreeMap::new(),
        };
        assert!(validate_package_linkage(
            "interface",
            &missing,
            &[("primitives", &dependency)],
            &[("primitives", &dependency)],
        )
        .unwrap_err()
        .to_string()
        .contains("no linkage"));

        let mut wrong_storage = stale;
        let link = wrong_storage
            .linkage
            .get_mut(&dependency.initial_id)
            .unwrap();
        link.version = dependency.version;
        link.storage_id = address("0xff");
        assert!(validate_package_linkage(
            "interface",
            &wrong_storage,
            &[],
            &[("primitives", &dependency)],
        )
        .unwrap_err()
        .to_string()
        .contains("expected version"));

        assert_eq!(
            package_version(&package("0x80")),
            PackageVersion::first_publication(address("0x80"))
        );
        assert_eq!(
            ProtocolExtras::default().priority_fee_vault_owner_cap,
            default_object_reference()
        );
    }
}
