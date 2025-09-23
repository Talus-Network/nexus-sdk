#!/usr/bin/env bash

set -euo pipefail

# Go to the root of the git repository
cd $(git rev-parse --show-toplevel)

# Install NPM stuff from helpers/npm-install-g.txt
while read -r tool_at_version; do
  # Split the tool name and version
  tool=$(echo "$tool_at_version" | cut -d'@' -f1)

  if ! command -v "$tool" &> /dev/null; then
    echo "Installing tool: $tool_at_version"
    npm install -g "$tool_at_version"
  else
    echo "Tool already installed: $tool_at_version"
  fi
done < helpers/npm-install-g.txt
