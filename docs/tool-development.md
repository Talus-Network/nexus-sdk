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

- Tools should be as _generic_ as possible
  - **dos**: Tool that encapsulates some API functinality like OpenAI chat completion with all its parameters.
  - **donts**: Tool that encapsulates a specific API call like OpenAI chat completion with a specific prompt.
- Keep the Nexus interface in mind
  - **dos**: OpenAI chat competion Tool should have a separate Input Port for `prompt` and `context` even though the API request merges them together. This allows for default values to be set in the DAG.
  - **donts**: OpenAI chat completion Tool that merges `prompt` and `context` into a single Input Port.
- If a Tool can be generic over its input, it should be
  - **dos**: HTTP request tool that accepts `json_schema` as an input and then validates the response against it, creating a variable output interface
  - **donts**: HTTP request tool that has hardcoded output schema and only serves 1 endpoint

## Documentation

Each Nexus Tool should have a clear and publicly accessible README file that describes the Tool's purpose and input/output schemas.

If using Rust, the `main.rs` file should include this documentation via `#![doc = include_str!("../path/to/README.md")]`.

An example README file can be found [here](../tools/llm-openai-chat-completion/README.md).
