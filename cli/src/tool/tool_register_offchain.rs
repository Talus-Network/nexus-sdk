//! Registers off-chain tools with client-owned collateral selection.

use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
        tool::tool_validate::{
            build_tool_http_client,
            output_schema_has_top_level_one_of,
            parse_tool_meta_json,
            validate_off_chain_tool_with_client,
        },
    },
    clap::{Args, ValueEnum},
    nexus_sdk::{
        move_bindings::{
            primitives::owner_cap::CloneableOwnerCap,
            struct_tag_matches,
            tool::{tool_authority::OverTool, tool_cashier::OverToolCashier},
        },
        nexus::{client::NexusClient, registry::preflight_external_verifier_registration},
        transactions::tool::{self, ToolVerifierContractInput},
        types::{NexusContext, ToolMeta},
    },
    std::io::Read as _,
};

/// Verifier contract stored in a new off chain Tool definition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
enum ToolVerifierKind {
    /// Store no result verifier.
    #[default]
    None,
    /// Use keys registered through the Network Auth root.
    RegisteredKey,
    /// Use one public Move verifier and its immutable shared objects.
    External,
}

/// Complete immutable verifier selection for Tool registration.
#[derive(Args, Clone, Debug)]
pub(crate) struct ToolVerifierArgs {
    /// Verifier contract stored in the Tool definition.
    #[arg(long = "verifier", value_enum, default_value_t = ToolVerifierKind::None)]
    kind: ToolVerifierKind,

    /// Package that publishes an external verifier.
    #[arg(long = "verifier-package", value_name = "PACKAGE_ID")]
    package: Option<sui::types::Address>,

    /// Module that contains an external verifier.
    #[arg(long = "verifier-module", value_name = "MODULE")]
    module: Option<sui::types::Identifier>,

    /// Public external verifier function.
    #[arg(long = "verifier-function", value_name = "FUNCTION")]
    function: Option<sui::types::Identifier>,

    /// Ordered immutable shared objects required by an external verifier.
    #[arg(long = "verifier-object", value_name = "OBJECT_ID")]
    objects: Vec<sui::types::Address>,
}

impl ToolVerifierArgs {
    async fn resolve(
        &self,
        client: &NexusClient,
        context: &NexusContext,
    ) -> AnyResult<ToolVerifierContractInput, NexusCliError> {
        let has_external_input = self.package.is_some()
            || self.module.is_some()
            || self.function.is_some()
            || !self.objects.is_empty();

        match self.kind {
            ToolVerifierKind::None => {
                if has_external_input {
                    return Err(NexusCliError::Any(anyhow!(
                        "External verifier arguments require '--verifier external'"
                    )));
                }
                Ok(ToolVerifierContractInput::None)
            }
            ToolVerifierKind::RegisteredKey => {
                if has_external_input {
                    return Err(NexusCliError::Any(anyhow!(
                        "External verifier arguments cannot be used with '--verifier registered-key'"
                    )));
                }
                Ok(ToolVerifierContractInput::RegisteredKey)
            }
            ToolVerifierKind::External => {
                let package = self.package.ok_or_else(|| {
                    NexusCliError::Any(anyhow!(
                        "'--verifier-package' is required for an external verifier"
                    ))
                })?;
                let module = self.module.as_ref().ok_or_else(|| {
                    NexusCliError::Any(anyhow!(
                        "'--verifier-module' is required for an external verifier"
                    ))
                })?;
                let function = self.function.as_ref().ok_or_else(|| {
                    NexusCliError::Any(anyhow!(
                        "'--verifier-function' is required for an external verifier"
                    ))
                })?;
                let input = preflight_external_verifier_registration(
                    client.crawler(),
                    context,
                    package,
                    module.as_str(),
                    function.as_str(),
                    &self.objects,
                )
                .await
                .map_err(NexusCliError::Any)?;
                Ok(ToolVerifierContractInput::External(input))
            }
        }
    }
}

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

    let mut meta = parse_tool_meta_json(&raw)
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

    if !output_schema_has_top_level_one_of(&meta).map_err(NexusCliError::Any)? {
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
    context: &NexusContext,
    verifier_contract: &ToolVerifierContractInput,
    collateral_coin: Option<sui::types::Address>,
    invocation_cost: u64,
) -> AnyResult<(serde_json::Value, Option<(ToolFqn, ToolOwnerCaps)>), NexusCliError> {
    if let Some(result) =
        super::tool_inspect::inspect_registration_result(nexus_client, &meta.fqn).await?
    {
        notify_success!(
            "Tool '{fqn}' is already registered.",
            fqn = meta.fqn.to_string().truecolor(100, 100, 100)
        );
        return Ok((result, None));
    }

    let address = nexus_client.owner().map_err(NexusCliError::Nexus)?;
    let nexus_objects = nexus_client.get_nexus_objects();
    let collateral_coin = nexus_client
        .fetch_coin_by_type(collateral_coin, 0, nexus_objects.us_token.coin_type_tag())
        .await
        .map_err(NexusCliError::Nexus)?;

    // Craft a TX to register the tool.
    let tx_handle = loading!("Crafting transaction...");

    let tx = match tool::register_off_chain_for_self_ptb(
        context,
        &meta,
        verifier_contract,
        address,
        &collateral_coin,
        invocation_cost,
    ) {
        Ok(tx) => tx,
        Err(e) => {
            tx_handle.error();
            return Err(NexusCliError::Any(e));
        }
    };

    tx_handle.success();

    // Sign and submit the TX.
    let response = match nexus_client.submit_transaction(tx, address).await {
        Ok(response) => response,
        Err(e) => {
            if let Ok(Some(result)) =
                super::tool_inspect::inspect_registration_result(nexus_client, &meta.fqn).await
            {
                notify_success!(
                    "Tool '{fqn}' is already registered.",
                    fqn = meta.fqn.to_string().truecolor(100, 100, 100)
                );
                return Ok((result, None));
            }

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
    let owner_caps: Vec<(sui::types::Address, sui::types::StructTag)> = response
        .objects
        .iter()
        .filter_map(|obj| {
            let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
                return None;
            };

            if struct_tag_matches::<CloneableOwnerCap<OverTool>>(context, &object_type) {
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
                if struct_tag_matches::<OverTool>(context, what_for.as_ref()) =>
            {
                Some(*object_id)
            }
            _ => None,
        }
    });

    let Some(over_tool_id) = over_tool else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the OwnerCap<OverTool> object ID in the transaction response."
        )));
    };

    // Find `CloneableOwnerCap<OverToolCashier>` object ID.
    let cashier_admin = owner_caps.iter().find_map(|(object_id, object_type)| {
        match object_type.type_params().first() {
            Some(sui::types::TypeTag::Struct(what_for))
                if struct_tag_matches::<OverToolCashier>(context, what_for.as_ref()) =>
            {
                Some(*object_id)
            }
            _ => None,
        }
    });

    let Some(cashier_admin_id) = cashier_admin else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the OwnerCap<OverToolCashier> object ID in the transaction response."
        )));
    };

    notify_success!(
        "OwnerCap<OverTool> object ID: {id}",
        id = over_tool_id.to_string().truecolor(100, 100, 100)
    );

    notify_success!(
        "OwnerCap<OverToolCashier> object ID: {id}",
        id = cashier_admin_id.to_string().truecolor(100, 100, 100)
    );

    notify_success!(
        "Transaction digest: {digest}",
        digest = response.digest.to_string().truecolor(100, 100, 100)
    );

    let caps = ToolOwnerCaps {
        over_tool: over_tool_id,
        cashier_admin: Some(cashier_admin_id),
    };

    // Re-fetch the freshly-registered Tool object so the JSON output carries
    // the same shape `nexus tool inspect` emits. Consumers only need to
    // learn one Tool contract.
    let inspection = nexus_client
        .tool()
        .inspect_tool(&meta.fqn)
        .await
        .map_err(NexusCliError::Nexus)?;
    let result = super::tool_inspect::registration_submission_result_json(
        &inspection,
        &response.digest,
        response.checkpoint,
        over_tool_id,
        Some(cashier_admin_id),
    )?;

    Ok((result, Some((meta.fqn, caps))))
}

/// Ensures every [`register_one_tool`] result represents success.
///
/// Batch registration still attempts every tool, then returns an error so
/// scripts cannot mistake partial registration for success.
fn ensure_registrations_succeeded(results: &[serde_json::Value]) -> AnyResult<(), NexusCliError> {
    let failure_count = results
        .iter()
        .filter(|result| result.get("error").is_some())
        .count();

    match failure_count {
        0 => Ok(()),
        1 => Err(NexusCliError::Any(anyhow!("1 tool registration failed"))),
        count => Err(NexusCliError::Any(anyhow!(
            "{count} tool registrations failed"
        ))),
    }
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
    verifier: ToolVerifierArgs,
    batch: bool,
    no_save: bool,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let nexus_objects = nexus_client.get_nexus_objects();
    let context = nexus_client
        .context_for_root(&nexus_objects.tool_registry)
        .await
        .map_err(NexusCliError::Nexus)?;
    let verifier_contract = verifier.resolve(&nexus_client, &context).await?;

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
            &context,
            &verifier_contract,
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
        let tool_http_client = build_tool_http_client()?;

        let urls = if batch {
            // Fetch all tools on the webserver.
            let response = tool_http_client
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
            let meta = validate_off_chain_tool_with_client(tool_url, &tool_http_client).await?;

            command_title!(
                "Registering Tool '{fqn}' at '{url}'",
                fqn = meta.fqn,
                url = meta.url
            );

            let (result, caps) = register_one_tool(
                meta,
                &nexus_client,
                &context,
                &verifier_contract,
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

    ensure_registrations_succeeded(&registration_results)
}

#[cfg(test)]
mod tests {
    use {super::*, clap::Parser};

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

    /// Verifies that an unsuccessful transaction makes the command fail.
    #[test]
    fn registration_results_reject_transaction_errors() {
        let results = vec![json!({
            "tool_fqn": "xyz.demo.tool@1",
            "error": "transaction rejected",
        })];

        let error = ensure_registrations_succeeded(&results).unwrap_err();

        assert_eq!(error.to_string(), "1 tool registration failed");
    }

    /// Verifies that repeated registration remains successful.
    #[test]
    fn registration_results_accept_already_registered_tools() {
        let results = vec![json!({
            "tool_fqn": "xyz.demo.tool@1",
            "already_registered": true,
        })];

        ensure_registrations_succeeded(&results).unwrap();
    }

    // -- Clap constraint tests --

    /// Verifies that `--url` is not required when `--from-meta` is provided.
    /// Guards against the `required_unless_present` constraint being removed.
    #[test]
    fn clap_accepts_from_meta_without_url() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "register",
            "offchain",
            "--from-meta",
            "meta.json",
        ])
        .is_ok());
    }

    /// Verifies that `--url` is required when `--from-meta` is absent.
    /// Guards against `required_unless_present` being accidentally removed.
    #[test]
    fn clap_rejects_offchain_without_url_or_from_meta() {
        assert!(crate::Cli::try_parse_from(["nexus", "tool", "register", "offchain",]).is_err());
    }

    /// Verifies that `--from-meta` and `--batch` cannot be used together.
    /// Guards against the `conflicts_with` constraint being removed.
    #[test]
    fn clap_rejects_from_meta_with_batch() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "register",
            "offchain",
            "--from-meta",
            "meta.json",
            "--batch",
        ])
        .is_err());
    }

    /// Verifies that `--from-meta` and `--url` can be used together (URL override).
    /// Guards against an accidental `conflicts_with` between the two.
    #[test]
    fn clap_accepts_from_meta_with_url_override() {
        assert!(crate::Cli::try_parse_from([
            "nexus",
            "tool",
            "register",
            "offchain",
            "--from-meta",
            "meta.json",
            "--url",
            "https://override.example.com",
        ])
        .is_ok());
    }
}
