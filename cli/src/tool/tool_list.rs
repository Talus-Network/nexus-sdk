use {
    crate::{command_title, display::json_output, item, loading, prelude::*, sui::*},
    nexus_sdk::{
        nexus::crawler::DynamicMap,
        types::{Tool, ToolRef},
    },
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

    let mut tools_json = Vec::new();

    for tool in tools {
        let tool = tool.data;

        let tool_type = if matches!(tool.reference, ToolRef::Sui { .. }) {
            "OnChain"
        } else {
            "OffChain"
        };

        let unregistered = match tool.unregistered_at {
            Some(unregistered_at) => format!(
                "(Unregistered at '{}') ",
                unregistered_at.timestamp_millis()
            ),
            None => "".to_string(),
        };

        tools_json.push(json!(tool));

        item!(
            "{unregistered}{tool_type} Tool '{fqn}' at '{reference}' registered '{registered_at}' - {description} {timeout}",
            unregistered = unregistered.truecolor(100, 100, 100),
            tool_type = tool_type.truecolor(100, 100, 100),
            fqn = tool.fqn.to_string().truecolor(100, 100, 100),
            reference = tool.reference.to_string().truecolor(100, 100, 100),
            registered_at = tool.registered_at.to_string().truecolor(100, 100, 100),
            description = tool.description.truecolor(100, 100, 100),
            timeout = timeouts
                .get(&tool.fqn)
                .unwrap_or(&"N/A".to_string())
        );
    }

    json_output(&tools_json)?;

    Ok(())
}
