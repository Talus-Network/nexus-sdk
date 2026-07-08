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

# Regenerate the committed Move binding IR (src/move_bindings/ir/*.json).
#
# Requires a published Nexus objects TOML and a running Sui gRPC endpoint.
# The wrapper does not start Sui or publish packages.
#
# The offline render happens automatically in build.rs on the next build.
rebind nexus_sui='../nexus/sui' grpc='http://127.0.0.1:9000': _check-cargo
    #!/usr/bin/env bash
    set -euo pipefail

    nexus_sui="{{ nexus_sui }}"
    case "$nexus_sui" in
    /*) ;;
    *) nexus_sui="{{ invocation_directory() }}/$nexus_sui" ;;
    esac

    cargo +stable run \
        --package {{ package }} \
        --features binding_codegen \
        --bin regenerate_bindings \
        -- "$nexus_sui/bin/target/objects.localnet.toml" "{{ grpc }}"

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
