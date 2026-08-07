#!/usr/bin/env bash

set -euo pipefail

event_name="$1"
ref_type="$2"
release_requested="$3"

create_release="false"

if [[ "$event_name" == "workflow_dispatch" && "$release_requested" == "true" ]]; then
    if [[ "$ref_type" != "tag" ]]; then
        echo "GitHub Release creation requires workflow_dispatch to target an existing tag" >&2
        exit 1
    fi

    create_release="true"
fi

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    printf 'create_release=%s\n' "$create_release" >> "$GITHUB_OUTPUT"
else
    printf 'create_release=%s\n' "$create_release"
fi
