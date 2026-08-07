#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
validator="$repo_root/.github/scripts/validate_release_tag.sh"
tmp_dir="$(mktemp -d)"

cleanup() {
    if [[ -n "$tmp_dir" && -d "$tmp_dir" ]]; then
        rm -rf -- "$tmp_dir"
    fi
}
trap cleanup EXIT

assert_release_metadata() {
    local version="$1"
    local tag="$2"
    local expected_prerelease="$3"
    local output_file="$tmp_dir/github-output"

    : > "$output_file"
    GITHUB_OUTPUT="$output_file" "$validator" "$version" "$tag"

    if ! grep -Fxq "prerelease=$expected_prerelease" "$output_file"; then
        echo "Expected prerelease=$expected_prerelease for $tag" >&2
        return 1
    fi
}

assert_release_metadata "2.0.0-rc.5" "v2.0.0-rc.5" "true"
assert_release_metadata "2.0.0" "v2.0.0" "false"

if GITHUB_OUTPUT="$tmp_dir/mismatch-output" \
    "$validator" "2.0.0-rc.5" "v2.0.0-rc.4" \
    > "$tmp_dir/mismatch-stdout" 2> "$tmp_dir/mismatch-stderr"; then
    echo "Expected a mismatched tag and package version to fail" >&2
    exit 1
fi

if ! grep -Fq \
    "Tag 'v2.0.0-rc.4' does not match package version '2.0.0-rc.5'" \
    "$tmp_dir/mismatch-stderr"; then
    echo "Expected the mismatch error to identify both versions" >&2
    exit 1
fi

echo "Release tag validation tests passed"
