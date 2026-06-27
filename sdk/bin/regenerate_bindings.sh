#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
SDK_CRATE_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
SDK_WORKSPACE_DIR="$(cd -- "$SDK_CRATE_DIR/.." && pwd -P)"

NEXUS_SUI_DIR="${NEXUS_SUI_DIR:-${1:-$SDK_WORKSPACE_DIR/../nexus/sui}}"
NEXUS_BINDING_GRPC_URL="${NEXUS_BINDING_GRPC_URL:-${2:-http://127.0.0.1:9000}}"
NEXUS_BINDING_OBJECTS_FILE="${NEXUS_BINDING_OBJECTS_FILE:-}"
NEXUS_BINDING_SKIP_PUBLISH="${NEXUS_BINDING_SKIP_PUBLISH:-0}"
NEXUS_BINDING_SDK_CHECK="${NEXUS_BINDING_SDK_CHECK:-1}"
NEXUS_BINDING_AUTO_START_SUI="${NEXUS_BINDING_AUTO_START_SUI:-1}"
NEXUS_BINDING_KEEP_SUI="${NEXUS_BINDING_KEEP_SUI:-0}"
NEXUS_BINDING_SUI_ENV="${NEXUS_BINDING_SUI_ENV:-localnet}"
NEXUS_BINDING_SUI_RPC_URL="${NEXUS_BINDING_SUI_RPC_URL:-$NEXUS_BINDING_GRPC_URL}"
NEXUS_BINDING_SUI_START_ARGS="${NEXUS_BINDING_SUI_START_ARGS:---with-faucet --force-regenesis}"
NEXUS_BINDING_SUI_START_TIMEOUT_SECS="${NEXUS_BINDING_SUI_START_TIMEOUT_SECS:-90}"
NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS="${NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS:-20}"
NEXUS_BINDING_SUI_LOG="${NEXUS_BINDING_SUI_LOG:-}"
NEXUS_BINDING_SUI_HOME="${NEXUS_BINDING_SUI_HOME:-/tmp/nexus-sdk-sui-home}"
NEXUS_BINDING_TMPDIR="${NEXUS_BINDING_TMPDIR:-/tmp}"
NEXUS_BINDING_XDG_RUNTIME_DIR="${NEXUS_BINDING_XDG_RUNTIME_DIR:-/tmp/nexus-sdk-runtime-dir}"
NEXUS_BINDING_MOVE_HOME="${NEXUS_BINDING_MOVE_HOME:-/tmp/nexus-sdk-move-home}"
NEXUS_BINDING_CARGO_TARGET_DIR="${NEXUS_BINDING_CARGO_TARGET_DIR:-${CARGO_TARGET_DIR:-/tmp/nexus-sdk-bindings-target}}"
SUI_START_PID=""

on_error() {
    local exit_code=$?
    local line="${1:-unknown}"
    printf 'ERROR: regenerate_bindings.sh failed at line %s with exit code %s\n' "$line" "$exit_code" >&2
    exit "$exit_code"
}
trap 'on_error "$LINENO"' ERR
trap cleanup EXIT

usage() {
    cat <<'EOF'
Usage: sdk/bin/regenerate_bindings.sh [NEXUS_SUI_DIR] [GRPC_URL]

Publishes Nexus Move packages, refreshes sdk/src/idents/generated/ir/*.json from the published package IDs, and checks that the SDK builds from the refreshed JSON.

Environment:
  NEXUS_SUI_DIR                 Nexus sui directory. Default: ../../nexus/sui from the nexus-sdk workspace
  NEXUS_BINDING_GRPC_URL        gRPC URL used by SDK binding codegen. Default: http://127.0.0.1:9000
  NEXUS_BINDING_OBJECTS_FILE    Published objects TOML. Default: $NEXUS_SUI_DIR/bin/target/objects.localnet.toml
  NEXUS_BINDING_SKIP_PUBLISH    Set to 1 to reuse an existing objects TOML instead of publishing. Default: 0
  NEXUS_BINDING_SDK_CHECK       Set to 0 to skip cargo check after JSON refresh. Default: 1
  NEXUS_BINDING_AUTO_START_SUI  Set to 0 to require an already-running local Sui env. Default: 1
  NEXUS_BINDING_KEEP_SUI        Set to 1 to leave an auto-started Sui process running after exit. Default: 0
  NEXUS_BINDING_SUI_ENV         Sui client env alias used for publish/faucet. Default: localnet
  NEXUS_BINDING_SUI_RPC_URL     RPC URL used for the local Sui env. Default: NEXUS_BINDING_GRPC_URL
  NEXUS_BINDING_SUI_START_ARGS  Args passed to `sui start`. Default: --with-faucet --force-regenesis
  NEXUS_BINDING_SUI_START_TIMEOUT_SECS
                                Seconds to wait for an auto-started local Sui env. Default: 90
  NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS
                                Extra seconds to wait after RPC readiness so faucet can bind. Default: 20
  NEXUS_BINDING_SUI_LOG         Log file for auto-started Sui. Default: /tmp/nexus-sdk-sui-localnet.<pid>.log
  NEXUS_BINDING_SUI_HOME        HOME used by Sui CLI/start. Default: /tmp/nexus-sdk-sui-home
  NEXUS_BINDING_TMPDIR          TMPDIR used by Sui/publish/cargo. Default: /tmp
  NEXUS_BINDING_XDG_RUNTIME_DIR XDG_RUNTIME_DIR used by Sui/publish/cargo. Default: /tmp/nexus-sdk-runtime-dir
  NEXUS_BINDING_MOVE_HOME       MOVE_HOME used by Sui Move caches. Default: /tmp/nexus-sdk-move-home
  NEXUS_BINDING_CARGO_TARGET_DIR
                                Cargo target dir for generator/check. Default: CARGO_TARGET_DIR or /tmp/nexus-sdk-bindings-target
  NEXUS_BINDING_NORMALIZE_PACKAGE_IDS
                                Replace locally published package IDs in committed IR JSON with stable placeholders. Default: 1
EOF
}

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'Missing required command: %s\n' "$1" >&2
        exit 1
    fi
}

env_flag_enabled() {
    local label="$1"
    local value="$2"

    case "$value" in
    1 | true | TRUE | yes | YES)
        return 0
        ;;
    0 | false | FALSE | no | NO | '')
        return 1
        ;;
    *)
        printf '%s must be 0/1, true/false, or yes/no: %s\n' "$label" "$value" >&2
        exit 1
        ;;
    esac
}

sui_cli() {
    HOME="$NEXUS_BINDING_SUI_HOME" \
        TMPDIR="$NEXUS_BINDING_TMPDIR" \
        XDG_RUNTIME_DIR="$NEXUS_BINDING_XDG_RUNTIME_DIR" \
        MOVE_HOME="$NEXUS_BINDING_MOVE_HOME" \
        sui "$@"
}

cleanup() {
    if [ -n "$SUI_START_PID" ] && ! env_flag_enabled NEXUS_BINDING_KEEP_SUI "$NEXUS_BINDING_KEEP_SUI"; then
        if kill -0 "$SUI_START_PID" 2>/dev/null; then
            printf '==> Stopping auto-started Sui localnet (pid %s)\n' "$SUI_START_PID" >&2
            kill "$SUI_START_PID" 2>/dev/null || true
            wait "$SUI_START_PID" 2>/dev/null || true
        fi
    fi
}

resolve_inputs() {
    local requested_nexus_sui_dir

    if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
        usage
        exit 0
    fi
    if [ "$#" -gt 2 ]; then
        printf 'Unsupported argument: %s\n' "$3" >&2
        usage >&2
        exit 1
    fi

    requested_nexus_sui_dir="$NEXUS_SUI_DIR"
    NEXUS_SUI_DIR="$(cd -- "$NEXUS_SUI_DIR" && pwd -P)" || {
        printf 'NEXUS_SUI_DIR does not exist: %s\n' "$requested_nexus_sui_dir" >&2
        exit 1
    }
    mkdir -p "$NEXUS_BINDING_SUI_HOME"
    mkdir -p "$NEXUS_BINDING_TMPDIR" "$NEXUS_BINDING_XDG_RUNTIME_DIR" "$NEXUS_BINDING_MOVE_HOME" "$NEXUS_BINDING_CARGO_TARGET_DIR"
    if [ ! -x "$NEXUS_SUI_DIR/bin/publish.sh" ]; then
        printf 'Missing executable Nexus publish script: %s/bin/publish.sh\n' "$NEXUS_SUI_DIR" >&2
        exit 1
    fi
    if [ -z "$NEXUS_BINDING_OBJECTS_FILE" ]; then
        NEXUS_BINDING_OBJECTS_FILE="$NEXUS_SUI_DIR/bin/target/objects.localnet.toml"
    fi
    case "$NEXUS_BINDING_OBJECTS_FILE" in
    /*) ;;
    *) NEXUS_BINDING_OBJECTS_FILE="$NEXUS_SUI_DIR/$NEXUS_BINDING_OBJECTS_FILE" ;;
    esac

    env_flag_enabled NEXUS_BINDING_SKIP_PUBLISH "$NEXUS_BINDING_SKIP_PUBLISH" || true
    env_flag_enabled NEXUS_BINDING_SDK_CHECK "$NEXUS_BINDING_SDK_CHECK" || true
    env_flag_enabled NEXUS_BINDING_AUTO_START_SUI "$NEXUS_BINDING_AUTO_START_SUI" || true
    env_flag_enabled NEXUS_BINDING_KEEP_SUI "$NEXUS_BINDING_KEEP_SUI" || true
    case "$NEXUS_BINDING_SUI_START_TIMEOUT_SECS" in
    '' | *[!0-9]*)
        printf 'NEXUS_BINDING_SUI_START_TIMEOUT_SECS must be an unsigned integer: %s\n' "$NEXUS_BINDING_SUI_START_TIMEOUT_SECS" >&2
        exit 1
        ;;
    esac
    case "$NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS" in
    '' | *[!0-9]*)
        printf 'NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS must be an unsigned integer: %s\n' "$NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS" >&2
        exit 1
        ;;
    esac
}

ensure_sui_client_env() {
    local envs_json

    if sui_cli client --client.env "$NEXUS_BINDING_SUI_ENV" chain-identifier >/dev/null 2>&1; then
        return
    fi

    envs_json="$(sui_cli client envs --json 2>/dev/null || true)"
    if ! python3 -c '
import json
import sys

target = sys.argv[1]
raw = sys.argv[2]
try:
    payload = json.loads(raw)
except Exception:
    raise SystemExit(1)

envs = payload[0] if isinstance(payload, list) and payload else payload
for env in envs:
    if isinstance(env, dict) and env.get("alias") == target:
        raise SystemExit(0)
raise SystemExit(1)
    ' "$NEXUS_BINDING_SUI_ENV" "$envs_json"
    then
        printf '==> Creating Sui client env %s -> %s\n' "$NEXUS_BINDING_SUI_ENV" "$NEXUS_BINDING_SUI_RPC_URL"
        sui_cli client -y new-env --alias "$NEXUS_BINDING_SUI_ENV" --rpc "$NEXUS_BINDING_SUI_RPC_URL" >/dev/null
    fi
}

local_sui_env_is_up() {
    sui_cli client --client.env "$NEXUS_BINDING_SUI_ENV" chain-identifier >/dev/null 2>&1
}

sui_rpc_port_is_open() {
    python3 - "$NEXUS_BINDING_SUI_RPC_URL" <<'PY'
import socket
import sys
from urllib.parse import urlparse

url = urlparse(sys.argv[1])
host = url.hostname or "127.0.0.1"
port = url.port
if port is None:
    port = 443 if url.scheme == "https" else 80

try:
    with socket.create_connection((host, port), timeout=1.0):
        pass
except OSError:
    raise SystemExit(1)
PY
}

start_sui_localnet() {
    local -a start_args=()
    local deadline now

    if local_sui_env_is_up; then
        return
    fi
    if ! env_flag_enabled NEXUS_BINDING_AUTO_START_SUI "$NEXUS_BINDING_AUTO_START_SUI"; then
        printf 'Sui env %s is not reachable. Start localnet or set NEXUS_BINDING_AUTO_START_SUI=1.\n' "$NEXUS_BINDING_SUI_ENV" >&2
        exit 1
    fi

    if [ -z "$NEXUS_BINDING_SUI_LOG" ]; then
        NEXUS_BINDING_SUI_LOG="/tmp/nexus-sdk-sui-localnet.$$.log"
    fi
    read -r -a start_args <<<"$NEXUS_BINDING_SUI_START_ARGS"
    printf '==> Starting Sui localnet: sui start %s\n' "$NEXUS_BINDING_SUI_START_ARGS"
    printf '    sui home: %s\n' "$NEXUS_BINDING_SUI_HOME"
    printf '    log: %s\n' "$NEXUS_BINDING_SUI_LOG"
    sui_cli start "${start_args[@]}" >"$NEXUS_BINDING_SUI_LOG" 2>&1 &
    SUI_START_PID="$!"
    printf '    pid: %s\n' "$SUI_START_PID"

    deadline=$((SECONDS + NEXUS_BINDING_SUI_START_TIMEOUT_SECS))
    while true; do
        if sui_rpc_port_is_open; then
            ensure_sui_client_env
        fi
        if local_sui_env_is_up; then
            printf '==> Sui env %s is ready\n' "$NEXUS_BINDING_SUI_ENV"
            if [ "$NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS" -gt 0 ]; then
                printf '==> Waiting %s seconds for Sui auxiliary services\n' "$NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS"
                sleep "$NEXUS_BINDING_SUI_POST_READY_SLEEP_SECS"
            fi
            return
        fi
        if ! kill -0 "$SUI_START_PID" 2>/dev/null; then
            printf 'Auto-started Sui localnet exited before becoming ready. Log follows:\n' >&2
            cat "$NEXUS_BINDING_SUI_LOG" >&2 || true
            exit 1
        fi
        now="$SECONDS"
        if [ "$now" -ge "$deadline" ]; then
            printf 'Timed out waiting for Sui env %s after %s seconds. Log follows:\n' "$NEXUS_BINDING_SUI_ENV" "$NEXUS_BINDING_SUI_START_TIMEOUT_SECS" >&2
            cat "$NEXUS_BINDING_SUI_LOG" >&2 || true
            exit 1
        fi
        sleep 2
    done
}

ensure_sui_localnet() {
    if ! local_sui_env_is_up; then
        start_sui_localnet
    fi
    ensure_sui_client_env
}

publish_nexus_packages() {
    if env_flag_enabled NEXUS_BINDING_SKIP_PUBLISH "$NEXUS_BINDING_SKIP_PUBLISH"; then
        if [ ! -f "$NEXUS_BINDING_OBJECTS_FILE" ]; then
            printf 'NEXUS_BINDING_SKIP_PUBLISH=1 but objects TOML does not exist: %s\n' "$NEXUS_BINDING_OBJECTS_FILE" >&2
            exit 1
        fi
        return
    fi

    printf '==> Publishing Nexus packages via %s/bin/publish.sh\n' "$NEXUS_SUI_DIR"
    (
        cd "$NEXUS_SUI_DIR"
        NEXUS_PUBLISH_OVERWRITE=1 \
            NEXUS_SUI_HOME="$NEXUS_BINDING_SUI_HOME" \
            TMPDIR="$NEXUS_BINDING_TMPDIR" \
            XDG_RUNTIME_DIR="$NEXUS_BINDING_XDG_RUNTIME_DIR" \
            MOVE_HOME="$NEXUS_BINDING_MOVE_HOME" \
            SUI_ENV="${SUI_ENV:-$NEXUS_BINDING_SUI_ENV}" \
            NEXUS_SUI_CLIENT_ENV_MODE="${NEXUS_SUI_CLIENT_ENV_MODE:-explicit}" \
            ./bin/publish.sh publish
    )
}

binding_package_spec_from_objects() {
    python3 - "$NEXUS_BINDING_OBJECTS_FILE" <<'PY'
import re
import sys
from pathlib import Path

objects_file = Path(sys.argv[1])
text = objects_file.read_text()
required = ["primitives", "interface", "registry", "workflow", "scheduler"]
package_ids = {}

for line in text.splitlines():
    match = re.match(r'^([a-z_]+)_pkg_id\s*=\s*"([^"]+)"\s*$', line.strip())
    if match:
        package_ids[match.group(1)] = match.group(2)

missing = [name for name in required if not package_ids.get(name)]
if missing:
    raise SystemExit(f"{objects_file} is missing package IDs for: {', '.join(missing)}")

entries = [f"{name}={package_ids[name]}" for name in required]
entries.extend(["move_std=0x1", "sui_framework=0x2"])
print(",".join(entries))
PY
}

validate_binding_json() {
    local ir_dir="$SDK_CRATE_DIR/src/idents/generated/ir"

    python3 - "$ir_dir" <<'PY'
import json
import sys
from pathlib import Path

ir_dir = Path(sys.argv[1])
required = ["primitives", "interface", "registry", "workflow", "scheduler", "move_std", "sui_framework"]
for name in required:
    path = ir_dir / f"{name}.json"
    if not path.is_file() or path.stat().st_size == 0:
        raise SystemExit(f"missing or empty generated binding JSON: {path}")
    with path.open() as handle:
        data = json.load(handle)
    if not isinstance(data.get("modules"), dict):
        raise SystemExit(f"generated binding JSON missing modules object: {path}")
PY
}

refresh_bindings() {
    local package_spec

    if [ ! -f "$NEXUS_BINDING_OBJECTS_FILE" ]; then
        printf 'Published objects TOML does not exist: %s\n' "$NEXUS_BINDING_OBJECTS_FILE" >&2
        exit 1
    fi

    package_spec="$(binding_package_spec_from_objects)"
    printf '==> Fetching IR over gRPC (%s)\n' "$NEXUS_BINDING_GRPC_URL"
    printf '    %s\n' "$package_spec"
    (
        cd "$SDK_WORKSPACE_DIR"
        TMPDIR="$NEXUS_BINDING_TMPDIR" \
            XDG_RUNTIME_DIR="$NEXUS_BINDING_XDG_RUNTIME_DIR" \
            MOVE_HOME="$NEXUS_BINDING_MOVE_HOME" \
            CARGO_TARGET_DIR="$NEXUS_BINDING_CARGO_TARGET_DIR" \
            NEXUS_BINDING_GRPC_URL="$NEXUS_BINDING_GRPC_URL" \
            NEXUS_BINDING_PACKAGES="$package_spec" \
            NEXUS_BINDING_NEXUS_SUI_DIR="$NEXUS_SUI_DIR" \
            cargo +stable run --package nexus-sdk --features binding_codegen --bin generate_binding
    )

    validate_binding_json
}

check_sdk_build() {
    if ! env_flag_enabled NEXUS_BINDING_SDK_CHECK "$NEXUS_BINDING_SDK_CHECK"; then
        return
    fi

    printf '==> Checking SDK build against refreshed binding JSON\n'
    (
        cd "$SDK_WORKSPACE_DIR"
        TMPDIR="$NEXUS_BINDING_TMPDIR" \
            XDG_RUNTIME_DIR="$NEXUS_BINDING_XDG_RUNTIME_DIR" \
            MOVE_HOME="$NEXUS_BINDING_MOVE_HOME" \
            CARGO_TARGET_DIR="$NEXUS_BINDING_CARGO_TARGET_DIR" \
            cargo +stable check --all-features --package nexus-sdk
    )
}

main() {
    require_command cargo
    require_command python3
    require_command sui
    resolve_inputs "$@"
    ensure_sui_localnet
    publish_nexus_packages
    refresh_bindings
    check_sdk_build
    printf '==> SDK binding JSON refresh complete\n'
}

main "$@"
