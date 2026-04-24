#
# $ just dag-dsl
#
# Commands related to the Nexus DAG DSL.
#

import '../helpers/helpers.just'

package := 'nexus-dag-dsl'

[private]
_default:
    @just --list dag-dsl

# Check the Nexus DAG DSL
check: _check-cargo
    cargo +stable check --all-features --package {{ package }}

# Run all tests in the Nexus DAG DSL
test: _check-cargo
    cargo +stable test --all-features --package {{ package }}

# Run rustfmt on the Nexus DAG DSL
fmt-check: _check-cargo
    #!/usr/bin/env bash

    nightly="$(just helpers::get-nightly-version)"

    cargo +"$nightly" fmt --package {{ package }} --check

# Run clippy on the Nexus DAG DSL
clippy: _check-cargo
    cargo +stable clippy --package {{ package }}
