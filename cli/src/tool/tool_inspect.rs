use {
    crate::{
        command_title,
        display::json_output,
        item,
        notify_error,
        notify_success,
        prelude::*,
        sui::get_nexus_client,
    },
    nexus_sdk::{
        nexus::tool::ToolInspection,
        types::{Tool, ToolRef},
    },
};

pub(crate) async fn inspect_tool(fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting tool '{fqn}'");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let inspection = nexus_client
        .tool()
        .inspect_tool(&fqn)
        .await
        .map_err(NexusCliError::Nexus)?;

    print_inspection(&inspection)?;
    json_output(&inspect_tool_result_json(&inspection))
}

/// Stable JSON contract for `nexus tool inspect`. The `tool` field carries
/// the fully-decoded on-chain `Tool` record (HTTP- or Sui-variant); callers
/// that need the package/module/witness can read them off `tool.ref`
/// without an extra round-trip. `tool_id` and `tool_gas_id` are locally
/// derived from the FQN so they're surfaced even when the tool does not
/// exist yet (a convenient pre-computation for deployments).
pub(crate) fn inspect_tool_result_json(inspection: &ToolInspection) -> serde_json::Value {
    json!({
        "tool_id": inspection.tool_id,
        "tool_gas_id": inspection.tool_gas_id,
        "exists": inspection.exists,
        "tool": inspection.tool,
    })
}

/// Render a human-readable inspection report. No-op in `--json` mode — the
/// underlying `notify_success!`/`notify_error!`/`item!` macros check
/// `JSON_MODE` themselves, mirroring how `dag inspect-execution` interleaves
/// progress notifications with the structured JSON.
fn print_inspection(inspection: &ToolInspection) -> AnyResult<(), NexusCliError> {
    let Some(tool) = inspection.tool.as_ref() else {
        notify_error!(
            "Tool '{fqn}' is not registered.",
            fqn = inspection.fqn.to_string().truecolor(100, 100, 100),
        );
        item!(
            "Derived Tool ID: {id}",
            id = inspection.tool_id.to_string().truecolor(100, 100, 100)
        );
        item!(
            "Derived ToolGas ID: {id}",
            id = inspection.tool_gas_id.to_string().truecolor(100, 100, 100)
        );
        return Ok(());
    };

    let fqn = tool.fqn_string().map_err(NexusCliError::Any)?;
    let description = tool.description_string().map_err(NexusCliError::Any)?;
    let registered_at = tool.registered_at_datetime().map_err(NexusCliError::Any)?;
    let unregistered_at = tool
        .unregistered_at_datetime()
        .map_err(NexusCliError::Any)?;

    let status = if unregistered_at.is_some() {
        "unregistered"
    } else {
        "active"
    };
    notify_success!(
        "Tool '{fqn}' registered ({status}).",
        fqn = fqn.truecolor(100, 100, 100),
        status = status.truecolor(100, 100, 100),
    );

    item!(
        "Tool ID: {id}",
        id = inspection.tool_id.to_string().truecolor(100, 100, 100)
    );
    item!(
        "ToolGas ID: {id}",
        id = inspection.tool_gas_id.to_string().truecolor(100, 100, 100)
    );
    print_tool_reference(tool)?;
    item!(
        "Cap-gated (WAC): {cap_first}",
        cap_first = tool
            .workflow_authorization_cap_first
            .to_string()
            .truecolor(100, 100, 100)
    );
    item!(
        "Description: {description}",
        description = description.truecolor(100, 100, 100)
    );
    item!(
        "Registered at: {at}",
        at = registered_at.to_string().truecolor(100, 100, 100)
    );
    if let Some(unregistered_at) = unregistered_at {
        item!(
            "Unregistered at: {at}",
            at = unregistered_at.to_string().truecolor(100, 100, 100)
        );
    }

    Ok(())
}

fn print_tool_reference(tool: &Tool) -> AnyResult<(), NexusCliError> {
    match &tool.r#ref {
        ToolRef::Http { .. } => {
            let url = tool
                .r#ref
                .http_url_string()
                .map_err(NexusCliError::Any)?
                .ok_or_else(|| NexusCliError::Any(anyhow!("expected HTTP tool reference")))?;
            item!(
                "Variant: {kind}",
                kind = "off-chain (HTTP)".truecolor(100, 100, 100)
            );
            item!("URL: {url}", url = url.truecolor(100, 100, 100));
        }
        ToolRef::Sui { .. } => {
            let Some((package_address, module_name, tool_witness_id)) =
                tool.r#ref.sui_parts().map_err(NexusCliError::Any)?
            else {
                return Err(NexusCliError::Any(anyhow!("expected Sui tool reference")));
            };
            item!(
                "Variant: {kind}",
                kind = "on-chain (Sui)".truecolor(100, 100, 100)
            );
            item!(
                "Package: {pkg}",
                pkg = package_address.to_string().truecolor(100, 100, 100)
            );
            item!(
                "Module: {module}",
                module = module_name.truecolor(100, 100, 100)
            );
            item!(
                "Witness ID: {witness}",
                witness = tool_witness_id.to_string().truecolor(100, 100, 100)
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::fqn};

    #[test]
    fn inspect_tool_result_json_reports_missing_tool() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_gas_id: sui::types::Address::from_static("0xbb"),
            exists: false,
            tool: None,
        };

        let json = inspect_tool_result_json(&inspection);
        assert_eq!(json["exists"], serde_json::Value::Bool(false));
        assert_eq!(
            json["tool_id"],
            serde_json::json!(sui::types::Address::from_static("0xaa").to_string())
        );
        assert!(json["tool"].is_null());
    }
}
