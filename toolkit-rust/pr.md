# feat: Add hot-reload support for toolkit config

## Summary

This PR adds automatic hot-reload of toolkit configuration, enabling tools to pick up config changes without requiring a restart. This is essential for Kubernetes deployments where ConfigMaps are updated independently of pod lifecycles.

## Motivation

- **Current behavior**: Tools load `NEXUS_TOOLKIT_CONFIG_PATH` once at startup. Adding new tool FQNs or rotating signing keys requires restarting tool containers.
- **New behavior**: Tools watch their config file and automatically reload when it changes. In-flight requests complete with old config; new requests use the updated config.

## Changes

### New Internal Types

- **`ConfigWatcher`** (`config.rs`): Wraps `ToolkitRuntimeConfig` with automatic file watching using the `notify` crate. Watches the parent directory to handle Kubernetes ConfigMap atomic updates (symlink swaps).

- **`InvokeAuth`** (`signed_http_warp.rs`): Internal auth runtime that holds a reference to `ConfigWatcher` and rebuilds the `InvokeAuthRuntime` when config changes.

### Modified

- **`bootstrap!` macro** (`runtime.rs`): Now uses `ConfigWatcher::from_env()` internally instead of `ToolkitRuntimeConfig::from_env()`. All tools bootstrapped via the macro automatically get hot-reload support.

- **`InvokeAuthRuntime::Signed`**: Changed to `Box<SignedHttpResponderV1>` to fix clippy warning about large enum variant size difference.

### Removed (API cleanup)

- `ReloadableToolkitConfig` - replaced by internal `ConfigWatcher`
- `ReloadableInvokeAuth` - replaced by internal `InvokeAuth`
- `ENV_TOOLKIT_CONFIG_DISABLE_WATCH` - no longer needed
- `routes_for_with_reloadable_config_` - replaced by internal `routes_for_with_watcher_`
- `handle_invoke_reloadable` - logic inlined

### Backward Compatibility

The public API remains unchanged:
- `ToolkitRuntimeConfig` - public, unchanged
- `InvokeAuthRuntime` - public enum, unchanged
- `routes_for_`, `routes_for_with_config_` - public, sync, non-reloadable
- `handle_invoke` - public, unchanged

Existing code using non-reloadable API continues to work:
```rust
let toolkit_cfg = Arc::new(ToolkitRuntimeConfig::from_env()?);
let routes = routes_for_with_config_::<MyTool>(toolkit_cfg);
```

## Implementation Details

### File Watching

- Uses `notify` crate v6.1 for cross-platform file system notifications
- Watches parent directory (not the file itself) to catch Kubernetes ConfigMap symlink swaps
- 500ms debounce to let writes settle before reloading
- Invalid config updates are logged and ignored (keeps using previous valid config)

### Config Reload Flow

```
File change detected
    → Debounce (500ms)
    → Parse new config
    → If valid: update Arc<RwLock<Config>>
    → If invalid: log error, keep old config
    → Next request uses new config via InvokeAuth::current()
```

## Testing

Added 4 new tests for `ConfigWatcher`:

1. `config_watcher_loads_default_without_env_var` - default config when no env var
2. `config_watcher_loads_from_file` - loading from file path
3. `config_watcher_reloads_on_file_change` - automatic reload on file change
4. `config_watcher_keeps_old_on_invalid_update` - graceful handling of invalid config

### Coverage

- `config.rs`: 78% → **96%** line coverage
- All 24 tests pass (19 unit + 5 integration)

## Dependencies

Added to `Cargo.toml`:
```toml
notify = "6.1"
tokio = { version = "1", features = ["sync", "time", "rt-multi-thread"] }
tracing = "0.1"
```

## Test Plan

- [x] All existing tests pass
- [x] New unit tests for ConfigWatcher
- [x] Clippy clean
- [x] Format check passes
- [x] Code coverage improved
