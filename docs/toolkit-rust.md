# Nexus Toolkit for Rust

> concerns [`nexus-toolkit-rust` repo][nexus-toolkit-rust-repo]

This library exports useful functinality to streamline the development of Nexus Tools in Rust. It is mainly used by **Tool developers** to bootstrap their efforts to extend the Nexus ecosystem.

This documentation will go over the main features of the library and how to use them.

## Installation

Using the [CLI][nexus-cli-docs] run the `$ nexus tool new --help` command to see the available options. This command creates a fresh Rust project with the necessary dependencies to get started.

Alternatively, you can add the following to your `Cargo.toml` file:

```toml
[dependencies.nexus-toolkit]
git = "https://github.com/Talus-Network/nexus-sdk"
tag = "v0.1.0"
package = "nexus-toolkit"
```

## Exports

### `trait nexus_toolkit::NexusTool`

If using `nexus-toolkit`, `NexusTool` is the trait that must be implemented by the tool developer. It defines functions that define the Tool interface, metadata, health and the main logic.

---

#### `NexusTool::new`

Tells the `nexus-toolkit` how to create a new instance of the Tool. This is where you can initialize dependencies, especially those that need to be injected for testing.

The `new` function takes no arguments and is called on every request. Current design of Nexus Tools is for them to be stateless.

```rs
use nexus_toolkit::*;

struct HttpStatus {
    client: reqwest::Client,
}

impl NexusTool for HttpStatus {
    // ...
    async fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
    // ...
}
```

---

#### `NexusTool::Input`

This associated type defines the input that the Tool expects. This type must derive the `serde::Deserialize` and `schemars::JsonSchema` traits.

The Tool's input schema is then derived from this type via the `schemars::schema_for!` macro.

```rs
use nexus_toolkit::*;

#[derive(Deserialize, JsonSchema)]
struct Input {
    url: String,
}

struct HttpStatus;

impl NexusTool for HttpStatus {
    type Input = Input;
    // ...
}
```

---

#### `NexusTool::Output`

This associated type defines the output that the Tool produces. This type must derive the `serde::Serialize` and `schemars::JsonSchema` traits.

The Tool's output schema is then derived from this type via the `schemars::schema_for!` macro.

To comply with [Nexus Workflow output variants][nexus-next-workflow-docs], the output schema **must include a top-level `oneOf`**. This is also enforced by the Tool's runtime and achievable in Rust simply by using an `enum`.

```rs
use nexus_toolkit::*;

#[derive(Serialize, JsonSchema)]
enum Output {
    Ok { status: u16 },
    Err { reason: String },
}

struct HttpStatus;

impl NexusTool for HttpStatus {
    type Output = Output;
    // ...
}
```

---

#### `NexusTool::fqn`

Defines the Tool's fully qualified name. This is used to uniquely identify the Tool in the Nexus ecosystem. Read more about FQNs in the [Nexus Tool documentation][nexus-next-tool-docs].

```rs
use nexus_toolkit::*;

impl NexusTool for HttpStatus {
    // ...
    fn fqn() -> ToolFqn {
        fqn!("com.example.http-status@1")
    }
    // ...
}
```

---

#### `NexusTool::path`

Defines the Tool's path relative to its webserver. The Toolkit allows for multiple tools to run on the same server, so this is used to differentiate between them.

This defaults to the root route.

#### `NexusTool::health`

Defines the Tool's health check. This is a simple function that returns a `anyhow::Result<warp::http::StatusCode>`. The Tool is considered healthy if this function returns `Ok(StatusCode::OK)`.

The health check **should check for the health of dependant** services and return an error if they are not healthy.

```rs
use nexus_toolkit::*;

struct HttpStatus;

impl NexusTool for HttpStatus {
    // ...
    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }
    // ...
}
```

---

#### `NexusTool::invoke`

Defines the Tool's main logic. This is where the Tool processes the input and produces the output.

```rs
use nexus_toolkit::*;

struct HttpStatus;

impl NexusTool for HttpStatus {
    // ...

    /// Fetches the HTTP status of a given URL.
    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let response = reqwest::Client::new().get(&input.url).send().await;

        match response {
            Ok(response) => Output::Ok { status: response.status().as_u16() },
            Err(e) => Output::Err { reason: e.to_string() },
        }
    }
    // ...
}
```
{% hint style="info" %}
Notice that the `invoke` function does not return a `Result`. This is because errors are valid output variants of a Nexus Tool. The `invoke` function should handle any errors and return them as part of the output.
{% endhint %}

---

### `nexus_toolkit::bootstrap!`

The `bootstrap!` macro hides away the boilerplate code needed to create the
underlying HTTP server that adheres to the [Nexus Tool interface][nexus-next-tool-docs].

It has a flexible interface that accepts an `Into<SocketAddr>` value and a struct that `impl NexusTool`.

If using the `bootstrap!` macro without the `Into<SocketAddr>` argument, a `BIND_ADDR` environment variable can be provided. This variable needs to be a string that can `.parse::<SocketAddr>`.

```rs
use nexus_toolkit::*;

// ...

/// Bootstrap a single Tool at 127.0.0.1:8080.
#[tokio::main]
async fn main() {
    bootstrap!(MyTool)
}

/// Bootstrap muliple Tools at 127.0.0.1:8080.
///
/// When definining multiple Tools, their `NexusTool::path` must be unique.
#[tokio::main]
async fn main() {
    bootstrap!([MyTool, MyOtherTool])
}

/// Bootstrap a single Tool at a custom address.
#[tokio::main]
async fn main() {
    bootstrap!(([0, 0, 0, 0], 8081), MyTool)
}

/// Bootstrap multiple Tools at a custom address.
///
/// When definining multiple Tools, their `NexusTool::path` must be unique.
#[tokio::main]
async fn main() {
    bootstrap!(([0, 0, 0, 0], 8081), [MyTool, MyOtherTool])
}
```

<!-- List of References -->

[nexus-toolkit-rust-repo]: https://github.com/Talus-Network/nexus-sdk/tree/main/toolkit-rust
[nexus-next-tool-docs]: ../nexus-next/tool.md
[nexus-next-workflow-docs]: ../nexus-next/packages/torkflow.md
[nexus-cli-docs]: ./cli.md
