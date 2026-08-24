use {
    super::tool_output::ToolOutput,
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::nexus::tool::{ToolCompatibility, ToolInspection},
    prettytable::{row, Table},
    serde::Serialize,
};

#[derive(Serialize)]
struct ToolInventoryOutput {
    fqn: ToolFqn,
    tool_id: sui::types::Address,
    tool_cashier_id: sui::types::Address,
    owner: Option<sui::types::Owner>,
    witness_type: Option<sui::types::StructTag>,
    inner_type: Option<sui::types::StructTag>,
    compatibility: &'static str,
    registered: Option<bool>,
    tool: Option<ToolOutput>,
    detail: Option<String>,
}

impl TryFrom<ToolInspection> for ToolInventoryOutput {
    type Error = anyhow::Error;

    fn try_from(inspection: ToolInspection) -> Result<Self, Self::Error> {
        let registered = inspection
            .tool
            .as_ref()
            .map(|tool| tool.unregistered_at_millis().map(|value| value.is_none()))
            .transpose()?;
        let tool = inspection
            .tool
            .as_ref()
            .map(|_| ToolOutput::try_from_inspection(&inspection))
            .transpose()?;
        Ok(Self {
            fqn: inspection.fqn,
            tool_id: inspection.tool_id,
            tool_cashier_id: inspection.tool_cashier_id,
            owner: inspection.owner,
            witness_type: inspection.witness_type,
            inner_type: inspection.inner_type,
            compatibility: compatibility_name(inspection.compatibility),
            registered,
            tool,
            detail: inspection.detail,
        })
    }
}

/// Lists the permanent Tool directory without allowing one incompatible item
/// to fail the collection.
pub(crate) async fn list_tools() -> AnyResult<(), NexusCliError> {
    command_title!("Listing Nexus Tools");
    let client = get_read_only_nexus_client().await?;
    let progress = loading!("Reading the Tool inventory...");
    let tools = client
        .tool()
        .list_tools()
        .await
        .map_err(NexusCliError::Nexus)?
        .into_iter()
        .map(ToolInventoryOutput::try_from)
        .collect::<anyhow::Result<Vec<_>>>()
        .map_err(NexusCliError::Any)?;
    progress.success();
    notify_success!("Fetched {} Tool entries", tools.len());

    if !JSON_MODE.load(Ordering::Relaxed) {
        let mut table = Table::new();
        table.add_row(row![
            "FQN",
            "Compatibility",
            "Registration",
            "Tool ID",
            "Detail"
        ]);
        for tool in &tools {
            let registration = match tool.registered {
                Some(true) => "registered",
                Some(false) => "unregistered",
                None => "unknown",
            };
            table.add_row(row![
                tool.fqn,
                tool.compatibility,
                registration,
                tool.tool_id,
                tool.detail.as_deref().unwrap_or("")
            ]);
        }
        table.printstd();
    }

    json_output(&tools)
}

fn compatibility_name(compatibility: ToolCompatibility) -> &'static str {
    match compatibility {
        ToolCompatibility::Current => "current",
        ToolCompatibility::LegacyUnderstood => "legacy_understood",
        ToolCompatibility::MigrationRequired => "migration_required",
        ToolCompatibility::Unsupported => "unsupported",
        ToolCompatibility::Unavailable => "unavailable",
    }
}
