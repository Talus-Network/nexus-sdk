//! Read-only helpers for registry-owned capability discovery.

use {
    crate::{
        nexus::crawler::{prost_value_to_json_value, Crawler},
        sui::{self, grpc::owner::OwnerKind, traits::FieldMaskUtil},
        types::{
            ExternalVerifierRuntimeCallV1,
            MoveTable,
            SharedObjectRef,
            TypeName,
            VerifierConfig,
        },
    },
    anyhow::{anyhow, bail},
    serde::Deserialize,
    std::{collections::HashMap, str::FromStr},
};

#[derive(serde::Deserialize)]
struct OwnerCapJson {
    what_for: sui::types::Address,
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
            "json",
            "owner",
        ]));

    let mut client = sui::grpc::Client::new(rpc_url)?;
    let response = client
        .state_client()
        .list_owned_objects(request)
        .await?
        .into_inner();

    Ok(response.objects().iter().find_map(|object| {
        let object_id = object
            .object_id_opt()
            .and_then(|value| value.parse().ok())?;
        let digest = object.digest_opt().and_then(|value| value.parse().ok())?;
        let version = object_version(object)?;

        let json = object.json_opt()?;
        let json = prost_value_to_json_value(json).ok()?;
        let parsed = serde_json::from_value::<OwnerCapJson>(json).ok()?;
        if parsed.what_for != expected_what_for {
            return None;
        }

        Some(sui::types::ObjectReference::new(object_id, version, digest))
    }))
}

fn object_version(object: &sui::grpc::Object) -> Option<u64> {
    let is_consensus = object
        .owner_opt()
        .and_then(|owner| owner.kind)
        .and_then(|kind| OwnerKind::try_from(kind).ok())
        .map(|kind| kind == OwnerKind::ConsensusAddress)
        .unwrap_or(false);

    if is_consensus {
        object.owner_opt().and_then(|owner| owner.version_opt())
    } else {
        object.version_opt()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalVerifierRuntimeMetadata {
    pub verifier: VerifierConfig,
    pub package_address: sui::types::Address,
    pub witness: sui::types::ObjectReference,
    pub shared_objects: Vec<(SharedObjectRef, sui::types::ObjectReference)>,
    pub call_shape: VerifierCallShapeV1,
}

impl ExternalVerifierRuntimeMetadata {
    pub fn runtime_call(&self) -> ExternalVerifierRuntimeCallV1 {
        ExternalVerifierRuntimeCallV1 {
            package_address: self.package_address,
            module_name: self.call_shape.module_name.clone(),
            function_name: self.call_shape.function_name.clone(),
            witness: self.witness.clone(),
            shared_objects: self.shared_objects.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct VerifierCallShapeV1 {
    pub module_name: String,
    pub function_name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct VerifierRegistryBcs {
    #[allow(dead_code)]
    id: sui::types::Address,
    methods: MoveTable<String, VerifierMethodRecordBcs>,
}

#[derive(Clone, Debug, Deserialize)]
enum VerifierImplementationBcs {
    BuiltIn,
    ExternalV1 {
        witness: sui::types::Address,
        #[allow(dead_code)]
        interface_version: InterfaceVersionBcs,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct InterfaceVersionBcs {
    #[allow(dead_code)]
    inner: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct VerifierMethodRecordBcs {
    #[allow(dead_code)]
    method: String,
    implementation: VerifierImplementationBcs,
    shared_objects: Vec<SharedObjectRef>,
    capabilities: VerifierMethodCapabilitiesBcs,
    call_shape: Option<VerifierCallShapeV1>,
    #[allow(dead_code)]
    credential_identity: Option<VerifierCredentialIdentityBcs>,
    witness_type: Option<TypeName>,
}

#[derive(Clone, Debug, Deserialize)]
struct VerifierMethodCapabilitiesBcs {
    supports_success: bool,
    supports_err_eval: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct VerifierCredentialIdentityBcs {
    #[allow(dead_code)]
    kind: String,
    #[allow(dead_code)]
    version: u64,
}

/// Fetch external verifier runtime metadata from the verifier registry.
///
/// This keeps verifier-registry BCS layout knowledge in the SDK. Callers get only the
/// object refs and call shape needed to build a verifier dry-run transaction.
pub async fn fetch_external_verifier_runtime_metadata(
    crawler: &Crawler,
    registry_ref: &sui::types::ObjectReference,
    verifier: VerifierConfig,
) -> anyhow::Result<ExternalVerifierRuntimeMetadata> {
    let registry = crawler
        .get_object_contents_bcs::<VerifierRegistryBcs>(*registry_ref.object_id())
        .await?;
    let mut methods = crawler
        .get_dynamic_fields_bcs::<String, VerifierMethodRecordBcs>(
            registry.data.methods.id,
            registry.data.methods.size(),
        )
        .await?;
    let record = methods.remove(&verifier.method).ok_or_else(|| {
        anyhow!(
            "Verifier method '{}' is not registered in the verifier registry",
            verifier.method
        )
    })?;

    if !record.capabilities.supports_success && !record.capabilities.supports_err_eval {
        bail!(
            "Verifier method '{}' has no supported submission modes",
            verifier.method
        );
    }

    let call_shape = record.call_shape.ok_or_else(|| {
        anyhow!(
            "Verifier method '{}' is missing runtime call-shape metadata",
            verifier.method
        )
    })?;
    let witness_type = record.witness_type.ok_or_else(|| {
        anyhow!(
            "Verifier method '{}' is missing witness type metadata",
            verifier.method
        )
    })?;
    let witness_type_name = if witness_type.name.starts_with("0x") {
        witness_type.name.clone()
    } else {
        format!("0x{}", witness_type.name)
    };
    let witness_tag = sui::types::StructTag::from_str(&witness_type_name).map_err(|source| {
        anyhow!(
            "failed to parse verifier witness type '{witness_type}' as a Move struct tag: {source}"
        )
    })?;
    if record.shared_objects.iter().any(|shared| shared.ref_mut) {
        bail!(
            "Verifier method '{}' uses mutable shared objects, which are unsupported",
            verifier.method
        );
    }

    let witness_id = match record.implementation {
        VerifierImplementationBcs::ExternalV1 { witness, .. } => witness,
        VerifierImplementationBcs::BuiltIn => {
            bail!(
                "Verifier method '{}' is not an external verifier contract",
                verifier.method
            )
        }
    };

    let object_ids = std::iter::once(witness_id)
        .chain(record.shared_objects.iter().map(|shared| shared.id))
        .collect::<Vec<_>>();
    let metadata = crawler.get_objects_metadata(&object_ids).await?;
    let mut metadata_by_id = metadata
        .into_iter()
        .map(|response| (response.object_id, response.object_ref()))
        .collect::<HashMap<_, _>>();
    let witness = metadata_by_id.remove(&witness_id).ok_or_else(|| {
        anyhow!("Verifier witness object '{witness_id}' metadata was not returned by the crawler")
    })?;
    let shared_objects = record
        .shared_objects
        .into_iter()
        .map(|shared| {
            let object_ref = metadata_by_id.remove(&shared.id).ok_or_else(|| {
                anyhow!(
                    "Verifier shared object '{}' metadata was not returned by the crawler",
                    shared.id
                )
            })?;
            Ok((shared, object_ref))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(ExternalVerifierRuntimeMetadata {
        verifier,
        package_address: *witness_tag.address(),
        witness,
        shared_objects,
        call_shape,
    })
}

#[cfg(test)]
mod tests {
    use {super::*, crate::test_utils::sui_mocks};

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
        object.json = Some(Box::new(prost_types::Value {
            kind: Some(prost_types::value::Kind::StructValue(prost_types::Struct {
                fields: std::collections::BTreeMap::from([(
                    "what_for".to_string(),
                    prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue(what_for.to_string())),
                    },
                )]),
            })),
        }));

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
}
