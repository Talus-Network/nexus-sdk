#
# $ just sdk
#
# Commands related the Nexus SDK.
#

import '../just/_helpers.just'

[private]
_default:
    @just --list sdk

# Check the Nexus SDK
check: _check-cargo
    cargo +stable check --all-features --package nexus-sdk

# Run all tests in the Nexus SDK
test: _check-cargo
    cargo +stable test --all-features --package nexus-sdk

# Run rustfmt on the Nexus SDK
fmt-check: _check-cargo
    cargo +nightly fmt --package nexus-sdk --check

# Run clippy on the Nexus SDK
clippy: _check-cargo
    cargo +stable clippy --package nexus-sdk
