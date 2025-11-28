# nexus-cli

The **Nexus CLI** provides easy-to-use command-line tools to manage and interact with Nexus, the Agentic Workflow Engine.

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
  --tag v0.3.0 \
  --locked
```

## Usage

Run the `nexus` command to see all the available commands and options:

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

<!-- List of references -->

[nexus-cli-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/cli
