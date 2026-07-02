use {
    crate::{command_title, display::json_output, loading, notify_success, prelude::*, sui::*},
    nexus_sdk::{
        move_bindings::{
            move_std::ascii::String as MoveAsciiString, registry::tool_registry::ToolRegistry,
            sui_framework::linked_table::Node as LinkedTableNode,
        },
        types::Tool,
    },
    prettytable::{row, Table},
};

/// List tools available in the tool registry.
pub(crate) async fn list_tools() -> AnyResult<(), NexusCliError> {
    command_title!("Listing all available Nexus tools");

    let nexus_client = get_nexus_client(None, DEFAULT_GAS_BUDGET).await?;
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let crawler = nexus_client.crawler();

    let tools_handle = loading!("Fetching tools from the tool registry...");

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

    let timeouts = match crawler
        .get_dynamic_fields::<MoveAsciiString, LinkedTableNode<MoveAsciiString, u64>>(
            tool_registry.timeouts.id(),
            tool_registry.timeouts.size(),
        )
        .await
    {
        Ok(timeouts) => timeouts
            .into_iter()
            .map(|(key, node)| (key.into_string(), node.value))
            .collect::<HashMap<_, _>>(),
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    let tool_ids = timeouts
        .keys()
        .filter_map(|fqn| {
            Tool::derive_id(*nexus_objects.tool_registry.object_id(), &fqn.parse().ok()?).ok()
        })
        .collect::<Vec<_>>();

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
        let fqn = tool.fqn_string().map_err(NexusCliError::Any)?;
        let registered_at = tool.registered_at_datetime().map_err(NexusCliError::Any)?;
        let unregistered_at = tool
            .unregistered_at_datetime()
            .map_err(NexusCliError::Any)?;
        let timeout = timeouts
            .get(&fqn)
            .map(|timeout| format!("{timeout} ms"))
            .unwrap_or_else(|| "N/A".to_string());

        tools_json.push(json!(tool));

        table.add_row(row![
            fqn,
            tool.r#ref.to_string(),
            timeout,
            registered_at.to_string(),
            unregistered_at.map_or("N/A".to_string(), |t| t.to_string())
        ]);
    }

    if !JSON_MODE.load(Ordering::Relaxed) {
        table.printstd();
    }

    json_output(&tools_json)?;

    Ok(())
}
