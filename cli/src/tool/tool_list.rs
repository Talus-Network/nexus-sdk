use {
    crate::{command_title, loading, prelude::*, sui::*},
    nexus_sdk::{
        object_crawler::{fetch_one, ObjectBag, Structure},
        types::deserialize_bytes_to_url,
    },
};

/// Create a new tool based on the provided name and template.
pub(crate) async fn list_tools() -> AnyResult<(), NexusCliError> {
    command_title!("Listing all available Neuxs tools");

    // Load CLI configuration.
    let conf = CliConf::load().await.unwrap_or_else(|_| CliConf::default());

    // Nexus objects must be present in the configuration.
    let NexusObjects {
        tool_registry_object_id,
        ..
    } = get_nexus_objects(&conf)?;

    // Build the Sui client.
    let sui = build_sui_client(conf.sui.net).await?;

    let tools_handle = loading!("Fetching tools from the tool registry...");

    let tool_registry =
        match fetch_one::<Structure<ToolRegistry>>(&sui, tool_registry_object_id).await {
            Ok(tool_registry) => tool_registry.data.into_inner(),
            Err(e) => {
                tools_handle.error();

                return Err(NexusCliError::Any(e));
            }
        };

    let tools = match tool_registry.tools.fetch_all(&sui).await {
        Ok(tools) => tools,
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tools_handle.success();

    for (fqn, tool) in tools {
        println!(
            "    {arrow} Tool '{fqn}' at '{url}'",
            arrow = "▶".truecolor(100, 100, 100),
            fqn = fqn.to_string().truecolor(100, 100, 100),
            url = tool.into_inner().url.as_str().truecolor(100, 100, 100),
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct ToolRegistry {
    tools: ObjectBag<ToolFqn, Structure<Tool>>,
}

#[derive(Debug, Clone, Deserialize)]
struct Tool {
    #[serde(deserialize_with = "deserialize_bytes_to_url")]
    url: reqwest::Url,
}
