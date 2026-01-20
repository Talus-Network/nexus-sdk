//! See <https://github.com/Talus-Network/gitbook-docs/blob/production/nexus-sdk/toolkit-rust.md>

use {
    anyhow::Result as AnyResult,
    nexus_sdk::ToolFqn,
    reqwest::Url,
    schemars::JsonSchema,
    serde::{de::DeserializeOwned, Serialize},
    serde_json::{json, Value},
    std::future::Future,
    warp::http::StatusCode,
};

/// Authenticated request context available to tools when signed HTTP is enabled.
///
/// This context is produced by the toolkit runtime after it has:
/// - verified the invoker's signature on the invocation request, and
/// - verified the request freshness window (`iat_ms`/`exp_ms`), and
/// - verified the request body hash, method/path/query binding, and tool id.
///
/// Tools can use this context inside [`NexusTool::authorize`] to implement their own
/// admission policy (allowlists, task gating, rate-limits, etc) without any on-chain reads.
///
/// Terminology:
/// - Invoker: the node calling the tool (in Nexus, the Leader).
/// - Responder: the node serving the request (in Nexus, the Tool).
pub type AuthContext = nexus_sdk::signed_http::v1::AuthContextV1;

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
    /// Invokes the tool with the given input. It is an asynchronous function
    /// that returns the output of the tool.
    ///
    /// It is used to generate the `/invoke` endpoint.
    fn invoke(&self, input: Self::Input) -> impl Future<Output = Self::Output> + Send;

    /// Authorize an invocation after it has been authenticated via signed HTTP.
    ///
    /// This is an optional ergonomic hook for tool developers to implement their
    /// own admission policy (allowlists, rate-limits, task gating, etc).
    ///
    /// Default: allow.
    ///
    /// # Example
    /// ```no_run
    /// use {
    ///     nexus_sdk::{fqn, ToolFqn},
    ///     nexus_toolkit::{AnyResult, AuthContext, NexusTool},
    ///     schemars::JsonSchema,
    ///     serde::{Deserialize, Serialize},
    ///     warp::http::StatusCode,
    /// };
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct Input {
    ///     prompt: String,
    /// }
    ///
    /// #[derive(Serialize, JsonSchema)]
    /// enum Output {
    ///     Ok { message: String },
    /// }
    ///
    /// struct MyTool;
    ///
    /// impl NexusTool for MyTool {
    ///     type Input = Input;
    ///     type Output = Output;
    ///
    ///     fn fqn() -> ToolFqn {
    ///         fqn!("example.my.tool@1")
    ///     }
    ///
    ///     async fn new() -> Self {
    ///         Self
    ///     }
    ///
    ///     fn authorize(&self, ctx: AuthContext) -> impl std::future::Future<Output = AnyResult<()>> + Send {
    ///         async move {
    ///             // Example policy: only allow a specific LeaderId.
    ///             if ctx.invoker_id != "0x1111" {
    ///                 anyhow::bail!("leader not allowed");
    ///             }
    ///             Ok(())
    ///         }
    ///     }
    ///
    ///     async fn invoke(&self, input: Self::Input) -> Self::Output {
    ///         Output::Ok { message: input.prompt }
    ///     }
    ///
    ///     async fn health(&self) -> AnyResult<StatusCode> {
    ///         Ok(StatusCode::OK)
    ///     }
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
    /// Returns the description of the tool. This defaults to an empty string.
    fn description() -> &'static str {
        ""
    }
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
        let description = Self::description();
        let input_schema = schemars::schema_for!(Self::Input);
        let output_schema = schemars::schema_for!(Self::Output);

        json!(
            {
                "fqn": fqn,
                "url": url,
                "description": description,
                "input_schema": input_schema,
                "output_schema": output_schema,
            }
        )
    }
}
