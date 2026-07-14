//! Read-only helpers for registry-owned capability and verifier discovery.

use {
    crate::{
        move_bindings::{
            interface::verifier::ToolVerifierSupport,
            primitives,
            registry::{
                self,
                leader::LeaderRegistry,
                tool_registry::ToolRegistry,
                verifier_registry::{ExternalVerifierRecord, VerifierRegistry},
            },
            sui_framework::object::ID,
        },
        nexus::crawler::Crawler,
        sui::{self, grpc::owner::OwnerKind, traits::FieldMaskUtil},
        transactions::tool::{ExternalVerifierObjectInput, ExternalVerifierRegistrationInput},
        types::{ExternalVerifierRuntimeCall, NexusObjects},
    },
    anyhow::{anyhow, bail},
    std::collections::HashMap,
};

type AnyCloneableOwnerCap =
    primitives::owner_cap::CloneableOwnerCap<registry::leader_cap::OverNetwork>;

/// Decode the registry network ID from a published `LeaderRegistry` Move object.
pub fn extract_network_id_from_leader_registry(
    leader_registry_object: &sui::types::Object,
) -> anyhow::Result<sui::types::Address> {
    LeaderRegistry::from_object(leader_registry_object).map(|registry| registry.network_id())
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

    let mut client = sui::grpc::Client::new(rpc_url)?;
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

/// Current live registration for one stable Tool ID.
///
/// Outer `None` means the Tool ID is absent from the authoritative
/// `ToolRegistry.registered_tools` table. A present registration may still have no verifier
/// support because per-vertex `None` requires no global verifier configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentToolRegistration {
    pub verifier_support: Option<ToolVerifierSupport>,
}

pub async fn fetch_current_tool_registration(
    crawler: &Crawler,
    registry_ref: &sui::types::ObjectReference,
    tool_id: sui::types::Address,
) -> anyhow::Result<Option<CurrentToolRegistration>> {
    let registry = crawler
        .get_object::<ToolRegistry>(*registry_ref.object_id())
        .await?;

    if registry.data.registered_tools.size() == 0 {
        return Ok(None);
    }

    let tool_id = ID::new(tool_id);
    let is_registered = crawler
        .get_optional_dynamic_field::<ID, bool>(registry.data.registered_tools.id(), tool_id)
        .await?
        .is_some();
    if !is_registered {
        return Ok(None);
    }

    let verifier_support = if registry.data.verifier_support.size() == 0 {
        None
    } else {
        crawler
            .get_optional_dynamic_field::<ID, ToolVerifierSupport>(
                registry.data.verifier_support.id(),
                tool_id,
            )
            .await?
    };

    Ok(Some(CurrentToolRegistration { verifier_support }))
}

/// Current Tool-bound external verifier record, when one exists.
pub async fn fetch_external_verifier_record(
    crawler: &Crawler,
    registry_ref: &sui::types::ObjectReference,
    tool_id: sui::types::Address,
) -> anyhow::Result<Option<ExternalVerifierRecord>> {
    let registry = crawler
        .get_object::<VerifierRegistry>(*registry_ref.object_id())
        .await?;
    if registry.data.external_methods.size() == 0 {
        return Ok(None);
    }
    let record = crawler
        .get_dynamic_fields::<ID, ExternalVerifierRecord>(
            registry.data.external_methods.id(),
            registry.data.external_methods.size(),
        )
        .await?
        .into_iter()
        .find_map(|(id, record)| (id.bytes == tool_id).then_some(record));
    if let Some(record) = record.as_ref() {
        validate_external_record(tool_id, record)?;
    }
    Ok(record)
}

/// Resolve one Tool-bound external verifier record and its ordered immutable shared objects.
pub async fn fetch_external_verifier_runtime_call(
    crawler: &Crawler,
    registry_ref: &sui::types::ObjectReference,
    tool_id: sui::types::Address,
) -> anyhow::Result<ExternalVerifierRuntimeCall> {
    let record = fetch_external_verifier_record(crawler, registry_ref, tool_id)
        .await?
        .ok_or_else(|| anyhow!("Tool '{tool_id}' has no registered external verifier"))?;

    let object_ids = record
        .immutable_shared_objects
        .iter()
        .map(|id| id.bytes)
        .collect::<Vec<_>>();
    let metadata = crawler.get_objects_metadata(&object_ids).await?;
    let mut by_id = metadata
        .into_iter()
        .map(|object| (object.object_id, object))
        .collect::<HashMap<_, _>>();
    let immutable_shared_objects = object_ids
        .iter()
        .map(|object_id| {
            let object = by_id.remove(object_id).ok_or_else(|| {
                anyhow!("External verifier object '{object_id}' metadata was not returned")
            })?;
            if !object.is_shared() {
                bail!("External verifier object '{object_id}' is not shared");
            }
            Ok(object.object_ref())
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ExternalVerifierRuntimeCall {
        method_id: record.method_id.clone(),
        witness_id: record.witness_id.bytes,
        immutable_shared_objects,
    })
}

/// Validate a published External verifier ABI and resolve its ordered shared objects.
pub async fn preflight_external_verifier_registration(
    crawler: &Crawler,
    objects: &NexusObjects,
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
    let object_types = validate_external_verifier_function(function, objects)?;
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
    objects: &NexusObjects,
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

    let worksheet = &function.parameters()[0];
    require_reference(worksheet, Reference::Mutable, "worksheet")?;
    require_struct(
        worksheet,
        objects.primitives_pkg_id,
        "proof_of_uid",
        "ProofOfUID",
        "worksheet",
    )?;
    require_bytes(&function.parameters()[1], "result")?;
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
        objects.interface_pkg_id,
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

fn validate_external_record(
    tool_id: sui::types::Address,
    record: &ExternalVerifierRecord,
) -> anyhow::Result<()> {
    if record.method_id.tool_id.bytes != tool_id {
        bail!("External verifier method is bound to a different Tool ID");
    }
    let first = record
        .immutable_shared_objects
        .first()
        .ok_or_else(|| anyhow!("External verifier record has no witness object"))?;
    if first.bytes != record.witness_id.bytes {
        bail!("External verifier witness must be immutable object zero");
    }
    if record
        .immutable_shared_objects
        .iter()
        .map(|id| id.bytes)
        .collect::<std::collections::HashSet<_>>()
        .len()
        != record.immutable_shared_objects.len()
    {
        bail!("External verifier record contains duplicate objects");
    }
    Ok(())
}

#[cfg(all(test, feature = "test_utils"))]
mod tests {
    use {
        super::*,
        crate::{
            move_bindings::{
                move_std::ascii,
                sui_framework::{linked_table::LinkedTable, object::UID, table::Table},
            },
            test_utils::sui_mocks,
        },
        std::sync::Arc,
        sui::grpc::{
            function_descriptor::Visibility,
            open_signature::Reference,
            open_signature_body::Type,
        },
        tokio::sync::Mutex,
    };

    fn sample_leader_registry_bytes(network: sui::types::Address) -> Vec<u8> {
        let object_id = sui::types::Address::generate(rand::thread_rng());
        bcs::to_bytes(&LeaderRegistry::new_for_test(object_id, network)).unwrap()
    }

    fn leader_registry_object(network: sui::types::Address) -> sui::types::Object {
        let contents = sample_leader_registry_bytes(network);
        let move_struct = sui::types::MoveStruct::new(
            sui::types::StructTag::new(
                sui::types::Address::from_static("0x1"),
                sui::types::Identifier::from_static("leader"),
                sui::types::Identifier::from_static("LeaderRegistry"),
                vec![],
            ),
            true,
            0,
            contents,
        )
        .expect("leader registry contents should include an object id");

        sui::types::Object::new(
            sui::types::ObjectData::Struct(move_struct),
            sui::types::Owner::Address(sui::types::Address::ZERO),
            sui::types::Digest::generate(rand::thread_rng()),
            0,
        )
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
        let object = leader_registry_object(network);

        let decoded = extract_network_id_from_leader_registry(&object).unwrap();

        assert_eq!(decoded, network);
    }

    fn tool_registry_fixture(
        registry_id: sui::types::Address,
        registered_tools_id: sui::types::Address,
        registered_tools_size: u64,
    ) -> ToolRegistry {
        let id = sui::types::Address::from_static;
        ToolRegistry::new(
            UID::new(registry_id),
            LinkedTable::<ascii::String, ID>::new(id("0x101"), 0),
            Table::<ID, bool>::new(registered_tools_id, registered_tools_size),
            LinkedTable::<ascii::String, u64>::new(id("0x103"), 0),
            Table::<ID, ToolVerifierSupport>::new(id("0x104"), 0),
            LinkedTable::<ascii::String, ID>::new(id("0x105"), 0),
            LinkedTable::<ascii::String, bool>::new(id("0x106"), 0),
            0,
            0,
        )
    }

    async fn current_registration_fixture(registered: bool) -> Option<CurrentToolRegistration> {
        #[derive(Clone, serde::Serialize)]
        struct DynamicFieldFixture<K, V> {
            id: sui::types::Address,
            name: K,
            value: V,
        }

        let registry_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x201"));
        let registered_tools_id = sui::types::Address::from_static("0x202");
        let tool_id = sui::types::Address::from_static("0x203");
        let registry = tool_registry_fixture(
            *registry_ref.object_id(),
            registered_tools_id,
            u64::from(registered),
        );

        let mut ledger_service_mock = sui_mocks::grpc::MockLedgerService::new();
        sui_mocks::grpc::mock_get_object_bcs(
            &mut ledger_service_mock,
            registry_ref.clone(),
            sui::types::Owner::Shared(1),
            bcs::to_bytes(&registry).unwrap(),
        );

        let mut state_service_mock = sui_mocks::grpc::MockStateService::new();
        if registered {
            let field_ref = sui_mocks::object_ref_for_id(sui::types::Address::from_static("0x204"));
            let key = ID::new(tool_id);
            sui_mocks::grpc::mock_list_dynamic_fields(
                &mut state_service_mock,
                vec![(key, *field_ref.object_id())],
            );
            sui_mocks::grpc::mock_get_object_bcs(
                &mut ledger_service_mock,
                field_ref.clone(),
                sui::types::Owner::Shared(1),
                bcs::to_bytes(&DynamicFieldFixture {
                    id: *field_ref.object_id(),
                    name: key,
                    value: true,
                })
                .unwrap(),
            );
        }

        let rpc_url = sui_mocks::grpc::mock_server(sui_mocks::grpc::ServerMocks {
            ledger_service_mock: Some(ledger_service_mock),
            state_service_mock: Some(state_service_mock),
            ..Default::default()
        });
        let client = sui::grpc::Client::new(rpc_url).unwrap();
        let crawler = Crawler::new(Arc::new(Mutex::new(client)));
        fetch_current_tool_registration(&crawler, &registry_ref, tool_id)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn current_registration_distinguishes_live_none_mode_from_unregistered_retained_tool() {
        let live_none = current_registration_fixture(true)
            .await
            .expect("registered Tool must be present even without verifier support");
        assert_eq!(live_none.verifier_support, None);

        assert!(
            current_registration_fixture(false).await.is_none(),
            "retained Tool identity is not live membership after unregister"
        );
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

    fn valid_external_function(objects: &NexusObjects) -> sui::grpc::FunctionDescriptor {
        let worksheet = datatype(objects.primitives_pkg_id, "proof_of_uid", "ProofOfUID")
            .with_reference(Reference::Mutable);
        let witness = datatype(sui::types::Address::from_static("0x42"), "state", "Witness")
            .with_reference(Reference::Immutable);
        let verdict = datatype(objects.interface_pkg_id, "verifier", "VerificationVerdict");
        sui::grpc::FunctionDescriptor::default()
            .with_name("verify")
            .with_visibility(Visibility::Public)
            .with_parameters(vec![worksheet, bytes(), bytes(), witness])
            .with_returns(vec![verdict])
    }

    #[test]
    fn external_verifier_abi_derives_ordered_object_type_tags() {
        let objects = sui_mocks::mock_nexus_objects();
        let object_types =
            validate_external_verifier_function(&valid_external_function(&objects), &objects)
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
        let objects = sui_mocks::mock_nexus_objects();
        let private = valid_external_function(&objects).with_visibility(Visibility::Private);
        assert!(validate_external_verifier_function(&private, &objects)
            .unwrap_err()
            .to_string()
            .contains("must be public"));

        let generic = valid_external_function(&objects)
            .with_type_parameters(vec![sui::grpc::TypeParameter::default()]);
        assert!(validate_external_verifier_function(&generic, &objects)
            .unwrap_err()
            .to_string()
            .contains("must not declare type parameters"));
    }

    #[test]
    fn external_verifier_abi_rejects_mutable_objects_and_wrong_return() {
        let objects = sui_mocks::mock_nexus_objects();
        let mut mutable_object = valid_external_function(&objects);
        mutable_object.parameters[3] = mutable_object.parameters[3]
            .clone()
            .with_reference(Reference::Mutable);
        assert!(
            validate_external_verifier_function(&mutable_object, &objects)
                .unwrap_err()
                .to_string()
                .contains("wrong reference kind")
        );

        let wrong_return = valid_external_function(&objects).with_returns(vec![bytes()]);
        assert!(validate_external_verifier_function(&wrong_return, &objects)
            .unwrap_err()
            .to_string()
            .contains("wrong type"));
    }
}
