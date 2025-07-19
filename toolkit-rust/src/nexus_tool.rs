//! See <https://docs.talus.network/developer-docs/index-1/toolkit-rust>

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
