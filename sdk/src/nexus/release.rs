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

/// Resolves and validates the exact snapshot selected by one stable [`Protocol`].
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
        let record = active_record(state)?;
        self.resolve_record_inner(protocol, &record).await
    }

    /// Resolve an event snapshot after confirming it is still canonical.
    pub async fn resolve_record(
        &self,
        record: &ReleaseRecordV1,
    ) -> Result<NexusObjects, NexusError> {
        self.resolve_record_release(record)
            .await
            .map(|release| release.objects)
    }

    /// Resolve an event snapshot and its consumer API requirements.
    pub async fn resolve_record_release(
        &self,
        record: &ReleaseRecordV1,
    ) -> Result<ResolvedRelease, NexusError> {
        let (protocol, state) = self.protocol_state().await?;
        let active = active_record(state)?;
        validate_record(record)?;
        validate_record(&active)?;
        if record != &active {
            return Err(release_error(format!(
                "Release event '{}' with manifest '{}' is not the active snapshot '{}' with \
                 manifest '{}'",
                record.release,
                hex::encode(&record.manifest_hash),
                active.release,
                hex::encode(&active.manifest_hash),
            )));
        }
        self.resolve_record_inner(protocol, record).await
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
            return Err(release_error(format!(
                "Protocol active option contains '{}' records",
                state.active.vec.len()
            )));
        }
        Ok((protocol, state))
    }

    async fn resolve_record_inner(
        &self,
        protocol: Response<Protocol>,
        record: &ReleaseRecordV1,
    ) -> Result<ResolvedRelease, NexusError> {
        let crawler = Crawler::new(Arc::clone(&self.client));
        validate_record(record)?;
        let release = record.release;

        if record.sdk_api_version != SUPPORTED_SDK_API_VERSION {
            return Err(NexusError::UnsupportedSdkApi {
                release,
                required: record.sdk_api_version,
                supported: SUPPORTED_SDK_API_VERSION,
            });
        }

        let packages = self.resolve_packages(record).await?;
        validate_type_origin_lineages(&self.client, &packages).await?;
        let refs = resolve_shared_objects(&crawler, record).await?;
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

fn validate_protocol_root(
    protocol_id: sui::types::Address,
    protocol: &Response<Protocol>,
) -> Result<(), NexusError> {
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
    Ok(())
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

fn active_record(state: ProtocolStateV1) -> Result<ReleaseRecordV1, NexusError> {
    match state.active.vec.as_slice() {
        [] => Err(release_error("Protocol has no active release")),
        [record] => Ok(record.clone()),
        records => Err(release_error(format!(
            "Protocol active option contains '{}' records",
            records.len()
        ))),
    }
}

fn validate_record(record: &ReleaseRecordV1) -> Result<(), NexusError> {
    if record.release == 0 {
        return Err(release_error("Release zero is not a published release"));
    }
    if record.manifest_hash.len() != sui::types::Digest::LENGTH {
        return Err(release_error(format!(
            "Release '{}' manifest is {} bytes, expected 32",
            record.release,
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
            "Release '{}' manifest hash does not match its contents",
            record.release
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
                    return Err(release_error(format!(
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
            release_error(format!(
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

    Ok(SharedReferences {
        tool_registry: resolve_shared_object(
            &by_id,
            "ToolRegistry",
            &record.objects.tool_registry,
        )?,
        verifier_registry: resolve_shared_object(
            &by_id,
            "VerifierRegistry",
            &record.objects.verifier_registry,
        )?,
        network_auth: resolve_shared_object(&by_id, "NetworkAuth", &record.objects.network_auth)?,
        agent_registry: resolve_shared_object(
            &by_id,
            "AgentRegistry",
            &record.objects.agent_registry,
        )?,
        gas_service: resolve_shared_object(&by_id, "GasService", &record.objects.gas_service)?,
        leader_registry: resolve_shared_object(
            &by_id,
            "LeaderRegistry",
            &record.objects.leader_registry,
        )?,
        priority_fee_vault: resolve_shared_object(
            &by_id,
            "PriorityFeeVault",
            &record.objects.priority_fee_vault,
        )?,
    })
}

fn resolve_shared_object(
    by_id: &HashMap<sui::types::Address, Response<()>>,
    name: &str,
    info: &SharedObjectInfo,
) -> Result<sui::types::ObjectReference, NexusError> {
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
            move_bindings::{
                move_std::option::Option as MoveOption,
                primitives::protocol::SystemObjectsV1,
                sui_framework::{
                    object::{ID, UID},
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

    fn protocol_response(
        protocol_id: sui::types::Address,
        owner: sui::types::Owner,
        state_version: u64,
    ) -> Response<Protocol> {
        Response {
            object_id: protocol_id,
            owner,
            version: 9,
            data: Protocol::new(
                UID::new(protocol_id),
                Versioned::new(UID::new(address("0x31")), state_version),
            ),
            digest: sui::types::Digest::ZERO,
            balance: None,
        }
    }

    fn protocol_state(records: Vec<ReleaseRecordV1>) -> ProtocolStateV1 {
        let mut active = MoveOption::from_option(None::<ReleaseRecordV1>);
        active.vec = records;
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
    fn manifest_hash_covers_every_release_field() {
        let record = record();
        validate_record(&record).unwrap();

        let mut tampered = record;
        tampered.leader_api_version += 1;
        let error = validate_record(&tampered).unwrap_err();
        assert!(error.to_string().contains("manifest hash"));
    }

    #[test]
    fn manifest_and_active_state_reject_invalid_release_shapes() {
        let mut release_zero = record();
        release_zero.release = 0;
        assert!(validate_record(&release_zero)
            .unwrap_err()
            .to_string()
            .contains("Release zero"));

        let mut short_hash = record();
        short_hash.manifest_hash.pop();
        assert!(validate_record(&short_hash)
            .unwrap_err()
            .to_string()
            .contains("expected 32"));

        assert!(active_record(protocol_state(vec![]))
            .unwrap_err()
            .to_string()
            .contains("no active release"));
        assert_eq!(
            active_record(protocol_state(vec![record()]))
                .unwrap()
                .release,
            1
        );
        assert!(active_record(protocol_state(vec![record(), record()]))
            .unwrap_err()
            .to_string()
            .contains("contains '2' records"));
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
    fn shared_release_objects_require_exact_initial_versions() {
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
    async fn resolver_rejects_unsupported_sdk_api_before_network_resolution() {
        let protocol_id = address("0x70");
        let protocol = protocol_response(protocol_id, sui::types::Owner::Shared(1), 1);
        let client = Arc::new(sui::grpc::client("http://127.0.0.1:1").unwrap());
        let protocol_ref = protocol.object_ref();
        let extras = ReleaseExtras {
            priority_fee_vault_owner_cap: sui::types::ObjectReference::new(
                address("0x71"),
                1,
                sui::types::Digest::ZERO,
            ),
            us_token: UsTokenConfig::new(address("0x72")),
        };
        let resolver = ReleaseResolver::new(protocol_ref.clone(), client).with_extras(extras);
        assert_eq!(resolver.protocol(), &protocol_ref);

        let mut unsupported = record();
        unsupported.sdk_api_version = SUPPORTED_SDK_API_VERSION + 1;
        let manifest = ReleaseManifestV1::new(
            unsupported.release,
            unsupported.primitives.clone(),
            unsupported.interface.clone(),
            unsupported.registry.clone(),
            unsupported.gas.clone(),
            unsupported.workflow.clone(),
            unsupported.scheduler.clone(),
            unsupported.objects.clone(),
            unsupported.sdk_api_version,
            unsupported.leader_api_version,
        );
        unsupported.manifest_hash =
            sui::types::hash::Hasher::digest(bcs::to_bytes(&manifest).unwrap())
                .as_bytes()
                .to_vec();

        let error = resolver
            .resolve_record_inner(protocol, &unsupported)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            NexusError::UnsupportedSdkApi {
                release: 1,
                required: 2,
                supported: 1
            }
        ));
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
        let resolved_package = ResolvedPackageRelease {
            release: PackageRelease::first_publication(address("0x30")),
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

        let missing = ResolvedPackageRelease {
            release: PackageRelease::first_publication(address("0x31")),
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
            package_release(&package("0x80")),
            PackageRelease::first_publication(address("0x80"))
        );
        assert_eq!(
            ReleaseExtras::default().priority_fee_vault_owner_cap,
            default_object_reference()
        );
    }
}
