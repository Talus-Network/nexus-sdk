use super::*;

pub(crate) async fn create_endpoint(
    package: sui::types::Address,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!("Creating standard TAP endpoint for package '{package}'");

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let result = nexus_client
        .tap()
        .create_standard_endpoint(package)
        .await
        .map_err(NexusCliError::Nexus)?;

    notify_success!(
        "Standard TAP endpoint {endpoint_id} created",
        endpoint_id = result
            .endpoint_object
            .object_id()
            .to_string()
            .truecolor(100, 100, 100)
    );

    json_output(&create_endpoint_result_json(package, &result))
}

pub(crate) async fn inspect_endpoint(
    endpoint_id: sui::types::Address,
) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting standard TAP endpoint '{endpoint_id}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let inspection = nexus_client
        .tap()
        .inspect_endpoint(endpoint_id)
        .await
        .map_err(NexusCliError::Nexus)?;

    if let Some(active) = inspection.active_record.as_ref() {
        notify_success!(
            "Active revision {revision} for agent {agent_id} skill {skill_id}",
            revision = active
                .key
                .interface_revision
                .0
                .to_string()
                .truecolor(100, 100, 100),
            agent_id = active.key.agent_id.to_string().truecolor(100, 100, 100),
            skill_id = active.key.skill_id.to_string().truecolor(100, 100, 100),
        );
    }

    json_output(&inspect_endpoint_result_json(&inspection))
}

pub(crate) fn create_endpoint_result_json(
    package: sui::types::Address,
    result: &nexus_sdk::nexus::tap::CreateStandardEndpointResult,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "function": "create_standard_endpoint",
        "package_id": package,
        "endpoint_object_id": result.endpoint_object.object_id(),
        "endpoint_object_version": result.endpoint_object.version(),
        "endpoint_object_digest": result.endpoint_object.digest(),
        "endpoint_object_digest_hex": hex::encode(result.endpoint_object.digest().inner()),
        "digest": result.tx_digest,
        "tx_checkpoint": result.tx_checkpoint,
    })
}

pub(crate) fn inspect_endpoint_result_json(
    inspection: &nexus_sdk::nexus::tap::EndpointInspection,
) -> serde_json::Value {
    json!({
        "standard_tap": true,
        "endpoint_object_id": inspection.object_ref.object_id(),
        "endpoint_object_version": inspection.object_ref.version(),
        "endpoint_object_digest": inspection.object_ref.digest(),
        "endpoint_object_digest_hex": hex::encode(inspection.object_ref.digest().inner()),
        "package_id": inspection.package_id,
        "active_record": inspection.active_record,
        "revisions": inspection.revisions,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::nexus::tap::{CreateStandardEndpointResult, EndpointInspection},
    };

    #[test]
    fn create_endpoint_result_json_exposes_endpoint_object_fields() {
        let package = sui::types::Address::from_static("0xabc");
        let endpoint_object = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xdef"),
            7,
            sui::types::Digest::from([3u8; 32]),
        );
        let result = CreateStandardEndpointResult {
            tx_digest: sui::types::Digest::from([1u8; 32]),
            tx_checkpoint: 42,
            endpoint_object,
        };
        let json = create_endpoint_result_json(package, &result);
        assert_eq!(json["standard_tap"], serde_json::Value::Bool(true));
        assert_eq!(
            json["endpoint_object_id"],
            serde_json::json!(sui::types::Address::from_static("0xdef").to_string())
        );
        assert_eq!(json["endpoint_object_version"], serde_json::json!(7));
        assert_eq!(json["tx_checkpoint"], serde_json::json!(42));
        assert_eq!(
            json["endpoint_object_digest_hex"].as_str().unwrap().len(),
            64
        );
        assert_eq!(json["package_id"], serde_json::json!(package.to_string()));
    }

    #[test]
    fn inspect_endpoint_result_json_omits_active_record_when_unbound() {
        let object_ref = sui::types::ObjectReference::new(
            sui::types::Address::from_static("0xeee"),
            3,
            sui::types::Digest::from([2u8; 32]),
        );
        let inspection = EndpointInspection {
            object_ref,
            package_id: None,
            active_record: None,
            revisions: vec![],
        };
        let json = inspect_endpoint_result_json(&inspection);
        assert_eq!(
            json["endpoint_object_id"],
            serde_json::json!(sui::types::Address::from_static("0xeee").to_string())
        );
        assert!(json["active_record"].is_null());
        assert!(json["package_id"].is_null());
        assert_eq!(json["revisions"], serde_json::json!([]));
    }
}
