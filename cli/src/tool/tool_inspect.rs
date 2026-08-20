use {
    super::tool_output::ToolOutput,
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
        nexus::{
            client::NexusClient,
            tool::{ToolCompatibility, ToolInspection},
        },
        types::{ToolRef, ToolState},
    },
};

pub(crate) async fn inspect_tool(fqn: ToolFqn) -> AnyResult<(), NexusCliError> {
    command_title!("Inspecting Tool '{fqn}'");
    let client = get_read_only_nexus_client().await?;
    let inspection = client
        .tool()
        .inspect_tool(&fqn)
        .await
        .map_err(NexusCliError::Nexus)?;

    print_inspection(&inspection)?;
    json_output(&inspect_tool_result_json(&inspection)?)
}

/// Returns the stable JSON representation of one [`ToolInspection`].
pub(crate) fn inspect_tool_result_json(
    inspection: &ToolInspection,
) -> AnyResult<serde_json::Value, NexusCliError> {
    let tool_ref = normalized_tool_ref_json(inspection.tool.as_ref().map(ToolState::reference))?;
    let tool = inspection
        .tool
        .as_ref()
        .map(ToolOutput::try_from_state)
        .transpose()
        .map_err(NexusCliError::Any)?;

    Ok(json!({
        "fqn": inspection.fqn,
        "tool_id": inspection.tool_id,
        "tool_cashier_id": inspection.tool_cashier_id,
        "owner": inspection.owner,
        "witness_type": inspection.witness_type,
        "inner_type": inspection.inner_type,
        "compatibility": compatibility_name(inspection.compatibility),
        "lifecycle": inspection.lifecycle,
        "tool_ref": tool_ref,
        "tool": tool,
        "detail": inspection.detail,
    }))
}

/// Inspects canonical state after a registration attempt.
pub(crate) async fn inspect_registration_result(
    client: &NexusClient,
    fqn: &ToolFqn,
) -> AnyResult<Option<serde_json::Value>, NexusCliError> {
    let inspection = client
        .tool()
        .inspect_tool(fqn)
        .await
        .map_err(NexusCliError::Nexus)?;
    registration_result_json(&inspection)
}

/// Converts an existing [`ToolInspection`] into a registration result.
pub(crate) fn registration_result_json(
    inspection: &ToolInspection,
) -> AnyResult<Option<serde_json::Value>, NexusCliError> {
    if inspection.definition.is_none() {
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

/// Adds transaction evidence to a successful registration result.
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
        ToolRef::Http { .. } => Ok(json!({
            "kind": "http",
            "url": reference
                .http_url_string()
                .map_err(NexusCliError::Any)?
                .ok_or_else(|| NexusCliError::Any(anyhow!("expected HTTP Tool reference")))?,
        })),
        ToolRef::Sui { .. } => {
            let (package_id, module, witness_id) = reference
                .sui_parts()
                .map_err(NexusCliError::Any)?
                .ok_or_else(|| NexusCliError::Any(anyhow!("expected Sui Tool reference")))?;
            Ok(json!({
                "kind": "sui",
                "package_id": package_id,
                "module": module,
                "witness_id": witness_id,
            }))
        }
    }
}

fn print_inspection(inspection: &ToolInspection) -> AnyResult<(), NexusCliError> {
    let Some(tool) = inspection.tool.as_ref() else {
        notify_error!(
            "Tool '{fqn}' is {compatibility}.",
            fqn = inspection.fqn.to_string().truecolor(100, 100, 100),
            compatibility = compatibility_name(inspection.compatibility),
        );
        item!("Tool ID: {id}", id = inspection.tool_id);
        if let Some(detail) = inspection.detail.as_deref() {
            item!("Detail: {detail}");
        }
        return Ok(());
    };

    notify_success!(
        "Tool '{fqn}' is {lifecycle}.",
        fqn = tool.fqn_string().map_err(NexusCliError::Any)?,
        lifecycle = match tool.inner.lifecycle {
            nexus_sdk::move_bindings::tool::tool_registry::ToolLifecycle::Open => "open",
            nexus_sdk::move_bindings::tool::tool_registry::ToolLifecycle::Closed { .. } => "closed",
            nexus_sdk::move_bindings::tool::tool_registry::ToolLifecycle::Retired { .. } =>
                "retired",
        }
    );
    item!("Tool ID: {id}", id = inspection.tool_id);
    item!("Tool cashier ID: {id}", id = inspection.tool_cashier_id);
    item!(
        "Compatibility: {value}",
        value = compatibility_name(inspection.compatibility)
    );
    print_tool_reference(tool)?;
    item!(
        "Description: {description}",
        description = tool.description_string().map_err(NexusCliError::Any)?
    );
    item!("Timeout: {timeout} ms", timeout = tool.timeout_ms);
    item!(
        "Invocation cost: {cost} MIST",
        cost = tool.invocation_cost_mist
    );
    Ok(())
}

fn print_tool_reference(tool: &ToolState) -> AnyResult<(), NexusCliError> {
    item!(
        "Reference: {reference}",
        reference = tool
            .reference()
            .display_string()
            .map_err(NexusCliError::Any)?
    );
    Ok(())
}

fn compatibility_name(compatibility: ToolCompatibility) -> &'static str {
    match compatibility {
        ToolCompatibility::Current => "current",
        ToolCompatibility::LegacyUnderstood => "legacy understood",
        ToolCompatibility::MigrationRequired => "migration required",
        ToolCompatibility::Unsupported => "unsupported",
        ToolCompatibility::Unavailable => "unavailable",
    }
}
