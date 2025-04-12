# Developing a Number to Message Tool for Chat Completion

This guide walks through the development of a tool that converts numbers into a message format compatible with the OpenAI chat completion tool. This is particularly useful when you want to use the results of mathematical operations as input for chat completion.

## Project Setup

First, create a new Rust project for the tool using the Nexus CLI:

```bash
# Create a new tool project using the Rust template
nexus tool new --name llm-openai-chat-prep --template rust --target tools
cd tools/llm-openai-chat-prep
```

This command creates a new project with the following structure:
- `Cargo.toml` with the default dependencies
- `src/main.rs` with a basic tool implementation
- `README.md` for documentation

The generated `Cargo.toml` will include all default dependencies.

## Implementation

The generated `src/main.rs` file contains a template implementation. Let's modify it to implement our number-to-message conversion tool. We'll go through each part of the implementation:

### 1. Define Input and Output Types

First, we need to define our input and output types. The input will take a number and an optional role, and the output will be a message that the chat completion tool can understand.

```rust
/// Input for the number to message conversion tool
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    /// The number to convert to a message
    number: i64,
    /// Optional role for the message (defaults to "user")
    #[serde(default)]
    role: Option<String>,
}

/// A message that can be sent to the chat completion API
#[derive(Serialize, JsonSchema)]
struct Message {
    /// The role of the author of the message
    role: String,
    /// The content of the message
    value: String,
}

/// Output variants for the number to message conversion tool
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Output {
    /// Successfully converted number to message
    Ok {
        /// The message containing the converted number
        message: Message,
    },
    /// Error during conversion
    Err {
        /// The reason for the error
        reason: String,
    },
}
```

### 2. Implement the NexusTool Trait

Now we implement the `NexusTool` trait for our tool:

```rust
struct LlmOpenaiChatPrep;

impl NexusTool for LlmOpenaiChatPrep {
    type Input = Input;
    type Output = Output;

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.llm.openai.chat-prep.number-to-message@1")
    }

    fn url() -> Url {
        Url::parse("http://localhost:8080").unwrap()
    }

    async fn health() -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(input: Self::Input) -> AnyResult<Self::Output> {
        // Validate the role if provided
        if let Some(ref role) = input.role {
            if !["user", "system", "assistant"].contains(&role.as_str()) {
                return Ok(Output::Err {
                    reason: format!("Invalid role: {}. Must be one of: user, system, assistant", role),
                });
            }
        }

        let role = input.role.unwrap_or_else(|| "user".to_string());
        
        // Convert the number to a string message, handling potential conversion errors
        let value = match input.number.to_string() {
            Ok(s) => s,
            Err(e) => {
                return Ok(Output::Err {
                    reason: format!("Failed to convert number to string: {}", e),
                });
            }
        };

        let message = Message {
            role,
            value,
        };

        Ok(Output::Ok { message })
    }
}
```

### 3. Add Tests

Let's add some tests to verify our implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_to_message() {
        let input = Input {
            number: 42,
            role: None,
        };

        let output = futures::executor::block_on(LlmOpenaiChatPrep::invoke(input)).unwrap();

        match output {
            Output::Ok { message } => {
                assert_eq!(message.role, "user");
                assert_eq!(message.value, "42");
            }
            Output::Err { .. } => panic!("Expected Ok variant"),
        }
    }

    #[test]
    fn test_number_to_message_with_role() {
        let input = Input {
            number: 42,
            role: Some("system".to_string()),
        };

        let output = futures::executor::block_on(LlmOpenaiChatPrep::invoke(input)).unwrap();

        match output {
            Output::Ok { message } => {
                assert_eq!(message.role, "system");
                assert_eq!(message.value, "42");
            }
            Output::Err { .. } => panic!("Expected Ok variant"),
        }
    }

    #[test]
    fn test_number_to_message_with_invalid_role() {
        let input = Input {
            number: 42,
            role: Some("invalid".to_string()),
        };

        let output = futures::executor::block_on(LlmOpenaiChatPrep::invoke(input)).unwrap();

        match output {
            Output::Ok { .. } => panic!("Expected Err variant"),
            Output::Err { reason } => {
                assert!(reason.contains("Invalid role"));
                assert!(reason.contains("user, system, assistant"));
            }
        }
    }

    #[test]
    fn test_number_to_message_with_invalid_number() {
        // Note: This test is more theoretical since i64::to_string() doesn't actually fail
        // We keep it to document the error handling path
        let input = Input {
            number: i64::MAX,
            role: None,
        };

        let output = futures::executor::block_on(LlmOpenaiChatPrep::invoke(input)).unwrap();

        match output {
            Output::Ok { message } => {
                assert_eq!(message.role, "user");
                assert_eq!(message.value, i64::MAX.to_string());
            }
            Output::Err { .. } => panic!("Expected Ok variant"),
        }
    }
}
```

### 4. Bootstrap the Tool

Finally, we bootstrap the tool using the `bootstrap!` macro:

```rust
#[tokio::main]
async fn main() {
    bootstrap::<LlmOpenaiChatPrep>(([127, 0, 0, 1], 8080)).await;
}
```

## Key Implementation Details

1. **Input Structure**:
   - `number`: The i64 number to convert
   - `role`: Optional role for the message (defaults to "user")

2. **Output Structure**:
   - `Ok` variant with a `Message` containing:
     - `role`: The message role
     - `value`: The string representation of the number
   - `Err` variant with an error reason

3. **Error Handling**:
   - Following the Nexus Tool development guidelines:
     - Error variants are named with `err` prefix (e.g., `Err`)
     - Error messages are descriptive and include the invalid value
     - Error messages list valid options when applicable
     - Errors are returned as part of the output enum rather than using `Result`
   - The tool validates the role if provided, ensuring it's one of: "user", "system", "assistant"
   - The tool explicitly handles number-to-string conversion, returning an error if the conversion fails
   - All error cases are handled gracefully with descriptive error messages

4. **Testing Strategy**:
   - Unit tests verify both default and custom role scenarios
   - Tests ensure the correct string representation of numbers
   - Tests verify error handling for invalid roles
   - Tests document the error handling path for number conversion
   - Tests check that error messages contain relevant information

## Creating the README

Every Nexus tool must include a README.md file that documents the tool's functionality, inputs, outputs, and provides usage examples. At minimum, the README should include:

1. A clear description of what the tool does
2. All input parameters with their types and descriptions
3. All output variants and ports with their types and descriptions
4. Error handling details
5. Example usage in a DAG

<details>
<summary>Example README.md</summary>

# `xyz.taluslabs.llm.openai.chat-prep.number-to-message@1`

Standard Nexus Tool that converts a number into a message format compatible with the OpenAI chat completion tool. This is particularly useful when you want to use the results of mathematical operations as input for chat completion.

## Input

**`number`: [`prim@i64`]**

The number to convert to a message. The tool will attempt to convert this number to a string representation. If the conversion fails, an error will be returned.

_opt_ **`role`: [`String`]** _default_: `"user"`

The role for the message. Must be one of: `"user"`, `"system"`, or `"assistant"`. Defaults to `"user"` if not specified.

## Output Variants & Ports

**`ok`**

The number was successfully converted to a message.

- **`ok.message.role`: [`String`]** - The role of the message (user, system, or assistant).
- **`ok.message.value`: [`String`]** - The string representation of the number.

**`err`**

The conversion failed due to an invalid input.

- **`err.reason`: [`String`]** - The reason for the error. This will include details about what went wrong:
  - For invalid roles: "Invalid role: {role}. Must be one of: user, system, assistant"
  - For number conversion failures: "Failed to convert number to string: {error}"

## Error Handling

This tool handles the following error cases:

1. **Invalid Role**: If the provided role is not one of `"user"`, `"system"`, or `"assistant"`, the tool returns an `err` variant with a descriptive error message.

2. **Number Conversion Failure**: The tool explicitly attempts to convert the input number to a string representation. If this conversion fails for any reason, the tool returns an `err` variant with details about the conversion failure.

## Example Usage

This tool is typically used in a DAG to convert the output of a mathematical operation into a format that can be used as input for the chat completion tool. For example:

```json
{
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.math.i64.add@1"
      },
      "name": "add",
      "input_ports": ["a", "b"]
    },
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.llm.openai.chat-prep.number-to-message@1"
      },
      "name": "format",
      "input_ports": ["number", "role"]
    },
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.llm.openai.chat-completion@1"
      },
      "name": "chat",
      "input_ports": ["prompt", "api_key", "context"]
    }
  ],
  "edges": [
    {
      "from": {
        "vertex": "add",
        "output_variant": "ok",
        "output_port": "result"
      },
      "to": {
        "vertex": "format",
        "input_port": "number"
      }
    },
    {
      "from": {
        "vertex": "format",
        "output_variant": "ok",
        "output_port": "message"
      },
      "to": {
        "vertex": "chat",
        "input_port": "prompt"
      }
    }
  ],
  "default_values": [
    {
      "vertex": "add",
      "input_port": "a",
      "value": {
        "storage": "inline",
        "data": 5
      }
    },
    {
      "vertex": "add",
      "input_port": "b",
      "value": {
        "storage": "inline",
        "data": 7
      }
    },
    {
      "vertex": "format",
      "input_port": "role",
      "value": {
        "storage": "inline",
        "data": "system"
      }
    },
    {
      "vertex": "chat",
      "input_port": "context",
      "value": {
        "storage": "inline",
        "data": {
          "role": "system",
          "value": "You are a helpful assistant that explains numbers. Please explain the following number:"
        }
      }
    }
  ]
}
```
</details>

## Next Steps

1. Build the tool:
   ```bash
   cargo build
   ```

2. Start the tool server:
   ```bash
   cargo run
   ```

3. Validate the tool:
   ```bash
   nexus tool validate --off-chain http://localhost:8080
   ```

4. Register the tool:
   ```bash
   nexus tool register --off-chain http://localhost:8080
   ```

This tool provides a simple but essential bridge between mathematical operations and chat completion, enabling the creation of more sophisticated DAGs that combine numerical computation with natural language processing. Follow along with the developer guides to expand the [Math Branching Example DAG with chat completion](./math_branching_with_chat.md).