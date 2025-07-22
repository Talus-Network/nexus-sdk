import '../just/_helpers.just'

[private]
_default:
    @just --list pre-commit


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


check-docker: _check-docker
    #!/usr/bin/env bash

    if ! docker info > /dev/null 2>&1; then
        echo "❌ Docker is not running. Please start Docker and try again." >&2
        exit 1
    fi


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
    # TODO: https://github.com/Talus-Network/nexus-sdk/issues/254
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
        # optimization: only check executable files
        if [[ ! -x "$f" ]]; then
            continue
        fi

        first_line=$(head -n 1 "$f")

        # see if the first line is a shell shebang
        if [[ "$first_line" =~ ^(#! *) ]]; then
            if ! [[ "$first_line" =~ ^(#! */usr/bin/env *) ]]; then
                echo "❌  [$first_line] $f" >&2
                error=1
            else
                echo "✅  [$first_line] $f" >&2
            fi
        fi
    done

    exit $error


check-just-shebang:
    #!/usr/bin/env bash

    error=0

    cd $(git rev-parse --show-toplevel)

    for justfile in $(git ls-files | grep -iE '(\.just|justfile)$'); do
        justfile_errors=0
        recipe=""

        while IFS= read -r line; do
            # a recipe
            if [[ $line =~ ^([a-zA-Z0-9_-]+: *) ]]; then
                recipe=$line
                # skip the first line, which is the recipe
                continue
            fi

            # a shebang
            if [[ $line =~ ^( *#! *) ]] && [[ -n "$recipe" ]]; then
                if [[ $line =~ ^( *#! */usr/bin/env +) ]]; then
                    echo "   ✅  $justfile [$line] [$recipe]" >&2
                else
                    ((justfile_errors++))
                    echo "   ❌  $justfile [$line] [$recipe]" >&2
                fi

                recipe=""

            fi
        done < "$justfile"

        if [[ $justfile_errors -gt 0 ]]; then
            echo "❌  [$justfile_errors error(s)] $justfile" >&2
            error=1
        else
            echo "✅  $justfile" >&2
        fi
    done

    exit $error


typos: _check-typos
    #!/usr/bin/env bash

    set -euo pipefail

    cd $(git rev-parse --show-toplevel)

    typos
