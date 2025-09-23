#
# $ just toolkit-rust
#
# Commands related the Nexus Toolkit for Rust
#

import '../helpers/helpers.just'

package := 'nexus-toolkit'

[private]
_default:
    @just --list toolkit-rust

# Build the Toolkit for Rust project
build: _check-cargo
    cargo +stable build --package {{ package }} --release

# Check the Toolkit for Rust project
check: _check-cargo
    cargo +stable check --package {{ package }}

# Run all tests in the Toolkit for Rust project
test: _check-cargo
    cargo +stable test --package {{ package }}

# Run rustfmt on the Toolkit for Rust project
fmt-check: _check-cargo
    #!/usr/bin/env bash

    nightly="$(just helpers::get-nightly-version)"

    cargo +"$nightly" fmt --package {{ package }} --check

# Run clippy on the Toolkit for Rust project
clippy: _check-cargo
    cargo +stable clippy --package {{ package }}

# Publishes the Toolkit for RUst package to crates.io
publish: _check-cargo
    cargo +stable publish --package {{ package }}
    @echo "Published {{ package }} to crates.io"
