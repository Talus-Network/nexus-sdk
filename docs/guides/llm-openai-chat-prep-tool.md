# Build the Missing Tool

This guide walks through the development of a tool that converts numbers into a message format compatible with the OpenAI chat completion tool. This is particularly useful when you want to use the results of mathematical operations from the [math tool](../../tools/math/README.md) as input for the [chat completion tool](../../tools/llm-openai-chat-completion/README.md).

{% hint style="info" %} Prerequisites
* Follow the [setup guide](setup.md) to make sure you've got the [Nexus CLI](../cli.md) installed.
* Read the [Toolkit Rust](../toolkit-rust.md) and [Tool development](../tool-development.md) docs that provide more context.
{% endhint %}

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

First, we need to define our input and output types (that will correspond to the `InputPort`s and `OutputVariant`s & `OutputPort`s). The input will take a number and an optional role (defaulting to "user"), and the output will be a message that the [Nexus standard library LLM chat completion tool](../../tools/llm-openai-chat-completion/README.md) can understand.

```rust
/// Input for the number to message conversion tool
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    /// The number to convert to a message
    number: i64,
    /// Optional role for the message (defaults to "user")
    #[serde(default)]
    role: Option<Role>,
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

where:

```rust
/// Represents the type of a message in a chat completion request or response.
#[derive(Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema, EnumString)]
#[serde(rename_all = "lowercase", try_from = "String")]
#[strum(ascii_case_insensitive)]
enum Role {
    #[default]
    User,
    System,
    Assistant,
}

/// Attempts to convert a String to a Role
impl TryFrom<String> for Role {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value
            .parse::<Role>()
            .map_err(|_| anyhow::anyhow!("Invalid Role: {}", value))
    }
}

/// A message that can be sent to the chat completion API
#[derive(Serialize, JsonSchema)]
struct Message {
    /// The role of the author of the message
    role: String,
    /// The content of the message
    value: String,
}
```

### 2. Implement the NexusTool Trait

Now we implement the `NexusTool` trait for our tool:

```rust
struct LlmOpenaiChatPrep;

impl NexusTool for LlmOpenaiChatPrep {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        // The constructor for the tool. This is where you can initialize any state.
        LlmOpenaiChatPrep
    }

    fn fqn() -> ToolFqn {
        // The fully qualified name of the tool.
        fqn!("xyz.taluslabs.llm-openai-chat-prep@1")
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        // The health endpoint should perform health checks on its dependencies.
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        // The role is unwrapped or defaulted to "user"
        let role = input.role.unwrap_or_default();

        let message = Message {
            role: role.to_string().to_lowercase(),
            value: input.number.to_string(),
        };

        Output::Ok { message }
    }
}
```

### 3. Add Tests

Make sure to add some test cases to your tool, although this is not covered in this guide.

### 4. Bootstrap the Tool

Finally, we bootstrap the tool using the `bootstrap!` macro:

```rust
#[tokio::main]
async fn main() {
    bootstrap!(([127, 0, 0, 1], 8080), LlmOpenaiChatPrep);
}
```

Note that if you want to specify the host at runtime with `BIND_ADDR` env variable, a different syntax can be used. Refer to [the toolkit doc](../toolkit-rust.md#nexus_toolkitbootstrap).

## Key Implementation Details

1. **Input Structure**:
   - `number`: The i64 number to convert
   - `role`: Optional role for the message (defaults to "user" via the `Role` enum's `Default` implementation)
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
   - The tool uses the `Role` enum to ensure only valid roles can be used
   - All error cases are handled gracefully with descriptive error messages
4. **Testing Strategy**:
   - Unit tests verify both default and custom role scenarios
   - Tests verify error handling for invalid roles
   - Tests check that error messages contain relevant information

## Creating the README

Every Nexus tool must include a README.md file that documents the tool's functionality, inputs, outputs, and provides usage examples. At minimum, the README should include:

1. A clear description of what the tool does
2. All input parameters with their types and descriptions
3. All output variants and ports with their types and descriptions
4. Error handling details
5. (Optional) Example usage in a DAG

<details>

<summary>Example README.md</summary>

## `xyz.taluslabs.llm-openai-chat-prep@1`

Standard Nexus Tool that converts a number into a message format compatible with the OpenAI chat completion tool. This is particularly useful when you want to use the results of mathematical operations as input for chat completion.

### Input

**`number`: \[`prim@i64`]**

The number to convert to a message. The tool will attempt to convert this number to a string representation. If the conversion fails, an error will be returned.

_opt_ **`role`: \[`Role`]** _default_: `Role::User`

The role for the message. Must be one of: `Role::User`, `Role::System`, or `Role::Assistant`. Defaults to `Role::User` if not specified.

### Output Variants & Ports

**`ok`**

The number was successfully converted to a message.

- **`ok.message.role`: \[`String`]** - The role of the message (user, system, or assistant).
- **`ok.message.value`: \[`String`]** - The string representation of the number.

**`err`**

The conversion failed due to an invalid input.

- **`err.reason`: \[`String`]** - The reason for the error. This will include details about what went wrong:
  - For invalid roles: "Invalid Role: {role}"
  - For number conversion failures: "Failed to validate number conversion: {error}"

### Error Handling

This tool handles the following error case:

**Invalid Role**: If the provided role is not one of the valid `Role` variants, the tool returns an `err` variant with a descriptive error message.

### Example Usage

This tool is typically used in a DAG to convert the output of a mathematical operation into a format that can be used as input for the chat completion tool. For example:

```json
{
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.math.i64.add@1"
      },
      "name": "add"
    },
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.llm.openai.chat-prep@1"
      },
      "name": "format"
    },
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.taluslabs.llm.openai.chat-completion@1"
      },
      "name": "chat",
      "entry_ports": ["api_key"]
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

Note that you'll have to add the chat completion api key still. It is recommended to use _entry ports_ for this.

</details>

## Build, Run, Register your Tool

1.  Build the tool:

    ```bash
    cargo build
    ```

2.  Start the tool server:

    ```bash
    cargo run
    ```

3.  Validate the tool:

    ```bash
    nexus tool validate --off-chain http://localhost:8080
    ```

4.  Register the tool:

    ```bash
    nexus tool register --off-chain http://localhost:8080
    ```

## Did you catch it?

The example provided here, while functional does not provide the optimal design according to the guidelines in [the Tool development guide](../tool-development.md). If you did not already think of this while going through the above setup, go through the best practices and see what you could improve about the Tool's design.

{% hint style="success" %}
Consider for what use cases you could use this tool to prepare it to add as an LLM chat completion prompt... is it widely useful or only in specific cases? How could you improve this?
{% endhint %}

## Next Steps

This tool provides a simple but essential bridge between mathematical operations and chat completion, enabling the creation of more sophisticated DAGs that combine numerical computation with natural language processing. Follow along with the developer guides to expand the [Math Branching Example DAG with LLM chat completion](math-branching-with-chat.md).
