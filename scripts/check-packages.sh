#!/usr/bin/env bash

set -euo pipefail

release_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$release_root"

workspace_version="$(awk -F ' *= *' '$1 == "version" { gsub(/"/, "", $2); print $2; exit }' Cargo.toml)"
if [[ -z "$workspace_version" ]]; then
    echo "Could not read the workspace version." >&2
    exit 1
fi

packages=(
    nexus-sdk
    nexus-toolkit
)

# The toolkit cannot resolve the new SDK version through crates.io before the
# first publication. This local patch verifies the exact toolkit archive.
cargo_config_args=(
    --config "patch.crates-io.nexus-sdk.path=\"$release_root/sdk\""
)

package_args=(--quiet --locked --all-features)
if [[ "${ALLOW_DIRTY:-0}" == "1" || "${ALLOW_DIRTY:-0}" == "true" ]]; then
    package_args+=(--allow-dirty)
fi

target_root="${CARGO_TARGET_DIR:-$release_root/target}"

for package in "${packages[@]}"; do
    echo "Checking $package $workspace_version"
    cargo "${cargo_config_args[@]}" package \
        -p "$package" \
        "${package_args[@]}"

    archive="$target_root/package/$package-$workspace_version.crate"
    archive_root="$package-$workspace_version"
    archive_manifest="$archive_root/Cargo.toml"
    if [[ ! -f "$archive" ]]; then
        echo "Cargo did not create $archive." >&2
        exit 1
    fi

    for required_file in Cargo.toml README.md; do
        if ! tar -tf "$archive" | grep -Fx "$archive_root/$required_file" >/dev/null; then
            echo "$package does not contain $required_file." >&2
            exit 1
        fi
    done

    if tar -xOf "$archive" "$archive_manifest" | awk '
        /^\[/ {
            dependency_section = ($0 ~ /(^\[|\.)((build|dev)-)?dependencies(\.|\])/)
        }
        dependency_section && /^[[:space:]]*(git|path|registry)[[:space:]]*=/ {
            source_dependency = 1
        }
        END { exit(source_dependency ? 0 : 1) }
    '; then
        echo "$package contains a source dependency in its published manifest." >&2
        exit 1
    fi
done

echo "All package archives passed verification."
