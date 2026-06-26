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

# Regenerate the committed Move-binding IR (src/idents/generated/ir/*.json).
#
# Publishes the Nexus packages to a localnet via the nexus publish script,
# then fetches their normalized IR over gRPC. Starts `sui start --with-faucet
# --force-regenesis` automatically when the configured localnet env is down, and
# requires the nexus sibling checked out.
#
# The offline render (IR JSON -> address-free idents and parsing types) then
# happens automatically in build.rs on the next build; review the diff under
# src/idents/generated/ir/.
rebind nexus_next_sui grpc='http://127.0.0.1:9000': _check-cargo
    @nexus_next_sui="{{ nexus_next_sui }}"; case "$nexus_next_sui" in /*) ;; *) nexus_next_sui="{{ invocation_directory() }}/$nexus_next_sui" ;; esac; ./bin/regenerate_bindings.sh "$nexus_next_sui" "{{ grpc }}"

# Backward-compatible name for the old command.
regenerate-idents nexus_next_sui grpc='http://127.0.0.1:9000': _check-cargo
    @just sdk rebind "{{ nexus_next_sui }}" "{{ grpc }}"

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
