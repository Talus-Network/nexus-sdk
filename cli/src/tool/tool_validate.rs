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
async fn validate_off_chain_tool(url: reqwest::Url) -> AnyResult<ToolMeta, NexusCliError> {
    command_title!("Validating off-chain Tool at '{url}'");

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

            return Err(NexusCliError::AnyError(anyhow!(
                "The tool did not respond with a 200 OK status code"
            )));
        }
        Err(error) => {
            health_handle.error();

            return Err(NexusCliError::HttpError(error));
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

            return Err(NexusCliError::HttpError(error));
        }
    };

    let meta = match response.json::<ToolMeta>().await {
        Ok(meta) => meta,
        Err(error) => {
            meta_handle.error();

            return Err(NexusCliError::HttpError(error));
        }
    };

    // Check that meta has a top-level `oneOf`.
    if meta.output_schema["oneOf"].is_null() {
        meta_handle.error();

        return Err(NexusCliError::AnyError(anyhow!(
            "The tool meta does not contain a top-level 'oneOf' key. Please make sure to use an enum as the Tool output type."
        )));
    }

    // Check that URL is present and matches the provided.
    //
    // TODO: check that URL is reachable publicly somehow.
    if meta.url.parse::<reqwest::Url>() != Ok(url.clone()) {
        meta_handle.error();

        return Err(NexusCliError::AnyError(anyhow!(
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
