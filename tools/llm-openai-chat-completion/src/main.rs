//! # `xyz.taluslabs.llm.openai.chat-completion@1`
//!
//! Standard Nexus Tool that interacts with the OpenAI chat completion API. It
//! allows for plain text completions as well as JSON. For JSON, expected output
//! schema has to be provided.
//!
//! It defines the input and output structures, as well as the logic for
//! invoking the OpenAI API to generate chat completions.
//!
//! It uses the [`async_openai`] crate to interact with the OpenAI API.
//!
//! ## Input
//!
//! - `api_key`: _encrypted_ [`String`] - The API key to invoke the OpenAI API
//!   with. Encrypted with the Tool's key pair.
//!   TODO: <https://github.com/Talus-Network/nexus-sdk/issues/29>.
//! - `prompt`: [`MessageBag`] - The message(s) to send to the chat completion
//!   API. The minimum length of the vector is 1 if using [`MessageBag::Many`].
//! - `context`: _optional_ [`MessageBag`] - The context to provide to the chat
//!   completion API. This is useful for providing additional context as a DAG
//!   default value. Defaults to [`Vec::default`]. Note that context messages
//!   are  **prepended** to the `messages` input port.
//! - `model`: _optional_ [`String`] - The model to use for chat completion.
//!   Defaults to [`DEFAULT_MODEL`].
//! - `max_completion_tokens`: _optional_ [`u32`] - The maximum number of tokens
//!   to generate. Defaults to [`DEFAULT_MAX_COMPLETION_TOKENS`].
//! - `temperature`: _optional_ [`f32`] - The temperature to use. This must be
//!   a floating point number between 0 and 2. Defaults to 1. Defaults to
//!   [`DEFAULT_TEMPERATURE`].
//! - `json_schema`: _optional_ [`OpenAIJsonSchema`] - The JSON schema for the
//!   expected output. Providing this will force the [`Output::Json`] variant.
//!   The LLM response will be parsed into this schema. Defaults to [`None`].
//!   Note that this is only supported for newer OpenAI models. See
//!   <https://platform.openai.com/docs/guides/structured-outputs>.
//!
//! ## Output Variants
//!
//! - `text` - The chat completion was successful and evaluated to plain text.
//! - `json` - The chat completion was successful and evaluated to JSON.
//! - `err` - An error occurred during the chat completion.
//!
//! ## Output Ports
//!
//! ### `text`
//!
//! - `id`: [`String`] - Unique identifier for the completion.
//! - `role`: [`MessageKind`] - The role of the author of the message.
//! - `completion`: [`String`] - The chat completion result as plain text.
//!
//! ### `json`
//!
//! - `id`: [`String`] - Unique identifier for the completion.
//! - `role`: [`MessageKind`] - The role of the author of the message.
//! - `completion`: [`serde_json::Value`] - The chat completion result as JSON.
//!   Note that this is opaque for the Tool but the structure is defined by
//!   [`Input::json_schema`]. One could say the Tool output is _generic over
//!   this schema_.
//!
//! ### `err`
//!
//! - `reason`: [`String`] - The reason for the error.

use {
    anyhow::anyhow,
    async_openai::{
        config::{OpenAIConfig, OPENAI_API_BASE},
        error::OpenAIError,
        types::{
            ChatCompletionRequestAssistantMessageArgs,
            ChatCompletionRequestMessage,
            ChatCompletionRequestSystemMessageArgs,
            ChatCompletionRequestUserMessageArgs,
            CreateChatCompletionRequestArgs,
            ResponseFormat,
            ResponseFormatJsonSchema,
            Role,
        },
        Client,
    },
    nexus_toolkit::*,
    nexus_types::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    strum_macros::EnumString,
};

mod status;

/// The default model to use for chat completions.
const DEFAULT_MODEL: &str = "gpt-4o-mini";
/// The maximum number of tokens to generate in a chat completion.
const DEFAULT_MAX_COMPLETION_TOKENS: u32 = 512;
/// The default temperature to use for chat completions.
const DEFAULT_TEMPERATURE: f32 = 1.0;

/// Represents a message that can be sent to the OpenAI Chat Completion API.
///
/// It can be either a full message with an explicit message type or a
/// short message, defaulting to the `User` type.
#[derive(Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(untagged)]
enum Message {
    /// A full message with an explicit message type and content and name.
    Full {
        /// The role of the author of the message.
        #[serde(default)]
        role: MessageKind,
        /// The content of the message.
        value: String,
        /// The name of the participant, this is used to differentiate between
        /// participants of the same role.
        name: Option<String>,
    },
    /// A short message, which defaults to the `User` message type.
    Short(String),
}

/// Attempts to convert a [`Message`] to an
/// [`async_openai::types::ChatCompletionRequestMessage`].
impl TryFrom<Message> for ChatCompletionRequestMessage {
    type Error = async_openai::error::OpenAIError;

    fn try_from(value: Message) -> Result<Self, Self::Error> {
        if let Message::Short(value) = value {
            return ChatCompletionRequestUserMessageArgs::default()
                .content(value)
                .build()
                .map(Into::into);
        }

        let Message::Full { role, value, name } = value else {
            unreachable!();
        };

        match role {
            MessageKind::System => {
                let mut message = ChatCompletionRequestSystemMessageArgs::default();

                if let Some(name) = name {
                    message.name(name);
                }

                message.content(value).build().map(Into::into)
            }
            MessageKind::User => {
                let mut message = ChatCompletionRequestUserMessageArgs::default();

                if let Some(name) = name {
                    message.name(name);
                }

                message.content(value).build().map(Into::into)
            }
            MessageKind::Assistant => {
                let mut message = ChatCompletionRequestAssistantMessageArgs::default();

                if let Some(name) = name {
                    message.name(name);
                }

                message.content(value).build().map(Into::into)
            }
            _ => unimplemented!("Tool and Function roles are not supported"),
        }
    }
}

/// Represents the type of a message in a chat completion request or response.
///
/// It can be `System`, `User`, or `Assistant`. When deserializing, the message
/// can be a string in which case this enum defaults to `User`.
///
/// This enum is case-insensitive.
#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema, EnumString)]
#[serde(rename_all = "lowercase", try_from = "String")]
#[strum(ascii_case_insensitive)]
enum MessageKind {
    #[default]
    User,
    System,
    Assistant,
    Tool,
    Funtion,
}

/// Attemps to convert a [`String`] to a [`MessageKind`].
impl TryFrom<String> for MessageKind {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value
            .parse::<MessageKind>()
            .map_err(|_| anyhow!("Invalid MessageKind: {}", value))
    }
}

/// Convert a [`async_openai::types::Role`] to a [`MessageKind`].
impl From<Role> for MessageKind {
    fn from(value: Role) -> Self {
        match value {
            Role::System => MessageKind::System,
            Role::User => MessageKind::User,
            Role::Assistant => MessageKind::Assistant,
            Role::Function => MessageKind::Funtion,
            Role::Tool => MessageKind::Tool,
        }
    }
}

/// Defines the structure of the `json_schema` input port.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenAIJsonSchema {
    /// The name of the schema. Must match `[a-zA-Z0-9-_]`, with a maximum
    /// length of 64.
    name: String,
    /// The JSON schema for the expected output.
    schema: schemars::Schema,
    /// A description of the response format, used by the model to determine
    /// how to respond in the format.
    description: Option<String>,
    /// Whether to enable strict schema adherence when generating the output. If
    /// set to true, the model will always follow the exact schema defined in the
    /// `schema` field. Only a subset of JSON Schema is supported when `strict`
    /// is `true`. See <https://platform.openai.com/docs/guides/structured-outputs>.
    strict: Option<bool>,
}

/// Allow the interface to accept a [`Vec`] or a single [`Message`].
#[derive(Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(untagged)]
enum MessageBag {
    One(Message),
    Many(Vec<Message>),
}

impl Default for MessageBag {
    fn default() -> Self {
        MessageBag::Many(Vec::default())
    }
}

impl From<MessageBag> for Vec<Message> {
    fn from(bag: MessageBag) -> Self {
        match bag {
            MessageBag::One(message) => vec![message],
            MessageBag::Many(messages) => messages,
        }
    }
}

/// Represents the input for the OpenAI chat completion Tool.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    /// The OpenAI API key.
    // TODO: <https://github.com/Talus-Network/nexus-sdk/issues/29>.
    api_key: String,
    /// The prompt to send to the chat completion API.
    prompt: MessageBag,
    /// The context to provide to the chat completion API.
    #[serde(default)]
    context: MessageBag,
    /// The model to use for chat completion.
    #[serde(default = "default_model")]
    model: String,
    /// The maximum number of tokens to generate.
    #[serde(default = "default_max_completion_tokens")]
    max_completion_tokens: u32,
    /// The temperature to use for chat completions.
    #[serde(default = "default_temperature")]
    temperature: f32,
    /// The JSON schema for the expected output.
    #[serde(default)]
    json_schema: Option<OpenAIJsonSchema>,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_max_completion_tokens() -> u32 {
    DEFAULT_MAX_COMPLETION_TOKENS
}

fn default_temperature() -> f32 {
    DEFAULT_TEMPERATURE
}

/// Represents the output of the OpenAI chat completion Tool.
#[derive(Debug, PartialEq, Eq, Serialize, JsonSchema)]
enum Output {
    Text {
        id: String,
        role: MessageKind,
        completion: String,
    },
    Json {
        id: String,
        role: MessageKind,
        completion: serde_json::Value,
    },
    Err {
        reason: String,
    },
}

/// The OpenAI Chat Completion tool.
///
/// This struct implements the `NexusTool` trait to integrate with the Nexus
/// framework. It provides the logic for invoking the OpenAI chat completion
/// API.
struct OpenaiChatCompletion {
    api_base: String,
}

impl NexusTool for OpenaiChatCompletion {
    type Input = Input;
    type Output = Output;

    fn new() -> Self {
        Self {
            api_base: OPENAI_API_BASE.to_string(),
        }
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.llm.openai.chat-completion@1")
    }

    /// Performs a health check on the Tool and its dependencies.
    async fn health(&self) -> AnyResult<StatusCode> {
        status::check_api_health().await
    }

    /// Invokes the tool logic to generate a chat completion.
    async fn invoke(&self, request: Self::Input) -> AnyResult<Self::Output> {
        let cfg = OpenAIConfig::new()
            .with_api_key(request.api_key)
            .with_api_base(&self.api_base);

        let client = Client::with_config(cfg);

        // Parse context messages into OpenAI message types.
        let context: Vec<Message> = request.context.into();
        let context = context.into_iter().map(TryInto::try_into);

        // Chain the input prompt and collect.
        let prompt: Vec<Message> = request.prompt.into();
        let messages = context
            .chain(prompt.into_iter().map(TryInto::try_into))
            .collect::<Result<Vec<ChatCompletionRequestMessage>, OpenAIError>>();

        // Should something go wrong, return an error. This is however very
        // unlikely as the inputs are validated against a schema defined here.
        let messages = match messages {
            Ok(messages) => messages,
            Err(err) => {
                return Ok(Output::Err {
                    reason: err.to_string(),
                })
            }
        };

        // Create the request to send to the OpenAI API and assign some basic
        // parameters.
        let mut openai_request = CreateChatCompletionRequestArgs::default();

        let mut openai_request = openai_request
            .max_completion_tokens(request.max_completion_tokens)
            .model(request.model)
            .temperature(request.temperature)
            .messages(messages);

        // If a JSON schema is provided, set it on the request.
        if let Some(schema) = request.json_schema.clone() {
            let json_schema = ResponseFormatJsonSchema {
                name: schema.name,
                schema: Some(schema.schema.to_value()),
                description: schema.description,
                strict: schema.strict,
            };

            openai_request =
                openai_request.response_format(ResponseFormat::JsonSchema { json_schema });
        }

        // Build the request and handle any errors.
        let openai_request = match openai_request.build() {
            Ok(request) => request,
            Err(err) => {
                return Ok(Output::Err {
                    reason: format!("Error building OpenAI request: {}", err),
                })
            }
        };

        let response = match client.chat().create(openai_request).await {
            Ok(response) => response,
            Err(err) => {
                return Ok(Output::Err {
                    reason: format!("Error calling OpenAI API: {}", err),
                })
            }
        };

        // Parse the response into the expected output format. Current Tool
        // design only supports a single choice so we take the first one.
        //
        // This design is also better for the Nexus interface as having a single
        // plaintext field with the completion is better suited.
        let choice = match response.choices.first() {
            Some(choice) => choice,
            None => {
                return Ok(Output::Err {
                    reason: "No choices returned from OpenAI API".to_string(),
                })
            }
        };

        if let Some(refusal) = &choice.message.refusal {
            return Ok(Output::Err {
                reason: refusal.to_string(),
            });
        }

        let completion = match &choice.message.content {
            Some(completion) => completion.to_string(),
            None => {
                return Ok(Output::Err {
                    reason: "No completion returned from OpenAI API".to_string(),
                })
            }
        };

        // Plain text completion.
        if request.json_schema.is_none() {
            return Ok(Output::Text {
                id: response.id,
                role: choice.message.role.into(),
                completion,
            });
        }

        // Parse the JSON completion into a serde_json::Value and validate it
        // against the provided schema.
        let completion = match serde_json::from_str(&completion) {
            Ok(completion) => completion,
            Err(err) => {
                return Ok(Output::Err {
                    reason: format!("Error parsing JSON completion: {}", err),
                })
            }
        };

        let Some(OpenAIJsonSchema { schema, .. }) = request.json_schema else {
            unreachable!();
        };

        match jsonschema::draft202012::validate(&schema.to_value(), &completion) {
            Ok(()) => Ok(Output::Json {
                id: response.id,
                role: choice.message.role.into(),
                completion,
            }),
            Err(e) => Ok(Output::Err {
                reason: format!("JSON completion does not match schema: {}", e),
            }),
        }
    }
}

/// The main entry point for the OpenAI Chat Completion tool.
///
/// This function bootstraps the tool and starts the server.
#[tokio::main]
async fn main() {
    bootstrap!(OpenaiChatCompletion)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        mockito::{Matcher, Server},
        schemars::schema_for,
        serde_json::json,
    };

    impl OpenaiChatCompletion {
        /// Creates a new instance of the OpenAI Chat Completion tool with the given
        /// API base URL.
        fn with_api_base(api_base: &str) -> Self {
            Self {
                api_base: api_base.to_string(),
            }
        }
    }

    #[test]
    fn test_message_kind_deserialization_case_insensitive() {
        let json = r#""system""#;
        let kind: MessageKind = serde_json::from_str(json).unwrap();
        assert_eq!(kind, MessageKind::System);

        let json = r#""USER""#;
        let kind: MessageKind = serde_json::from_str(json).unwrap();
        assert_eq!(kind, MessageKind::User);

        let json = r#""aSSiStaNt""#;
        let kind: MessageKind = serde_json::from_str(json).unwrap();
        assert_eq!(kind, MessageKind::Assistant);

        let json = r#""invalid""#;
        let kind: Result<MessageKind, _> = serde_json::from_str(json);
        assert!(kind.is_err());
    }

    #[test]
    fn test_message_kind_from_role() {
        let role = Role::System;
        let kind: MessageKind = role.into();
        assert_eq!(kind, MessageKind::System);

        let role = Role::User;
        let kind: MessageKind = role.into();
        assert_eq!(kind, MessageKind::User);

        let role = Role::Assistant;
        let kind: MessageKind = role.into();
        assert_eq!(kind, MessageKind::Assistant);

        let role = Role::Function;
        let kind: MessageKind = role.into();
        assert_eq!(kind, MessageKind::Funtion);

        let role = Role::Tool;
        let kind: MessageKind = role.into();
        assert_eq!(kind, MessageKind::Tool);
    }

    #[test]
    fn test_message_deserialization_full() {
        let json = r#"{"role": "system", "value": "Hello"}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(
            message,
            Message::Full {
                role: MessageKind::System,
                name: None,
                value: "Hello".to_string()
            }
        );

        let json = r#"{"role": "system", "name": "robot", "value": "Hello"}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(
            message,
            Message::Full {
                role: MessageKind::System,
                name: Some("robot".to_string()),
                value: "Hello".to_string()
            }
        );
    }

    #[test]
    fn test_message_deserialization_short() {
        let json = r#""Hello""#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(message, Message::Short("Hello".to_string()));
    }

    #[test]
    fn test_message_deserialization_default_type() {
        let json = r#"{"value": "Hello"}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(
            message,
            Message::Full {
                role: MessageKind::User,
                name: None,
                value: "Hello".to_string()
            }
        );
    }

    #[test]
    fn test_input_deserialization() {
        let json = r#"{
            "api_key": "your_api_key",
            "prompt": {"role": "system", "name": "robot", "value": "You are a helpful assistant."}
        }"#;
        let input: Input = serde_json::from_str(json).unwrap();

        assert!(
            matches!(input.prompt, MessageBag::One(Message::Full { role, value, name })
                if role == MessageKind::System
                    && value == "You are a helpful assistant."
                    && name == Some("robot".to_string())
            ),
        );

        let json = r#"{
            "api_key": "your_api_key",
            "prompt": "hello"
        }"#;
        let input: Input = serde_json::from_str(json).unwrap();

        assert!(
            matches!(input.prompt, MessageBag::One(Message::Short(s)) if s == "hello".to_string())
        );

        let json = r#"{
            "api_key": "your_api_key",
            "prompt": [
                {"role": "system", "name": "robot", "value": "You are a helpful assistant."},
                "Hello",
                {"role": "assistant", "value": "The Los Angeles Dodgers won the World Series in 2020."}
            ]
        }"#;
        let input: Input = serde_json::from_str(json).unwrap();
        assert_eq!(input.api_key, "your_api_key");
        assert_eq!(input.model, DEFAULT_MODEL);
        assert_eq!(
            input.prompt,
            MessageBag::Many(vec![
                Message::Full {
                    role: MessageKind::System,
                    name: Some("robot".to_string()),
                    value: "You are a helpful assistant.".to_string()
                },
                Message::Short("Hello".to_string()),
                Message::Full {
                    role: MessageKind::Assistant,
                    name: None,
                    value: "The Los Angeles Dodgers won the World Series in 2020.".to_string()
                }
            ])
        );
    }

    #[test]
    fn test_input_empty_message() {
        let json = r#"{
            "api_key": "your_api_key",
            "prompt": []
        }"#;
        let input: Input = serde_json::from_str(json).unwrap();
        assert_eq!(input.api_key, "your_api_key");
        assert_eq!(input.model, DEFAULT_MODEL);
        assert!(matches!(input.prompt, MessageBag::Many(messages) if messages.is_empty()));
    }

    #[test]
    fn test_input_missing_message() {
        let json = r#"{
            "api_key": "your_api_key"
        }"#;

        let input: Result<Input, _> = serde_json::from_str(json);

        assert!(input.is_err());
    }

    async fn create_server_and_tool() -> (mockito::ServerGuard, OpenaiChatCompletion) {
        let server = Server::new_async().await;

        let tool = OpenaiChatCompletion::with_api_base(
            format!("http://{}/v1", server.host_with_port()).as_str(),
        );

        (server, tool)
    }

    fn mock_response_body(completion: &str) -> String {
        json!({
            "id": "completion_id",
            "created": 1234567890,
            "model": DEFAULT_MODEL,
            "object": "chat.completion",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "system",
                        "content": completion,
                    }
                }
            ],
        })
        .to_string()
    }

    #[tokio::test]
    async fn test_minimal_config() {
        let (mut server, tool) = create_server_and_tool().await;

        let json = r#"{
            "api_key": "your_api_key",
            "prompt": "Hello"
        }"#;

        let input: Input = serde_json::from_str(json).unwrap();

        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer your_api_key")
            .match_body(Matcher::PartialJson(json!({
                "model": DEFAULT_MODEL,
                "max_completion_tokens": DEFAULT_MAX_COMPLETION_TOKENS,
                "temperature": DEFAULT_TEMPERATURE,
                "messages": [
                    {
                        "role": "user",
                        "content": "Hello"
                    }
                ]
            })))
            .with_body(mock_response_body("Hello, world!"))
            .create_async()
            .await;

        let output = tool.invoke(input).await.unwrap();

        assert_eq!(
            output,
            Output::Text {
                id: "completion_id".to_string(),
                role: MessageKind::System,
                completion: "Hello, world!".to_string()
            }
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_all_config() {
        let (mut server, tool) = create_server_and_tool().await;

        let json = r#"{
            "api_key": "your_api_key",
            "max_completion_tokens": 256,
            "temperature": 0.5,
            "model": "gpt-4o",
            "context": [
                {
                    "role": "system",
                    "name": "robot",
                    "value": "You are a helpful assistant."
                }
            ],
            "prompt": [
                {
                    "role": "assistant",
                    "value": "You are another helpful assistant."
                },
                "Hello"
            ]
        }"#;

        let input: Input = serde_json::from_str(json).unwrap();

        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer your_api_key")
            .match_body(Matcher::PartialJson(json!({
                "model": "gpt-4o",
                "max_completion_tokens": 256,
                "temperature": 0.5,
                "messages": [
                    {
                        "role": "system",
                        "name": "robot",
                        "content": "You are a helpful assistant."
                    },
                    {
                        "role": "assistant",
                        "content": "You are another helpful assistant."
                    },
                    {
                        "role": "user",
                        "content": "Hello"
                    }
                ]
            })))
            .with_body(mock_response_body("Hello, world!"))
            .create_async()
            .await;

        let output = tool.invoke(input).await.unwrap();

        assert_eq!(
            output,
            Output::Text {
                id: "completion_id".to_string(),
                role: MessageKind::System,
                completion: "Hello, world!".to_string()
            }
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_json_output() {
        let (mut server, tool) = create_server_and_tool().await;

        #[derive(PartialEq, Eq, Serialize, JsonSchema)]
        struct Completion {
            hello: String,
            world: String,
        }

        let schema = schema_for!(Completion);

        let json = json!({
            "api_key": "your_api_key",
            "json_schema": {
                "name": "completion",
                "description": "A completion JSON schema",
                "strict": true,
                "schema": schema
            },
            "prompt": "generate json"
        });

        let input: Input = serde_json::from_value(json).unwrap();

        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer your_api_key")
            .match_body(Matcher::PartialJson(json!({
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "completion",
                        "description": "A completion JSON schema",
                        "strict": true,
                        "schema": schema
                    }
                },
                "messages": [
                    {
                        "role": "user",
                        "content": "generate json"
                    }
                ]
            })))
            .with_body(mock_response_body(
                r#"{"hello": "world", "world": "hello"}"#,
            ))
            .create_async()
            .await;

        let output = tool.invoke(input).await.unwrap();

        let expected = Completion {
            hello: "world".to_string(),
            world: "hello".to_string(),
        };

        assert_eq!(
            output,
            Output::Json {
                id: "completion_id".to_string(),
                role: MessageKind::System,
                completion: serde_json::to_value(&expected).unwrap()
            }
        );

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_schema_mismatch() {
        let (mut server, tool) = create_server_and_tool().await;

        #[derive(PartialEq, Eq, Serialize, JsonSchema)]
        struct Completion {
            hello: String,
            world: String,
        }

        let schema = schema_for!(Completion);

        let json = json!({
            "api_key": "your_api_key",
            "json_schema": {
                "name": "completion",
                "description": "A completion JSON schema",
                "strict": true,
                "schema": schema
            },
            "prompt": "generate json"
        });

        let input: Input = serde_json::from_value(json).unwrap();

        let mock = server
            .mock("POST", "/v1/chat/completions")
            .match_header("authorization", "Bearer your_api_key")
            .match_body(Matcher::PartialJson(json!({
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": "completion",
                        "description": "A completion JSON schema",
                        "strict": true,
                        "schema": schema
                    }
                },
                "messages": [
                    {
                        "role": "user",
                        "content": "generate json"
                    }
                ]
            })))
            .with_body(mock_response_body(r#"{"a": "world", "b": "hello"}"#))
            .create_async()
            .await;

        let output = tool.invoke(input).await.unwrap();

        assert!(
            matches!(output, Output::Err {  reason} if reason.to_string().contains("JSON completion does not match schema"))
        );

        mock.assert_async().await;
    }
}
