#
# $ just types
#
# Commands related the Nexus Types library.
#

import '../just/_helpers.just'

[private]
_default:
    @just --list types

# Check the Nexus Types library
check: _check-cargo
    cargo +stable check --features full --package nexus-sdk

# Run all tests in the Nexus Types library
test: _check-cargo
    cargo +stable test --features full --package nexus-sdk

# Run rustfmt on the Nexus Types library
fmt-check: _check-cargo
    cargo +nightly fmt --package nexus-sdk --check

# Run clippy on the Nexus Types library
clippy: _check-cargo
    cargo +stable clippy --package nexus-sdk
