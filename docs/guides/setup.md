# Nexus SDK Developer Setup Guide

This guide will help you quickly set up your development environment and start using Nexus SDK, including initializing your wallet, funding it through a faucet, and accessing the `devnet` Sui explorer.

## Installation and Setup

Follow these steps to install the Nexus CLI and set up your environment:

### Prerequisites

Make sure you have installed:

- [Rust](https://rustup.rs/) (latest stable)
- [Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html)
- [Sui](https://docs.sui.io/guides/developer/getting-started)

### Install the Nexus CLI

#### Using Homebrew (macOS/Linux)

```bash
brew tap talus-network/tap
brew install nexus-cli
```

#### Arch Linux

The [nexus-cli](https://aur.archlinux.org/packages/nexus-cli) is also available
in the AUR (Arch User Repository). You can install it using your preferred
[AUR helper](https://wiki.archlinux.org/title/AUR_helpers):

```bash
yay -S nexus-cli
```

#### Using cargo-binstall (recommended for faster binaries)

If you prefer quicker binary installation, use [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall --git https://github.com/talus-network/nexus-sdk nexus-cli
```

#### Using Cargo

To install directly from the source using `cargo`, run:

```bash
cargo install nexus-cli \
  --git https://github.com/talus-network/nexus-sdk \
  --tag v0.1.0 \
  --locked
```

### Verify the installation

```bash
nexus --version
```

## Configure the Talus devnet

> [!IMPORTANT]
> The Talus `devnet` is currently private and accessible only through approved
> credentials. To request access, please submit your details using this [Google Form]().

Once you receive your credentials, configure your Nexus CLI to connect to the
Talus `devnet` by running:

```bash
nexus conf --sui.net devnet \
  --sui.basic-auth-user YOUR_USERNAME \
  --sui.basic-auth-password YOUR_PASSWORD
```

Next, create a `.envrc` file to conveniently store your RPC and faucet URLs:

```bash
export SUI_RPC_URL=https://rpc.ssfn.devnet.production.taluslabs.dev
export SUI_FAUCET_URL=https://faucet.devnet.production.taluslabs.dev
```

Activate these environment variables using:

```bash
source .envrc
```

> [!TIP]
> To automatically load these variables every time you navigate to the project
> directory, consider using [direnv](https://direnv.net/). After installing it,
> run direnv allow within your project directory.

### Configure the Sui client

After installing the Sui binaries, configure and activate your Talus `devnet`
environment:

```bash
sui client new-env --alias devnet --rpc $SUI_RPC_URL \
  --basic-auth YOUR_USERNAME:YOUR_PASSWORD
sui client switch --env devnet
```

## Create a wallet and request funds from the faucet

Create a new wallet with the following command:

```bash
sui client new-address ed25519 tally
```

> [!IMPORTANT]
> This command will output your wallet details, including your address and
> recovery phrase. Ensure you store this information securely.

To request funds from the faucet, run:

```bash
sui client faucet --address tally \
  --url https://USERNAME:PASSWORD@faucet.devnet.production.taluslabs.dev
```

## Access Devnet Sui Explorer

Open the [Talus Sui Explorer](https://explorer.devnet.taluslabs.dev/) and
request an access code.
---

After completing these steps, you are ready to build and execute workflows using
the Nexus SDK. To build your first workflow, check the [Dev Quickstart guide](math-branching-quickstart.md).
