#
# $ just dsl-codegen
#
# Commands related to the Nexus DSL codegen.
#

import '../helpers/helpers.just'

package := 'nexus-dsl-codegen'

[private]
_default:
    @just --list dsl-codegen

# Check the Nexus DSL codegen
check: _check-cargo
    cargo +stable check --all-features --package {{ package }}

# Run all tests
test: _check-cargo
    cargo +stable test --all-features --package {{ package }}

# Run rustfmt
fmt-check: _check-cargo
    #!/usr/bin/env bash

    nightly="$(just helpers::get-nightly-version)"

    cargo +"$nightly" fmt --package {{ package }} --check

# Run clippy
clippy: _check-cargo
    cargo +stable clippy --package {{ package }}

# Run the codegen binary locally
run *args: _check-cargo
    cargo +stable run --package {{ package }} -- {{ args }}
