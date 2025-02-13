#
# $ just toolkit-rust
#
# Commands related the Nexus Rust Toolkit
#

import '../just/_helpers.just'

[private]
_default:
    @just --list toolkit-rust

# Build the Rust Toolkit project
build: _check-cargo
    cargo build --release


# Run all tests in the Rust Toolkit project
test: _check-cargo
    cargo test --package nexus-rust-toolkit -- tests --show-output
