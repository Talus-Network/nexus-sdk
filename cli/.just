#
# $ just cli
#
# Commands related the Nexus CLI
#

import '../helpers/helpers.just'

package := 'nexus-cli'

[private]
_default:
    @just --list cli

# Build the CLI project
build: _check-cargo
    cargo +stable build --package {{ package }} --release

# Check the CLI project
check: _check-cargo
    cargo +stable check --package {{ package }}

# Run all tests in the CLI project
test: _check-cargo
    cargo +stable test --package {{ package }}

# Run rustfmt on the CLI project
fmt-check: _check-cargo
    #!/usr/bin/env bash

    nightly=$(just helpers::get-nightly-version)

    cargo +$nightly fmt --package {{ package }} --check

# Run clippy on the CLI project
clippy: _check-cargo
    cargo +stable clippy --package {{ package }}

# Publishes the CLI package to crates.io
publish: _check-cargo
    cargo +stable publish --package {{ package }}
    @echo "Published {{ package }} to crates.io"

# When released: `just cli run` == `nexus`
#
# Use the CLI locally for development purposes
[positional-arguments]
run *args: _check-cargo
    cargo +stable run --package {{ package }} -- "$@"
