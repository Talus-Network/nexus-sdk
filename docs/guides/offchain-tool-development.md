# Offchain Tool Development Guide

This guide walks you through building, running, and registering an *offchain
tool* for Nexus end to end. Offchain tools are HTTP services that the Nexus
Leader invokes during a workflow execution — they are where you wrap external
APIs, run arbitrary computation, or talk to any system the chain cannot reach
directly.

By the end you'll have a working tool that fetches a live crypto spot price from
a public API, validated and ready to register in the Nexus tool registry.

{% hint style="info" %} Prerequisites

- Follow the [setup guide](setup.md) to make sure you've got the
  [Nexus CLI](../cli.md) installed and your environment configured.
- Skim the [Nexus Toolkit for Rust](../toolkit-rust.md) reference and the
  [Tool Development Guidelines](../tool-development.md) — this guide applies
  those conventions to a concrete example.
  {% endhint %}

## 1. What Is an Offchain Tool?

An offchain tool is an HTTP server that implements the `NexusTool` trait from
the [`nexus-toolkit`](../toolkit-rust.md) crate. The toolkit runtime turns your
implementation into a server that exposes three endpoints:

- `GET /health` — liveness/readiness of the tool and its dependencies.
- `GET /meta` — the tool's metadata: FQN, input schema, output schema, timeout.
- `POST /invoke` — runs the tool's logic against an input and returns an output.

The Leader calls these endpoints; you never call them by hand in production. How
offchain tools compare to onchain tools:

| Aspect | Offchain tool | Onchain tool |
| --- | --- | --- |
| Runtime | HTTP service (Rust) | Sui Move module |
| Execution | Leader invokes over HTTPS | Runs on Sui as part of the PTB |
| Output schema | Derived from your Rust `Output` enum | Derived from the Move `Output` enum |
| Best for | External APIs, LLMs, arbitrary compute | On-chain state changes, asset moves |

If you need to mutate on-chain state instead, see the
[Onchain Tool Development Guide](onchain-tool-development.md).

## 2. Scaffold the Project

Create a new Rust tool project with the Nexus CLI:

```bash
nexus tool new --name spot-price --template rust --target ./
cd spot-price
```

This generates a ready-to-build project:

- `Cargo.toml` — the default Nexus dependencies (`nexus-sdk`, `nexus-toolkit`,
  `tokio`, `serde`, `schemars`).
- `src/main.rs` — a template `NexusTool` implementation and a `bootstrap!` call.
- `README.md` — a stub for your tool's documentation.

Our example calls an external HTTP API, so add the two dependencies the template
doesn't ship with. Edit `Cargo.toml`:

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde_json = "1"
```

## 3. Define the Input and Output Types

The tool's input and output types *are* its interface: the `Input` fields become
the tool's *input ports*, and each `Output` enum variant becomes an *output
variant* whose fields become *output ports*.

```rust
/// Input ports for the spot-price tool.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Input {
    /// The trading pair to price, e.g. `BTC-USD` or `ETH-EUR`.
    currency_pair: String,
}

/// Output variants for the spot-price tool.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Output {
    /// The spot price was fetched successfully.
    Ok {
        /// The price amount as a string, e.g. `"61234.56"`.
        amount: String,
        /// The base currency, e.g. `BTC`.
        base: String,
        /// The quote currency, e.g. `USD`.
        currency: String,
    },
    /// The price could not be fetched.
    Err {
        /// A human-readable description of what went wrong.
        reason: String,
    },
}
```

A few conventions worth calling out (see the full
[Tool Development Guidelines](../tool-development.md)):

- `Input` must derive `Deserialize` and `JsonSchema`. `#[serde(deny_unknown_fields)]`
  rejects malformed inputs early.
- `Output` must be an `enum` so the generated schema has a top-level `oneOf` —
  the toolkit runtime enforces this. Derive `Serialize` and `JsonSchema`, and
  use `#[serde(rename_all = "snake_case")]` so variants serialize as `ok`/`err`.
- Name error variants with an `err` prefix; Nexus treats `err`-prefixed variants
  specially and always forwards their ports on-chain.
- Keep ports flat and stable — return `err` rather than an `ok` with optional
  fields when you cannot produce the requested data.

## 4. Implement the `NexusTool` Trait

Now implement the trait. The interesting method is `invoke`: it performs the
outbound HTTP request and maps both success and failure onto output variants.

```rust
struct SpotPrice {
    client: reqwest::Client,
}

impl NexusTool for SpotPrice {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        // `new` runs on every request; initialize shared, cheap-to-clone state.
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn fqn() -> ToolFqn {
        // The fully qualified name uniquely identifies this tool: `domain.name@version`.
        fqn!("xyz.your-domain.exchanges.coinbase.spot-price@1")
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        // Health checks should verify dependencies; here the tool is stateless.
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let url = format!(
            "https://api.coinbase.com/v2/prices/{}/spot",
            input.currency_pair
        );

        let response = match self.client.get(&url).send().await {
            Ok(response) => response,
            Err(e) => return Output::Err { reason: e.to_string() },
        };

        let body: serde_json::Value = match response.json().await {
            Ok(body) => body,
            Err(e) => return Output::Err { reason: e.to_string() },
        };

        match (
            body["data"]["amount"].as_str(),
            body["data"]["base"].as_str(),
            body["data"]["currency"].as_str(),
        ) {
            (Some(amount), Some(base), Some(currency)) => Output::Ok {
                amount: amount.to_string(),
                base: base.to_string(),
                currency: currency.to_string(),
            },
            _ => Output::Err {
                reason: format!("unexpected response for pair `{}`", input.currency_pair),
            },
        }
    }
}
```

{% hint style="info" %}
Notice that `invoke` returns `Self::Output`, never a `Result`. Errors are valid
*output variants* in Nexus, so the tool catches every failure and returns it as
an `err` variant rather than panicking or bubbling up an error.
{% endhint %}

The FQN format is `domain.name@version` (for example
`xyz.your-domain.exchanges.coinbase.spot-price@1`): the domain has at least two
dot-separated parts, each part is lowercase alphanumeric (plus `-`/`_`), and the
version is a positive integer. The `fqn!` macro validates this at compile time.

## 5. Bootstrap the Server

Finally, start the HTTP server with the `bootstrap!` macro:

```rust
#[tokio::main]
async fn main() {
    bootstrap!(SpotPrice);
}
```

`bootstrap!(SpotPrice)` serves the tool on `127.0.0.1:8080`. The macro is
flexible:

```rust
// Bind to a specific address.
bootstrap!(([0, 0, 0, 0], 8081), SpotPrice);

// Serve several tools from one server (each must have a unique `path`).
bootstrap!(([127, 0, 0, 1], 8080), [SpotPrice, AnotherTool]);
```

If you call `bootstrap!(SpotPrice)` without an address, you can set the bind
address at runtime via the `BIND_ADDR` environment variable. See the
[toolkit reference](../toolkit-rust.md#nexus_toolkitbootstrap) for details.

The top of `src/main.rs` already imports everything you need:

```rust
use {
    nexus_sdk::*,
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};
```

## 6. Add Tests

Add unit tests for `invoke` that cover both the success path and the error
paths (a bad pair, an unreachable endpoint). Although not shown here, every tool
should ship tests; mocking the HTTP dependency (for example with `mockito`)
keeps them fast and deterministic.

## 7. Run, Inspect, and Validate

Build and start the tool:

```bash
cargo run
```

Inspect the metadata the tool advertises without starting the server by passing
`--meta`, which prints the metadata JSON and exits:

```bash
cargo run -- --meta
```

With the server running, validate it with the CLI. Validation checks that
`GET /health` returns `200`, that `GET /meta` is valid JSON, and that the output
schema has the required top-level `oneOf`:

```bash
nexus tool validate offchain --url http://localhost:8080
```

## 8. Register the Tool

Register the running tool in the Nexus tool registry. The CLI fetches the
tool's metadata over HTTP, then submits the registration transaction:

```bash
nexus tool register offchain --url http://localhost:8080
```

Useful flags:

- `--from-meta <FILE|->` — register from a metadata JSON file (as produced by
  `cargo run -- --meta`) or stdin instead of a live URL. This skips the live
  HTTP validation step, which is handy in CI where the tool isn't running.
- `--batch` — register *every* tool served by the webserver at the URL at once.
- `--invocation-cost <MIST>` — the per-invocation cost charged to callers
  (defaults to `0`).
- `--collateral-coin <OBJECT_ID>` — the coin object to use as collateral (the
  second gas coin is chosen automatically if omitted).

{% hint style="info" %}
Tool registration is currently restricted during the beta phase. To register
your tool, please contact the team to be added to the allow list.
{% endhint %}

You can confirm the registration and inspect the stored record with:

```bash
nexus tool list
nexus tool inspect --tool-fqn "xyz.your-domain.exchanges.coinbase.spot-price@1"
```

## 9. Deploy for Production

In production, Nexus Leader nodes reach tools over **HTTPS** and authenticate
`POST /invoke` requests with **signed HTTP** (Ed25519 signatures in
`X-Nexus-Sig-*` headers). The toolkit runtime is a plain HTTP server, so:

- Run it behind a TLS terminator (reverse proxy or load balancer). Leaders
  validate your certificate against the system root trust store.
- Enable signed HTTP so the tool can verify the calling Leader and the Leader
  can verify your responses.
- Keep your signing keys secret, rotate them if exposed, and keep the host clock
  accurate (signed requests use validity windows).

For the full end-to-end setup — TLS termination options, key registration, and
runtime config — see the
[Tool Communication (HTTPS + Signed HTTP)](tool-communication.md) guide.

## 10. Use the Tool in a Workflow

Once registered, reference the tool from a DAG by its FQN with the `off_chain`
variant:

```json
{
  "vertices": [
    {
      "kind": {
        "variant": "off_chain",
        "tool_fqn": "xyz.your-domain.exchanges.coinbase.spot-price@1"
      },
      "name": "price",
      "entry_ports": [
        {
          "name": "currency_pair"
        }
      ]
    }
  ]
}
```

The `price` vertex's `ok` variant exposes the `amount`, `base`, and `currency`
output ports, which you can wire into downstream tools via edges.

## Next Steps

- Read the [DAG Construction Guide](dag-construction.md) to compose this tool
  with others into a full workflow.
- Build a tool that runs on Sui instead with the
  [Onchain Tool Development Guide](onchain-tool-development.md).
- Revisit the [Tool Development Guidelines](../tool-development.md) and ask
  whether your tool is as generic, flat, and stable as it could be — a tool is a
  reusable library for an API, not a one-off for a single DAG.
