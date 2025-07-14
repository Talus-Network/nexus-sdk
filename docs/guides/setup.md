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

The [nexus-cli](https://aur.archlinux.org/packages/nexus-cli) is also available in the AUR (Arch User Repository). You can install it using your preferred [AUR helper](https://wiki.archlinux.org/title/AUR_helpers):

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
  --tag v0.2.0 \
  --locked
```

### Verify the installation

```bash
nexus --version
```

## Configure the Talus devnet

Configure your Nexus CLI to connect to the Talus `devnet` by running:

```bash
nexus conf --sui.net devnet \
  --sui.rpc-url https://rpc.ssfn.devnet.production.taluslabs.dev
```

### Upload some gas budget to Nexus

In order to pay for the network transaction fees and the tool invocations, you need to upload some gas budget to Nexus. You can do this by running the following command:

```bash
nexus gas add-budget --coin <object_id>
```

{% hint style="info" %}
Note that this coin can only be used to pay for Nexus and tool invocation fees only if the DAG is executed from the **same address**.
{% endhint %}

### Configure the Sui client

After installing the Sui binaries, configure and activate your Talus `devnet` environment:

```bash
sui client new-env --alias devnet --rpc https://rpc.ssfn.devnet.production.taluslabs.dev
sui client switch --env devnet
```

## Create a wallet and request funds from the faucet

Create a new wallet with the following command:

```bash
sui client new-address ed25519 tally
```

{% hint style="danger" %}
This command will output your wallet details, including your address and recovery phrase. Ensure you store this information securely.
{% endhint %}

To request funds from the faucet, run:

```bash
# Pick any alias for your address, here we pick the Talus mascot name tally.
sui client faucet --address tally \
  --url https://faucet.devnet.production.taluslabs.dev/gas
```

To check the balance, run:

```bash
sui client balance tally
```

Certainly! Here’s an improved and slightly expanded version of your documentation. It maintains the original structure while enhancing clarity, flow, and technical precision:

---

## (Optional) Configure Encryption for Nexus workflows

To ensure end-to-end encryption of data flowing through workflows, Nexus employs a customized implementation of the [Signal Protocol](https://signal.org/docs/). To establish a secure communication channel, you must claim a pre-key from the on-chain Nexus module `pre_key_vault`, perform an X3DH (Extended Triple Diffie-Hellman) key exchange, and derive a session key used to send an initial encrypted message.

The Nexus CLI abstracts away these cryptographic operations. You can initialize this process by simply running:

```bash
nexus crypto auth
```

This command generates two programmable transactions:

- The first claims a pre-key from the `pre_key_vault` module.
- The second, after performing the X3DH handshake, sends the initial message to finalize the secure channel setup.

{% hint style="info" %}
Keep in mind that the `claim_pre_key` operation is subject to rate limiting. Additionally, it requires a small gas budget to be deposited into Nexus. You can do this using the `nexus gas add-budget` command.
{% endhint %}

---

## (Optional) Access Devnet Sui Explorer

Open the [Talus Sui Explorer](https://explorer.devnet.taluslabs.dev/).

---

After completing these steps, you are ready to build and execute workflows using the Nexus SDK. To build your first workflow, check the [Dev Quickstart guide](math-branching-quickstart.md).
