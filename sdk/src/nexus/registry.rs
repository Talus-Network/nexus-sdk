//! Registry capability discovery and verifier validation.

use {
    crate::{
        move_bindings::{
            move_std::type_name::TypeName,
            primitives,
            registry::{
                self,
                era::V1 as RegistryWitnessV1,
                leader::{Leader, LeaderRegistry, LeaderRegistryInnerV1, LeaderStatus},
            },
            sui_framework::object::ID,
        },
        nexus::{client::NexusClient, crawler::Crawler},
        sui::{self, grpc::owner::OwnerKind, traits::FieldMaskUtil},
        transactions::tool::{ExternalVerifierObjectInput, ExternalVerifierRegistrationInput},
        types::{NexusContext, PackageRole},
    },
    anyhow::{anyhow, bail},
    std::collections::HashMap,
};

type AnyCloneableOwnerCap =
    primitives::owner_cap::CloneableOwnerCap<registry::leader_cap::OverNetwork>;

const STAKE_WEIGHTED_RANK_DOMAIN: &[u8] = b"nexus_registry::leader::stake_weighted_rank_v1";

/// Current eligible leaders and the exact Move work domain used for ranking.
#[derive(Clone, Debug)]
pub struct WorkAdmissionCommittee {
    eligible: Vec<(sui::types::Address, u64)>,
    work_type: TypeName,
}

impl WorkAdmissionCommittee {
    /// Select the two leaders for one Scheduler seed.
    pub fn rank(&self, seed: &[u8]) -> anyhow::Result<[sui::types::Address; 2]> {
        rank_stake_weighted(self.eligible.clone(), &self.work_type, seed)
    }

    /// Returns whether `leader` is currently eligible for any work seed.
    pub fn contains(&self, leader: sui::types::Address) -> bool {
        self.eligible
            .iter()
            .any(|(candidate, _)| *candidate == leader)
    }
}

/// Read the current Move inputs shared by all Scheduler committee rankings.
pub async fn fetch_work_admission_committee(
    client: &NexusClient,
    context: &NexusContext,
) -> anyhow::Result<WorkAdmissionCommittee> {
    let registry_id = client.get_nexus_objects().leader_registry.object_id();
    let registry = client
        .state_resolver()
        .load_inner::<LeaderRegistry, RegistryWitnessV1, LeaderRegistryInnerV1>(
            registry_id,
            context,
        )
        .await?;
    let leader_ids = registry.data.leaders.contents.clone();
    let records = client
        .crawler()
        .get_dynamic_fields_by_keys::<ID, Leader, _>(
            registry.data.records.id(),
            leader_ids.iter().cloned(),
            &crate::move_bindings::type_tag::<ID>(context),
        )
        .await?;

    let mut eligible = Vec::with_capacity(leader_ids.len());
    for id in leader_ids {
        let leader = records.get(&id).ok_or_else(|| {
            anyhow!(
                "leader registry record {} is missing from its dynamic field table",
                id.bytes
            )
        })?;
        if leader.status == LeaderStatus::Active
            && leader.stake_manager.pool.value >= registry.data.min_stake_us
        {
            eligible.push((id.bytes, leader.stake_manager.pool.value));
        }
    }
    let origin = context.type_origin(PackageRole::Scheduler, "era", "WorkAdmissionV1")?;
    let work_type = TypeName::new(&format!(
        "{}::era::WorkAdmissionV1",
        hex::encode(origin.as_bytes())
    ));

    Ok(WorkAdmissionCommittee {
        eligible,
        work_type,
    })
}

/// Reproduce the current Scheduler work admission committee selected by Move.
///
/// This reads the current [`LeaderRegistry`] and mirrors
/// `leader::rank_active_leaders_stake_weighted` exactly. The returned order is
/// advisory. A transaction must still pass Move admission because registry
/// state can change after this read.
pub async fn rank_work_admission_leaders(
    client: &NexusClient,
    context: &NexusContext,
    seed: &[u8],
) -> anyhow::Result<[sui::types::Address; 2]> {
    fetch_work_admission_committee(client, context)
        .await?
        .rank(seed)
}

fn rank_stake_weighted(
    mut remaining: Vec<(sui::types::Address, u64)>,
    work_type: &TypeName,
    seed: &[u8],
) -> anyhow::Result<[sui::types::Address; 2]> {
    anyhow::ensure!(!remaining.is_empty(), "no eligible leader exists");
    if remaining.len() == 1 {
        return Ok([remaining[0].0, remaining[0].0]);
    }

    let mut selected = [sui::types::Address::ZERO; 2];
    for (step, slot) in selected.iter_mut().enumerate() {
        let total = remaining
            .iter()
            .try_fold(0_u128, |total, (_, weight)| {
                total.checked_add(u128::from(*weight))
            })
            .ok_or_else(|| anyhow!("eligible leader stake overflow"))?;
        anyhow::ensure!(total > 0, "eligible leader stake is zero");
        let random = stake_weighted_u64(work_type, seed, step as u64)? as u128;
        let mut cursor = random % total;
        let chosen = remaining
            .iter()
            .position(|(_, weight)| {
                if cursor < u128::from(*weight) {
                    true
                } else {
                    cursor -= u128::from(*weight);
                    false
                }
            })
            .ok_or_else(|| anyhow!("stake weighted selection did not choose a leader"))?;
        *slot = remaining.swap_remove(chosen).0;
    }
    Ok(selected)
}

fn stake_weighted_u64(work_type: &TypeName, seed: &[u8], step: u64) -> anyhow::Result<u64> {
    let mut message = STAKE_WEIGHTED_RANK_DOMAIN.to_vec();
    message.extend(bcs::to_bytes(work_type)?);
    message.extend(bcs::to_bytes(&seed.to_vec())?);
    message.extend(bcs::to_bytes(&step)?);
    let digest = sui::types::hash::Hasher::digest(message).into_inner();
    Ok(u64::from_be_bytes(
        digest[..8].try_into().expect("digest has eight bytes"),
    ))
}

/// Decode the registry network ID from a loaded leader registry payload.
pub fn extract_network_id_from_leader_registry(
    state: &LeaderRegistryInnerV1,
) -> sui::types::Address {
    state.network_id()
}

pub async fn find_owned_capability_by_what_for(
    rpc_url: &str,
    owner: sui::types::Address,
    object_type: sui::types::StructTag,
    expected_what_for: sui::types::Address,
) -> anyhow::Result<Option<sui::types::ObjectReference>> {
    let request = sui::grpc::ListOwnedObjectsRequest::default()
        .with_owner(owner)
        .with_page_size(500)
        .with_object_type(object_type)
        .with_read_mask(sui::grpc::FieldMask::from_paths([
            "object_id",
            "version",
            "digest",
            "contents",
            "owner",
        ]));

    let mut client = sui::grpc::client(rpc_url)?;
    let response = client
        .state_client()
        .list_owned_objects(request)
        .await?
        .into_inner();

    Ok(response.objects().iter().find_map(|object| {
        let object_id = object.object_id_opt()?.parse().ok()?;
        let digest = object.digest_opt()?.parse().ok()?;
        let version = object_version(object)?;
        let bytes = object.contents_opt()?.value_opt()?;
        let parsed = bcs::from_bytes::<AnyCloneableOwnerCap>(bytes).ok()?;
        (parsed.what_for.bytes == expected_what_for)
            .then(|| sui::types::ObjectReference::new(object_id, version, digest))
    }))
}

fn object_version(object: &sui::grpc::Object) -> Option<u64> {
    let is_consensus = object
        .owner_opt()
        .and_then(|owner| owner.kind)
        .and_then(|kind| OwnerKind::try_from(kind).ok())
        .is_some_and(|kind| kind == OwnerKind::ConsensusAddress);

    if is_consensus {
        object.owner_opt().and_then(|owner| owner.version_opt())
    } else {
        object.version_opt()
    }
}

/// Validates one published external verifier ABI and resolves its shared objects.
///
/// The immutable package graph in `context` defines the exact Nexus parameter
/// and return types accepted by the Tool registration operation.
///
/// # Errors
///
/// Returns an error when the verifier function does not match the required ABI,
/// an object is absent or mutable, or the requested object order is invalid.
pub async fn preflight_external_verifier_registration(
    crawler: &Crawler,
    context: &NexusContext,
    package_id: sui::types::Address,
    module_name: &str,
    function_name: &str,
    verifier_object_ids: &[sui::types::Address],
) -> anyhow::Result<ExternalVerifierRegistrationInput> {
    if verifier_object_ids.is_empty() {
        bail!("External verifier requires its witness as object zero");
    }
    if verifier_object_ids.contains(&sui::types::Address::ZERO) {
        bail!("External verifier object IDs must not be zero");
    }
    if verifier_object_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != verifier_object_ids.len()
    {
        bail!("External verifier objects must be unique");
    }

    let package = crawler.get_package(package_id).await?;
    let module = package
        .modules()
        .iter()
        .find(|module| module.name() == module_name)
        .ok_or_else(|| anyhow!("Module '{module_name}' not found in package '{package_id}'"))?;
    let function = module
        .functions()
        .iter()
        .find(|function| function.name() == function_name)
        .ok_or_else(|| {
            anyhow!(
                "Function '{function_name}' not found in module '{module_name}' of package '{package_id}'"
            )
        })?;
    let object_types = validate_external_verifier_function(function, context)?;
    if object_types.len() != verifier_object_ids.len() {
        bail!(
            "External verifier ABI requires {} immutable shared objects, but {} were supplied",
            object_types.len(),
            verifier_object_ids.len()
        );
    }

    let metadata = crawler.get_objects_metadata(verifier_object_ids).await?;
    let mut by_id = metadata
        .into_iter()
        .map(|object| (object.object_id, object))
        .collect::<HashMap<_, _>>();
    let verifier_objects = verifier_object_ids
        .iter()
        .zip(object_types)
        .map(|(object_id, object_type)| {
            let object = by_id.remove(object_id).ok_or_else(|| {
                anyhow!("External verifier object '{object_id}' metadata was not returned")
            })?;
            if !object.is_shared() {
                bail!("External verifier object '{object_id}' is not shared");
            }
            Ok(ExternalVerifierObjectInput {
                object_ref: object.object_ref(),
                object_type,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ExternalVerifierRegistrationInput {
        package_id,
        module_name: module_name.to_owned(),
        function_name: function_name.to_owned(),
        verifier_objects,
    })
}

fn validate_external_verifier_function(
    function: &sui::grpc::FunctionDescriptor,
    context: &NexusContext,
) -> anyhow::Result<Vec<sui::types::TypeTag>> {
    use sui::grpc::{function_descriptor::Visibility, open_signature::Reference};

    let visibility = function
        .visibility
        .and_then(|visibility| Visibility::try_from(visibility).ok())
        .unwrap_or(Visibility::Unknown);
    if visibility != Visibility::Public {
        bail!("External verifier function must be public");
    }
    if !function.type_parameters().is_empty() {
        bail!("External verifier function must not declare type parameters");
    }
    if function.parameters().len() < 4 {
        bail!(
            "External verifier function must accept worksheet, result, auxiliary, and at least one witness object"
        );
    }

    let primitives = context.require_package(PackageRole::Primitives)?;
    let proof_of_uid = primitives.type_origin("proof_of_uid", "ProofOfUID")?;
    let tagged_output = primitives.type_origin("tagged_output", "TaggedOutput")?;
    let interface = context.require_package(PackageRole::Interface)?;
    let verification_verdict = interface.type_origin("verifier", "VerificationVerdict")?;

    let worksheet = &function.parameters()[0];
    require_reference(worksheet, Reference::Mutable, "worksheet")?;
    require_struct(
        worksheet,
        proof_of_uid,
        "proof_of_uid",
        "ProofOfUID",
        "worksheet",
    )?;
    let result = &function.parameters()[1];
    require_reference(result, Reference::Unknown, "result")?;
    require_struct(
        result,
        tagged_output,
        "tagged_output",
        "TaggedOutput",
        "result",
    )?;
    require_bytes(&function.parameters()[2], "auxiliary")?;

    let mut object_types = Vec::with_capacity(function.parameters().len() - 3);
    for (index, parameter) in function.parameters()[3..].iter().enumerate() {
        require_reference(parameter, Reference::Immutable, "verifier object")?;
        let object_type = signature_body_to_type_tag(
            parameter
                .body_opt()
                .ok_or_else(|| anyhow!("External verifier object {index} has no type"))?,
        )?;
        if !matches!(object_type, sui::types::TypeTag::Struct(_)) {
            bail!("External verifier object {index} must have a concrete object type");
        }
        object_types.push(object_type);
    }

    if function.returns().len() != 1 {
        bail!("External verifier function must return exactly one VerificationVerdict");
    }
    let verdict = &function.returns()[0];
    require_reference(verdict, Reference::Unknown, "return value")?;
    require_struct(
        verdict,
        verification_verdict,
        "verifier",
        "VerificationVerdict",
        "return value",
    )?;

    Ok(object_types)
}

fn require_reference(
    signature: &sui::grpc::OpenSignature,
    expected: sui::grpc::open_signature::Reference,
    label: &str,
) -> anyhow::Result<()> {
    let actual = signature
        .reference
        .and_then(|reference| sui::grpc::open_signature::Reference::try_from(reference).ok())
        .unwrap_or(sui::grpc::open_signature::Reference::Unknown);
    if actual != expected {
        bail!("External verifier {label} has the wrong reference kind");
    }
    Ok(())
}

fn require_bytes(signature: &sui::grpc::OpenSignature, label: &str) -> anyhow::Result<()> {
    use sui::grpc::open_signature_body::Type;

    require_reference(
        signature,
        sui::grpc::open_signature::Reference::Unknown,
        label,
    )?;
    let body = signature
        .body_opt()
        .ok_or_else(|| anyhow!("External verifier {label} has no type"))?;
    let kind = body
        .r#type
        .and_then(|kind| Type::try_from(kind).ok())
        .unwrap_or(Type::Unknown);
    let inner = body.type_parameter_instantiation.as_slice();
    let is_u8 = inner.len() == 1
        && inner[0].r#type.and_then(|kind| Type::try_from(kind).ok()) == Some(Type::U8);
    if kind != Type::Vector || !is_u8 {
        bail!("External verifier {label} must be vector<u8>");
    }
    Ok(())
}

fn require_struct(
    signature: &sui::grpc::OpenSignature,
    package: sui::types::Address,
    module: &str,
    name: &str,
    label: &str,
) -> anyhow::Result<()> {
    let tag = signature_body_to_type_tag(
        signature
            .body_opt()
            .ok_or_else(|| anyhow!("External verifier {label} has no type"))?,
    )?;
    let sui::types::TypeTag::Struct(tag) = tag else {
        bail!("External verifier {label} has the wrong type");
    };
    if *tag.address() != package
        || tag.module().as_str() != module
        || tag.name().as_str() != name
        || !tag.type_params().is_empty()
    {
        bail!("External verifier {label} has the wrong type");
    }
    Ok(())
}

fn signature_body_to_type_tag(
    body: &sui::grpc::OpenSignatureBody,
) -> anyhow::Result<sui::types::TypeTag> {
    use sui::grpc::open_signature_body::Type;

    let kind = body
        .r#type
        .and_then(|kind| Type::try_from(kind).ok())
        .unwrap_or(Type::Unknown);
    Ok(match kind {
        Type::Address => sui::types::TypeTag::Address,
        Type::Bool => sui::types::TypeTag::Bool,
        Type::U8 => sui::types::TypeTag::U8,
        Type::U16 => sui::types::TypeTag::U16,
        Type::U32 => sui::types::TypeTag::U32,
        Type::U64 => sui::types::TypeTag::U64,
        Type::U128 => sui::types::TypeTag::U128,
        Type::U256 => sui::types::TypeTag::U256,
        Type::Vector => {
            let [inner] = body.type_parameter_instantiation.as_slice() else {
                bail!("Move vector type must have exactly one element type");
            };
            sui::types::TypeTag::Vector(Box::new(signature_body_to_type_tag(inner)?))
        }
        Type::Datatype => {
            let base = body
                .type_name_opt()
                .ok_or_else(|| anyhow!("Move datatype is missing its type name"))?
                .parse::<sui::types::StructTag>()
                .map_err(|e| anyhow!("Invalid Move datatype: {e}"))?;
            let type_params = body
                .type_parameter_instantiation
                .iter()
                .map(signature_body_to_type_tag)
                .collect::<anyhow::Result<Vec<_>>>()?;
            sui::types::TypeTag::Struct(Box::new(sui::types::StructTag::new(
                *base.address(),
                base.module().clone(),
                base.name().clone(),
                type_params,
            )))
        }
        Type::Parameter => bail!("External verifier object types must be concrete"),
        _ => bail!("Unsupported Move signature type in External verifier ABI"),
    })
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::test_utils::sui_mocks,
        std::sync::Arc,
        sui::grpc::{
            function_descriptor::Visibility,
            move_package_service_server::{MovePackageService, MovePackageServiceServer},
            open_signature::Reference,
            open_signature_body::Type,
        },
    };

    fn sample_leader_registry_bytes(network: sui::types::Address) -> Vec<u8> {
        let object_id = sui::types::Address::generate(rand::thread_rng());
        bcs::to_bytes(&LeaderRegistryInnerV1::new_for_test(object_id, network)).unwrap()
    }

    #[test]
    fn stake_weighted_ranking_matches_the_move_hash_vector() {
        let work_type = TypeName::new(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa::era::WorkAdmissionV1",
        );
        let seed = b"fixed-seed";

        assert_eq!(
            stake_weighted_u64(&work_type, seed, 0).unwrap(),
            8_542_078_142_993_533_861
        );
        assert_eq!(
            stake_weighted_u64(&work_type, seed, 1).unwrap(),
            6_899_840_498_555_412_980
        );

        let leader_a = sui::types::Address::from_static("0xa");
        let leader_b = sui::types::Address::from_static("0xb");
        let leader_c = sui::types::Address::from_static("0xc");
        let ranked = rank_stake_weighted(
            vec![(leader_a, 10), (leader_b, 20), (leader_c, 70)],
            &work_type,
            seed,
        )
        .unwrap();

        assert_eq!(ranked, [leader_c, leader_b]);
    }

    #[test]
    fn stake_weighted_ranking_matches_move_edge_cases() {
        let work_type = TypeName::new("a::m::W");
        let leader = sui::types::Address::from_static("0xa");

        assert_eq!(
            rank_stake_weighted(vec![(leader, 1)], &work_type, b"seed").unwrap(),
            [leader, leader]
        );
        assert!(rank_stake_weighted(Vec::new(), &work_type, b"seed").is_err());
        assert!(rank_stake_weighted(
            vec![
                (sui::types::Address::from_static("0xa"), 0),
                (sui::types::Address::from_static("0xb"), 0),
            ],
            &work_type,
            b"seed",
        )
        .is_err());
    }

    fn owned_capability_object(
        object_ref: sui::types::ObjectReference,
        owner: sui::types::Address,
        what_for: sui::types::Address,
        consensus_owner: bool,
    ) -> sui::grpc::Object {
        let mut object = sui::grpc::Object::default();
        object.set_object_id(object_ref.object_id().to_string());
        object.set_digest(*object_ref.digest());
        object.set_version(object_ref.version());
        let cap = AnyCloneableOwnerCap {
            id: crate::move_bindings::sui_framework::object::UID::new(*object_ref.object_id()),
            what_for: crate::move_bindings::sui_framework::object::ID::new(what_for),
            inner: primitives::owner_cap::OwnerCap {
                unique: crate::move_bindings::sui_framework::object::ID::new(
                    sui::types::Address::ZERO,
                ),
                phantom_t0: std::marker::PhantomData,
            },
            phantom_t0: std::marker::PhantomData,
        };
        let mut contents = sui::grpc::Bcs::default();
        contents.set_value(bcs::to_bytes(&cap).expect("owner cap bcs"));
        object.contents = Some(contents);

        if consensus_owner {
            let mut grpc_owner = sui::grpc::Owner::default();
            grpc_owner.kind = Some(OwnerKind::ConsensusAddress as i32);
            grpc_owner.address = Some(owner.into());
            grpc_owner.version = Some(object_ref.version());
            object.owner = Some(grpc_owner);
        } else {
            object.set_owner(sui::types::Owner::Address(owner));
        }

        object
    }

    #[tokio::test]
    async fn finds_matching_owned_capability_from_plain_owner() {
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        let owner = sui_mocks::mock_sui_address();
        let expected_what_for = sui_mocks::mock_sui_address();
        let first_ref = sui_mocks::mock_sui_object_ref();
        let second_ref = sui_mocks::mock_sui_object_ref();
        let first_ref_for_rpc = first_ref.clone();
        let second_ref_for_rpc = second_ref.clone();

        state_service
            .expect_list_owned_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(vec![
                    owned_capability_object(
                        first_ref_for_rpc.clone(),
                        owner,
                        sui_mocks::mock_sui_address(),
                        false,
                    ),
                    owned_capability_object(
                        second_ref_for_rpc.clone(),
                        owner,
                        expected_what_for,
                        false,
                    ),
                ]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service),
            ..Default::default()
        });

        let found = find_owned_capability_by_what_for(
            &rpc_url,
            owner,
            sui::types::StructTag::gas_coin(),
            expected_what_for,
        )
        .await
        .unwrap();

        assert_eq!(found, Some(second_ref));
    }

    #[tokio::test]
    async fn prefers_consensus_owner_version_when_present() {
        let mut state_service = sui_mocks::grpc::MockStateService::new();
        let owner = sui_mocks::mock_sui_address();
        let expected_what_for = sui_mocks::mock_sui_address();
        let object_ref = sui::types::ObjectReference::new(
            sui_mocks::mock_sui_address(),
            99,
            sui::types::Digest::generate(rand::thread_rng()),
        );
        let object_ref_for_rpc = object_ref.clone();

        state_service
            .expect_list_owned_objects()
            .times(1)
            .returning(move |_request| {
                let mut response = sui::grpc::ListOwnedObjectsResponse::default();
                response.set_objects(vec![owned_capability_object(
                    object_ref_for_rpc.clone(),
                    owner,
                    expected_what_for,
                    true,
                )]);
                Ok(tonic::Response::new(response))
            });

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            state_service_mock: Some(state_service),
            ..Default::default()
        });

        let found = find_owned_capability_by_what_for(
            &rpc_url,
            owner,
            sui::types::StructTag::gas_coin(),
            expected_what_for,
        )
        .await
        .unwrap()
        .expect("capability should be found");

        assert_eq!(found.version(), object_ref.version());
    }

    #[test]
    fn object_version_uses_consensus_owner_version() {
        let owner = sui_mocks::mock_sui_address();
        let object_ref = sui_mocks::mock_sui_object_ref();
        let object = owned_capability_object(
            object_ref.clone(),
            owner,
            sui_mocks::mock_sui_address(),
            true,
        );
        assert_eq!(object_version(&object), Some(object_ref.version()));
    }

    #[test]
    fn object_version_uses_object_version_for_address_owner() {
        let owner = sui_mocks::mock_sui_address();
        let object_ref = sui_mocks::mock_sui_object_ref();
        let object = owned_capability_object(
            object_ref.clone(),
            owner,
            sui_mocks::mock_sui_address(),
            false,
        );
        assert_eq!(object_version(&object), Some(object_ref.version()));
    }

    #[test]
    fn extracts_network_id_from_leader_registry_object_contents() {
        let network = sui::types::Address::generate(rand::thread_rng());
        let state: LeaderRegistryInnerV1 =
            bcs::from_bytes(&sample_leader_registry_bytes(network)).unwrap();

        let decoded = extract_network_id_from_leader_registry(&state);

        assert_eq!(decoded, network);
    }

    #[tokio::test]
    async fn external_preflight_rejects_invalid_object_lists_before_package_lookup() {
        let context = sui_mocks::mock_nexus_context();
        let rpc_url = sui_mocks::grpc::mock_server(Default::default());
        let crawler = Crawler::new(Arc::new(sui::grpc::Client::new(rpc_url).unwrap()));
        let package = sui::types::Address::from_static("0x401");
        let object = sui::types::Address::from_static("0x402");

        let missing_witness = preflight_external_verifier_registration(
            &crawler,
            &context,
            package,
            "verifier",
            "verify",
            &[],
        )
        .await
        .unwrap_err();
        assert!(missing_witness
            .to_string()
            .contains("witness as object zero"));

        let zero = preflight_external_verifier_registration(
            &crawler,
            &context,
            package,
            "verifier",
            "verify",
            &[sui::types::Address::ZERO],
        )
        .await
        .unwrap_err();
        assert!(zero.to_string().contains("must not be zero"));

        let duplicate = preflight_external_verifier_registration(
            &crawler,
            &context,
            package,
            "verifier",
            "verify",
            &[object, object],
        )
        .await
        .unwrap_err();
        assert!(duplicate.to_string().contains("must be unique"));
    }

    struct PackageServiceFixture {
        package: sui::grpc::Package,
    }

    #[tonic::async_trait]
    impl MovePackageService for PackageServiceFixture {
        async fn get_package(
            &self,
            _request: tonic::Request<sui::grpc::GetPackageRequest>,
        ) -> Result<tonic::Response<sui::grpc::GetPackageResponse>, tonic::Status> {
            let mut response = sui::grpc::GetPackageResponse::default();
            response.package = Some(self.package.clone());
            Ok(tonic::Response::new(response))
        }
    }

    fn preflight_crawler(
        package: sui::grpc::Package,
        ledger_service: Option<sui_mocks::grpc::MockLedgerService>,
    ) -> Crawler {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let incoming = tokio_stream::wrappers::TcpListenerStream::new(
            tokio::net::TcpListener::from_std(listener).unwrap(),
        );
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(MovePackageServiceServer::new(PackageServiceFixture {
                    package,
                }))
                .add_optional_service(
                    ledger_service.map(sui::grpc::ledger_service_server::LedgerServiceServer::new),
                )
                .serve_with_incoming(incoming)
                .await
                .unwrap();
        });
        Crawler::new(Arc::new(
            sui::grpc::Client::new(format!("http://{address}")).unwrap(),
        ))
    }

    fn package_with_function(
        package_id: sui::types::Address,
        module_name: &str,
        function: sui::grpc::FunctionDescriptor,
    ) -> sui::grpc::Package {
        let mut module = sui::grpc::Module::default();
        module.name = Some(module_name.to_string());
        module.functions = vec![function];
        let mut package = sui::grpc::Package::default();
        package.storage_id = Some(package_id.to_string());
        package.modules = vec![module];
        package
    }

    #[tokio::test]
    async fn external_preflight_resolves_abi_and_preserves_shared_object_order() {
        let context = sui_mocks::mock_nexus_context();
        let package_id = sui::types::Address::from_static("0x411");
        let witness = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x412"));
        let config = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x413"));
        let mut function = valid_external_function(&context);
        function.parameters.push(
            datatype(sui::types::Address::from_static("0x42"), "state", "Config")
                .with_reference(Reference::Immutable),
        );
        let package = package_with_function(package_id, "verifier", function);
        let mut ledger_service = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_objects_metadata(
            &mut ledger_service,
            vec![
                (config.clone(), sui::types::Owner::Shared(5), None),
                (witness.clone(), sui::types::Owner::Shared(4), None),
            ],
        );
        let crawler = preflight_crawler(package, Some(ledger_service));

        let registration = preflight_external_verifier_registration(
            &crawler,
            &context,
            package_id,
            "verifier",
            "verify",
            &[*witness.object_id(), *config.object_id()],
        )
        .await
        .unwrap();

        assert_eq!(registration.package_id, package_id);
        assert_eq!(registration.module_name, "verifier");
        assert_eq!(registration.function_name, "verify");
        assert_eq!(
            registration
                .verifier_objects
                .iter()
                .map(|object| *object.object_ref.object_id())
                .collect::<Vec<_>>(),
            vec![*witness.object_id(), *config.object_id()]
        );
    }

    #[tokio::test]
    async fn external_preflight_reports_missing_module_function_and_object_count() {
        let context = sui_mocks::mock_nexus_context();
        let package_id = sui::types::Address::from_static("0x421");
        let witness = sui::types::Address::from_static("0x422");

        let missing_module = preflight_external_verifier_registration(
            &preflight_crawler(sui::grpc::Package::default(), None),
            &context,
            package_id,
            "verifier",
            "verify",
            &[witness],
        )
        .await
        .unwrap_err();
        assert!(missing_module
            .to_string()
            .contains("Module 'verifier' not found"));

        let package =
            package_with_function(package_id, "verifier", valid_external_function(&context));
        let missing_function = preflight_external_verifier_registration(
            &preflight_crawler(package.clone(), None),
            &context,
            package_id,
            "verifier",
            "other",
            &[witness],
        )
        .await
        .unwrap_err();
        assert!(missing_function
            .to_string()
            .contains("Function 'other' not found"));

        let wrong_count = preflight_external_verifier_registration(
            &preflight_crawler(package, None),
            &context,
            package_id,
            "verifier",
            "verify",
            &[witness, sui::types::Address::from_static("0x423")],
        )
        .await
        .unwrap_err();
        assert!(wrong_count
            .to_string()
            .contains("requires 1 immutable shared objects"));
    }

    fn datatype(
        package: sui::types::Address,
        module: &str,
        name: &str,
    ) -> sui::grpc::OpenSignature {
        sui::grpc::OpenSignature::default().with_body(
            sui::grpc::OpenSignatureBody::default()
                .with_type(Type::Datatype)
                .with_type_name(format!("{package}::{module}::{name}")),
        )
    }

    fn bytes() -> sui::grpc::OpenSignature {
        sui::grpc::OpenSignature::default().with_body(
            sui::grpc::OpenSignatureBody::default()
                .with_type(Type::Vector)
                .with_type_parameter_instantiation(vec![
                    sui::grpc::OpenSignatureBody::default().with_type(Type::U8)
                ]),
        )
    }

    fn valid_external_function(context: &NexusContext) -> sui::grpc::FunctionDescriptor {
        let primitives = context
            .require_package(PackageRole::Primitives)
            .unwrap()
            .initial_id;
        let interface = context
            .require_package(PackageRole::Interface)
            .unwrap()
            .initial_id;
        let worksheet =
            datatype(primitives, "proof_of_uid", "ProofOfUID").with_reference(Reference::Mutable);
        let witness = datatype(sui::types::Address::from_static("0x42"), "state", "Witness")
            .with_reference(Reference::Immutable);
        let verdict = datatype(interface, "verifier", "VerificationVerdict");
        let result = datatype(primitives, "tagged_output", "TaggedOutput");
        sui::grpc::FunctionDescriptor::default()
            .with_name("verify")
            .with_visibility(Visibility::Public)
            .with_parameters(vec![worksheet, result, bytes(), witness])
            .with_returns(vec![verdict])
    }

    #[test]
    fn external_verifier_abi_derives_ordered_object_type_tags() {
        let context = sui_mocks::mock_nexus_context();
        let object_types =
            validate_external_verifier_function(&valid_external_function(&context), &context)
                .unwrap();
        assert_eq!(object_types.len(), 1);
        let sui::types::TypeTag::Struct(witness) = &object_types[0] else {
            panic!("witness must be a struct type");
        };
        assert_eq!(*witness.address(), sui::types::Address::from_static("0x42"));
        assert_eq!(witness.module().as_str(), "state");
        assert_eq!(witness.name().as_str(), "Witness");
    }

    #[test]
    fn external_verifier_abi_rejects_non_public_or_generic_functions() {
        let context = sui_mocks::mock_nexus_context();
        let private = valid_external_function(&context).with_visibility(Visibility::Private);
        assert!(validate_external_verifier_function(&private, &context)
            .unwrap_err()
            .to_string()
            .contains("must be public"));

        let generic = valid_external_function(&context)
            .with_type_parameters(vec![sui::grpc::TypeParameter::default()]);
        assert!(validate_external_verifier_function(&generic, &context)
            .unwrap_err()
            .to_string()
            .contains("must not declare type parameters"));
    }

    #[test]
    fn external_verifier_abi_rejects_mutable_objects_and_wrong_return() {
        let context = sui_mocks::mock_nexus_context();
        let mut mutable_object = valid_external_function(&context);
        mutable_object.parameters[3] = mutable_object.parameters[3]
            .clone()
            .with_reference(Reference::Mutable);
        assert!(
            validate_external_verifier_function(&mutable_object, &context)
                .unwrap_err()
                .to_string()
                .contains("wrong reference kind")
        );

        let wrong_return = valid_external_function(&context).with_returns(vec![bytes()]);
        assert!(validate_external_verifier_function(&wrong_return, &context)
            .unwrap_err()
            .to_string()
            .contains("wrong type"));
    }

    #[test]
    fn external_verifier_abi_rejects_invalid_fixed_parameters() {
        let context = sui_mocks::mock_nexus_context();

        let too_short = valid_external_function(&context).with_parameters(vec![bytes(); 3]);
        assert!(validate_external_verifier_function(&too_short, &context)
            .unwrap_err()
            .to_string()
            .contains("at least one witness object"));

        let mut wrong_worksheet_reference = valid_external_function(&context);
        wrong_worksheet_reference.parameters[0].reference = Some(Reference::Unknown as i32);
        assert!(
            validate_external_verifier_function(&wrong_worksheet_reference, &context)
                .unwrap_err()
                .to_string()
                .contains("worksheet has the wrong reference kind")
        );

        let mut wrong_worksheet_type = valid_external_function(&context);
        wrong_worksheet_type.parameters[0] = datatype(
            context
                .require_package(PackageRole::Primitives)
                .unwrap()
                .initial_id,
            "proof_of_uid",
            "Other",
        )
        .with_reference(Reference::Mutable);
        assert!(
            validate_external_verifier_function(&wrong_worksheet_type, &context)
                .unwrap_err()
                .to_string()
                .contains("worksheet has the wrong type")
        );

        let mut wrong_result = valid_external_function(&context);
        wrong_result.parameters[1] = sui::grpc::OpenSignature::default()
            .with_body(sui::grpc::OpenSignatureBody::default().with_type(Type::U64));
        assert!(validate_external_verifier_function(&wrong_result, &context)
            .unwrap_err()
            .to_string()
            .contains("result has the wrong type"));

        let mut missing_auxiliary_type = valid_external_function(&context);
        missing_auxiliary_type.parameters[2] = sui::grpc::OpenSignature::default();
        assert!(
            validate_external_verifier_function(&missing_auxiliary_type, &context)
                .unwrap_err()
                .to_string()
                .contains("auxiliary has no type")
        );
    }

    #[test]
    fn external_verifier_abi_rejects_invalid_object_and_return_shapes() {
        let context = sui_mocks::mock_nexus_context();

        let mut missing_object_type = valid_external_function(&context);
        missing_object_type.parameters[3] =
            sui::grpc::OpenSignature::default().with_reference(Reference::Immutable);
        assert!(
            validate_external_verifier_function(&missing_object_type, &context)
                .unwrap_err()
                .to_string()
                .contains("object 0 has no type")
        );

        let mut primitive_object = valid_external_function(&context);
        primitive_object.parameters[3] = sui::grpc::OpenSignature::default()
            .with_reference(Reference::Immutable)
            .with_body(sui::grpc::OpenSignatureBody::default().with_type(Type::U64));
        assert!(
            validate_external_verifier_function(&primitive_object, &context)
                .unwrap_err()
                .to_string()
                .contains("must have a concrete object type")
        );

        let no_return = valid_external_function(&context).with_returns(vec![]);
        assert!(validate_external_verifier_function(&no_return, &context)
            .unwrap_err()
            .to_string()
            .contains("return exactly one"));

        let two_returns = valid_external_function(&context).with_returns(vec![bytes(), bytes()]);
        assert!(validate_external_verifier_function(&two_returns, &context)
            .unwrap_err()
            .to_string()
            .contains("return exactly one"));

        let mut referenced_return = valid_external_function(&context);
        referenced_return.returns[0].reference = Some(Reference::Immutable as i32);
        assert!(
            validate_external_verifier_function(&referenced_return, &context)
                .unwrap_err()
                .to_string()
                .contains("return value has the wrong reference kind")
        );
    }

    #[test]
    fn signature_body_decoder_covers_supported_and_rejected_shapes() {
        for (kind, expected) in [
            (Type::Address, sui::types::TypeTag::Address),
            (Type::Bool, sui::types::TypeTag::Bool),
            (Type::U8, sui::types::TypeTag::U8),
            (Type::U16, sui::types::TypeTag::U16),
            (Type::U32, sui::types::TypeTag::U32),
            (Type::U64, sui::types::TypeTag::U64),
            (Type::U128, sui::types::TypeTag::U128),
            (Type::U256, sui::types::TypeTag::U256),
        ] {
            let body = sui::grpc::OpenSignatureBody::default().with_type(kind);
            assert_eq!(signature_body_to_type_tag(&body).unwrap(), expected);
        }

        let vector = sui::grpc::OpenSignatureBody::default()
            .with_type(Type::Vector)
            .with_type_parameter_instantiation(vec![
                sui::grpc::OpenSignatureBody::default().with_type(Type::U16)
            ]);
        assert_eq!(
            signature_body_to_type_tag(&vector).unwrap(),
            sui::types::TypeTag::Vector(Box::new(sui::types::TypeTag::U16))
        );

        let malformed_vector = sui::grpc::OpenSignatureBody::default().with_type(Type::Vector);
        assert!(signature_body_to_type_tag(&malformed_vector)
            .unwrap_err()
            .to_string()
            .contains("exactly one element type"));

        let concrete = sui::grpc::OpenSignatureBody::default()
            .with_type(Type::Datatype)
            .with_type_name("0x42::state::Box")
            .with_type_parameter_instantiation(vec![
                sui::grpc::OpenSignatureBody::default().with_type(Type::U8)
            ]);
        let sui::types::TypeTag::Struct(tag) = signature_body_to_type_tag(&concrete).unwrap()
        else {
            panic!("datatype must decode to a struct tag");
        };
        assert_eq!(tag.name().as_str(), "Box");
        assert_eq!(tag.type_params(), &[sui::types::TypeTag::U8]);

        let missing_name = sui::grpc::OpenSignatureBody::default().with_type(Type::Datatype);
        assert!(signature_body_to_type_tag(&missing_name)
            .unwrap_err()
            .to_string()
            .contains("missing its type name"));

        let parameter = sui::grpc::OpenSignatureBody::default().with_type(Type::Parameter);
        assert!(signature_body_to_type_tag(&parameter)
            .unwrap_err()
            .to_string()
            .contains("must be concrete"));

        assert!(
            signature_body_to_type_tag(&sui::grpc::OpenSignatureBody::default())
                .unwrap_err()
                .to_string()
                .contains("Unsupported Move signature type")
        );
    }
}
