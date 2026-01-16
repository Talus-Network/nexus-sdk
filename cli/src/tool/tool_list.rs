use {
    crate::{command_title, display::json_output, item, loading, prelude::*, sui::*},
    nexus_sdk::{
        nexus::crawler::DynamicObjectMap,
        types::{
            deserialize_bytes_to_string,
            deserialize_bytes_to_url,
            deserialize_string_to_datetime,
        },
        ToolLocation,
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

        let (location, description, registered_at_ms, witness_id, input_schema) = match &tool {
            ToolVariant::OffChain(t) => (
                ToolLocation::from(t.url.clone()),
                t.description.clone(),
                t.registered_at_ms,
                None,
                None,
            ),
            ToolVariant::OnChain(t) => (
                ToolLocation::new_sui(&t.package_address, &t.module_name).map_err(|_| {
                    NexusCliError::Any(anyhow!(
                        "Invalid package address or module name in onchain tool"
                    ))
                })?,
                t.description.clone(),
                t.registered_at_ms,
                Some(t.witness_id.clone()),
                Some(t.input_schema.clone()),
            ),
        };

        let tool_type = if location.is_onchain() {
            "OnChain"
        } else {
            "OffChain"
        };

        // Build JSON output with common fields plus type-specific ones.
        let mut tool_json = json!({
            "fqn": fqn,
            "location": location.to_string(),
            "type": tool_type,
            "registered_at_ms": registered_at_ms,
            "description": description,
        });

        if let Some(wid) = &witness_id {
            tool_json["witness_id"] = json!(wid);
        }
        if let Some(schema) = &input_schema {
            tool_json["input_schema"] = json!(schema);
        }

        tools_json.push(tool_json);

        item!(
            "{tool_type} Tool '{fqn}' at '{location}' registered '{registered_at}' - {description}",
            tool_type = tool_type.truecolor(100, 100, 100),
            fqn = fqn.to_string().truecolor(100, 100, 100),
            location = location.to_string().truecolor(100, 100, 100),
            registered_at = registered_at_ms.to_string().truecolor(100, 100, 100),
            description = description.truecolor(100, 100, 100),
        );
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
