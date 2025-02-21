#
# $ just types
#
# Commands related the Nexus Types library.
#

import '../just/_helpers.just'

[private]
_default:
    @just --list types

# Build the Nexus Types library
build: _check-cargo
    cargo build --package nexus-types --release

# Check the Nexus Types library
check: _check-cargo
    cargo check --package nexus-types

# Run all tests in the Nexus Types library
test: _check-cargo
    cargo test --package nexus-types

# Run rustfmt on the Nexus Types library
fmt-check: _check-cargo
    cargo +nightly fmt --package nexus-types --check

# Run clippy on the Nexus Types library
clippy: _check-cargo
    cargo clippy --package nexus-types
