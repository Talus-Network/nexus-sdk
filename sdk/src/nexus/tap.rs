//! Read-only helpers for standard TAP endpoint recovery.

use crate::{
    nexus::crawler::{Crawler, Response},
    sui,
    types::{
        resolve_active_tap_endpoint,
        resolve_active_tap_skill_execution_target,
        AgentId,
        InterfaceRevision,
        NexusObjects,
        SkillId,
        TapActiveSkillExecutionTarget,
        TapEndpointKey,
        TapEndpointRecord,
        TapEndpointResolutionError,
        TapRegistry,
    },
};

/// Fetch the shared standard TAP registry object from chain storage.
pub async fn fetch_tap_registry(
    crawler: &Crawler,
    registry_id: sui::types::Address,
) -> anyhow::Result<Response<TapRegistry>> {
    crawler
        .get_object_contents_bcs::<TapRegistry>(registry_id)
        .await
}

/// Fetch a pinned TAP endpoint from the real `TapRegistry` vector layout.
pub async fn fetch_tap_endpoint(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
    interface_revision: InterfaceRevision,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let registry = fetch_tap_registry(crawler, registry_id).await?;
    let record = registry.data.endpoint_record(TapEndpointKey {
        agent_id,
        skill_id,
        interface_revision,
    })?;

    Ok(registry_response_with_data(registry, record))
}

/// Resolve a fresh execution endpoint through `TapRegistry.active_endpoints`.
pub async fn fetch_active_tap_endpoint(
    crawler: &Crawler,
    registry_id: sui::types::Address,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let registry = fetch_tap_registry(crawler, registry_id).await?;
    let record = registry.data.active_endpoint_record(agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, record))
}

/// Return the configured TAP registry object ID, failing clearly for
/// deployments created before the standard registry was added to metadata.
pub fn configured_tap_registry_id(objects: &NexusObjects) -> anyhow::Result<sui::types::Address> {
    objects
        .tap_registry()
        .map(|registry| *registry.object_id())
        .ok_or_else(|| anyhow::anyhow!("NexusObjects missing tap_registry object reference"))
}

/// Fetch the shared TAP registry named by `NexusObjects`.
pub async fn fetch_configured_tap_registry(
    crawler: &Crawler,
    objects: &NexusObjects,
) -> anyhow::Result<Response<TapRegistry>> {
    fetch_tap_registry(crawler, configured_tap_registry_id(objects)?).await
}

/// Resolve a fresh execution endpoint through the configured TAP registry.
pub async fn fetch_configured_active_tap_endpoint(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    fetch_active_tap_endpoint(
        crawler,
        configured_tap_registry_id(objects)?,
        agent_id,
        skill_id,
    )
    .await
}

/// Resolve the active skill registration plus endpoint from the configured TAP registry.
pub async fn fetch_configured_active_tap_skill_execution_target(
    crawler: &Crawler,
    objects: &NexusObjects,
    agent_id: AgentId,
    skill_id: SkillId,
) -> anyhow::Result<Response<TapActiveSkillExecutionTarget>> {
    let registry = fetch_configured_tap_registry(crawler, objects).await?;
    let target = resolve_active_tap_skill_execution_target(&registry.data, agent_id, skill_id)?;

    Ok(registry_response_with_data(registry, target))
}

/// Resolve a fresh execution endpoint from already fetched records.
pub fn resolve_active_endpoint_record(
    records: &[TapEndpointRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&TapEndpointRecord, TapEndpointResolutionError> {
    resolve_active_tap_endpoint(records, agent_id, skill_id)
}

fn registry_response_with_data<T>(registry: Response<TapRegistry>, data: T) -> Response<T> {
    Response {
        object_id: registry.object_id,
        owner: registry.owner,
        version: registry.version,
        data,
        digest: registry.digest,
        balance: registry.balance,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{
            InterfaceRevision,
            TapAgentRecord,
            TapEndpointActivation,
            TapEndpointKey,
            TapEndpointRevision,
            TapPaymentPolicy,
            TapSchedulePolicy,
            TapSharedObjectRef,
            TapSkillRecord,
            TapSkillRequirements,
            TapVertexAuthorizationSchema,
        },
    };

    fn endpoint(active: bool) -> TapEndpointRecord {
        TapEndpointRecord {
            key: TapEndpointKey {
                agent_id: AgentId(sui::types::Address::from_static("0xa")),
                skill_id: SkillId(sui::types::Address::from_static("0xb")),
                interface_revision: InterfaceRevision(1),
            },
            package_id: sui::types::Address::from_static("0xc"),
            endpoint_object: sui::types::ObjectReference::new(
                sui::types::Address::from_static("0xd"),
                1,
                sui::types::Digest::from([7; 32]),
            ),
            shared_objects: vec![TapSharedObjectRef::immutable(
                sui::types::Address::from_static("0xe"),
                2,
            )],
            config_digest: vec![1],
            requirements: TapSkillRequirements {
                input_schema_hash: vec![2],
                workflow_hash: vec![3],
                metadata_hash: vec![4],
                payment_policy: TapPaymentPolicy::default(),
                schedule_policy: TapSchedulePolicy::default(),
                vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
            },
            active_for_new_executions: active,
        }
    }

    fn endpoint_revision(revision: u64, active: bool) -> TapEndpointRevision {
        let endpoint = endpoint(active);
        TapEndpointRevision {
            agent_id: endpoint.key.agent_id,
            skill_id: endpoint.key.skill_id,
            interface_revision: InterfaceRevision(revision),
            package_id: endpoint.package_id,
            endpoint_object_id: *endpoint.endpoint_object.object_id(),
            endpoint_object_version: endpoint.endpoint_object.version(),
            endpoint_object_digest: endpoint.endpoint_object.digest().inner().to_vec(),
            shared_objects: endpoint.shared_objects,
            requirements: endpoint.requirements,
            config_digest: endpoint.config_digest,
            active_for_new_executions: active,
        }
    }

    fn registry() -> TapRegistry {
        let agent_id = AgentId(sui::types::Address::from_static("0xa"));
        let skill_id = SkillId(sui::types::Address::from_static("0xb"));
        let requirements = TapSkillRequirements {
            input_schema_hash: vec![2],
            workflow_hash: vec![3],
            metadata_hash: vec![4],
            payment_policy: TapPaymentPolicy::default(),
            schedule_policy: TapSchedulePolicy::default(),
            vertex_authorization_schema: TapVertexAuthorizationSchema::default(),
        };

        TapRegistry {
            id: sui::types::Address::from_static("0xf"),
            agents: vec![TapAgentRecord {
                agent_id,
                owner: sui::types::Address::from_static("0x1"),
                operator: sui::types::Address::from_static("0x2"),
                metadata_hash: vec![1],
                auth_mode: 0,
                active: true,
            }],
            skills: vec![TapSkillRecord {
                agent_id,
                skill_id,
                dag_id: sui::types::Address::from_static("0x3"),
                tap_package_id: sui::types::Address::from_static("0xc"),
                workflow_hash: requirements.workflow_hash.clone(),
                requirements_hash: requirements.input_schema_hash.clone(),
                metadata_hash: requirements.metadata_hash.clone(),
                payment_policy: requirements.payment_policy.clone(),
                schedule_policy: requirements.schedule_policy.clone(),
                capability_schema_hash: vec![5],
                active: true,
            }],
            endpoints: vec![
                endpoint_revision(1, true),
                TapEndpointRevision {
                    requirements,
                    ..endpoint_revision(2, false)
                },
            ],
            active_endpoints: vec![TapEndpointActivation {
                agent_id,
                skill_id,
                interface_revision: InterfaceRevision(2),
            }],
        }
    }

    #[test]
    fn resolve_active_endpoint_record_reuses_sdk_fail_closed_rule() {
        let records = vec![endpoint(false), endpoint(true)];
        let resolved = resolve_active_endpoint_record(
            &records,
            AgentId(sui::types::Address::from_static("0xa")),
            SkillId(sui::types::Address::from_static("0xb")),
        )
        .expect("one active endpoint");

        assert!(resolved.active_for_new_executions);
    }

    #[test]
    fn registry_active_resolution_uses_activation_vector() {
        let registry = registry();
        let records = registry.endpoint_records().expect("endpoint records");

        assert_eq!(records.len(), 2);
        assert!(!records[0].active_for_new_executions);
        assert!(records[1].active_for_new_executions);

        let resolved = registry
            .active_endpoint_record(
                AgentId(sui::types::Address::from_static("0xa")),
                SkillId(sui::types::Address::from_static("0xb")),
            )
            .expect("active endpoint");

        assert_eq!(resolved.key.interface_revision, InterfaceRevision(2));
    }

    #[test]
    fn active_skill_execution_target_reuses_sdk_registry_resolution() {
        let registry = registry();
        let target = resolve_active_tap_skill_execution_target(
            &registry,
            AgentId(sui::types::Address::from_static("0xa")),
            SkillId(sui::types::Address::from_static("0xb")),
        )
        .expect("active skill target");

        assert_eq!(target.skill.dag_id, sui::types::Address::from_static("0x3"));
        assert_eq!(target.endpoint.key.interface_revision, InterfaceRevision(2));
    }
}
