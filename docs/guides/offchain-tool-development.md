# Offchain Tool Development Guide

This guide walks you through building, running, registering, and operating an
*offchain tool* for Nexus end to end. Offchain tools are HTTP services that the
Nexus Leader invokes during a workflow execution — they are where you wrap
external APIs, run arbitrary computation, or talk to any system the chain cannot
reach directly.

By the end you'll understand how a tool is registered, discovered, invoked,
verified, and paid, and you'll have a working tool that fetches a live crypto
spot price from a public API.

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

## 2. Tool FQN and Metadata Model

Every tool is identified by a *fully qualified name* (FQN) with the shape
`domain.name@version`, for example `xyz.your-domain.exchanges.coinbase.spot-price@1`:

- the domain has at least two dot-separated parts,
- each part is lowercase alphanumeric (plus `-`/`_`) and does not start with a
  digit, `-`, or `_`,
- the version is a positive integer (`0` is rejected).

The `fqn!` macro validates this at compile time; the parsing rules live in
[sdk/src/tool_fqn.rs](../../sdk/src/tool_fqn.rs).

Everything the network needs to discover and call your tool is captured in its
*metadata*, served from `GET /meta` and modeled by `ToolMeta` in
[sdk/src/types/tool_meta.rs](../../sdk/src/types/tool_meta.rs):

| Field | Meaning |
| --- | --- |
| `fqn` | The tool's fully qualified name. |
| `url` | The base URL the Leader invokes. |
| `description` | Human-readable summary. |
| `timeout` | How long the Leader waits for `/invoke` (milliseconds on the wire). |
| `input_schema` | JSON Schema (draft 2020-12) generated from your `Input`. |
| `output_schema` | JSON Schema generated from your `Output` enum. |

The toolkit derives both schemas for you from the Rust types — you never write
JSON Schema by hand.

## 3. Scaffold the Project

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

## 4. Design the Input and Output Schemas

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

## 5. Implement the `NexusTool` Trait

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
an `err` variant rather than panicking or bubbling up an error. Any variant whose
name starts with `err` has all of its ports forwarded on-chain automatically, so
downstream vertices can branch on the failure.
{% endhint %}

The `authorize` hook (optional) runs after a request is authenticated via signed
HTTP and lets you apply tool-side policy — allowing only specific Leaders, rate
limiting, or gating sensitive functionality. See
[Signed HTTP](#11-signed-http-authentication-and-replay-protection) below.

## 6. Bootstrap the Server

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

## 7. Test the Tool

Add unit tests for `invoke` that cover both the success path and the error
paths (a bad pair, an unreachable endpoint). Although not shown here, every tool
should ship tests; mocking the HTTP dependency (for example with `mockito`)
keeps them fast and deterministic.

## 8. Run, Inspect, and Validate

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

## 9. Register the Tool

Registration writes your tool's metadata into the on-chain *tool registry* so
that workflows can discover and price it. The CLI fetches `GET /meta`, then
submits the registration transaction:

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

### How registration maps to the tool registry and SDK types

- On-chain, registration stores a tool record in the registry module
  ([registry::tool_registry](../../nexus-next/sui/registry/sources/tool_registry.move))
  and mints an `OwnerCap<OverTool>` to your address. That capability authorizes
  later management calls (`unregister`, `set-invocation-cost`, `update-timeout`,
  `claim-collateral`). By default the CLI saves the cap ids to
  `~/.nexus/conf.toml`; pass `--no-save` to skip that.
- Off-chain, the SDK models the same record. An offchain tool is a
  `ToolRef::Http { url }`; the full record is the `Tool` struct in
  [sdk/src/types/tool.rs](../../sdk/src/types/tool.rs), and the `/meta` payload
  is `ToolMeta` in [sdk/src/types/tool_meta.rs](../../sdk/src/types/tool_meta.rs).
- High-level actions live in [sdk/src/nexus/tool.rs](../../sdk/src/nexus/tool.rs)
  and the PTB builders in
  [sdk/src/transactions/tool.rs](../../sdk/src/transactions/tool.rs). The full
  set of CLI verbs is documented under `nexus tool` in the [CLI reference](../cli.md).

Confirm the registration and inspect the stored record with:

```bash
nexus tool list
nexus tool inspect --tool-fqn "xyz.your-domain.exchanges.coinbase.spot-price@1"
```

## 10. How a Tool Is Invoked and Its Output Enters a Workflow

When a workflow reaches your tool's vertex, the Leader:

1. gathers the vertex's input ports (from default values and incoming edges) into
   a single JSON object matching your `input_schema`,
1. signs and sends `POST /invoke` with that object (see
   [Signed HTTP](#11-signed-http-authentication-and-replay-protection)),
1. receives your `Output` JSON — a single selected variant (for example `ok`)
   with its named ports,
1. converts that variant and its ports into Nexus data and submits it on-chain,
   where it becomes the input for downstream vertices connected by edges.

So the *variant you return* selects which downstream edges fire, and the *ports
in that variant* become the values flowing along those edges. Keep ports flat
and typed so they can be wired directly into the next tool's input ports.

## 11. Signed HTTP: Authentication and Replay Protection

In production, Nexus Leader nodes authenticate `POST /invoke` with **signed
HTTP** — application-layer Ed25519 signatures carried in `X-Nexus-Sig-*`
headers. This is what lets a tool trust the caller and lets the Leader trust the
response, independently of TLS.

The request carries three headers (see the
[Tool Communication guide](tool-communication.md) for the full spec):

- `X-Nexus-Sig-V` — protocol version (currently `"1"`).
- `X-Nexus-Sig-Input` — base64url of the raw JSON *claims* bytes.
- `X-Nexus-Sig` — base64url of the 64-byte Ed25519 signature.

The claims bind the request and give it a validity window: `iat_ms`/`exp_ms`
(millisecond UTC window), a `nonce` (unique per invocation), and the request
identity (`method`, `path`, `query`, `body_sha256`). Responses are signed the
same way and echo the nonce.

- **Authentication:** the tool verifies the signature against its local
  allowlist of permitted Leaders (exported with
  `nexus tool auth export-allowed-leaders`), so no on-chain read is needed at
  runtime.
- **Replay protection:** the tool tracks `(tool_id, nonce)`. A repeat with the
  *same* request identity is a safe retry; a repeat with a *different* identity
  is a conflicting replay and must be rejected.
- **Signing keys:** generate and register a key with
  `nexus tool auth keygen` and `nexus tool auth register-key`. The signed-HTTP
  machinery lives in [sdk/src/signed_http](../../sdk/src/signed_http/).

The `authorize(ctx)` hook receives the verified `AuthContext` (Leader identity,
key id, validity window, nonce, HTTP target) so you can add tool-side policy;
returning an error yields a signed `403`.

## 12. Deploy for Production

The toolkit runtime is a plain HTTP server, so run it behind a TLS terminator
(reverse proxy or load balancer):

- Nexus Leaders reach tools over **HTTPS** and validate your certificate against
  the system root trust store — use a publicly trusted certificate.
- Ensure the proxy forwards all `X-Nexus-Sig-*` headers and does not rewrite the
  request body (the signature binds `body_sha256`).
- Keep your signing keys secret, rotate them if exposed, and keep the host clock
  accurate (signed requests use validity windows).

For the full end-to-end setup — TLS termination options, key registration,
runtime config, and troubleshooting — see the
[Tool Communication (HTTPS + Signed HTTP)](tool-communication.md) guide.

## 13. Versioning Your Tool

The `@version` suffix in the FQN is how Nexus tracks breaking changes. The rule
of thumb:

- **Changing the input or output schema is a breaking change.** Adding a
  required input port, removing an output port, renaming a variant, or changing a
  type all change the contract that DAGs depend on. Bump the version — publish
  the new behavior as `…@2` while `…@1` stays registered and callable.
- **Non-contract changes** (a bug fix in `invoke`, faster code, a new upstream
  endpoint that returns the same shape) can be redeployed under the same FQN.

Because old versions remain in the registry, existing DAGs that pin `…@1` keep
working when you ship `…@2`. Update `docs`/`README.md` for the new version and
register it like any other tool.

## 14. Use the Tool in a Workflow

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
output ports, which you can wire into downstream tools via edges. See the
[DAG Construction Guide](dag-construction.md) for the full DAG format.

## 15. Troubleshooting

| Symptom | Likely cause and fix |
| --- | --- |
| `FQN mismatch` / tool not found | The FQN in `fqn!` differs from the one you registered or referenced in the DAG. They must match exactly, version included. |
| Schema mismatch / input rejected | The DAG supplies ports that don't match `input_schema`, or the response doesn't match `output_schema`. Re-check with `cargo run -- --meta` and `nexus tool inspect`. |
| Signature verification failure | The Leader isn't in the tool's allowlist, keys are stale, clocks are skewed, or a proxy stripped/altered the `X-Nexus-Sig-*` headers or body. |
| Wrong network/package config | The CLI is pointed at the wrong network. Re-check `nexus conf` and the `~/.nexus/objects.*.toml` you configured in the [setup guide](setup.md). |
| Unsupported output type | The output isn't an `enum` (no top-level `oneOf`), or a port serializes to a shape Nexus can't carry. Keep ports flat and JSON-scalar where possible. |
| Stale tool registration | You changed the schema without bumping the version. Register a new `@version` (see [Versioning](#13-versioning-your-tool)). |
| Insufficient payment | The caller can't cover the tool's `invocation-cost`. Lower it with `nexus tool set-invocation-cost`, or fund the caller's Nexus gas budget (see the setup guide). |
| Verifier rejection | The response failed verification (bad signature, replayed nonce, or expired window). Confirm signing config and host clock. |

## 16. Acceptance Checklist

You've built a complete offchain tool when you can:

- run the tool locally (`cargo run`) and get `200` from `nexus tool validate offchain --url …`;
- publish/register its metadata (`nexus tool register offchain …`);
- fetch it back by FQN through the CLI (`nexus tool inspect --tool-fqn …`) or SDK;
- invoke it inside a sample DAG (`off_chain` vertex);
- inspect the submitted result of that execution.

## 17. Reference

- Move: [registry::tool_registry](../../nexus-next/sui/registry/sources/tool_registry.move)
- SDK: [sdk/src/nexus/tool.rs](../../sdk/src/nexus/tool.rs),
  [sdk/src/transactions/tool.rs](../../sdk/src/transactions/tool.rs),
  [sdk/src/types/tool.rs](../../sdk/src/types/tool.rs),
  [sdk/src/types/tool_meta.rs](../../sdk/src/types/tool_meta.rs),
  [sdk/src/tool_fqn.rs](../../sdk/src/tool_fqn.rs),
  [sdk/src/signed_http](../../sdk/src/signed_http/)
- CLI: the `nexus tool` command group in the [CLI reference](../cli.md)

## Next Steps

- Read the [DAG Construction Guide](dag-construction.md) to compose this tool
  with others into a full workflow.
- Build a tool that runs on Sui instead with the
  [Onchain Tool Development Guide](onchain-tool-development.md).
- Revisit the [Tool Development Guidelines](../tool-development.md) and ask
  whether your tool is as generic, flat, and stable as it could be — a tool is a
  reusable library for an API, not a one-off for a single DAG.
