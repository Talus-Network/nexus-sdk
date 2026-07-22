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
    json_output(&inspect_tool_result_json(&inspection)?)
}

/// Stable JSON contract for `nexus tool inspect`.
///
/// `tool_ref` is a CLI-owned projection that hides generated Move enum and
/// byte-string details. The fully decoded on-chain `tool` remains available
/// for callers that need the complete record.
pub(crate) fn inspect_tool_result_json(
    inspection: &ToolInspection,
) -> AnyResult<serde_json::Value, NexusCliError> {
    let tool_ref = normalized_tool_ref_json(inspection.tool.as_ref().map(Tool::reference))?;

    Ok(json!({
        "tool_id": inspection.tool_id,
        "tool_gas_id": inspection.tool_gas_id,
        "exists": inspection.exists,
        "tool_ref": tool_ref,
        "tool": inspection.tool,
        "verifier_support": inspection.verifier_support,
        "external_verifier": inspection.external_verifier,
    }))
}

pub(crate) fn normalized_tool_ref_json(
    reference: Option<&ToolRef>,
) -> AnyResult<serde_json::Value, NexusCliError> {
    let Some(reference) = reference else {
        return Ok(serde_json::Value::Null);
    };

    match reference {
        ToolRef::Http { .. } => {
            let url = reference
                .http_url_string()
                .map_err(NexusCliError::Any)?
                .ok_or_else(|| NexusCliError::Any(anyhow!("expected HTTP tool reference")))?;
            Ok(json!({
                "kind": "http",
                "url": url,
            }))
        }
        ToolRef::Sui { .. } => {
            let (package_id, module, witness_id) = reference
                .sui_parts()
                .map_err(NexusCliError::Any)?
                .ok_or_else(|| NexusCliError::Any(anyhow!("expected Sui tool reference")))?;
            Ok(json!({
                "kind": "sui",
                "package_id": package_id,
                "module": module,
                "witness_id": witness_id,
            }))
        }
    }
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
    match inspection.verifier_support.as_ref() {
        None => item!("Verifier support: none"),
        Some(nexus_sdk::move_bindings::interface::verifier::ToolVerifierSupport::RegisteredKey) => {
            item!("Verifier support: RegisteredKey")
        }
        Some(nexus_sdk::move_bindings::interface::verifier::ToolVerifierSupport::External {
            method_id,
        }) => {
            item!(
                "Verifier support: External ({package}::{module}::{function})",
                package = method_id.package_id.bytes.to_string(),
                module = String::from(method_id.module_name.clone()),
                function = String::from(method_id.function_name.clone()),
            );
        }
    }
    if let Some(record) = inspection.external_verifier.as_ref() {
        item!("Verifier witness: {id}", id = record.witness_id.bytes);
        item!(
            "Verifier objects: {objects}",
            objects = record
                .immutable_shared_objects
                .iter()
                .map(|id| id.bytes.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
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
    use {
        super::*,
        nexus_sdk::{
            fqn,
            move_bindings::{move_std::ascii, sui_framework::object::ID},
        },
    };

    fn ascii(value: &str) -> ascii::String {
        ascii::String::from(value)
    }

    #[test]
    fn inspect_tool_result_json_reports_missing_tool() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_gas_id: sui::types::Address::from_static("0xbb"),
            exists: false,
            tool: None,
            verifier_support: None,
            external_verifier: None,
        };

        let json = inspect_tool_result_json(&inspection).expect("inspection JSON should build");
        assert_eq!(json["exists"], serde_json::Value::Bool(false));
        assert_eq!(
            json["tool_id"],
            serde_json::json!(sui::types::Address::from_static("0xaa").to_string())
        );
        assert!(json["tool_ref"].is_null());
        assert!(json["tool"].is_null());
        assert!(json["verifier_support"].is_null());
        assert!(json["external_verifier"].is_null());
    }

    #[test]
    fn normalized_tool_ref_json_reports_missing_reference() {
        assert_eq!(
            normalized_tool_ref_json(None).unwrap(),
            serde_json::Value::Null
        );
    }

    #[test]
    fn normalized_tool_ref_json_reports_http_reference() {
        let reference = ToolRef::Http {
            _variant_name: ascii("Http"),
            url: b"https://example.com/tool".to_vec(),
        };

        assert_eq!(
            normalized_tool_ref_json(Some(&reference)).unwrap(),
            serde_json::json!({
                "kind": "http",
                "url": "https://example.com/tool",
            })
        );
    }

    #[test]
    fn normalized_tool_ref_json_reports_sui_reference() {
        let package_id = sui::types::Address::from_static("0x1234");
        let witness_id = sui::types::Address::from_static("0xabcd");
        let reference = ToolRef::Sui {
            _variant_name: ascii("Sui"),
            package_address: package_id,
            module_name: ascii("demo_tool"),
            tool_witness_id: ID::new(witness_id),
        };

        assert_eq!(
            normalized_tool_ref_json(Some(&reference)).unwrap(),
            serde_json::json!({
                "kind": "sui",
                "package_id": package_id.to_string(),
                "module": "demo_tool",
                "witness_id": witness_id.to_string(),
            })
        );
    }

    #[test]
    fn normalized_tool_ref_json_rejects_invalid_generated_strings() {
        let invalid_http = ToolRef::Http {
            _variant_name: ascii("Http"),
            url: vec![0xff],
        };
        assert!(normalized_tool_ref_json(Some(&invalid_http)).is_err());

        let mut invalid_module = ascii("demo_tool");
        invalid_module.bytes = vec![0xff];
        let invalid_sui = ToolRef::Sui {
            _variant_name: ascii("Sui"),
            package_address: sui::types::Address::from_static("0x1234"),
            module_name: invalid_module,
            tool_witness_id: ID::new(sui::types::Address::from_static("0xabcd")),
        };
        assert!(normalized_tool_ref_json(Some(&invalid_sui)).is_err());
    }
}
