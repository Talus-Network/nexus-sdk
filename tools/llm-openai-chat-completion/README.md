# `xyz.taluslabs.llm.openai.chat-completion@1`

Standard Nexus Tool that interacts with the OpenAI chat completion API. It allows for plain text completions as well as JSON. For JSON, expected output schema has to be provided.

It defines the input and output structures, as well as the logic for invoking the OpenAI API to generate chat completions.

It uses the [`async_openai`] crate to interact with the OpenAI API.

## Input

**`api_key`: [`String`]**

The API key to invoke the OpenAI API with.

**`prompt`: [`MessageBag`]**

The message(s) to send to the chat completion API. The minimum length of the vector is 1 if using [`MessageBag::Many`].

_opt_ **`context`: [`MessageBag`]** _default_: [`Vec::default`]

The context to provide to the chat completion API. This is useful for providing additional context as a DAG default value. Note that context messages are **prepended** to the `messages` input port.

_opt_ **`model`: [`String`]** _default_: [`DEFAULT_MODEL`]

The model to use for chat completion.

_opt_ **`max_completion_tokens`: [`u32`]** _default_: [`DEFAULT_MAX_COMPLETION_TOKENS`]

The maximum number of tokens to generate.

_opt_ **`temperature`: [`f32`]** _default_: [`DEFAULT_TEMPERATURE`]

The temperature to use. This must be a floating point number between 0 and 2. Defaults to 1.

_opt_ **`json_schema`: [`OpenAIJsonSchema`]** _default_: [`None`]

The JSON schema for the expected output. Providing this will force the [`Output::Json`] variant. The LLM response will be parsed into this schema. Note that this is only supported for newer OpenAI models. See <https://platform.openai.com/docs/guides/structured-outputs>.

## Output Variants & Ports

**`text`**

The chat completion was successful and evaluated to plain text.

- **`text.id`: [`String`]** - Unique identifier for the completion.
- **`text.role`: [`MessageKind`]** - The role of the author of the message.
- **`text.completion`: [`String`]** - The chat completion result as plain text.

**`json`**

The chat completion was successful and evaluated to JSON.

- **`json.id`: [`String`]** - Unique identifier for the completion.
- **`json.role`: [`MessageKind`]** - The role of the author of the message.
- **`json.completion`: [`serde_json::Value`]** - The chat completion result as JSON. Note that this is opaque for the Tool but the structure is defined by [`Input::json_schema`]. One could say the Tool output is _generic over this schema_.

**`err`**

An error occurred during the chat completion.

- **`err.reason`: [`String`]** - The reason for the error.
