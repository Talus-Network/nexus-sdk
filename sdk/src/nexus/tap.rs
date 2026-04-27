//! Read-only helpers for standard TAP endpoint recovery.

use crate::{
    nexus::crawler::{Crawler, Response},
    sui,
    types::{
        resolve_active_tap_endpoint,
        AgentId,
        SkillId,
        TapActiveEndpoint,
        TapEndpointRecord,
        TapEndpointResolutionError,
    },
};

/// Fetch a pinned TAP endpoint record by object ID and validate fail-closed
/// metadata before returning it to callers such as the leader.
pub async fn fetch_tap_endpoint(
    crawler: &Crawler,
    endpoint_id: sui::types::Address,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let endpoint = crawler.get_object::<TapEndpointRecord>(endpoint_id).await?;
    endpoint.data.validate()?;
    Ok(endpoint)
}

/// Fetch an active endpoint pointer and then fetch the pinned endpoint it names.
pub async fn fetch_active_tap_endpoint(
    crawler: &Crawler,
    active_endpoint_id: sui::types::Address,
) -> anyhow::Result<Response<TapEndpointRecord>> {
    let active = crawler
        .get_object::<TapActiveEndpoint>(active_endpoint_id)
        .await?;
    let endpoint = fetch_tap_endpoint(crawler, active.data.endpoint_object_id).await?;

    if endpoint.data.key.agent_id != active.data.agent_id
        || endpoint.data.key.skill_id != active.data.skill_id
        || endpoint.data.key.interface_revision != active.data.active_revision
    {
        anyhow::bail!(
            "active TAP endpoint pointer '{}' does not match fetched endpoint '{}'",
            active.object_id,
            endpoint.object_id
        );
    }

    Ok(endpoint)
}

/// Resolve a fresh execution endpoint from already fetched records.
pub fn resolve_active_endpoint_record(
    records: &[TapEndpointRecord],
    agent_id: AgentId,
    skill_id: SkillId,
) -> Result<&TapEndpointRecord, TapEndpointResolutionError> {
    resolve_active_tap_endpoint(records, agent_id, skill_id)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{
            InterfaceRevision,
            TapEndpointKey,
            TapPaymentPolicy,
            TapSchedulePolicy,
            TapSharedObjectRef,
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
}
