use {
    crate::{command_title, loading, prelude::*},
    nexus_sdk::types::ToolMeta,
    reqwest::StatusCode,
    serde::Deserialize,
    std::time::Duration,
};

#[derive(Deserialize)]
struct ToolMetaDocument {
    fqn: ToolFqn,
    url: String,
    description: String,
    timeout: u64,
    input_schema: serde_json::Value,
    output_schema: serde_json::Value,
}

impl TryFrom<ToolMetaDocument> for ToolMeta {
    type Error = serde_json::Error;

    fn try_from(document: ToolMetaDocument) -> Result<Self, Self::Error> {
        Ok(Self {
            fqn: document.fqn,
            url: document.url,
            description: document.description,
            timeout: Duration::from_millis(document.timeout),
            input_schema: serde_json::to_vec(&document.input_schema)?,
            output_schema: serde_json::to_vec(&document.output_schema)?,
        })
    }
}

pub(crate) fn parse_tool_meta_json(raw: &str) -> anyhow::Result<ToolMeta> {
    let document = serde_json::from_str::<ToolMetaDocument>(raw)?;
    Ok(ToolMeta::try_from(document)?)
}

/// Validate an off-chain tool based on the provided URL.
pub(crate) async fn validate_off_chain_tool(
    url: reqwest::Url,
) -> AnyResult<ToolMeta, NexusCliError> {
    command_title!("Validating off-chain Tool at '{url}'");

    // Strip the trailing slash from the URL path.
    let path = match url.path().strip_suffix('/') {
        Some(path) => path,
        None => url.path(),
    };

    // Append the path to the base URL with a trailing slash.
    let full_path = format!("{path}/");
    let base_url = url
        .join(full_path.as_str())
        .expect("Joining URL must be valid");

    // Check health.
    let health_handle = loading!("Checking tool health...");

    let health_url = base_url
        .join("health")
        .expect("Appending health must be valid");

    match reqwest::Client::new().get(health_url).send().await {
        Ok(response) if response.status() == StatusCode::OK => (),
        Ok(_) => {
            health_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "The tool did not respond with a 200 OK status code"
            )));
        }
        Err(error) => {
            health_handle.error();

            return Err(NexusCliError::Http(error));
        }
    };

    health_handle.success();

    // Check meta.
    let meta_handle = loading!("Checking tool meta...");

    let meta_url = base_url.join("meta").expect("Appending meta must be valid");

    let response = match reqwest::Client::new().get(meta_url).send().await {
        Ok(response) => response,
        Err(error) => {
            meta_handle.error();

            return Err(NexusCliError::Http(error));
        }
    };

    let meta_text = match response.text().await {
        Ok(meta_text) => meta_text,
        Err(error) => {
            meta_handle.error();

            return Err(NexusCliError::Http(error));
        }
    };
    let meta = match parse_tool_meta_json(&meta_text) {
        Ok(meta) => meta,
        Err(error) => {
            meta_handle.error();

            return Err(NexusCliError::Any(anyhow!(
                "failed to parse tool meta JSON: {error}"
            )));
        }
    };

    // Check that meta has a top-level `oneOf`.
    if !output_schema_has_top_level_one_of(&meta).map_err(NexusCliError::Any)? {
        meta_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "The tool meta does not contain a top-level 'oneOf' key. Please make sure to use an enum as the Tool output type."
        )));
    }

    // TODO: <https://github.com/Talus-Network/nexus-sdk/issues/107>

    meta_handle.success();

    Ok(meta)
}

/// Validate an on-chain tool based on the provided ident.
pub(crate) async fn validate_on_chain_tool(_ident: String) -> AnyResult<ToolMeta, NexusCliError> {
    todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
}

pub(crate) fn output_schema_has_top_level_one_of(meta: &ToolMeta) -> anyhow::Result<bool> {
    let value = serde_json::from_slice::<serde_json::Value>(&meta.output_schema)?;
    Ok(value.get("oneOf").is_some_and(|one_of| !one_of.is_null()))
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_toolkit::*, schemars::JsonSchema, warp::http::StatusCode};

    // == Dummy tools setup ==

    #[derive(Debug, Deserialize, JsonSchema)]
    struct Input {
        prompt: String,
    }

    #[derive(Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
    enum Output {
        Ok { message: String },
    }

    struct DummyTool;

    impl NexusTool for DummyTool {
        type Input = Input;
        type Output = Output;

        async fn new() -> Self {
            Self
        }

        fn fqn() -> ToolFqn {
            fqn!("xyz.dummy.tool@1")
        }

        async fn health(&self) -> AnyResult<StatusCode> {
            Ok(StatusCode::OK)
        }

        async fn invoke(&self, Self::Input { prompt }: Self::Input) -> Self::Output {
            Self::Output::Ok {
                message: format!("You said: {}", prompt),
            }
        }
    }

    struct DummyToolWithPath;

    impl NexusTool for DummyToolWithPath {
        type Input = Input;
        type Output = Output;

        async fn new() -> Self {
            Self
        }

        fn fqn() -> ToolFqn {
            fqn!("xyz.dummy.tool@1")
        }

        fn path() -> &'static str {
            "/dummy/tool/"
        }

        async fn health(&self) -> AnyResult<StatusCode> {
            Ok(StatusCode::OK)
        }

        async fn invoke(&self, Self::Input { prompt }: Self::Input) -> Self::Output {
            Self::Output::Ok {
                message: format!("You said: {}", prompt),
            }
        }
    }

    #[tokio::test]
    async fn test_validate_oks_valid_off_chain_tools() {
        tokio::spawn(
            async move { bootstrap!(([127, 0, 0, 1], 8042), [DummyTool, DummyToolWithPath]) },
        );

        // Give the webserver some time to start.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // No path with slash
        let meta =
            validate_off_chain_tool(reqwest::Url::parse("http://localhost:8042/").unwrap()).await;
        assert!(meta.is_ok());
        let meta = meta.unwrap();
        assert_eq!(meta.fqn, fqn!("xyz.dummy.tool@1"));

        // No path no slash
        let meta =
            validate_off_chain_tool(reqwest::Url::parse("http://localhost:8042").unwrap()).await;
        assert!(meta.is_ok());
        let meta = meta.unwrap();
        assert_eq!(meta.fqn, fqn!("xyz.dummy.tool@1"));

        // Path with slash
        let meta = validate_off_chain_tool(
            reqwest::Url::parse("http://localhost:8042/dummy/tool/").unwrap(),
        )
        .await;
        assert!(meta.is_ok());
        let meta = meta.unwrap();
        assert_eq!(meta.fqn, fqn!("xyz.dummy.tool@1"));

        // Path no slash
        let meta = validate_off_chain_tool(
            reqwest::Url::parse("http://localhost:8042/dummy/tool").unwrap(),
        )
        .await;
        assert!(meta.is_ok());
        let meta = meta.unwrap();
        assert_eq!(meta.fqn, fqn!("xyz.dummy.tool@1"));
    }
}
