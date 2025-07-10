import '../just/_helpers.just'

[private]
_default:
    @just --list pre-commit


# cargo_version == toolchain_version
check-toolchain-version: _check-cargo
    #!/usr/bin/env bash

    set -euo pipefail

    cd $(git rev-parse --show-toplevel)

    cargo_version=$(cargo --version | awk '{print $2}')
    toolchain_version=$(grep '^channel *= *"[^"]\+"' rust-toolchain.toml | sed -E 's/.*"([^"]+)".*/\1/')

    echo "Cargo version: $cargo_version"
    echo "Toolchain version: $toolchain_version"

    # cargo_version is 1.86.0 and toolchain_version is 1.86, so we need to compare them
    if [[ "$cargo_version" != "$toolchain_version"* ]]; then
      echo "❌ Cargo version ($cargo_version) does not match toolchain version ($toolchain_version)." >&2
      exit 1
    fi

    echo "Cargo version matches toolchain version."


# cargo check --locked --workspace --bins --examples
cargo-check: _check-cargo
    cargo check --locked --workspace --bins --examples


# cargo +nightly-2025-01-06 fmt --all --check
cargo-nightly-fmt: _check-cargo
    #!/usr/bin/env bash

    cargo +nightly-2025-01-06 fmt --all --check || {
        echo "❌ Formatting failed. Please run 'cargo +nightly fmt --all' to format the code." >&2
        exit 1
    }

# cargo clippy --all-targets --all-features
cargo-clippy: _check-cargo
    cargo clippy --all-targets --all-features


# TODO: Perhaps consider this stricter version in the future.
# cargo clippy --all-targets --all-features -- -D warnings
cargo-clippy-strict: _check-cargo
    cargo clippy --all-targets --all-features -- -D warnings


# cargo nextest run --locked --no-run --workspace --all-features --bins --examples --tests
cargo-nextest-build: _check-cargo-nextest
    cargo nextest run --locked --no-run --workspace --all-features --bins --examples --tests


# Note: The idea for `--fail-fast` is that the change we are commiting is the one responsible for
# the failure, so there is no need to keep running the remaining tests...
# cargo nextest run --locked --fail-fast --workspace --all-features --bins --examples --tests
cargo-nextest-run: _check-cargo-nextest
    # Note that tests require docker to be running.
    # TODO: Perhaps we should add a check for that?
    cargo nextest run --locked --fail-fast --workspace --all-features --bins --examples --tests


# markdownlint-cli2 '**/*.md' '!.git/' --config .markdownlint.json
md-lint: _check-markdownlint-cli2
    #!/usr/bin/env bash

    set -euo pipefail

    # Ensure we are in the root of the repository, makes things easier.
    cd $(git rev-parse --show-toplevel)

    markdownlint-cli2 '**/*.md' '!.git/' --config .markdownlint.json


check-shebang:
    #!/usr/bin/env bash

    set -euo pipefail

    error=0

    cd $(git rev-parse --show-toplevel)

    for f in $(git ls-files); do
        first_line=$(head -n 1 "$f")
        # see if the first line is a shell shebang
        if [[ "$first_line" =~ ^#!\ */bin/(ba|z|da|fi)?sh ]]; then
            echo "❌ Script $f has incorrect shebang line: $first_line" >&2
            error=1
        fi
    done

    exit $error


check-just-shebang:
    #!/usr/bin/env bash

    set -euo pipefail

    error=0

    cd $(git rev-parse --show-toplevel)

    for justfile in $(git ls-files |  grep -iE '.just|justfile'); do
        echo "Checking Justfile: $justfile"
        if grep -qE '^ +#! */(/usr/)?bin/(ba|z|da|fi)?sh' $justfile; then
            echo "❌ Justfile $justfile has incorrect shebang line." >&2
            error=1
        fi
    done

    exit $error


typos: _check-typos
    #!/usr/bin/env bash

    set -euo pipefail

    cd $(git rev-parse --show-toplevel)

    typos
