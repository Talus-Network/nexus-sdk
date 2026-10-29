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
# Accepts either deployment metadata JSON or a Nexus `sui` source directory and a running Sui gRPC
# endpoint. Move source is required to regenerate protocol limits and parameter names. A source
# directory resolves `bin/target/deployment.localnet.json`; set source_root when passing metadata.
# The wrapper does not start Sui or publish packages. It routes generated files to the SDK checkout
# containing the root Justfile even when Cargo reuses an older binary.
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
    output_root="{{ justfile_directory() }}/sdk"
    if [ -n "$source_root" ]; then
        case "$source_root" in
        /*) ;;
        *) source_root="{{ invocation_directory() }}/$source_root" ;;
        esac
        cargo +stable run \
            --package {{ package }} \
            --features binding_codegen \
            --bin regenerate_bindings \
            -- "$objects_file" "{{ grpc }}" --source-root "$source_root" --output-root "$output_root"
    else
        cargo +stable run \
            --package {{ package }} \
            --features binding_codegen \
            --bin regenerate_bindings \
            -- "$objects_file" "{{ grpc }}" --output-root "$output_root"
    fi

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
