use {
    super::tool_output::{ExternalVerifierOutput, ToolOutput, ToolVerifierSupportOutput},
    crate::{
        command_title,
        display::json_output,
        item,
        notify_error,
        notify_success,
        prelude::*,
        sui::get_read_only_nexus_client,
    },
    nexus_sdk::{
        nexus::{client::NexusClient, tool::ToolInspection},
        types::{ToolRef, ToolStateV2},
    },
};

pub(crate) async fn inspect_tool(fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting tool '{fqn}'");

    let nexus_client = get_read_only_nexus_client().await?;
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
/// `tool_ref` and `tool` expose semantic values instead of generated Move
/// storage wrappers. `tool` contains the complete CLI record.
pub(crate) fn inspect_tool_result_json(
    inspection: &ToolInspection,
) -> AnyResult<serde_json::Value, NexusCliError> {
    let tool_ref = normalized_tool_ref_json(inspection.tool.as_ref().map(ToolStateV2::reference))?;
    let tool = inspection
        .tool
        .as_ref()
        .map(|tool| ToolOutput::try_from_state(tool, None))
        .transpose()
        .map_err(NexusCliError::Any)?;
    let verifier_support = inspection
        .verifier_support
        .as_ref()
        .map(ToolVerifierSupportOutput::try_from)
        .transpose()
        .map_err(NexusCliError::Any)?;
    let external_verifier = inspection
        .external_verifier
        .as_ref()
        .map(ExternalVerifierOutput::try_from)
        .transpose()
        .map_err(NexusCliError::Any)?;

    Ok(json!({
        "fqn": inspection.fqn,
        "tool_id": inspection.tool_id,
        "tool_cashier_id": inspection.tool_cashier_id,
        "exists": inspection.exists,
        "tool_ref": tool_ref,
        "tool": tool,
        "verifier_support": verifier_support,
        "external_verifier": external_verifier,
        "invocation_cost_mist": inspection.invocation_cost_mist,
    }))
}

/// Inspects canonical chain state for a completed registration.
///
/// A missing Tool returns `None`. An existing Tool returns the same stable
/// projection used by [`inspect_tool_result_json`] with registration status.
pub(crate) async fn inspect_registration_result(
    nexus_client: &NexusClient,
    fqn: &ToolFqn,
) -> AnyResult<Option<serde_json::Value>, NexusCliError> {
    let inspection = nexus_client
        .tool()
        .inspect_tool(fqn)
        .await
        .map_err(NexusCliError::Nexus)?;

    registration_result_json(&inspection)
}

/// Converts an existing [`ToolInspection`] into a registration result.
///
/// Registration uses this canonical state projection before submission and
/// after submission errors. Transaction error text is not a state contract.
pub(crate) fn registration_result_json(
    inspection: &ToolInspection,
) -> AnyResult<Option<serde_json::Value>, NexusCliError> {
    if !inspection.exists {
        return Ok(None);
    }

    let mut result = inspect_tool_result_json(inspection)?;
    let object = result.as_object_mut().ok_or_else(|| {
        NexusCliError::Any(anyhow!("Tool inspection must serialize as an object"))
    })?;
    object.insert("tool_fqn".to_owned(), json!(inspection.fqn));
    object.insert("already_registered".to_owned(), json!(true));

    Ok(Some(result))
}

/// Adds transaction evidence to the semantic result from [`inspect_tool_result_json`].
pub(crate) fn registration_submission_result_json(
    inspection: &ToolInspection,
    digest: &sui::types::Digest,
    tx_checkpoint: u64,
    owner_cap_over_tool_id: sui::types::Address,
    cashier_admin_cap_id: Option<sui::types::Address>,
) -> AnyResult<serde_json::Value, NexusCliError> {
    let mut result = inspect_tool_result_json(inspection)?;
    let object = result.as_object_mut().ok_or_else(|| {
        NexusCliError::Any(anyhow!("Tool inspection must serialize as an object"))
    })?;
    object.insert("already_registered".to_owned(), json!(false));
    object.insert("digest".to_owned(), json!(digest));
    object.insert("tx_checkpoint".to_owned(), json!(tx_checkpoint));
    object.insert(
        "owner_cap_over_tool_id".to_owned(),
        json!(owner_cap_over_tool_id),
    );
    object.insert(
        "cashier_admin_cap_id".to_owned(),
        json!(cashier_admin_cap_id),
    );

    Ok(result)
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

/// Renders a human readable inspection report.
///
/// This does nothing in `--json` mode because the output macros inspect
/// `JSON_MODE` before emitting progress notifications.
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
            "Derived ToolCashier ID: {id}",
            id = inspection
                .tool_cashier_id
                .to_string()
                .truecolor(100, 100, 100)
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
        "ToolCashier ID: {id}",
        id = inspection
            .tool_cashier_id
            .to_string()
            .truecolor(100, 100, 100)
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
        item!("Verifier witness: {id}", id = record.witness.bytes);
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
    match inspection.invocation_cost_mist {
        Some(cost) => item!("Invocation cost: {cost} MIST"),
        None => item!("Invocation cost: unavailable"),
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

fn print_tool_reference(tool: &ToolStateV2) -> AnyResult<(), NexusCliError> {
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
            move_bindings::{
                interface::{
                    meta_schema::{MetaSchema, OutputVariantSchema},
                    verifier::{ToolVerifierSupport, VerifierMethodId},
                },
                move_std::{ascii, option::Option as MoveOption},
                sui_framework::{balance::Balance, object::ID},
                tool::external_verifier::ExternalVerifier,
            },
        },
    };

    fn ascii(value: &str) -> ascii::String {
        ascii::String::from(value)
    }

    fn fixture_tool() -> ToolStateV2 {
        ToolStateV2 {
            minimum_protocol_version: 1,
            registry: ID::new(sui::types::Address::from_static("0x42")),
            fqn: ascii("xyz.taluslabs.example@1"),
            r#ref: ToolRef::Http {
                url: b"https://example.com/tool".to_vec(),
            },
            description: b"Example tool".to_vec(),
            meta_schema: fixture_meta_schema(),
            verified: true,
            vault: Balance {
                value: 25,
                phantom_t0: std::marker::PhantomData,
            },
            workflow_authorization_cap_first: false,
            lock_duration_ms: 5_000,
            registered_at_ms: 0,
            unregistered_at_ms: MoveOption::from(None),
        }
    }

    fn fixture_meta_schema() -> MetaSchema {
        MetaSchema::new(
            vec![],
            vec![OutputVariantSchema::new(b"Ok".to_vec(), vec![])],
        )
    }

    #[test]
    fn inspect_tool_result_json_uses_the_semantic_tool_contract() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_cashier_id: sui::types::Address::from_static("0xbb"),
            exists: true,
            tool: Some(fixture_tool()),
            verifier_support: None,
            external_verifier: None,
            invocation_cost_mist: Some(7),
        };

        let json = inspect_tool_result_json(&inspection).expect("inspection JSON should build");

        assert_eq!(json["fqn"], "xyz.taluslabs.example@1");
        assert_eq!(json["tool"]["fqn"], "xyz.taluslabs.example@1");
        assert_eq!(json["tool"]["description"], "Example tool");
        assert_eq!(json["tool"]["meta_schema"]["input_ports"], json!([]));
        assert_eq!(
            json["tool"]["meta_schema"]["output_variants"][0]["variant_name"],
            "Ok"
        );
        assert!(!json.to_string().contains("\"bytes\""));
    }

    #[test]
    fn inspect_tool_result_json_uses_semantic_verifier_values() {
        let method = VerifierMethodId::new(
            ID::new(sui::types::Address::from_static("0xaa")),
            ID::new(sui::types::Address::from_static("0xcc")),
            ascii("verifier"),
            ascii("verify"),
        );
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_cashier_id: sui::types::Address::from_static("0xbb"),
            exists: true,
            tool: Some(fixture_tool()),
            verifier_support: Some(ToolVerifierSupport::External {
                method_id: method.clone(),
            }),
            external_verifier: Some(ExternalVerifier::new(
                method,
                ID::new(sui::types::Address::from_static("0xdd")),
                vec![ID::new(sui::types::Address::from_static("0xee"))],
            )),
            invocation_cost_mist: Some(7),
        };

        let json = inspect_tool_result_json(&inspection).expect("inspection JSON should build");

        assert_eq!(json["verifier_support"]["kind"], "external");
        assert_eq!(json["verifier_support"]["method"]["module"], "verifier");
        assert_eq!(json["verifier_support"]["method"]["function"], "verify");
        assert_eq!(
            json["external_verifier"]["witness_id"],
            sui::types::Address::from_static("0xdd").to_string()
        );
        assert_eq!(
            json["external_verifier"]["immutable_shared_object_ids"][0],
            sui::types::Address::from_static("0xee").to_string()
        );
        assert!(!json.to_string().contains("\"bytes\""));
    }

    #[test]
    fn registration_submission_reuses_the_inspection_contract() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_cashier_id: sui::types::Address::from_static("0xbb"),
            exists: true,
            tool: Some(fixture_tool()),
            verifier_support: None,
            external_verifier: None,
            invocation_cost_mist: Some(7),
        };
        let digest = sui::types::Digest::from([3; 32]);

        let json = registration_submission_result_json(
            &inspection,
            &digest,
            9,
            sui::types::Address::from_static("0xcc"),
            Some(sui::types::Address::from_static("0xdd")),
        )
        .expect("registration JSON should build");

        assert_eq!(json["fqn"], "xyz.taluslabs.example@1");
        assert_eq!(json["tool"]["fqn"], "xyz.taluslabs.example@1");
        assert_eq!(json["digest"], digest.to_string());
        assert_eq!(json["tx_checkpoint"], 9);
        assert_eq!(
            json["owner_cap_over_tool_id"],
            sui::types::Address::from_static("0xcc").to_string()
        );
        assert_eq!(
            json["cashier_admin_cap_id"],
            sui::types::Address::from_static("0xdd").to_string()
        );
        assert!(!json.to_string().contains("\"bytes\""));
    }

    #[test]
    fn inspect_tool_result_json_reports_missing_tool() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_cashier_id: sui::types::Address::from_static("0xbb"),
            exists: false,
            tool: None,
            verifier_support: None,
            external_verifier: None,
            invocation_cost_mist: None,
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

    /// Verifies that registration treats canonical chain existence as success.
    #[test]
    fn registration_result_json_reports_existing_tool() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_cashier_id: sui::types::Address::from_static("0xbb"),
            exists: true,
            tool: None,
            verifier_support: None,
            external_verifier: None,
            invocation_cost_mist: None,
        };

        let json = registration_result_json(&inspection)
            .expect("registration JSON should build")
            .expect("the Tool exists");

        assert_eq!(json["already_registered"], serde_json::Value::Bool(true));
        assert_eq!(json["tool_fqn"], "xyz.taluslabs.example@1");
        assert_eq!(
            json["tool_cashier_id"],
            serde_json::json!(sui::types::Address::from_static("0xbb").to_string())
        );
    }

    /// Verifies that registration can distinguish an unused name.
    #[test]
    fn registration_result_json_reports_missing_tool() {
        let inspection = ToolInspection {
            fqn: fqn!("xyz.taluslabs.example@1"),
            tool_id: sui::types::Address::from_static("0xaa"),
            tool_cashier_id: sui::types::Address::from_static("0xbb"),
            exists: false,
            tool: None,
            verifier_support: None,
            external_verifier: None,
            invocation_cost_mist: None,
        };

        assert!(registration_result_json(&inspection).unwrap().is_none());
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
        let invalid_http = ToolRef::Http { url: vec![0xff] };
        assert!(normalized_tool_ref_json(Some(&invalid_http)).is_err());

        let mut invalid_module = ascii("demo_tool");
        invalid_module.bytes = vec![0xff];
        let invalid_sui = ToolRef::Sui {
            package_address: sui::types::Address::from_static("0x1234"),
            module_name: invalid_module,
            tool_witness_id: ID::new(sui::types::Address::from_static("0xabcd")),
        };
        assert!(normalized_tool_ref_json(Some(&invalid_sui)).is_err());
    }
}
