use {
    crate::{command_title, display::json_output, item, loading, prelude::*, sui::*},
    nexus_sdk::{
        nexus::crawler::DynamicObjectMap,
        types::{
            deserialize_bytes_to_string,
            deserialize_bytes_to_url,
            deserialize_string_to_datetime,
        },
    },
};

/// List tools available in the tool registry.
pub(crate) async fn list_tools() -> AnyResult<(), NexusCliError> {
    command_title!("Listing all available Neuxs tools");

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

    let tools = match crawler
        .get_dynamic_field_objects(&tool_registry.tools)
        .await
    {
        Ok(tools) => tools,
        Err(e) => {
            tools_handle.error();

            return Err(NexusCliError::Any(e));
        }
    };

    tools_handle.success();

    let mut tools_json = Vec::new();

    for (fqn, tool) in tools {
        let tool = tool.data;

        match tool {
            ToolVariant::OffChain(offchain_tool) => {
                tools_json.push(json!({
                    "fqn": fqn,
                    "url": offchain_tool.url,
                    "registered_at_ms": offchain_tool.registered_at_ms,
                    "description": offchain_tool.description,
                }));

                item!(
                    "OffChain Tool '{fqn}' at '{url}' registered '{registered_at}' - {description}",
                    fqn = fqn.to_string().truecolor(100, 100, 100),
                    url = offchain_tool.url.as_str().truecolor(100, 100, 100),
                    registered_at = offchain_tool
                        .registered_at_ms
                        .to_string()
                        .truecolor(100, 100, 100),
                    description = offchain_tool.description.truecolor(100, 100, 100),
                );
            }
            ToolVariant::OnChain(onchain_tool) => {
                tools_json.push(json!({
                    "fqn": fqn,
                    "package_address": onchain_tool.package_address,
                    "module_name": onchain_tool.module_name,
                    "witness_id": onchain_tool.witness_id,
                    "registered_at_ms": onchain_tool.registered_at_ms,
                    "description": onchain_tool.description,
                    "input_schema": onchain_tool.input_schema
                }));

                item!(
                    "OnChain Tool '{fqn}' at '{package}::{module}' registered '{registered_at}' - {description}",
                    fqn = fqn.to_string().truecolor(100, 100, 100),
                    package = onchain_tool.package_address.truecolor(100, 100, 100),
                    module = onchain_tool.module_name.truecolor(100, 100, 100),
                    registered_at = onchain_tool.registered_at_ms.to_string().truecolor(100, 100, 100),
                    description = onchain_tool.description.truecolor(100, 100, 100),
                );
            }
        }
    }

    json_output(&tools_json)?;

    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
struct ToolRegistry {
    tools: DynamicObjectMap<ToolFqn, ToolVariant>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ToolVariant {
    OffChain(OffChainTool),
    OnChain(OnChainTool),
}

#[derive(Debug, Clone, Deserialize)]
struct OffChainTool {
    #[serde(deserialize_with = "deserialize_bytes_to_url")]
    url: reqwest::Url,
    #[serde(deserialize_with = "deserialize_bytes_to_string")]
    description: String,
    #[serde(deserialize_with = "deserialize_string_to_datetime")]
    registered_at_ms: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize)]
struct OnChainTool {
    package_address: String,
    module_name: String,
    witness_id: String,
    #[serde(deserialize_with = "deserialize_bytes_to_string")]
    description: String,
    #[serde(deserialize_with = "deserialize_bytes_to_string")]
    input_schema: String,
    #[serde(deserialize_with = "deserialize_string_to_datetime")]
    registered_at_ms: chrono::DateTime<chrono::Utc>,
}
