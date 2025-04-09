# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [`0.1.0`] - Unreleased

### `nexus-cli`

#### Added

- commands to validate, register, unregister and claim collateral for Nexus Tools
- commands to scaffold a new Nexus Tool
- commands to validate, publish, execute and inspect DAGs
- commands to load and save configuration
- commands to create a new Nexus network
- `nexus tool list` supports the new `description` and `registered_at_ms` attributes.

#### Changed

- changing the notion of entry vertices to entry input ports and adjusting parsing, validation and PTB templates in accordance

#### Fixed

- fixing tool registration, unregistration and collateral claiming based on changes in tool registry

### `nexus-toolkit-rust`

#### Added

- added basic strcuture for Nexus Tools written in Rust in the form of a trait
- added a macro that starts a webserver for one or multiple tools, providing all necessary endpoints
- added a first, dumb version of secret manager

### `nexus-sdk`

#### Added

- added Nexus Sui identifiers module
- added `object_crawler` that parses Sui objects to structs
- added `test_utils` that handle spinning up Redis or Sui containers for testing, along with some helper functions
- added `types` module and `tool_fqn` that holds some reusable types
- added `events` module that holds definitions of Nexus events fired from Sui
- added `sui` module that holds and categorizes all `sui_sdk` types
- Introduce `pub const CLOCK_OBJ_ARG` and use it where the Sui `Clock` is passed as an
  argument when submitting transactions.
- Introduce a `description` field each Tool needs to report via its `/meta` endpoint.
- Use the Tool-provided `description` when registering it on-chain.
- Introduce a lossy UTF-8 deserializer for the `description`, since the on-chain representation is a
  `vector<u8>`. Perhaps we can be stricter in the future.

#### Fixed

- added implicit dependencies to `test_utils`
