# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [`0.2.0`] - Unreleased

### `nexus-cli`

#### Changed

- JSON DAG definition no longer specifies entry input ports
- renamed JSON DAG `vertices.input_ports` to `vertices.entry_ports`

### `nexus-sdk`

#### Added

- added Walrus Client module to interact with Walrus decentralized storage

#### Fixed

- `test_utils::contracts` now creates a `Move.lock` if it doesn't exist yet
- Fixed a bug that erases the current basic auth credentials from the config when any value is updated

### `nexus-tool`

#### Added

- added the Walrus Client tools: JSON upload, File upload.

## [`0.1.0`] - 2025-04-14

### `nexus-cli`

#### Added

- commands to validate, register, unregister and claim collateral for Nexus Tools
- commands to scaffold a new Nexus Tool
- commands to validate, publish, execute and inspect DAGs
- commands to load and save configuration
- commands to create a new Nexus network
- release workflow
- added dev guides that showcase how to use CLI to publish and register tools, and publish and execute DAGs

#### Changed

- changing the notion of entry vertices to entry input ports and adjusting parsing, validation and PTB templates in accordance

#### Fixed

- fixing tool registration, unregistration and collateral claiming based on changes in tool registry

### `nexus-toolkit-rust`

#### Added

- added basic structure for Nexus Tools written in Rust in the form of a trait
- added a macro that starts a webserver for one or multiple tools, providing all necessary endpoints
- added a first, dumb version of secret manager
- added a dev guide that goes through the steps to use CLI to scaffold a boilerplate tool and implement NexusTool trait

### `nexus-sdk`

#### Added

- added Nexus Sui identifiers module
- added `object_crawler` that parses Sui objects to structs
- added `test_utils` that handle spinning up Redis or Sui containers for testing, along with some helper functions
- added `types` module and `tool_fqn` that holds some reusable types
- added `events` module that holds definitions of Nexus events fired from Sui
- added `sui` module that holds and categorizes all `sui_sdk` types

#### Fixed

- added implicit dependencies to `test_utils`
