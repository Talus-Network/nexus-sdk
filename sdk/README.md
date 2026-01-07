# nexus-sdk

> [!NOTE]
> This is an internal crate intended primarily for use within other Nexus
> packages. For Nexus Tool development, please use the higher-level
> [Nexus Toolkit][nexus-toolkit-docs].

## Usage

Generally, you won't need to depend on this crate directly. Instead, use the
[Nexus Toolkit][nexus-toolkit-docs], which provides interfaces for Nexus Tool
development.

However, if you specifically require direct access to internal helper functions,
you can include this crate in your project's `Cargo.toml` file:

```toml
[dependencies.nexus-sdk]
git = "https://github.com/Talus-Network/nexus-sdk"
tag = "v0.4.0"
package = "nexus-sdk"
```

<!-- List of references -->

[nexus-toolkit-docs]: https://docs.talus.network/talus-documentation/developer-docs/index-1/toolkit-rust
