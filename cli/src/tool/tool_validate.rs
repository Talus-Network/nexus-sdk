use {
    crate::{
        command_title,
        loading,
        prelude::*,
        tool::{ToolIdent, ToolMeta},
    },
    reqwest::StatusCode,
};

/// Validate either an off-chain or an on-chain tool.
pub(crate) async fn validate_tool(ident: ToolIdent) -> AnyResult<ToolMeta, NexusCliError> {
    match (ident.off_chain, ident.on_chain) {
        (Some(url), None) => validate_off_chain_tool(url).await,
        (None, Some(ident)) => validate_on_chain_tool(ident).await,
        _ => unreachable!("Checked by clap"),
    }
}

/// Validate an off-chain tool based on the provided URL.
async fn validate_off_chain_tool(mut url: reqwest::Url) -> AnyResult<ToolMeta, NexusCliError> {
    command_title!("Validating off-chain Tool at '{url}'");

    // Ensure that trailing slash exists.
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }

    // Check health.
    let health_handle = loading!("Checking tool health...");

    match reqwest::Client::new()
        .get(url.join("health").expect("Appending health must be valid"))
        .send()
        .await
    {
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

    let response = match reqwest::Client::new()
        .get(url.join("meta").expect("Appending meta must be valid"))
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            meta_handle.error();

            return Err(NexusCliError::Http(error));
        }
    };

    let meta = match response.json::<ToolMeta>().await {
        Ok(meta) => meta,
        Err(error) => {
            meta_handle.error();

            return Err(NexusCliError::Http(error));
        }
    };

    // Check that meta has a top-level `oneOf`.
    if meta.output_schema["oneOf"].is_null() {
        meta_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "The tool meta does not contain a top-level 'oneOf' key. Please make sure to use an enum as the Tool output type."
        )));
    }

    // Check that URL is present and matches the provided.
    //
    // TODO: check that URL is reachable publicly somehow.
    if meta.url != url.clone() {
        meta_handle.error();

        return Err(NexusCliError::Any(anyhow!(
            "The tool meta does not contain a 'url' key or it does not match the provided URL.\n\
            Found: {}\nExpected: {}",
            meta.url,
            url.as_str()
        )));
    }

    meta_handle.success();

    Ok(meta)
}

/// Validate an on-chain tool based on the provided ident.
async fn validate_on_chain_tool(_ident: String) -> AnyResult<ToolMeta, NexusCliError> {
    todo!("TODO: <https://github.com/Talus-Network/nexus-next/issues/96>")
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

        fn fqn() -> ToolFqn {
            fqn!("xyz.dummy.tool@1")
        }

        fn url() -> reqwest::Url {
            reqwest::Url::parse("http://localhost:8080").unwrap()
        }

        async fn health() -> AnyResult<StatusCode> {
            Ok(StatusCode::OK)
        }

        async fn invoke(Self::Input { prompt }: Self::Input) -> AnyResult<Self::Output> {
            Ok(Self::Output::Ok {
                message: format!("You said: {}", prompt),
            })
        }
    }

    #[tokio::test]
    async fn test_validate_oks_valid_off_chain_tool() {
        tokio::spawn(async move { bootstrap!(DummyTool) });

        let meta = validate_tool(ToolIdent {
            off_chain: Some(reqwest::Url::parse("http://localhost:8080").unwrap()),
            on_chain: None,
        })
        .await;

        assert!(meta.is_ok());

        let meta = meta.unwrap();

        assert_eq!(meta.fqn, fqn!("xyz.dummy.tool@1"));
    }
}
