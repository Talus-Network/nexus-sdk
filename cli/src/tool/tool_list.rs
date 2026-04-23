use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{nexus::crawler::DynamicMap, types::Tool},
    prettytable::{row, Table},
};

/// List tools available in the tool registry.
pub(crate) async fn list_tools() -> AnyResult<(), NexusCliError> {
    command_title!("Listing all available Nexus tools");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();

    let tools_handle = loading!("Fetching tools from the tool registry...");

    #[derive(Deserialize)]
    struct ToolRegistry {
        timeouts: DynamicMap<ToolFqn, String>,
    }

    let tool_registry = match crawler
        .get_object::<ToolRegistry>(*nexus_objects.tool_registry.object_id())
        .await
    {
        Ok(tool_registry) => tool_registry.data,
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    let timeouts = match crawler.get_dynamic_fields(&tool_registry.timeouts).await {
        Ok(timeouts) => timeouts,
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    let tool_ids = match timeouts
        .iter()
        .map(|(fqn, _)| Tool::derive_id(*nexus_objects.tool_registry.object_id(), fqn))
        .collect::<AnyResult<Vec<_>>>()
    {
        Ok(ids) => ids,
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    let tools = match crawler.get_objects::<Tool>(&tool_ids).await {
        Ok(tools) => tools,
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tools_handle.success();

    notify_success!("Successfully fetched {} tools", tools.len());

    let mut tools_json = Vec::new();

    let mut table = Table::new();

    table.add_row(row![
        "FQN",
        "Reference",
        "Timeout",
        "Registered At",
        "Unregistered At"
    ]);

    for tool in tools {
        let tool = tool.data;

        tools_json.push(json!(tool));

        table.add_row(row![
            tool.fqn.to_string(),
            tool.reference.to_string(),
            format!(
                "{} ms",
                timeouts.get(&tool.fqn).unwrap_or(&"N/A".to_string())
            ),
            tool.registered_at.to_string(),
            tool.unregistered_at
                .map_or("N/A".to_string(), |t| t.to_string())
        ]);
    }

    if !JSON_MODE.load(Ordering::Relaxed) {
        table.printstd();
    }

    json_output(&tools_json)?;

    Ok(())
}
