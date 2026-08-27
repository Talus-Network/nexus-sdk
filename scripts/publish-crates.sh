#!/usr/bin/env bash

set -euo pipefail

release_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$release_root"

release_tag="${1:-}"
workspace_version="$(awk -F ' *= *' '$1 == "version" { gsub(/"/, "", $2); print $2; exit }' Cargo.toml)"
expected_tag="v$workspace_version"

if [[ -z "$release_tag" || "$release_tag" != "$expected_tag" ]]; then
    echo "Usage: $0 $expected_tag" >&2
    exit 1
fi

if [[ "${PUBLISH_CRATES_CONFIRM:-}" != "$expected_tag" ]]; then
    echo "Set PUBLISH_CRATES_CONFIRM=$expected_tag to authorize publication." >&2
    exit 1
fi

head_commit="$(git rev-parse HEAD)"
tag_commit="$(git rev-parse "${release_tag}^{commit}")"
if [[ "$(git cat-file -t "$release_tag")" != "tag" ]]; then
    echo "$release_tag must be an annotated tag." >&2
    exit 1
fi

if [[ "$head_commit" != "$tag_commit" ]]; then
    echo "$release_tag does not point to the current commit." >&2
    exit 1
fi

if [[ -n "$(git status --porcelain)" ]]; then
    echo "The work tree must be clean before publication." >&2
    exit 1
fi

git fetch --quiet origin "release/v2.0.x:refs/remotes/origin/release/v2.0.x"
if ! git merge-base --is-ancestor "$tag_commit" origin/release/v2.0.x; then
    echo "$release_tag must point to a commit on origin/release/v2.0.x." >&2
    exit 1
fi

./scripts/check-packages.sh

packages=(
    nexus-sdk
    nexus-toolkit
)

allow_existing=0
if [[ "${ALLOW_EXISTING:-0}" == "1" || "${ALLOW_EXISTING:-0}" == "true" ]]; then
    allow_existing=1
fi

api_resource_exists() {
    local api_path="$1"
    local http_status
    http_status="$(curl \
        --silent \
        --show-error \
        --output /dev/null \
        --write-out '%{http_code}' \
        --header 'User-Agent: Nexus SDK release (https://github.com/Talus-Network/nexus-sdk)' \
        "https://crates.io/api/v1/crates/$api_path")" || return 2

    case "$http_status" in
        200) return 0 ;;
        404) return 1 ;;
        *)
            echo "crates.io returned HTTP $http_status for $api_path." >&2
            return 2
            ;;
    esac
}

crate_exists() {
    api_resource_exists "$1/$workspace_version"
}

for package in "${packages[@]}"; do
    if crate_exists "$package"; then
        if [[ "$allow_existing" == "1" ]]; then
            echo "$package $workspace_version already exists and may be skipped."
        else
            echo "$package $workspace_version already exists." >&2
            echo "Use ALLOW_EXISTING=1 only to resume a reviewed partial publication." >&2
            exit 1
        fi
    elif [[ "$?" != "1" ]]; then
        exit 1
    fi
done

for package in "${packages[@]}"; do
    if crate_exists "$package"; then
        if [[ "$allow_existing" == "1" ]]; then
            echo "$package $workspace_version already exists; skipping it."
            continue
        fi
        echo "$package $workspace_version appeared after the initial check." >&2
        exit 1
    elif [[ "$?" != "1" ]]; then
        exit 1
    fi

    cargo publish --registry crates-io --locked -p "$package"

    for attempt in {1..60}; do
        if crate_exists "$package"; then
            break
        elif [[ "$?" != "1" ]]; then
            exit 1
        fi
        if [[ "$attempt" == "60" ]]; then
            echo "$package $workspace_version did not appear in the crates.io API." >&2
            exit 1
        fi
        sleep 5
    done
done

echo "Published all crates.io packages for $release_tag."
