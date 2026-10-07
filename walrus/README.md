# Native Walrus storage

`nexus-walrus` provides wallet funded uploads, storage management, and recovery through the shared Nexus wallet.

The adapter is distributed through Git because the upstream Walrus dependencies are not published on crates.io. It depends on the registry SDK so applications share the same wallet and protocol types:

```toml
[dependencies]
nexus-sdk = { version = "2.1.1", features = ["nexus", "walrus"] }
nexus-walrus = { git = "https://github.com/Talus-Network/nexus-sdk", tag = "v2.1.1" }
```

Building the adapter requires a C++ compiler and libclang. On Debian or Ubuntu, install `clang` and `libclang-dev` alongside the Rust build tools.

Pass an existing wallet to native storage:

```rust,ignore
use nexus_walrus::{UploadOptions, WalrusStorage};

let storage = WalrusStorage::new(client.wallet()?.clone(), None).await?;
let blob = storage.upload(bytes, UploadOptions::default()).await?;
let output = blob.nexus_data()?;
```

Nexus and Walrus share one signing key and RPC connection. The adapter owns no additional key store. Uploads retain their registration, certification, and verified readback steps. Persist `UploadRegistration` and `PendingUpload` when an operation must survive cancellation or restart. Retry the saved signed registration after an uncertain submission.

See the [SDK storage guide](../sdk/README.md#walrus-storage) for payment policy, ownership, expiry, references, and recovery. Reading blobs and resolving protocol references requires only the SDK.

The workspace patches the registry SDK to the local source during development. Downstream applications use the published SDK and do not inherit that patch.
