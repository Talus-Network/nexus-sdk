# Nexus SDK

[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/Talus-Network/nexus-sdk/blob/main/LICENSE)
[![Actions](https://img.shields.io/badge/GitHub_Actions-Active-brightgreen)](https://github.com/Talus-Network/nexus-sdk/actions)
[![codecov](https://codecov.io/gh/Talus-Network/nexus-sdk/graph/badge.svg?token=Q9I01BXJSE)](https://codecov.io/gh/Talus-Network/nexus-sdk)

The **Nexus SDK** is a collection of tools that simplifies building with **Nexus**, the Agentic Workflow Engine. Developers can quickly create [Talus agents][talus-agents] or [Talus tools][talus-tools].

This repository includes open-source Nexus packages:

- [`nexus-cli`][nexus-cli-repo]
- [`nexus-sdk`][nexus-sdk-repo]
- [`nexus-toolkit-rust`][nexus-toolkit-rust-repo]
- [Standard Nexus Tools][nexus-tools-repo]

---

For complete documentation, visit the [official Nexus SDK docs][nexus-docs].

## Installation

You can install Nexus CLI using several convenient methods:

### Using Homebrew (macOS/Linux)

```sh
brew tap talus-network/tap
brew install nexus-cli
```

### Arch Linux

The [nexus-cli](https://aur.archlinux.org/packages/nexus-cli) is also available in the AUR (Arch User Repository). You can install it using your preferred [AUR helper](https://wiki.archlinux.org/title/AUR_helpers):

```bash
yay -S nexus-cli
```

### Using cargo-binstall (recommended for faster binaries)

If you prefer quicker binary installation, use [cargo-binstall]:

```bash
cargo binstall --git https://github.com/talus-network/nexus-sdk nexus-cli
```

### Using Cargo

To install directly from the source using `cargo`, run:

```bash
cargo install nexus-cli \
  --git https://github.com/talus-network/nexus-sdk \
  --tag v2.0.0 \
  --locked
```

## Usage

Run the `nexus` command to see all the available options:

```console
$ nexus help
Nexus CLI

Usage: nexus [OPTIONS] <COMMAND>

Commands:
  tool        Manage Nexus Tools
  conf        Manage Nexus Configuration
  dag         Validate and publish Nexus DAGs
  task        Create and operate scheduled Tasks
  gas         Manage Nexus gas budgets and tickets
  tap         Prepare, test and operate TAP applications
  completion  Provide shell completions
  help        Print this message or the help of the given subcommand(s)

Options:
      --json        Emit machine readable JSON
  -v, --verbose...  More output per occurrence
  -q, --quiet...    Less output per occurrence
  -h, --help        Print help
  -V, --version     Print version

```

Scheduled work follows one model:

```text
Task -> Schedule -> Occurrence
```

Create an empty Task when composing it across later transactions. Use
`nexus task schedule` when the initial Schedule is already known; the command
requires at least one occurrence or recurrence and applies the Schedule
atomically. Run `nexus task --help` for timing, funding, recurrence, and object
inspection examples. Use
`nexus task occurrence list --task-id <OBJECT_ID> --json` to page through
every retained occurrence record.

TAP authors can run Move unit tests against published Nexus bytecode with
`nexus tap test --path tap`. The command runs exact published Nexus functions
and developer test extensions in one local Sui VM. See the canonical
[TAP development and testing guide] for an embedded application that owns its
Agent, builds its DAG, schedules a Task, authenticates an onchain Tool callback
and finalizes a Nexus result. The guide also defines the boundary between local
unit tests and Testnet integration.

`nexus tap scaffold` creates the standard CLI managed TAP form. The
[embedded application example] is the starting point when Move state should
own the Agent and lifecycle.

For more detailed instructions, visit the [Nexus CLI documentation][nexus-cli-docs].

## Development

We use [just][just-repo], a straightforward command runner similar to `make`.

To explore the available tasks, run:

```console
$ just --list
Available recipes:
    cli ...          # Commands concerning Nexus CLI
    sdk ...          # Commands concerning the Nexus SDK
    toolkit-rust ... # Commands concerning Nexus Toolkit for Rust
```

Learn more about `just` in the [official manual][just-manual].

<!-- List of references -->

[talus-agents]: https://docs.talus.network/talus-documentation/developer-docs/index/index
[talus-tools]: https://docs.talus.network/talus-documentation/developer-docs/index/tool
[nexus-cli-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/cli
[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
[nexus-sdk-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/sdk
[nexus-toolkit-rust-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/toolkit-rust
[nexus-tools-repo]: https://github.com/Talus-Network/nexus-tools
[nexus-docs]: https://docs.talus.network
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[embedded application example]: https://github.com/Talus-Network/nexus-move-packages/tree/main/examples/local_testing
[TAP development and testing guide]: https://github.com/Talus-Network/nexus-move-packages/blob/main/docs/tap_development.md
[just-repo]: https://github.com/casey/just
[just-manual]: https://just.systems/man/en/
