//! This is a Nexus tool that interacts with the OpenAI Chat Completion API.
//!
//! It defines the input and output structures, as well as the logic for
//! invoking the OpenAI API to generate chat completions.
//!
//! It uses the `async-openai` crate to interact with the OpenAI API, and the
//! `nexus-toolkit-rust` crate to integrate as a Nexus tool.

use {
    async_openai::{
        config::OpenAIConfig,
        types::{
            ChatCompletionRequestAssistantMessageArgs,
            ChatCompletionRequestMessage,
            ChatCompletionRequestSystemMessageArgs,
            ChatCompletionRequestUserMessageArgs,
            CreateChatCompletionRequestArgs,
        },
        Client,
    },
    nexus_toolkit::*,
    nexus_types::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::str::FromStr,
    strum_macros::EnumString,
};

mod status;

/// The maximum number of tokens to generate in a chat completion.
const MAX_TOKENS: u32 = 512;
/// The default model to use for chat completions.
const DEFAULT_MODEL: &str = "gpt-4o-mini";

/// Represents a message that can be sent to the OpenAI Chat Completion API.
///
/// It can be either a full message with an explicit message type or a
/// short message, defaulting to the `User` type.
#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(untagged)]
enum Message {
    /// A full message with an explicit message type and content.
    Full {
        /// The type of the message.
        #[serde(default, rename = "type")]
        message_type: MessageType,
        /// The content of the message.
        value: String,
    },
    /// A short message, which defaults to the `User` message type.
    Short(String),
}

/// Represents the type of a message in a chat completion.
///
/// It can be `System`, `User`, or `Assistant`. This enum supports
/// case-insensitive deserialization from JSON and provides a default
/// value of `User`.
#[derive(Debug, Deserialize, JsonSchema, EnumString, Default, PartialEq)]
#[strum(ascii_case_insensitive)]
#[serde(rename_all = "lowercase", try_from = "String")]
enum MessageType {
    /// A system message, used to set the behavior of the assistant.
    System,
    #[default]
    /// A user message, representing a message from the user.
    User,
    /// An assistant message, representing a message from the assistant.
    Assistant,
}

impl TryFrom<String> for MessageType {
    type Error = String;

    /// Converts a `String` to a `MessageType`, performing a case-insensitive
    /// match.
    ///
    /// # Arguments
    ///
    /// * `value` - The string to convert.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `MessageType` if the conversion is
    /// successful, or an error string if the conversion fails.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        MessageType::from_str(&value).map_err(|_| format!("Invalid MessageType: {}", value))
    }
}

/// Represents the input for the OpenAI Chat Completion tool.
///
/// It defines the structure for the input data required to invoke the tool,
/// including the API key, the model to use, and the messages to send.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    // TODO: Add encryption to the API key.
    /// The OpenAI API key.
    pub api_key: String,
    /// The model to use for chat completion.
    /// If not specified, the [DEFAULT_MODEL] will be used.
    #[serde(default = "default_model")]
    pub model: String,
    /// The messages to send to the chat completion API.
    pub messages: Vec<Message>,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

/// Represents a single response choice from the OpenAI Chat Completion API.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    /// The role of the author of the message.
    pub role: String,
    /// The content of the message.
    pub message: String,
}

/// Represents the output of the OpenAI Chat Completion tool.
///
/// It defines the structure for the output data returned by the tool,
/// including the list of response choices.
#[derive(Serialize, JsonSchema)]
enum Output {
    Ok { choices: Vec<Response> },
    Err { reason: String },
}

/// The OpenAI Chat Completion tool.
///
/// This struct implements the `NexusTool` trait to integrate with the Nexus
/// framework. It provides the logic for invoking the OpenAI Chat Completion
/// API and handling the input and output data.
struct OpenaiChatCompletion;

impl NexusTool for OpenaiChatCompletion {
    type Input = Input;
    type Output = Output;

    /// Returns the fully qualified name of the tool.
    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.llm.openai.chat-completion@1")
    }

    /// Returns the URL of the tool.
    fn url() -> Url {
        Url::parse("http://localhost:8080").unwrap()
    }

    /// Performs a health check on the tool's dependencies.
    ///
    /// # Returns
    ///
    /// A `Result` containing the HTTP status code indicating the health of
    /// the tool.
    async fn health() -> AnyResult<StatusCode> {
        status::check_api_health().await
    }

    /// Invokes the tool logic to generate a chat completion.
    ///
    /// # Arguments
    ///
    /// * `request` - The input data for the tool.
    ///
    /// # Returns
    ///
    /// A `Result` containing the output of the tool. Possible output variants are:
    ///
    /// *   `Ok(Output::Ok { choices: Vec<Response> })`: The API call was successful, and the response contains a list of choices.
    /// *   `Ok(Output::Err { reason: String })`: An error occurred. The `reason` contains a description of the error.
    async fn invoke(request: Self::Input) -> AnyResult<Self::Output> {
        let cfg = OpenAIConfig::new().with_api_key(request.api_key);
        let client = Client::with_config(cfg);

        let message_results: Vec<Result<ChatCompletionRequestMessage, String>> = request
            .messages
            .into_iter()
            .map(|m| match m {
                Message::Full {
                    message_type,
                    value,
                } => match message_type {
                    MessageType::System => ChatCompletionRequestSystemMessageArgs::default()
                        .content(value)
                        .build()
                        .map(|m| m.into())
                        .map_err(|e| e.to_string()),
                    MessageType::User => ChatCompletionRequestUserMessageArgs::default()
                        .content(value)
                        .build()
                        .map(|m| m.into())
                        .map_err(|e| e.to_string()),
                    MessageType::Assistant => ChatCompletionRequestAssistantMessageArgs::default()
                        .content(value)
                        .build()
                        .map(|m| m.into())
                        .map_err(|e| e.to_string()),
                },
                Message::Short(value) => ChatCompletionRequestUserMessageArgs::default()
                    .content(value)
                    .build()
                    .map(|m| m.into())
                    .map_err(|e| e.to_string()),
            })
            .collect();

        let messages: Result<Vec<ChatCompletionRequestMessage>, String> =
            message_results.into_iter().collect();

        let messages = match messages {
            Ok(messages) => messages,
            Err(err) => return Ok(Output::Err { reason: err }),
        };

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(MAX_TOKENS)
            .model(request.model)
            .messages(messages)
            .build();

        if request.is_err() {
            return Ok(Output::Err {
                reason: format!("Error building request: {}", request.err().unwrap()),
            });
        }
        let request = request.unwrap();

        let response = match client.chat().create(request).await {
            Ok(response) => response,
            Err(err) => {
                return Ok(Output::Err {
                    reason: format!("Error calling OpenAI API: {}", err),
                })
            }
        };

        let mut choices = Vec::with_capacity(response.choices.len());
        for choice in response.choices.into_iter() {
            let role = choice.message.role.to_string();
            let message = if let Some(msg) = choice.message.content {
                msg.to_string()
            } else {
                return Ok(Output::Err {
                    reason: "Invalid message in API response".to_string(),
                });
            };

            choices.push(Response { role, message });
        }

        Ok(Output::Ok { choices })
    }
}

/// The main entry point for the OpenAI Chat Completion tool.
///
/// This function bootstraps the tool and starts the server.
#[tokio::main]
async fn main() {
    bootstrap::<OpenaiChatCompletion>(([127, 0, 0, 1], 8080)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_type_deserialization_case_insensitive() {
        let json = r#""system""#;
        let message_type: MessageType = serde_json::from_str(json).unwrap();
        assert_eq!(message_type, MessageType::System);

        let json = r#""USER""#;
        let message_type: MessageType = serde_json::from_str(json).unwrap();
        assert_eq!(message_type, MessageType::User);

        let json = r#""aSsIsTaNt""#;
        let message_type: MessageType = serde_json::from_str(json).unwrap();
        assert_eq!(message_type, MessageType::Assistant);
    }

    #[test]
    fn test_message_deserialization_full() {
        let json = r#"{"type": "system", "value": "Hello"}"#;
        let message: Message = serde_json::from_str(json).unwrap();
        assert_eq!(
            message,
            Message::Full {
                message_type: MessageType::System,
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
                message_type: MessageType::User,
                value: "Hello".to_string()
            }
        );
    }

    #[test]
    fn test_input_deserialization() {
        let json = r#"{
            "apiKey": "your_api_key",
            "messages": [
                {"type": "system", "value": "You are a helpful assistant."},
                "Hello",
                {"type": "assistant", "value": "The Los Angeles Dodgers won the World Series in 2020."}
            ]
        }"#;
        let input: Input = serde_json::from_str(json).unwrap();
        assert_eq!(input.api_key, "your_api_key");
        assert_eq!(input.model, DEFAULT_MODEL);
        assert_eq!(
            input.messages,
            vec![
                Message::Full {
                    message_type: MessageType::System,
                    value: "You are a helpful assistant.".to_string()
                },
                Message::Short("Hello".to_string()),
                Message::Full {
                    message_type: MessageType::Assistant,
                    value: "The Los Angeles Dodgers won the World Series in 2020.".to_string()
                }
            ]
        );
    }

    #[test]
    fn test_input_empty_message() {
        let json = r#"{
            "apiKey": "your_api_key",
            "messages": []
        }"#;
        let input: Input = serde_json::from_str(json).unwrap();
        assert_eq!(input.api_key, "your_api_key");
        assert_eq!(input.model, DEFAULT_MODEL);
        assert!(input.messages.is_empty());
    }

    #[test]
    fn test_input_missing_message() {
        let json = r#"{
            "apiKey": "your_api_key"
        }"#;

        let input: Result<Input, _> = serde_json::from_str(json);

        assert!(input.is_err());
    }
}
