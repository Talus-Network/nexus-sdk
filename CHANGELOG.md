# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [`nexus-cli` 0.2.0] - 2025-04-07

### Changed

- upgraded Sui from `testnet-1.38.1` to `mainnet-v1.45.3`
- changing the notion of entry vertices to entry input ports and adjusting parsing, validation and PTB templates in accordance

### Fixed

- fixing tool registration, unregistration and collateral claiming based on changes in tool registry

## [`nexus-toolkit-rust` 0.2.0] - 2025-04-07

### Changed

- upgraded Sui from `testnet-1.38.1` to `mainnet-v1.45.3`

## [`nexus-sdk` 0.2.0] - 2025-04-07

### Changed

- upgraded Sui from `testnet-1.38.1` to `mainnet-v1.45.3`

### Fixed

- added implicit dependencies to `test_utils`

## [`nexus-cli` 0.0.1] - 2025-03-28

### Added

- commands to validate, register, unregister and claim collateral for Nexus Tools
- commands to scaffold a new Nexus Tool
- commands to validate, publish, execute and inspect DAGs
- commands to load and save configuration
- commands to create a new Nexus network

## [`nexus-toolkit-rust` 0.0.1] - 2025-03-28

### Added

- added basic strcuture for Nexus Tools written in Rust in the form of a trait
- added a macro that starts a webserver for one or multiple tools, providing all necessary endpoints
- added a first, dumb version of secret manager

## [`nexus-sdk` 0.0.1] - 2025-03-28

### Added

- added Nexus Sui identifiers module
- added `object_crawler` that parses Sui objects to structs
- added `test_utils` that handle spinning up Redis or Sui containers for testing, along with some helper functions
- added `types` module and `tool_fqn` that holds some reusable types
- added `events` module that holds definitions of Nexus events fired from Sui
- added `sui` module that holds and categorizes all `sui_sdk` types

## [`xyz.taluslabs.math` 1] - 2025-03-28

### Added

- added support for comparing, adding and multiplying `i64` numbers

## [`xyz.taluslabs.llm-openai-chat-completion` 1] - 2025-03-28

### Added

- added support for OpenAI chat completion
