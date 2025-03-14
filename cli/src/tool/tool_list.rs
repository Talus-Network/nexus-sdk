use {
    crate::{command_title, loading, prelude::*, sui::*},
    nexus_sdk::{
        object_crawler::{fetch_one, ObjectBag, Structure},
        types::{deserialize_bytes_to_json_value, deserialize_bytes_to_url},
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

    let tr = fetch_one::<Structure<ToolRegistry>>(&sui, tool_registry_object_id)
        .await
        .unwrap()
        .data
        .into_inner();

    let tls = tr.tools.fetch_all(&sui).await.unwrap();

    println!("{:#?}", tls);

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
    #[serde(deserialize_with = "deserialize_bytes_to_json_value")]
    input_schema: serde_json::Value,
    #[serde(deserialize_with = "deserialize_bytes_to_json_value")]
    output_schema: serde_json::Value,
}
