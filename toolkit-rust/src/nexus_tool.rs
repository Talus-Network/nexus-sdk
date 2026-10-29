//! See <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

use {
    anyhow::Result as AnyResult,
    nexus_sdk::ToolFqn,
    reqwest::Url,
    schemars::JsonSchema,
    serde::{de::DeserializeOwned, Serialize},
    serde_json::{json, Value},
    std::{future::Future, time::Duration},
    warp::http::StatusCode,
};

/// Authenticated request context available to tools when signed HTTP is enabled.
///
/// This context is produced after the runtime verifies the declared signed HTTP version, the
/// allowed Leader, and the canonical input commitment. V4 contexts include the authenticated
/// Tool ID; v3 contexts do not, because v3 does not sign one. The nonce cache is process-local.
///
/// Tools can use this context inside [`NexusTool::authorize`] to implement their own
/// admission policy (allowlists, task gating, rate-limits, etc) without any on-chain reads.
///
/// Terminology:
/// - Invoker: the node calling the tool (in Nexus, the Leader).
/// - Responder: the node serving the request (in Nexus, the Tool).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignedHttpVersion {
    /// The request used the v3 input-hash transcript, which does not authenticate a Tool ID.
    V3,
    /// The request used the v4 Tool-ID, nonce, and input-hash transcript.
    V4,
}

/// Authenticated request data passed to a Tool's authorization hook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthContext {
    /// Declared and verified signed HTTP protocol version.
    pub protocol_version: SignedHttpVersion,
    /// Leader identity supplied by the request and checked against the local allowlist.
    pub leader_id: String,
    /// Leader key identifier checked against the local allowlist.
    pub leader_key_id: u64,
    /// Authenticated Tool ID for v4, or `None` for v3.
    pub tool_id: Option<[u8; 32]>,
    /// Canonical resolved-input commitment authenticated by the Leader.
    pub input_hash: [u8; 32],
    /// Verified Leader signature bytes.
    pub leader_signature: [u8; 64],
    /// Invocation nonce carried by the request.
    pub nonce: [u8; 32],
}

impl AuthContext {
    pub(crate) fn from_v3(request: nexus_sdk::signed_http::v3::wire::AuthenticatedRequest) -> Self {
        Self {
            protocol_version: SignedHttpVersion::V3,
            leader_id: request.leader_id,
            leader_key_id: request.leader_key_id,
            tool_id: None,
            input_hash: request.input_hash,
            leader_signature: request.leader_signature,
            nonce: request.nonce,
        }
    }

    pub(crate) fn from_v4(request: nexus_sdk::signed_http::v4::wire::AuthenticatedRequest) -> Self {
        Self {
            protocol_version: SignedHttpVersion::V4,
            leader_id: request.leader_id,
            leader_key_id: request.leader_key_id,
            tool_id: Some(request.tool_id),
            input_hash: request.input_hash,
            leader_signature: request.leader_signature,
            nonce: request.nonce,
        }
    }
}

/// This trait defines the interface for a Nexus Tool. It forces implementation
/// of the following methods:
///
/// - `fqn`: Returns the tool fully qualified name.
/// - `invoke`: Invokes the tool with the given input.
/// - `health`: Returns the health status of the tool.
///
/// And the following associated types:
///
/// - `Input`: The input type of the tool.
/// - `Output`: The output type of the tool.
///
/// Based on the provided methods and associated types, the trait automatically
/// generates the following endpoints:
///
/// - `GET /health`: Returns the health status of the tool.
/// - `GET /meta`: Returns the metadata of the tool.
/// - `POST /invoke`: Invokes the tool with the given input.
///
/// The metadata of the tool includes the domain, name, version, input schema,
/// and output schema.
pub trait NexusTool: Send + Sync + 'static {
    /// The input type of the tool. It must implement `JsonSchema` and
    /// `DeserializeOwned`. It is used to generate the input schema of the tool.
    /// It is also used to deserialize the input payload.
    type Input: JsonSchema + DeserializeOwned + Send;
    /// The output type of the tool. It must implement `JsonSchema` and
    /// `Serialize`. It is used to generate the output schema of the tool. It is
    /// also used to serialize the output payload.
    ///
    /// **Important:** The output type must be a Rust `enum` so that a top-level
    /// `oneOf` is generated. This is to adhere to Nexus' output variants. This
    /// fact is validated by the CLI.
    type Output: JsonSchema + Serialize + Send;
    /// Returns the FQN of the Tool.
    fn fqn() -> ToolFqn;
    /// Returns the Tool timeout duration. Defaults to 10 seconds.
    ///
    /// The runtime applies this deadline to input decoding, construction,
    /// authorization, invocation, and output serialization. Expiry returns an
    /// unsigned HTTP 504 response.
    ///
    /// Cancellation cannot preempt synchronous work or undo external side effects.
    fn timeout() -> Duration {
        Duration::from_secs(10)
    }
    /// Invokes the tool with the given input. It is an asynchronous function
    /// that returns the output of the tool.
    ///
    /// It is used to generate the `/invoke` endpoint.
    fn invoke(&self, input: Self::Input) -> impl Future<Output = Self::Output> + Send;

    /// Encodes output ports before the runtime validates and signs them.
    /// Override this when an invocation uploads data through the SDK and returns
    /// explicit Walrus values. Storage policy and wallet authority belong to the
    /// tool operator; the toolkit does not upload data automatically.
    fn encode_output(output: Self::Output) -> AnyResult<nexus_sdk::types::OffchainToolOutput> {
        nexus_sdk::types::OffchainToolOutput::from_json(serde_json::to_value(
            crate::WithSerdeErrorPath(output),
        )?)
    }

    /// Authorize an invocation after it has been authenticated via signed HTTP.
    ///
    /// This is an optional ergonomic hook for tool developers to implement their
    /// own admission policy (allowlists, rate-limits, task gating, etc).
    ///
    /// Default: allow.
    ///
    /// # Example
    /// ```ignore
    /// async fn authorize(&self, ctx: AuthContext) -> AnyResult<()> {
    ///     // Example policy: only allow a specific LeaderId.
    ///     if ctx.invoker_id != "0x1111" {
    ///         anyhow::bail!("leader not allowed");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    fn authorize(&self, _ctx: AuthContext) -> impl Future<Output = AnyResult<()>> + Send {
        async { Ok(()) }
    }
    /// Returns the health status of the tool. For now, this only returns an
    /// HTTP status code.
    ///
    /// TODO: <https://github.com/Talus-Network/nexus-sdk/issues/7>
    ///
    /// It is used to generate the `/health` endpoint.
    fn health(&self) -> impl Future<Output = AnyResult<StatusCode>> + Send;
    /// Returns the relative path on a webserver that the tool resides on. This
    /// defaults to an empty path (root URL). But can be overridden by the
    /// implementor.
    fn path() -> &'static str {
        ""
    }
    /// Returns a concise description of the Tool's observable behavior.
    ///
    /// This value is part of the durable registry metadata that users and
    /// Agent developers rely on when selecting a Tool. Implementations must
    /// provide meaningful text. SDK registration rejects blank descriptions.
    fn description() -> &'static str;
    /// Construct a new instance of the tool. This is mainly here so that
    /// dependencies can be injected for testing purposes.
    fn new() -> impl Future<Output = Self> + Send;
    /// Returns the metadata of the tool. It includes the domain, name, version,
    /// input schema, and output schema.
    ///
    /// It is used to generate the `/meta` endpoint.
    fn meta(url: Url) -> Value {
        let fqn = Self::fqn();
        let url = url.to_string();
        let timeout = Self::timeout().as_millis() as u64;
        let description = Self::description();
        let input_schema = schemars::schema_for!(Self::Input);
        let output_schema = schemars::schema_for!(Self::Output);

        json!(
            {
                "fqn": fqn,
                "url": url,
                "timeout": timeout,
                "description": description,
                "input_schema": input_schema,
                "output_schema": output_schema,
            }
        )
    }
}
