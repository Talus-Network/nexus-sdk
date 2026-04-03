use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
        tool::tool_validate::validate_off_chain_tool,
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        nexus::{client::NexusClient, error::NexusError},
        transactions::tool,
        types::ToolMeta,
    },
    std::io::Read as _,
};

/// Load `ToolMeta` from a file path or stdin (`-`), optionally overriding the `url` field.
///
/// This is used by `--from-meta` to bypass the live HTTP validation step. The
/// `output_schema["oneOf"]` invariant is still checked here so that invalid
/// meta is rejected before any on-chain transaction is attempted.
fn load_meta_from_source(
    source: &str,
    url_override: Option<reqwest::Url>,
) -> AnyResult<ToolMeta, NexusCliError> {
    let raw = if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| NexusCliError::Any(anyhow!("failed to read meta from stdin: {e}")))?;
        buf
    } else {
        std::fs::read_to_string(source)
            .map_err(|e| NexusCliError::Any(anyhow!("failed to read meta file '{source}': {e}")))?
    };

    let mut meta: ToolMeta = serde_json::from_str(&raw)
        .map_err(|e| NexusCliError::Any(anyhow!("failed to parse meta JSON: {e}")))?;

    if let Some(url) = url_override {
        meta.url = url.to_string();
    }

    // Validate that meta.url is a syntactically valid URL. The live-endpoint
    // path gets this for free via reqwest::Url parsing + an actual HTTP request;
    // the --from-meta path must check explicitly to prevent registering tools
    // with empty or malformed URLs on-chain.
    reqwest::Url::parse(&meta.url).map_err(|e| {
        NexusCliError::Any(anyhow!(
            "tool meta contains an invalid URL '{}': {e}",
            meta.url
        ))
    })?;

    if meta.output_schema["oneOf"].is_null() {
        return Err(NexusCliError::Any(anyhow!(
            "The tool meta does not contain a top-level 'oneOf' key. Please make sure to use an enum as the Tool output type."
        )));
    }

    Ok(meta)
}

/// Register a single tool from its already-validated `ToolMeta`.
///
/// Handles the "already registered" and "registration error" cases as non-fatal
/// results so that batch mode can continue to the next tool. Fatal errors
/// (e.g. missing OwnerCap in the response) are returned as `Err`.
///
/// On success, returns the JSON result and optionally the `(ToolFqn, ToolOwnerCaps)`
/// pair for the caller to persist in `CliConf`.
async fn register_one_tool(
    meta: ToolMeta,
    nexus_client: &NexusClient,
    grpc_client: std::sync::Arc<tokio::sync::Mutex<sui::grpc::Client>>,
    owner: sui::types::Address,
    collateral_coin: Option<sui::types::Address>,
    invocation_cost: u64,
) -> AnyResult<(serde_json::Value, Option<(ToolFqn, ToolOwnerCaps)>), NexusCliError> {
    let signer = nexus_client.signer();
    let gas_config = nexus_client.gas_config();
    let address = signer.get_active_address();
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let collateral_coin = fetch_coin(grpc_client, owner, collateral_coin, 1).await?;

    // Craft a TX to register the tool.
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::tx::TransactionBuilder::new();

    if let Err(e) = tool::register_off_chain_for_self(
        &mut tx,
        nexus_objects,
        &meta,
        address,
        &collateral_coin,
        invocation_cost,
    ) {
        tx_handle.error();
        return Err(NexusCliError::Any(e));
    }

    tx_handle.success();

    let mut gas_coin = gas_config.acquire_gas_coin().await;

    tx.set_sender(address);
    tx.set_gas_budget(gas_config.get_budget());
    tx.set_gas_price(nexus_client.get_reference_gas_price());

    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas_coin.object_id(),
        gas_coin.version(),
        *gas_coin.digest(),
    )]);

    let tx = tx.finish().map_err(|e| NexusCliError::Any(e.into()))?;

    let signature = signer.sign_tx(&tx).await.map_err(NexusCliError::Nexus)?;

    // Sign and submit the TX.
    let response = match signer.execute_tx(tx, signature, &mut gas_coin).await {
        Ok(response) => {
            gas_config.release_gas_coin(gas_coin).await;
            response
        }
        // If the tool is already registered, treat as a non-fatal result so
        // batch mode can continue to the next tool.
        Err(NexusError::Wallet(e)) if e.to_string().contains("register_off_chain_tool_") => {
            gas_config.release_gas_coin(gas_coin).await;

            notify_error!(
                "Tool '{fqn}' is already registered.",
                fqn = meta.fqn.to_string().truecolor(100, 100, 100)
            );

            return Ok((
                json!({
                    "tool_fqn": meta.fqn,
                    "already_registered": true,
                }),
                None,
            ));
        }
        // Any other error is also non-fatal for batch mode.
        Err(e) => {
            gas_config.release_gas_coin(gas_coin).await;

            notify_error!(
                "Failed to register tool '{fqn}': {error}",
                fqn = meta.fqn.to_string().truecolor(100, 100, 100),
                error = e
            );

            return Ok((
                json!({
                    "tool_fqn": meta.fqn,
                    "error": e.to_string(),
                }),
                None,
            ));
        }
    };

    // Parse the owner cap object IDs from the response.
    let owner_caps = response
        .objects
        .iter()
        .filter_map(|obj| {
            let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                return None;
            };

            if *object_type.address() == nexus_objects.primitives_pkg_id
                && *object_type.module() == primitives::OwnerCap::CLONEABLE_OWNER_CAP.module
                && *object_type.name() == primitives::OwnerCap::CLONEABLE_OWNER_CAP.name
            {
                Some((obj.object_id(), object_type))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    // Find `CloneableOwnerCap<OverTool>` object ID.
    let over_tool = owner_caps.iter().find_map(|(object_id, object_type)| {
        match object_type.type_params().first() {
            Some(sui::types::TypeTag::Struct(what_for))
                if *what_for.module() == workflow::ToolRegistry::OVER_TOOL.module
                    && *what_for.name() == workflow::ToolRegistry::OVER_TOOL.name =>
            {
                Some(object_id)
            }
            _ => None,
        }
    });

    let Some(over_tool_id) = over_tool else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the OwnerCap<OverTool> object ID in the transaction response."
        )));
    };

    // Find `CloneableOwnerCap<OverGas>` object ID.
    let over_gas = owner_caps.iter().find_map(|(object_id, object_type)| {
        match object_type.type_params().first() {
            Some(sui::types::TypeTag::Struct(what_for))
                if *what_for.module() == workflow::Gas::OVER_GAS.module
                    && *what_for.name() == workflow::Gas::OVER_GAS.name =>
            {
                Some(object_id)
            }
            _ => None,
        }
    });

    let Some(over_gas_id) = over_gas else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the OwnerCap<OverGas> object ID in the transaction response."
        )));
    };

    notify_success!(
        "OwnerCap<OverTool> object ID: {id}",
        id = over_tool_id.to_string().truecolor(100, 100, 100)
    );

    notify_success!(
        "OwnerCap<OverGas> object ID: {id}",
        id = over_gas_id.to_string().truecolor(100, 100, 100)
    );

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    let caps = ToolOwnerCaps {
        over_tool: *over_tool_id,
        over_gas: Some(*over_gas_id),
    };

    Ok((
        json!({
            "digest": response.digest,
            "tool_fqn": meta.fqn,
            "owner_cap_over_tool_id": over_tool_id,
            "owner_cap_over_gas_id": over_gas_id,
            "already_registered": false,
        }),
        Some((meta.fqn, caps)),
    ))
}

/// Validate and then register a new offchain Tool.
///
/// When `from_meta` is provided, the tool metadata is read from a file or stdin
/// instead of being fetched from a live HTTP endpoint. `url` overrides the URL
/// field in the metadata when both are provided. `batch` and `from_meta` are
/// mutually exclusive.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn register_off_chain_tool(
    url: Option<reqwest::Url>,
    from_meta: Option<String>,
    collateral_coin: Option<sui::types::Address>,
    invocation_cost: u64,
    batch: bool,
    no_save: bool,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    if collateral_coin.is_some() && collateral_coin == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "The coin used for collateral cannot be the same as the gas coin."
        )));
    }

    let conf = CliConf::load().await.unwrap_or_default();
    let client = build_sui_grpc_client(&conf).await?;
    let pk = get_signing_key(&conf).await?;
    let owner = pk.public_key().derive_address();
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;

    let mut registration_results = Vec::new();
    let mut caps_to_save: Vec<(ToolFqn, ToolOwnerCaps)> = Vec::new();

    if let Some(meta_source) = from_meta {
        // Load metadata from file/stdin without hitting a live HTTP endpoint.
        let meta = load_meta_from_source(&meta_source, url)?;

        command_title!(
            "Registering Tool '{fqn}' at '{url}'",
            fqn = meta.fqn,
            url = meta.url
        );

        let (result, caps) = register_one_tool(
            meta,
            &nexus_client,
            client,
            owner,
            collateral_coin,
            invocation_cost,
        )
        .await?;

        registration_results.push(result);
        caps_to_save.extend(caps);
    } else {
        // Live-endpoint path: require --url and optionally batch-discover tools.
        let url = url.expect(
            "--url is required when --from-meta is not provided (clap should enforce this)",
        );

        let urls = if batch {
            // Fetch all tools on the webserver.
            let response = reqwest::Client::new()
                .get(url.join("/tools").expect("Joining URL must be valid"))
                .send()
                .await
                .map_err(NexusCliError::Http)?
                .json::<Vec<String>>()
                .await
                .map_err(NexusCliError::Http)?;

            response
                .iter()
                .filter_map(|s| url.join(s).ok())
                .collect::<Vec<_>>()
        } else {
            vec![url]
        };

        for tool_url in urls {
            let meta = validate_off_chain_tool(tool_url).await?;

            command_title!(
                "Registering Tool '{fqn}' at '{url}'",
                fqn = meta.fqn,
                url = meta.url
            );

            let (result, caps) = register_one_tool(
                meta,
                &nexus_client,
                client.clone(),
                owner,
                collateral_coin,
                invocation_cost,
            )
            .await?;

            registration_results.push(result);
            caps_to_save.extend(caps);
        }
    }

    // Persist all owner caps in a single load+save cycle.
    if !no_save && !caps_to_save.is_empty() {
        let save_handle = loading!("Saving the owner caps to the CLI configuration...");

        let mut conf = CliConf::load().await.unwrap_or_default();
        for (fqn, caps) in caps_to_save {
            conf.tools.insert(fqn, caps);
        }

        if let Err(e) = conf.save().await {
            save_handle.error();
            return Err(NexusCliError::Any(e));
        }

        save_handle.success();
    }

    json_output(&registration_results)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: returns a valid `ToolMeta` JSON string with the given URL.
    fn valid_meta_json(url: &str) -> String {
        serde_json::json!({
            "fqn": "xyz.demo.tool@1",
            "url": url,
            "description": "A demo tool",
            "timeout": 5000,
            "input_schema": { "type": "object" },
            "output_schema": { "oneOf": [{ "type": "string" }] }
        })
        .to_string()
    }

    /// Verifies that `load_meta_from_source` correctly reads and deserializes a
    /// valid meta JSON file with all fields present.
    /// Guards against regressions in the happy-path file read + JSON parse pipeline.
    #[test]
    fn load_meta_from_file_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");
        std::fs::write(&path, valid_meta_json("https://example.com")).unwrap();

        let meta = load_meta_from_source(path.to_str().unwrap(), None).unwrap();

        assert_eq!(meta.fqn.to_string(), "xyz.demo.tool@1");
        assert_eq!(meta.url, "https://example.com");
        assert_eq!(meta.description, "A demo tool");
        assert_eq!(meta.timeout, std::time::Duration::from_millis(5000));
    }

    /// Verifies that `--url` override replaces the URL from the JSON file.
    /// Guards against the url_override branch being accidentally removed.
    #[test]
    fn load_meta_url_override_replaces_file_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");
        std::fs::write(&path, valid_meta_json("https://original.com")).unwrap();

        let override_url = reqwest::Url::parse("https://override.com").unwrap();
        let meta = load_meta_from_source(path.to_str().unwrap(), Some(override_url)).unwrap();

        assert_eq!(meta.url, "https://override.com/");
    }

    /// Verifies that when `url_override` is `None`, the file's URL is preserved.
    /// Guards against accidental URL clearing when no override is provided.
    #[test]
    fn load_meta_preserves_file_url_when_no_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");
        std::fs::write(&path, valid_meta_json("https://preserved.com")).unwrap();

        let meta = load_meta_from_source(path.to_str().unwrap(), None).unwrap();

        assert_eq!(meta.url, "https://preserved.com");
    }

    /// Verifies that an empty URL in the meta file is rejected.
    /// Guards against registering tools with empty URLs on-chain.
    #[test]
    fn load_meta_rejects_empty_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");
        std::fs::write(&path, valid_meta_json("")).unwrap();

        let err = load_meta_from_source(path.to_str().unwrap(), None).unwrap_err();
        assert!(err.to_string().contains("invalid URL"), "got: {err}");
    }

    /// Verifies that a malformed URL in the meta file is rejected.
    /// Guards against registering tools with non-URL strings on-chain.
    #[test]
    fn load_meta_rejects_malformed_url() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");
        std::fs::write(&path, valid_meta_json("not a url")).unwrap();

        let err = load_meta_from_source(path.to_str().unwrap(), None).unwrap_err();
        assert!(err.to_string().contains("invalid URL"), "got: {err}");
    }

    /// Verifies that a meta file without `output_schema.oneOf` is rejected.
    /// Guards against the oneOf validation being accidentally removed.
    #[test]
    fn load_meta_rejects_missing_one_of() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");

        let json = serde_json::json!({
            "fqn": "xyz.demo.tool@1",
            "url": "https://example.com",
            "description": "A demo tool",
            "timeout": 5000,
            "input_schema": { "type": "object" },
            "output_schema": { "type": "object" }
        })
        .to_string();

        std::fs::write(&path, json).unwrap();

        let err = load_meta_from_source(path.to_str().unwrap(), None).unwrap_err();
        assert!(err.to_string().contains("oneOf"), "got: {err}");
    }

    /// Verifies that malformed JSON is reported as a parse error.
    /// Guards against silent acceptance of non-JSON input.
    #[test]
    fn load_meta_rejects_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("meta.json");
        std::fs::write(&path, "{ not valid json }").unwrap();

        let err = load_meta_from_source(path.to_str().unwrap(), None).unwrap_err();
        assert!(
            err.to_string().contains("failed to parse meta JSON"),
            "got: {err}"
        );
    }

    /// Verifies that a non-existent file path produces an IO error.
    /// Guards against silent fallback when the file doesn't exist.
    #[test]
    fn load_meta_rejects_nonexistent_file() {
        let err = load_meta_from_source("/nonexistent/path/meta.json", None).unwrap_err();
        assert!(
            err.to_string().contains("failed to read meta file"),
            "got: {err}"
        );
    }
}
