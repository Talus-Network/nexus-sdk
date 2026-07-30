//! Canonical Nexus protocol release resolution.

use {
    crate::{
        move_bindings::primitives::protocol::{
            PackageInfo,
            Protocol,
            ProtocolStateV1,
            ReleaseManifestV1,
            ReleaseRecordV1,
            SharedObjectInfo,
        },
        nexus::{
            crawler::{Crawler, Response},
            error::NexusError,
            tap,
        },
        sui,
        types::{
            nexus_objects::{
                default_object_reference,
                resolve_package_release_metadata,
                ResolvedPackageRelease,
            },
            NexusObjects,
            NexusPackages,
            PackageRelease,
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

/// Newest release manifest API understood by this SDK.
pub const SUPPORTED_SDK_API_VERSION: u64 = 1;

/// Configuration that is intentionally outside the six package release.
///
/// The default DAG executor is refreshed from [`crate::types::AgentRegistrySnapshot`].
/// The US token is an external dependency, and the priority fee capability is
/// optional operator authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseExtras {
    pub priority_fee_vault_owner_cap: sui::types::ObjectReference,
    pub us_token: UsTokenConfig,
}

impl Default for ReleaseExtras {
    fn default() -> Self {
        Self {
            priority_fee_vault_owner_cap: default_object_reference(),
            us_token: UsTokenConfig::default(),
        }
    }
}

impl From<&NexusObjects> for ReleaseExtras {
    fn from(objects: &NexusObjects) -> Self {
        Self {
            priority_fee_vault_owner_cap: objects.priority_fee_vault_owner_cap.clone(),
            us_token: objects.us_token.clone(),
        }
    }
}

/// A validated protocol release and its declared consumer API requirements.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRelease {
    pub objects: NexusObjects,
    pub sdk_api_version: u64,
    pub leader_api_version: u64,
}

/// Resolves and validates releases registered under one stable [`Protocol`].
#[derive(Clone)]
pub struct ReleaseResolver {
    protocol: sui::types::ObjectReference,
    client: Arc<sui::grpc::Client>,
    extras: ReleaseExtras,
}

impl ReleaseResolver {
    pub fn new(protocol: sui::types::ObjectReference, client: Arc<sui::grpc::Client>) -> Self {
        Self {
            protocol,
            client,
            extras: ReleaseExtras::default(),
        }
    }

    pub fn with_extras(mut self, extras: ReleaseExtras) -> Self {
        self.extras = extras;
        self
    }

    pub fn protocol(&self) -> &sui::types::ObjectReference {
        &self.protocol
    }

    /// Resolve the release selected by the canonical protocol root.
    pub async fn resolve_active(&self) -> Result<NexusObjects, NexusError> {
        self.resolve_active_release()
            .await
            .map(|release| release.objects)
    }

    /// Resolve the active release and its consumer API requirements.
    pub async fn resolve_active_release(&self) -> Result<ResolvedRelease, NexusError> {
        let (protocol, state) = self.protocol_state().await?;
        let release = state.active_release;
        if release == 0 {
            return Err(release_error("Protocol has no active release"));
        }
        self.resolve_record(protocol, state, release).await
    }

    /// Resolve one registered release that is not below the protocol floor.
    pub async fn resolve(&self, release: u64) -> Result<NexusObjects, NexusError> {
        self.resolve_release(release)
            .await
            .map(|release| release.objects)
    }

    /// Resolve one registered release and its consumer API requirements.
    pub async fn resolve_release(&self, release: u64) -> Result<ResolvedRelease, NexusError> {
        let (protocol, state) = self.protocol_state().await?;
        self.resolve_record(protocol, state, release).await
    }

    async fn protocol_state(&self) -> Result<(Response<Protocol>, ProtocolStateV1), NexusError> {
        let crawler = Crawler::new(Arc::clone(&self.client));
        let protocol_id = *self.protocol.object_id();
        let protocol = crawler
            .get_object::<Protocol>(protocol_id)
            .await
            .map_err(NexusError::Rpc)?;
        if protocol.data.id.id.bytes != protocol_id {
            return Err(release_error(format!(
                "Protocol '{protocol_id}' contains embedded identity '{}'",
                protocol.data.id.id.bytes
            )));
        }
        if protocol.data.state.version != 1 {
            return Err(release_error(format!(
                "Protocol '{protocol_id}' uses unsupported state schema '{}'",
                protocol.data.state.version
            )));
        }
        if !protocol.is_shared() {
            return Err(release_error(format!(
                "Protocol '{protocol_id}' is not shared"
            )));
        }
        let state = crawler
            .get_versioned_state::<ProtocolStateV1>(&protocol.data.state)
            .await
            .map_err(NexusError::Rpc)?;
        if state.release_floor > state.active_release {
            return Err(release_error(format!(
                "Protocol release floor '{}' exceeds active release '{}'",
                state.release_floor, state.active_release
            )));
        }
        Ok((protocol, state))
    }

    async fn resolve_record(
        &self,
        protocol: Response<Protocol>,
        state: ProtocolStateV1,
        release: u64,
    ) -> Result<ResolvedRelease, NexusError> {
        if release == 0 {
            return Err(release_error("Release zero is not a published release"));
        }
        if release < state.release_floor {
            return Err(release_error(format!(
                "Release '{release}' is below protocol floor '{}'",
                state.release_floor
            )));
        }
        if release > state.active_release {
            return Err(release_error(format!(
                "Release '{release}' is not active; current release is '{}'",
                state.active_release
            )));
        }

        let crawler = Crawler::new(Arc::clone(&self.client));
        let record = crawler
            .get_dynamic_field_by_key::<u64, ReleaseRecordV1>(
                state.releases.id(),
                release,
                &sui::types::TypeTag::U64,
            )
            .await
            .map_err(NexusError::Rpc)?
            .ok_or_else(|| release_error(format!("Protocol has no release record '{release}'")))?;
        validate_record(&record, release)?;

        if record.sdk_api_version != SUPPORTED_SDK_API_VERSION {
            return Err(NexusError::UnsupportedSdkApi {
                release,
                required: record.sdk_api_version,
                supported: SUPPORTED_SDK_API_VERSION,
            });
        }

        let packages = self.resolve_packages(&record).await?;
        validate_type_origin_lineages(&self.client, &packages).await?;
        let refs = resolve_shared_objects(&crawler, &record).await?;
        let default_dag_executor =
            tap::fetch_default_dag_executor(&crawler, *refs.agent_registry.object_id())
                .await
                .map_err(NexusError::Rpc)?
                .ok_or_else(|| {
                    release_error("Activated AgentRegistry has no default DAG executor")
                })?
                .target();
        let priority_fee_vault_owner_cap =
            refresh_optional_authority(&crawler, &self.extras.priority_fee_vault_owner_cap).await?;

        Ok(ResolvedRelease {
            objects: NexusObjects {
                release,
                protocol: protocol.object_ref(),
                packages,
                manifest_hash: record.manifest_hash.clone(),
                network_id: record.objects.network.bytes,
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
            },
            sdk_api_version: record.sdk_api_version,
            leader_api_version: record.leader_api_version,
        })
    }

    async fn resolve_packages(
        &self,
        record: &ReleaseRecordV1,
    ) -> Result<NexusPackages, NexusError> {
        let primitives = package_release(&record.primitives);
        let interface = package_release(&record.interface);
        let registry = package_release(&record.registry);
        let gas = package_release(&record.gas);
        let workflow = package_release(&record.workflow);
        let scheduler = package_release(&record.scheduler);
        let (primitives, interface, registry, gas, workflow, scheduler) = tokio::try_join!(
            resolve_package_release_metadata(&self.client, &primitives, "primitives"),
            resolve_package_release_metadata(&self.client, &interface, "interface"),
            resolve_package_release_metadata(&self.client, &registry, "registry"),
            resolve_package_release_metadata(&self.client, &gas, "gas"),
            resolve_package_release_metadata(&self.client, &workflow, "workflow"),
            resolve_package_release_metadata(&self.client, &scheduler, "scheduler"),
        )
        .map_err(NexusError::ReleaseValidation)?;

        let families = [
            ("primitives", &primitives.release),
            ("interface", &interface.release),
            ("registry", &registry.release),
            ("gas", &gas.release),
            ("workflow", &workflow.release),
            ("scheduler", &scheduler.release),
        ];
        validate_package_linkage("primitives", &primitives, &[], &families)?;
        validate_package_linkage(
            "interface",
            &interface,
            &[("primitives", &primitives.release)],
            &families,
        )?;
        validate_package_linkage(
            "registry",
            &registry,
            &[
                ("primitives", &primitives.release),
                ("interface", &interface.release),
            ],
            &families,
        )?;
        validate_package_linkage(
            "gas",
            &gas,
            &[
                ("primitives", &primitives.release),
                ("interface", &interface.release),
                ("registry", &registry.release),
            ],
            &families,
        )?;
        validate_package_linkage(
            "workflow",
            &workflow,
            &[
                ("primitives", &primitives.release),
                ("interface", &interface.release),
                ("registry", &registry.release),
                ("gas", &gas.release),
            ],
            &families,
        )?;
        validate_package_linkage(
            "scheduler",
            &scheduler,
            &[
                ("primitives", &primitives.release),
                ("interface", &interface.release),
                ("registry", &registry.release),
                ("workflow", &workflow.release),
            ],
            &families,
        )?;

        Ok(NexusPackages {
            primitives: primitives.release,
            interface: interface.release,
            registry: registry.release,
            gas: gas.release,
            workflow: workflow.release,
            scheduler: scheduler.release,
        })
    }
}

fn validate_package_linkage(
    package_name: &str,
    package: &ResolvedPackageRelease,
    required: &[(&str, &PackageRelease)],
    families: &[(&str, &PackageRelease)],
) -> Result<(), NexusError> {
    for (dependency_name, dependency) in required {
        let link = package.linkage.get(&dependency.initial_id).ok_or_else(|| {
            release_error(format!(
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
    expected: &PackageRelease,
) -> Result<(), NexusError> {
    if link.storage_id != expected.storage_id || link.version != expected.version {
        return Err(release_error(format!(
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

fn package_release(info: &PackageInfo) -> PackageRelease {
    PackageRelease::new(
        info.initial_id.bytes,
        info.storage_id.bytes,
        info.version,
        Default::default(),
    )
}

fn validate_record(record: &ReleaseRecordV1, expected_release: u64) -> Result<(), NexusError> {
    if record.release != expected_release {
        return Err(release_error(format!(
            "Release record key '{expected_release}' contains release '{}'",
            record.release
        )));
    }
    if record.manifest_hash.len() != sui::types::Digest::LENGTH {
        return Err(release_error(format!(
            "Release '{expected_release}' manifest is {} bytes, expected 32",
            record.manifest_hash.len()
        )));
    }
    let manifest = ReleaseManifestV1 {
        release: record.release,
        primitives: record.primitives.clone(),
        interface: record.interface.clone(),
        registry: record.registry.clone(),
        gas: record.gas.clone(),
        workflow: record.workflow.clone(),
        scheduler: record.scheduler.clone(),
        objects: record.objects.clone(),
        sdk_api_version: record.sdk_api_version,
        leader_api_version: record.leader_api_version,
    };
    let bytes = bcs::to_bytes(&manifest)
        .context("Could not encode release manifest")
        .map_err(NexusError::ReleaseValidation)?;
    let digest = sui::types::hash::Hasher::digest(bytes);
    if record.manifest_hash.as_slice() != digest.as_bytes() {
        return Err(release_error(format!(
            "Release '{expected_release}' manifest hash does not match its contents"
        )));
    }
    Ok(())
}

async fn validate_type_origin_lineages(
    client: &Arc<sui::grpc::Client>,
    packages: &NexusPackages,
) -> Result<(), NexusError> {
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
                    return Err(release_error(format!(
                        "Package '{origin}' is claimed by lineages '{previous}' and '{}'",
                        package.initial_id
                    )));
                }
            }
        }
    }

    try_join_all(origins.into_iter().map(|(storage_id, initial_id)| {
        validate_origin_lineage(Arc::clone(client), storage_id, initial_id)
    }))
    .await?;
    Ok(())
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
            release_error(format!(
                "Datatype origin package '{storage_id}' was not returned"
            ))
        })?;
    let observed_storage: sui::types::Address = package
        .storage_id
        .as_deref()
        .ok_or_else(|| {
            release_error(format!(
                "Datatype origin package '{storage_id}' has no storage ID"
            ))
        })?
        .parse()
        .map_err(|error| {
            release_error(format!(
                "Datatype origin package '{storage_id}' has invalid storage ID: {error}"
            ))
        })?;
    let observed_initial: sui::types::Address = package
        .original_id
        .as_deref()
        .ok_or_else(|| {
            release_error(format!(
                "Datatype origin package '{storage_id}' has no original ID"
            ))
        })?
        .parse()
        .map_err(|error| {
            release_error(format!(
                "Datatype origin package '{storage_id}' has invalid original ID: {error}"
            ))
        })?;
    if observed_storage != storage_id || observed_initial != expected_initial_id {
        return Err(release_error(format!(
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
    record: &ReleaseRecordV1,
) -> Result<SharedReferences, NexusError> {
    let entries = [
        ("ToolRegistry", &record.objects.tool_registry),
        ("VerifierRegistry", &record.objects.verifier_registry),
        ("NetworkAuth", &record.objects.network_auth),
        ("AgentRegistry", &record.objects.agent_registry),
        ("GasService", &record.objects.gas_service),
        ("LeaderRegistry", &record.objects.leader_registry),
        ("PriorityFeeVault", &record.objects.priority_fee_vault),
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

    let resolve =
        |name: &str, info: &SharedObjectInfo| -> Result<sui::types::ObjectReference, NexusError> {
            let response = by_id.get(&info.id.bytes).ok_or_else(|| {
                release_error(format!(
                    "{name} object '{}' was not returned",
                    info.id.bytes
                ))
            })?;
            if !response.is_shared() {
                return Err(release_error(format!(
                    "{name} object '{}' is not shared",
                    info.id.bytes
                )));
            }
            if response.get_initial_version() != info.initial_shared_version {
                return Err(release_error(format!(
                    "{name} object '{}' has initial version '{}', expected '{}'",
                    info.id.bytes,
                    response.get_initial_version(),
                    info.initial_shared_version
                )));
            }
            Ok(response.object_ref())
        };

    Ok(SharedReferences {
        tool_registry: resolve("ToolRegistry", &record.objects.tool_registry)?,
        verifier_registry: resolve("VerifierRegistry", &record.objects.verifier_registry)?,
        network_auth: resolve("NetworkAuth", &record.objects.network_auth)?,
        agent_registry: resolve("AgentRegistry", &record.objects.agent_registry)?,
        gas_service: resolve("GasService", &record.objects.gas_service)?,
        leader_registry: resolve("LeaderRegistry", &record.objects.leader_registry)?,
        priority_fee_vault: resolve("PriorityFeeVault", &record.objects.priority_fee_vault)?,
    })
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

fn release_error(error: impl std::fmt::Display) -> NexusError {
    NexusError::ReleaseValidation(anyhow!("{error}"))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{primitives::protocol::SystemObjectsV1, sui_framework::object::ID},
            types::nexus_objects::PackageLink,
        },
    };

    fn address(value: &'static str) -> sui::types::Address {
        sui::types::Address::from_static(value)
    }

    fn package(value: &'static str) -> PackageInfo {
        let id = ID::new(address(value));
        PackageInfo::new(id.clone(), id, 1)
    }

    fn shared(value: &'static str) -> SharedObjectInfo {
        SharedObjectInfo::new(ID::new(address(value)), 1)
    }

    fn record() -> ReleaseRecordV1 {
        let primitives = package("0x11");
        let interface = package("0x12");
        let registry = package("0x13");
        let gas = package("0x14");
        let workflow = package("0x15");
        let scheduler = package("0x16");
        let objects = SystemObjectsV1::new(
            ID::new(address("0x20")),
            shared("0x21"),
            shared("0x22"),
            shared("0x23"),
            shared("0x24"),
            shared("0x25"),
            shared("0x26"),
            shared("0x27"),
        );
        let manifest = ReleaseManifestV1::new(
            1,
            primitives.clone(),
            interface.clone(),
            registry.clone(),
            gas.clone(),
            workflow.clone(),
            scheduler.clone(),
            objects.clone(),
            1,
            1,
        );
        let hash = sui::types::hash::Hasher::digest(bcs::to_bytes(&manifest).unwrap());
        ReleaseRecordV1::new(
            1,
            primitives,
            interface,
            registry,
            gas,
            workflow,
            scheduler,
            objects,
            1,
            1,
            hash.as_bytes().to_vec(),
        )
    }

    #[test]
    fn manifest_hash_covers_every_release_field() {
        let record = record();
        validate_record(&record, 1).unwrap();

        let mut tampered = record;
        tampered.leader_api_version += 1;
        let error = validate_record(&tampered, 1).unwrap_err();
        assert!(error.to_string().contains("manifest hash"));
    }

    #[test]
    fn dependency_linkage_must_select_the_exact_storage_version() {
        let dependency =
            PackageRelease::new(address("0x11"), address("0x21"), 2, Default::default());
        let mut linkage = BTreeMap::new();
        linkage.insert(
            dependency.initial_id,
            PackageLink {
                storage_id: dependency.storage_id,
                version: dependency.version,
            },
        );
        let package = ResolvedPackageRelease {
            release: PackageRelease::first_publication(address("0x30")),
            linkage,
        };
        validate_package_linkage(
            "interface",
            &package,
            &[("primitives", &dependency)],
            &[("primitives", &dependency)],
        )
        .unwrap();

        let mut stale = package;
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
    }
}
