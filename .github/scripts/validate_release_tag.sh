#!/usr/bin/env bash

set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <package-version> <git-tag>" >&2
    exit 2
fi

version="$1"
tag="$2"
expected_tag="v$version"

if [[ "$tag" != "$expected_tag" ]]; then
    echo "Tag '$tag' does not match package version '$version' (expected '$expected_tag')" >&2
    exit 1
fi

if [[ "$version" == *-* ]]; then
    prerelease="true"
else
    prerelease="false"
fi

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'prerelease=%s\n' "$prerelease" >> "$GITHUB_OUTPUT"
else
    printf 'prerelease=%s\n' "$prerelease"
fi
