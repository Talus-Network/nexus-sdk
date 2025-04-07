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
    cargo +stable build --package nexus-cli --release

# Install the CLI with `cargo install`
_install: build
    cargo +stable install --path . --bin nexus-cli

# Install the CLI as `nexus` alias in `~/.local/bin`
_install-alias: _install
    cp ../target/release/nexus-cli ~/.local/bin/nexus

# Check the CLI project
check: _check-cargo
    cargo +stable check --package nexus-cli

# Run all tests in the CLI project
test: _check-cargo
    cargo +stable test --package nexus-cli

# Run rustfmt on the CLI project
fmt-check: _check-cargo
    cargo +nightly fmt --package nexus-cli --check

# Run clippy on the CLI project
clippy: _check-cargo
    cargo +stable clippy --package nexus-cli

# When released: `just cli run` == `nexus`
#
# Use the CLI locally for development purposes
[positional-arguments]
run *args: _check-cargo
    cargo +stable run --package nexus-cli -- "$@"
