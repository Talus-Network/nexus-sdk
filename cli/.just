#
# $ just cli
#
# Commands related the Nexus CLI
#

import '../just/_helpers.just'

[private]
_default:
    @just --list cli

# Build the CLI project
build: _check-cargo
    cargo build --package nexus-cli --release


# Run all tests in the CLI project
test: _check-cargo
    cargo test --package nexus-cli

# Run rustfmt on the CLI project
fmt-check: _check-cargo
    cargo +nightly fmt --package nexus-cli --check

# Run clippy on the CLI project
clippy: _check-cargo
    cargo clippy --package nexus-cli

# Use the CLI locally for development purposes
# `$ just cli run tool new --name my-tool --template rust`
#
# When released: `just cli run` == `nexus`
run *args: _check-cargo
    cargo run --package nexus-cli -- {{args}}
