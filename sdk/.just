#
# $ just sdk
#
# Commands related the Nexus SDK.
#

import '../helpers/helpers.just'

package := 'nexus-sdk'

[private]
_default:
    @just --list sdk

# Check the Nexus SDK
check: _check-cargo
    cargo +stable check --all-features --package {{ package }}

# Run all tests in the Nexus SDK
test: _check-cargo
    cargo +stable test --all-features --package {{ package }}

# Regenerate the committed Nexus Move binding IR (src/move_bindings/ir/*.json).
#
# Accepts either a deployment objects TOML or a Nexus `sui` source directory and a running Sui
# gRPC endpoint. A source directory resolves `bin/target/objects.localnet.toml` and restores source
# parameter names automatically. Set source_root explicitly when using an objects TOML.
# The wrapper does not start Sui or publish packages.
#
# The offline render happens automatically in build.rs on the next build.
rebind objects_file grpc='http://127.0.0.1:9000' source_root='': _check-cargo
    #!/usr/bin/env bash
    set -euo pipefail

    objects_file="{{ objects_file }}"
    case "$objects_file" in
    /*) ;;
    *) objects_file="{{ invocation_directory() }}/$objects_file" ;;
    esac

    source_root="{{ source_root }}"
    source_args=()
    if [ -n "$source_root" ]; then
        case "$source_root" in
        /*) ;;
        *) source_root="{{ invocation_directory() }}/$source_root" ;;
        esac
        source_args=(--source-root "$source_root")
    fi

    cargo +stable run \
        --package {{ package }} \
        --features binding_codegen \
        --bin regenerate_bindings \
        -- "$objects_file" "{{ grpc }}" "${source_args[@]}"

# Run rustfmt on the Nexus SDK
fmt-check: _check-cargo
    #!/usr/bin/env bash

    nightly="$(just helpers::get-nightly-version)"

    cargo +"$nightly" fmt --package {{ package }} --check

# Run clippy on the Nexus SDK
clippy: _check-cargo
    cargo +stable clippy --package {{ package }}

# Publishes the Nexus SDK package to crates.io
publish: _check-cargo
    cargo +stable publish --package {{ package }}
    @echo "Published {{ package }} to crates.io"
