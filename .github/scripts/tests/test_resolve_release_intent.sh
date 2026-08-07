#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
resolver="$repo_root/.github/scripts/resolve_release_intent.sh"
tmp_dir="$(mktemp -d)"

cleanup() {
    if [[ -n "$tmp_dir" && -d "$tmp_dir" ]]; then
        rm -rf -- "$tmp_dir"
    fi
}
trap cleanup EXIT

assert_release_intent() {
    local event_name="$1"
    local ref_type="$2"
    local release_requested="$3"
    local expected="$4"
    local output_file="$tmp_dir/${event_name}-${ref_type}-${release_requested}"

    GITHUB_OUTPUT="$output_file" \
        "$resolver" "$event_name" "$ref_type" "$release_requested"

    if ! grep -Fxq "create_release=$expected" "$output_file"; then
        echo "Expected create_release=$expected for $event_name/$ref_type/$release_requested" >&2
        return 1
    fi
}

assert_release_intent "pull_request" "branch" "false" "false"
assert_release_intent "push" "tag" "false" "false"
assert_release_intent "push" "tag" "true" "false"
assert_release_intent "workflow_dispatch" "branch" "false" "false"
assert_release_intent "workflow_dispatch" "tag" "false" "false"
assert_release_intent "workflow_dispatch" "tag" "true" "true"

if GITHUB_OUTPUT="$tmp_dir/invalid-branch-output" \
    "$resolver" "workflow_dispatch" "branch" "true" \
    > "$tmp_dir/invalid-branch-stdout" 2> "$tmp_dir/invalid-branch-stderr"; then
    echo "Expected a branch-scoped release request to fail" >&2
    exit 1
fi

if ! grep -Fq \
    "GitHub Release creation requires workflow_dispatch to target an existing tag" \
    "$tmp_dir/invalid-branch-stderr"; then
    echo "Expected the branch rejection to explain that a tag is required" >&2
    exit 1
fi

echo "Release intent tests passed"
