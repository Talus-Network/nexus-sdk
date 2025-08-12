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

## Download the Nexus objects

```bash
wget https://storage.googleapis.com/production-talus-sui-objects/v0.2.0/objects.devnet.toml
```

## Configure the Talus devnet

Configure your Nexus CLI to connect to the Talus `devnet` by running:

```bash
nexus conf set --sui.net devnet \
  --sui.rpc-url https://rpc.ssfn.devnet.production.taluslabs.dev \
  --nexus.objects objects.devnet.toml
```

### Configure the Sui client

After installing the Sui binaries, configure and activate your Talus `devnet` environment:

{% hint style="info" %}
Assuming you have no prior sui configuration

```bash
sui client --yes
```

{% endhint %}

```bash
sui client new-env --alias devnet --rpc https://rpc.ssfn.devnet.production.taluslabs.dev
sui client switch --env devnet
```

## Create a wallet and request funds from the faucet

Create a new wallet with the following command:

```bash
sui client new-address ed25519 tally
sui client switch --address tally
```

{% hint style="danger" %}
This command will output your wallet details, including your address and recovery phrase. Ensure you store this information securely.
{% endhint %}

To request funds from the faucet, run the following command twice to get 2 gas coins:

```bash
# Pick any alias for your address, here we pick the Talus mascot name tally.
sui client faucet --address tally \
  --url https://faucet.devnet.production.taluslabs.dev/gas
```

```bash
sui client faucet --address tally \
  --url https://faucet.devnet.production.taluslabs.dev/gas
```

To check the balance, run:

```bash
sui client balance tally
```

### Upload some gas budget to Nexus

In order to pay for the network transaction fees and the tool invocations, you need to upload some gas budget to Nexus. You can do this by running the following command:

```bash
GAS_INFO=$(sui client gas --json)

echo $GAS_INFO

nexus gas add-budget \
  --coin $(echo $GAS_INFO | jq -r '.[0].gasCoinId') \
  --sui-gas-coin $(echo $GAS_INFO | jq -r '.[1].gasCoinId')
```

{% hint style="info" %}
Note that this coin can only be used to pay for Nexus and tool invocation fees only if the DAG is executed from the **same address**.
{% endhint %}

## Configure Encryption for Nexus workflows

To ensure end-to-end encryption of data flowing through workflows, Nexus employs a customized implementation of the [Signal Protocol](https://signal.org/docs/). To establish a secure communication channel, you must claim a pre-key from the on-chain Nexus module `pre_key_vault`, perform an X3DH (Extended Triple Diffie-Hellman) key exchange, and derive a session key used to send an initial encrypted message.

The Nexus CLI abstracts away these cryptographic operations. You can initialize this process by simply running:

```bash
nexus crypto init-key --force
nexus crypto generate-identity-key
nexus crypto auth
```

This command generates two programmable transactions:

- The first claims a pre-key from the `pre_key_vault` module.
- The second, after performing the X3DH handshake, sends the initial message to finalize the secure channel setup.

{% hint style="info" %}
Keep in mind that the `claim_pre_key` operation is subject to rate limiting. Additionally, it requires a small gas budget to be deposited into Nexus. See `nexus gas add-budget` command.
{% endhint %}

## (Optional) Access Devnet Sui Explorer

Open the [Talus Sui Explorer](https://explorer.devnet.taluslabs.dev/).

---

After completing these steps, you are ready to build and execute workflows using the Nexus SDK. To build your first workflow, check the [Dev Quickstart guide](math-branching-quickstart.md).
