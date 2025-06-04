# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [`0.2.0`] - Unreleased

### `nexus-cli`

#### Added

- `nexus gas add-budget` command to be able to pay for evaluations
- `nexus gas expiry enable` to enable the expiry gas extension for a tool
- `nexus gas expiry disable` to disable the expiry gas extension for a tool
- `nexus gas expiry buy-ticket` to buy an expiry gas ticket for a tool
- `nexus tool set-invocation-cost` to set the invocation cost for a tool
- `indicatif` crate to handle progress spinners
- `--batch` flag to `nexus tool register` command to allow registering multiple tools at once
- upon tool registration, the CLI will save the owner caps to the CLI conf; these are then used to fall back to if no `--owner-cap` arg is provided for some commands
- Added `--crypto.generate-identity-key` flag to `nexus conf set` command to generate and store a fresh identity key
- Added a `secrets` module that provides a wrapper to encrypt and decrypt its inner values
- `crypto` section to the CLI configuration to save the current state of the `crypto`
- `nexus conf set --sui.rpc-url` to set a custom Sui RPC URL for the CLI to use

#### Changed

- JSON DAG definition no longer specifies entry input ports
- renamed JSON DAG `vertices.input_ports` to `vertices.entry_ports`
- `nexus tool list` supports the new `description` and `registered_at_ms` attributes
- tool registration now takes `invocation_cost` parameter and returns 2 owner caps `OverTool` and `OverGas`
- `nexus conf --nexus.objects` is now the only way to populate the `nexus.objects` field in the config
- `nexus conf` changed to have `set` and `get` subcommands
- `nexus dag execute` now takes `--encrypt` argument that accepts `vertex.port` pairs to encrypt before sending data on-chain
- JSON DAG now accepts `encrypted` field on `edges.[].from`

#### Removed

- automated faucet calls for gas and collateral coins
- basic auth from the CLI configuration

#### Fixed

- `create_wallet_context` takes `SUI_RPC_URL` into consideration when checking active env
- when `nexus conf get` fails to parse the config it shows the error instead of defaulting

### `nexus-sdk`

#### Added

- Walrus Client module to interact with Walrus decentralized storage
- `transactions::gas` module containing PTB templates for gas-related transactions
- support for generating shell completions
- `crypto` module
- `x3dh` in `crypto` that implements X3DH key-exchange protocol according to the Signal Specs.
- `double_ratchet` in `crypto` that implements Double Ratchet with header encryption.
- `session` in `crypto` that glues together X3DH + Double Ratchet for a complete e2d Secure Session Layer.
- generic `secret` type interface for encrypting and decrypting wrapped values
- `transactions::crypto` module containing PTB templates for crypto-related transactions
- `idents::workflow::PreKeyVault` struct that contains pre key vault identifiers
- `pre_key_vault` key to `NexusObjects`
- pre key vault related Nexus events and their definitions

#### Changed

- `transactions::tool` register PTB template now accepts invocation cost
- all transaction templates now accept an `objects` argument instead of accepting objects one by one

#### Fixed

- `test_utils::contracts` now creates a `Move.lock` if it doesn't exist yet
- Fixed a bug that erases the current basic auth credentials from the config when any value is updated

### `nexus-toolkit-rust`

#### Added

- `/tools` endpoint to the `bootstrap!` macro that returns a list of all tools registered on the webserver

### `docs`

#### Added

- added markdown linter configuration
- added Github workflow for markdown linting and spellcheck actions
- added markdown style guide

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
