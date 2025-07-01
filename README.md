# Nexus SDK

[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.1-4baaaa.svg)](CODE_OF_CONDUCT.md)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://github.com/Talus-Network/nexus-sdk/blob/main/LICENSE.txt)
[![Actions](https://img.shields.io/badge/GitHub_Actions-Active-brightgreen)](https://github.com/Talus-Network/nexus-sdk/actions)

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
  --tag v0.1.0 \
  --locked
```

## Usage

Run the `nexus` command to see all the available options:

```console
$ nexus help
Nexus CLI

Usage: nexus [OPTIONS] <COMMAND>

Commands:
  tool     Manage Nexus Tools
  conf     Manage Nexus Configuration
  dag      Validate, publish and execute Nexus DAGs
  network  Mange Nexus networks and leader caps
  help     Print this message or the help of the given subcommand(s)

Options:
      --json     Change the output format to JSON
  -h, --help     Print help
  -V, --version  Print version

```

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
    tools ...        # Commands concerning native Nexus Tools
```

Learn more about `just` in the [official manual][just-manual].

<!-- List of references -->

[talus-agents]: https://docs.talus.network/talus-documentation/developer-docs/index/index
[talus-tools]: https://docs.talus.network/talus-documentation/developer-docs/index/tool
[nexus-cli-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/cli
[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
[nexus-sdk-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/sdk
[nexus-toolkit-rust-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/toolkit-rust
[nexus-tools-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/tools
[nexus-docs]: https://docs.talus.network
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[just-repo]: https://github.com/casey/just
[just-manual]: https://just.systems/man/en/
