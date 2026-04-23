# Nexus SDK Developer Setup Guide

This guide will help you quickly set up your development environment and start using Nexus SDK, including initializing your wallet, funding it, and accessing the Sui explorer.

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
  --tag v1.0.0 \
  --locked
```

### Verify the installation

```bash
nexus --version
```

## Download the Nexus objects

{% tabs %}
{% tab title="Testnet" %}

```bash
wget -O ~/.nexus/objects.testnet.toml https://storage.googleapis.com/production-talus-sui-objects/v1.0.0/objects.testnet.toml
```

{% endtab %}

{% tab title="Mainnet" %}

```bash
wget -O ~/.nexus/objects.mainnet.toml https://storage.googleapis.com/production-talus-sui-objects/v1.0.0/objects.mainnet.toml
```

{% endtab %}
{% endtabs %}

## Configure the Sui network

{% tabs %}
{% tab title="Testnet" %}
Configure your Nexus CLI to connect to the Sui `testnet` by running:

```bash
nexus conf set \
  --sui.rpc-url https://fullnode.testnet.sui.io \
  --nexus.objects ~/.nexus/objects.testnet.toml
```

{% endtab %}

{% tab title="Mainnet" %}
Configure your Nexus CLI to connect to the Sui `mainnet` by running:

```bash
nexus conf set \
  --sui.rpc-url https://fullnode.mainnet.sui.io \
  --nexus.objects ~/.nexus/objects.mainnet.toml
```

{% endtab %}
{% endtabs %}

### Configure the Sui client

After installing the Sui binaries, configure and activate your Sui environment:

{% hint style="info" %}
Assuming you have no prior sui configuration

```bash
sui client --yes
```

{% endhint %}

{% tabs %}
{% tab title="Testnet" %}

```bash
sui client new-env --alias testnet --rpc https://fullnode.testnet.sui.io
sui client switch --env testnet
```

{% endtab %}

{% tab title="Mainnet" %}

```bash
sui client new-env --alias mainnet --rpc https://fullnode.mainnet.sui.io
sui client switch --env mainnet
```

{% endtab %}
{% endtabs %}

## Create a wallet and fund it

Create a new wallet with the following command:

```bash
sui client new-address ed25519 tally
sui client switch --address tally
```

{% hint style="danger" %}
This command will output your wallet details, including your address and recovery phrase. Ensure you store this information securely.
{% endhint %}

Import the newly created wallet to `nexus`:

```bash
PK=$(sui keytool export --key-identity tally --json | jq -er '.exportedPrivateKey')
BASE64_PK=$(sui keytool convert "$PK" --json | jq -er '.base64WithFlag')
nexus conf set --sui.pk "$BASE64_PK"
```

{% tabs %}
{% tab title="Testnet" %}
Request funds from the faucet by visiting [Sui faucet](https://faucet.sui.io/) and entering your wallet address. You'll need at least 2 coins: one for the Nexus gas budget and one for transaction gas fees.
{% endtab %}

{% tab title="Mainnet" %}
You will need SUI tokens to pay for transaction gas fees and the Nexus gas budget. Acquire SUI through an exchange and transfer to your wallet address.
{% endtab %}
{% endtabs %}

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

---

After completing these steps, you are ready to build and execute workflows using the Nexus SDK. To build your first workflow, check the [Dev Quickstart guide](math-branching-quickstart.md).
