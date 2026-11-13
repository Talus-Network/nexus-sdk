#!/usr/bin/env bash

set -euo pipefail

release_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version="$(awk -F ' *= *' '$1 == "version" { gsub(/"/, "", $2); print $2; exit }' "$release_root/Cargo.toml")"
target_root="${CARGO_TARGET_DIR:-$release_root/target}"
if [[ "$target_root" != /* ]]; then
    target_root="$release_root/$target_root"
fi
export CARGO_TARGET_DIR="$target_root"

consumer_root="$(mktemp -d)"
trap 'rm -rf "$consumer_root"' EXIT
cd "$consumer_root"
mkdir -p sdk/src toolkit/src
cat > Cargo.toml <<'TOML'
[workspace]
members = ["sdk", "toolkit"]
resolver = "3"
TOML

case "${1:-}" in
    "")
        printf '\n[patch.crates-io]\n' >> Cargo.toml
        for package in nexus-sdk nexus-toolkit; do
            tar -xzf "$target_root/package/$package-$version.crate"
            printf '%s = { path = "%s-%s" }\n' "$package" "$package" "$version" >> Cargo.toml
        done
        ;;
    --registry) ;;
    *) echo "Usage: $0 [--registry]" >&2; exit 1 ;;
esac

cat > sdk/Cargo.toml <<TOML
[package]
name = "sdk-consumer"
version = "0.0.0"
edition = "2021"

[dependencies]
nexus-sdk = "=$version"

[features]
full = ["nexus-sdk/full", "nexus-sdk/move_publish", "nexus-sdk/remote-signer"]
TOML
printf 'pub use nexus_sdk::fqn;\n' > sdk/src/lib.rs

cat > toolkit/Cargo.toml <<TOML
[package]
name = "toolkit-consumer"
version = "0.0.0"
edition = "2021"

[dependencies]
nexus-sdk = "=$version"
nexus-toolkit = "=$version"
TOML
printf 'pub use nexus_sdk::fqn;\npub use nexus_toolkit::NexusTool;\n' > toolkit/src/lib.rs

cargo check -p sdk-consumer
cargo check -p sdk-consumer --features full
cargo check -p toolkit-consumer
