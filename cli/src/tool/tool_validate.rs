use {
    super::tool_output::ToolOutput,
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_success,
        prelude::*,
        sui::get_read_only_nexus_client,
    },
    nexus_sdk::{
        move_bindings::interface::meta_schema::MetaSchema,
        types::{OnchainToolMode, ToolMeta, ToolStateV2},
    },
    reqwest::StatusCode,
    serde::Deserialize,
    std::{path::Path, time::Duration},
};

const TOOL_TLS_ROOT_PEM_ENV: &str = "NEXUS_TOOL_TLS_ROOT_PEM_PATH";

pub(crate) fn build_tool_http_client() -> AnyResult<reqwest::Client, NexusCliError> {
    let root_path = std::env::var_os(TOOL_TLS_ROOT_PEM_ENV).map(std::path::PathBuf::from);
    build_tool_http_client_with_root(root_path.as_deref()).map_err(NexusCliError::Any)
}

fn build_tool_http_client_with_root(root_path: Option<&Path>) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    if let Some(path) = root_path {
        let pem = std::fs::read(path).map_err(|error| {
            anyhow!(
                "failed to read {TOOL_TLS_ROOT_PEM_ENV}={}: {error}",
                path.display()
            )
        })?;
        let certificates = reqwest::Certificate::from_pem_bundle(&pem).map_err(|error| {
            anyhow!(
                "invalid PEM in {TOOL_TLS_ROOT_PEM_ENV}={}: {error}",
                path.display()
            )
        })?;
        if certificates.is_empty() {
            anyhow::bail!(
                "invalid PEM in {TOOL_TLS_ROOT_PEM_ENV}={}: no certificates found",
                path.display()
            );
        }
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    builder
        .build()
        .map_err(|error| anyhow!("failed to initialize Tool HTTPS client: {error}"))
}

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
    let client = build_tool_http_client()?;
    validate_off_chain_tool_with_client(url, &client).await
}

pub(crate) fn output_validation(meta: &ToolMeta) -> AnyResult<(), NexusCliError> {
    notify_success!("Tool '{}' is valid.", meta.fqn);
    json_output(&validation_result_json(meta)?)
}

fn validation_result_json(meta: &ToolMeta) -> AnyResult<serde_json::Value, NexusCliError> {
    let timeout_ms = u64::try_from(meta.timeout.as_millis())
        .map_err(|_| NexusCliError::Any(anyhow!("Tool timeout exceeds u64 milliseconds")))?;
    let input_schema = serde_json::from_slice::<serde_json::Value>(&meta.input_schema)
        .map_err(|error| NexusCliError::Any(error.into()))?;
    let output_schema = serde_json::from_slice::<serde_json::Value>(&meta.output_schema)
        .map_err(|error| NexusCliError::Any(error.into()))?;

    Ok(json!({
        "valid": true,
        "fqn": meta.fqn,
        "url": meta.url,
        "description": meta.description,
        "timeout_ms": timeout_ms,
        "input_schema": input_schema,
        "output_schema": output_schema,
    }))
}

pub(crate) async fn validate_off_chain_tool_with_client(
    url: reqwest::Url,
    client: &reqwest::Client,
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

    match client.get(health_url).send().await {
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

    let response = match client.get(meta_url).send().await {
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

/// Validate a registered on-chain Tool against its live Move signature.
pub(crate) async fn validate_on_chain_tool(fqn: ToolFqn) -> AnyResult<ToolStateV2, NexusCliError> {
    command_title!("Validating on-chain Tool '{fqn}'");

    let nexus_client = get_read_only_nexus_client().await?;
    let inspection = nexus_client
        .tool()
        .inspect_tool(&fqn)
        .await
        .map_err(NexusCliError::Nexus)?;
    let tool = require_on_chain_tool(&fqn, inspection.tool).map_err(NexusCliError::Any)?;
    let (package_address, module_name, _) = tool
        .reference()
        .sui_parts()
        .map_err(NexusCliError::Any)?
        .expect("require_on_chain_tool accepted a Sui Tool reference");

    let grpc_client = nexus_client.grpc_client();
    let (input_schema, mode) =
        nexus_sdk::onchain_schema_gen::generate_input_schema_with_mode(
            grpc_client.clone(),
            package_address,
            &module_name,
            "execute",
        )
        .await
        .map_err(|error| {
            NexusCliError::Any(anyhow!(
                "Failed to resolve on-chain Tool '{fqn}' execute signature at {package_address}::{module_name}: {error}"
            ))
        })?;
    let output_schema = nexus_sdk::onchain_schema_gen::generate_output_schema(
        grpc_client,
        package_address,
        &module_name,
        "Output",
    )
    .await
    .map_err(|error| {
        NexusCliError::Any(anyhow!(
            "Failed to resolve on-chain Tool '{fqn}' Output signature at {package_address}::{module_name}: {error}"
        ))
    })?;
    let generated_meta_schema =
        MetaSchema::from_onchain_json_schemas(&input_schema, &output_schema).map_err(|error| {
            NexusCliError::Any(anyhow!(
                "Failed to normalize on-chain Tool '{fqn}' signature: {error}"
            ))
        })?;

    validate_on_chain_tool_signature(&fqn, &tool, &generated_meta_schema, mode)
        .map_err(NexusCliError::Any)?;
    Ok(tool)
}

fn require_on_chain_tool(fqn: &ToolFqn, tool: Option<ToolStateV2>) -> anyhow::Result<ToolStateV2> {
    let Some(tool) = tool else {
        bail!("On-chain Tool '{fqn}' does not exist")
    };
    if tool.reference().sui_parts()?.is_none() {
        bail!("Tool '{fqn}' is registered as an off-chain Tool, not an on-chain Tool");
    }
    Ok(tool)
}

fn validate_on_chain_tool_signature(
    fqn: &ToolFqn,
    tool: &ToolStateV2,
    generated_meta_schema: &MetaSchema,
    generated_mode: OnchainToolMode,
) -> anyhow::Result<()> {
    if &tool.meta_schema != generated_meta_schema {
        bail!("On-chain Tool '{fqn}' signature does not match its registered MetaSchema");
    }

    let configured_mode = if tool.workflow_authorization_cap_first {
        OnchainToolMode::WorkflowAuthorization
    } else {
        OnchainToolMode::Standard
    };
    if configured_mode != generated_mode {
        bail!(
            "On-chain Tool '{fqn}' configuration expects {configured_mode:?}, but its live execute signature uses {generated_mode:?}"
        );
    }
    Ok(())
}

pub(crate) fn output_on_chain_validation(tool: &ToolStateV2) -> AnyResult<(), NexusCliError> {
    let fqn = tool.parsed_fqn().map_err(NexusCliError::Any)?;
    notify_success!("Tool '{fqn}' is valid.");
    json_output(&on_chain_validation_result_json(tool)?)
}

fn on_chain_validation_result_json(
    tool: &ToolStateV2,
) -> AnyResult<serde_json::Value, NexusCliError> {
    let fqn = tool.fqn_string().map_err(NexusCliError::Any)?;
    let tool = ToolOutput::try_from_state(tool, None).map_err(NexusCliError::Any)?;
    Ok(json!({
        "valid": true,
        "fqn": fqn,
        "tool": tool,
    }))
}

pub(crate) fn output_schema_has_top_level_one_of(meta: &ToolMeta) -> anyhow::Result<bool> {
    let value = serde_json::from_slice::<serde_json::Value>(&meta.output_schema)?;
    Ok(value.get("oneOf").is_some_and(|one_of| !one_of.is_null()))
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        nexus_sdk::{
            move_bindings::{
                interface::meta_schema::{OutputVariantSchema, PortSchema, ValueKind},
                move_std::{ascii, option::Option as MoveOption},
                sui_framework::{balance::Balance, object::ID},
            },
            types::ToolRef,
        },
        nexus_toolkit::*,
        schemars::JsonSchema,
        warp::http::StatusCode,
    };

    // == Dummy tools setup ==

    #[test]
    fn validation_result_json_exposes_readable_metadata() {
        let meta = ToolMeta {
            fqn: "xyz.taluslabs.example@1".parse().unwrap(),
            url: "https://example.com/tool".to_owned(),
            description: "Example tool".to_owned(),
            timeout: Duration::from_millis(5_000),
            input_schema: br#"{"type":"object"}"#.to_vec(),
            output_schema: br#"{"oneOf":[]}"#.to_vec(),
        };

        let value = validation_result_json(&meta).expect("valid metadata should project");

        assert_eq!(
            value,
            serde_json::json!({
                "valid": true,
                "fqn": "xyz.taluslabs.example@1",
                "url": "https://example.com/tool",
                "description": "Example tool",
                "timeout_ms": 5_000,
                "input_schema": {"type": "object"},
                "output_schema": {"oneOf": []},
            })
        );
        assert!(!value.to_string().contains("\"bytes\""));
    }

    fn on_chain_meta_schema() -> MetaSchema {
        MetaSchema::new(
            vec![PortSchema::new(b"0".to_vec(), false, ValueKind::Data)],
            vec![OutputVariantSchema::new(
                b"ok".to_vec(),
                vec![PortSchema::new(b"result".to_vec(), false, ValueKind::Data)],
            )],
        )
    }

    fn on_chain_tool(workflow_authorization_cap_first: bool) -> ToolStateV2 {
        ToolStateV2 {
            minimum_protocol_version: 1,
            registry: ID::new(sui::types::Address::from_static("0x42")),
            fqn: ascii::String::from("xyz.taluslabs.example@1"),
            r#ref: ToolRef::Sui {
                package_address: sui::types::Address::from_static("0x1234"),
                module_name: ascii::String::from("example_tool"),
                tool_witness_id: ID::new(sui::types::Address::from_static("0x5678")),
            },
            description: b"Example on-chain Tool".to_vec(),
            meta_schema: on_chain_meta_schema(),
            verified: true,
            vault: Balance {
                value: 25,
                phantom_t0: std::marker::PhantomData,
            },
            workflow_authorization_cap_first,
            lock_duration_ms: 5_000,
            registered_at_ms: 0,
            unregistered_at_ms: MoveOption::from(None),
        }
    }

    #[test]
    fn on_chain_validation_rejects_nonexistent_tool() {
        let fqn = fqn!("xyz.taluslabs.example@1");

        let error = require_on_chain_tool(&fqn, None).unwrap_err();

        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn on_chain_validation_rejects_off_chain_tool() {
        let fqn = fqn!("xyz.taluslabs.example@1");
        let mut tool = on_chain_tool(false);
        tool.r#ref = ToolRef::Http {
            url: b"https://example.com/tool".to_vec(),
        };

        let error = require_on_chain_tool(&fqn, Some(tool)).unwrap_err();

        assert!(error
            .to_string()
            .contains("registered as an off-chain Tool"));
    }

    #[test]
    fn on_chain_validation_accepts_aligned_signature_and_configuration() {
        let fqn = fqn!("xyz.taluslabs.example@1");
        let tool = require_on_chain_tool(&fqn, Some(on_chain_tool(false))).unwrap();

        validate_on_chain_tool_signature(
            &fqn,
            &tool,
            &on_chain_meta_schema(),
            OnchainToolMode::Standard,
        )
        .unwrap();
    }

    #[test]
    fn on_chain_validation_rejects_meta_schema_mismatch() {
        let fqn = fqn!("xyz.taluslabs.example@1");
        let tool = on_chain_tool(false);
        let generated = MetaSchema::new(
            vec![PortSchema::new(
                b"different".to_vec(),
                false,
                ValueKind::Data,
            )],
            tool.meta_schema.output_variants.clone(),
        );

        let error =
            validate_on_chain_tool_signature(&fqn, &tool, &generated, OnchainToolMode::Standard)
                .unwrap_err();

        assert!(error.to_string().contains("registered MetaSchema"));
    }

    #[test]
    fn on_chain_validation_rejects_configuration_mismatch() {
        let fqn = fqn!("xyz.taluslabs.example@1");
        let tool = on_chain_tool(false);

        let error = validate_on_chain_tool_signature(
            &fqn,
            &tool,
            &on_chain_meta_schema(),
            OnchainToolMode::WorkflowAuthorization,
        )
        .unwrap_err();

        assert!(error.to_string().contains("configuration expects Standard"));
        assert!(error.to_string().contains("WorkflowAuthorization"));
    }

    #[test]
    fn on_chain_validation_result_exposes_semantic_tool_state() {
        let value = on_chain_validation_result_json(&on_chain_tool(false)).unwrap();

        assert_eq!(value["valid"], true);
        assert_eq!(value["fqn"], "xyz.taluslabs.example@1");
        assert_eq!(value["tool"]["reference"]["kind"], "sui");
        assert_eq!(value["tool"]["reference"]["module"], "example_tool");
        assert_eq!(
            value["tool"]["meta_schema"]["input_ports"][0]["port_name"],
            "0"
        );
        assert_eq!(value["tool"]["workflow_authorization_cap_first"], false);
        assert!(!value.to_string().contains("\"bytes\""));
    }

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

    #[test]
    fn tool_http_client_rejects_invalid_custom_root() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.pem");
        std::fs::write(&path, b"not a certificate").unwrap();

        let error = build_tool_http_client_with_root(Some(&path)).unwrap_err();
        assert!(error.to_string().contains("invalid PEM"));
        assert!(error.to_string().contains(path.to_string_lossy().as_ref()));
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
