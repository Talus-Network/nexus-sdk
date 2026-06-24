#
# $ just sdk
#
# Commands related the Nexus SDK.
#

import '../helpers/helpers.just'

package := 'nexus-sdk'

[private]
_default:
    @just --list sdk

# Check the Nexus SDK
check: _check-cargo
    cargo +stable check --all-features --package {{ package }}

# Run all tests in the Nexus SDK
test: _check-cargo
    cargo +stable test --all-features --package {{ package }}

# Regenerate the committed Move-binding IR (src/idents/generated/ir/*.json).
#
# Publishes the Nexus packages to a localnet via the nexus-next publish script,
# then fetches their normalized IR over gRPC. Requires a running Sui localnet
# (`sui start --with-faucet --force-regenesis`) whose active client env points at
# it with a funded active address, and the nexus-next sibling checked out.
#
# The offline render (IR JSON -> address-free idents) then happens automatically
# in build.rs on the next build; review the diff under src/idents/generated/ir/.
regenerate-idents nexus_next_sui='../../nexus-next/sui' grpc='http://127.0.0.1:9000': _check-cargo
    #!/usr/bin/env bash
    set -Eeuo pipefail

    objects="{{ nexus_next_sui }}/bin/target/objects.localnet.toml"

    echo "==> Publishing Nexus packages via {{ nexus_next_sui }}/bin/publish.sh"
    ( cd "{{ nexus_next_sui }}" && NEXUS_PUBLISH_OVERWRITE=1 ./bin/publish.sh publish )

    echo "==> Collecting package ids from $objects"
    nexus="$(grep -E '_pkg_id = ' "$objects" | sed -E 's/_pkg_id = "/=/; s/"//' | tr -d ' ' | paste -sd,)"
    # The 0x1/0x2 framework packages are present on every node.
    pkgs="$nexus,move_std=0x1,sui_framework=0x2"
    echo "    $pkgs"

    echo "==> Fetching IR over gRPC ({{ grpc }})"
    NEXUS_BINDING_GRPC_URL='{{ grpc }}' NEXUS_BINDING_PACKAGES="$pkgs" \
        cargo +stable run --package {{ package }} --features binding_codegen \
        --bin generate_binding

    echo "==> Done. Review the diff under sdk/src/idents/generated/ir/."

# Run rustfmt on the Nexus SDK
fmt-check: _check-cargo
    #!/usr/bin/env bash

    nightly="$(just helpers::get-nightly-version)"

    cargo +"$nightly" fmt --package {{ package }} --check

# Run clippy on the Nexus SDK
clippy: _check-cargo
    cargo +stable clippy --package {{ package }}

# Publishes the Nexus SDK package to crates.io
publish: _check-cargo
    cargo +stable publish --package {{ package }}
    @echo "Published {{ package }} to crates.io"
