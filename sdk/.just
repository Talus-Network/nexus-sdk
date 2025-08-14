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
