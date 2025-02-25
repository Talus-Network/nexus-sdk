#
# $ just toolkit-rust
#
# Commands related the Nexus Toolkit for Rust
#

import '../just/_helpers.just'

[private]
_default:
    @just --list toolkit-rust

# Build the Toolkit for Rust project
build: _check-cargo
    cargo build --package nexus-toolkit --release

# Check the Toolkit for Rust project
check: _check-cargo
    cargo check --package nexus-toolkit

# Run all tests in the Toolkit for Rust project
test: _check-cargo
    cargo test --package nexus-toolkit

# Run rustfmt on the Toolkit for Rust project
fmt-check: _check-cargo
    cargo +nightly fmt --package nexus-toolkit --check

# Run clippy on the Toolkit for Rust project
clippy: _check-cargo
    cargo clippy --package nexus-toolkit
