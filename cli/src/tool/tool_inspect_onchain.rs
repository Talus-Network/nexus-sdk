use {
    crate::{command_title, display::json_output, prelude::*, sui::get_nexus_client},
    nexus_sdk::nexus::tool::OnChainToolInspection,
};

pub(crate) async fn inspect_onchain(fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting on-chain tool '{fqn}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let inspection = nexus_client
        .tool()
        .inspect_on_chain_tool(&fqn)
        .await
        .map_err(NexusCliError::Nexus)?;

    json_output(&inspect_on_chain_tool_result_json(&inspection))
}

pub(crate) fn inspect_on_chain_tool_result_json(
    inspection: &OnChainToolInspection,
) -> serde_json::Value {
    let tool_meta = inspection.tool.as_ref();
    json!({
        "tool_fqn": inspection.fqn,
        "tool_id": inspection.tool_id,
        "tool_gas_id": inspection.tool_gas_id,
        "exists": inspection.exists,
        "package_address": inspection.package_address,
        "module_name": inspection.module_name.as_ref().map(|m| m.as_str()),
        "tool_witness_id": inspection.tool_witness_id,
        "description": tool_meta.map(|t| t.description.clone()),
        "input_schema": tool_meta.map(|t| t.input_schema.clone()),
        "output_schema": tool_meta.map(|t| t.output_schema.clone()),
        "workflow_authorization_cap_first": tool_meta.map(|t| t.workflow_authorization_cap_first),
        "registered_at": tool_meta.map(|t| t.registered_at),
        "unregistered_at": tool_meta.and_then(|t| t.unregistered_at),
    })
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::fqn};

    #[test]
    fn inspect_on_chain_tool_result_json_reports_missing_tool() {
        let inspection = OnChainToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_gas_id: sui::types::Address::from_static("0xbb"),
            exists: false,
            tool: None,
            package_address: None,
            module_name: None,
            tool_witness_id: None,
        };

        let json = inspect_on_chain_tool_result_json(&inspection);
        assert_eq!(json["exists"], serde_json::Value::Bool(false));
        assert_eq!(
            json["tool_id"],
            serde_json::json!(sui::types::Address::from_static("0xaa").to_string())
        );
        assert!(json["package_address"].is_null());
        assert!(json["module_name"].is_null());
    }
}
