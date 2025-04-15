# Tool Development Guidelines

This document will highlight some high-level guidelines for developing Nexus Tools.

These guidelines are not meant to be technical specifications but rather a set of best practices that will help you create a Tool that is easy to use and maintain.

## Interface Naming

- Names of Input Ports, Output Variants and Output Ports should be `snake_case`.
  - **dos**: `api_key`
  - **donts**: `apiKey`, `ApiKey`, `APIKey`
- Names of Input Ports and Output Variants should be descriptive and concise.
  - **dos**: `api_key`
  - **donts**: `key`, `k`, `apk`
- Names of erroneous Output Variants should start with `err`.
  - **dos**: `err`, `err_http`
  - **donts**: `error`, `failure`, `http_exception`

## Interface Design

### ... should be as generic as possible

- **dos**: Tool that encapsulates some API functinality like OpenAI chat completion with all its parameters.
- **donts**: Tool that encapsulates a specific API call like OpenAI chat completion with a specific prompt.

> Specific Tools are not reusable, they're created ad-hoc for a single DAG. Each Tool (or set of Tools) should be thought of as a library for a given API, allowing for a wide variaty of use cases.

### ... should keep the Nexus interface in mind

- **dos**: OpenAI chat competion Tool should have a separate Input Port for `prompt` and `context` even though the API request merges them together. This allows for default values to be set in the DAG.
- **donts**: OpenAI chat completion Tool that merges `prompt` and `context` into a single Input Port.

> The Nexus interface for constructing DAGs does not allow for a default value to be merged with the data from an incoming edge. If it makes sense for an Input Port to be used by both at the same time, it should be split into two separate Input Ports and then merged in the Tool logic.

### ... should be generic over its input if possible

- **dos**: HTTP request tool that accepts `json_schema` as an input and then validates the response against it, creating a variable output interface.
- **donts**: HTTP request tool that has hardcoded output schema and only serves 1 endpoint.

> The Workflow "doesn't care" about the structure of the data it receives, it simply passes the bytes onto the next Tool. Therefore, if a Tool has a variable output (like an HTTP request), it should be able to return any JSON as long as it's hinted in the `json_schema` Input Port. An example of this can be found [here][generic-port-example].

### ... should be as stable as possible, avoiding optional Output Ports on crucial data

- **dos**: Tweet reading Tool has `id` and `text` Output Ports that are _not_ optional and the Tool returns `err` Output Variant if they are not present in the response.
- **donts**: Tweet reading Tool has `id` and `text` Output Ports that are optional and the Tool returns `ok` Output Variant with these fields being `None` if they are not present in the response.

> Even though an Output Port _can_ be optional, it should not be overused. It can become very cumbersome if an Agent Developer is forced to check whether they did or did not receive the requested data from a Tool every time. If a Tool is not able to return the requested data, it should return an erroneous Output Variant instead.

### ... should be as flat as possible

- **dos**: Tweet reading tool has `id` and `text` Output Ports that are direct fields of the `ok` Output Variant
- **donts**: Tweet reading tool has `id` and `text` Output Ports that are nested inside a `response.tweet` field of the `ok` Output Variant.

> Every time a Tool Developer creates an Output Port, they should ask themselves: "Is this data usable as an Input Port of another Tool?". If a Tweet text is nested inside a `response.tweet` field, it cannot easily be passed into an Input Port of another Tool (like an LLM) because it is _very unlikely_ to accept a Twitter-specific object. Plain text, however, could be passed directly as a prompt to an LLM.

## Documentation

Each Nexus Tool should have a clear and publicly accessible README file that describes the Tool's purpose and input/output schemas.

If using Rust, the `main.rs` file should include this documentation via `#![doc = include_str!("../path/to/README.md")]`.

An example README file can be found [here](../tools/llm-openai-chat-completion/README.md).

<!-- List of References -->

[generic-port-example]: ../tools/llm-openai-chat-completion/src/main.rs#242
