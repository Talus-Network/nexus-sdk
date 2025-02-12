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
    cargo build --release


# Run all tests in the CLI project
test: _check-cargo
    cargo test --package neuxus-cli -- tests --show-output

# Use the CLI locally for development purposes
# `$ just cli run tool new --name my-tool --template rust`
#
# When released: `just cli run` == `nexus`
run *args: _check-cargo
    cargo run -- {{args}}
