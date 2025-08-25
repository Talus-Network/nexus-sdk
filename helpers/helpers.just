
[private]
_check-cargo:
    #!/usr/bin/env bash

    if ! command -v cargo &> /dev/null
    then
        echo "cargo could not be found"
        echo "Install cargo from https://doc.rust-lang.org/cargo/getting-started/installation.html"
        exit 1
    fi

[private]
_check-cargo-nextest: _check-cargo
    #!/usr/bin/env bash

    if ! command -v cargo-nextest &> /dev/null
    then
        echo "cargo-nextest could not be found"
        echo "Install cargo-nextest from https://nexte.st/"
        exit 1
    fi

[private]
_check-markdownlint-cli2:
    #!/usr/bin/env bash

    exe=markdownlint-cli2
    if ! command -v $exe &> /dev/null
    then
        echo "$exe could not be found"
        echo "Install $exe from https://github.com/DavidAnson/$exe"
        exit 1
    fi

[private]
_check-typos:
    #!/usr/bin/env bash

    exe=typos
    if ! command -v $exe &> /dev/null
    then
        echo "$exe could not be found"
        echo "Install $exe from https://github.com/crate-ci/typos"
        exit 1
    fi

[private]
detect-changes +GLOBS:
    #!/usr/bin/env bash

    set -euo pipefail

    GLOBS="{{GLOBS}}"
    REGEXES=() # technically not needed but useful for debugging
    BIG_REGEX=""

    function LOG() {
        echo "== $*" >&2
    }

    # Get list of changed files compared to last commit
    changed_files=$(git diff --name-only HEAD)
    LOG "Changed files: $changed_files"
    ## If GLOBS is not set, we just output the changed files
    #if [[ -z "${GLOBS}" ]]; then
    #  echo "$changed_files"
    #  exit 0
    #fi

    # Convert simple globs to regexes. By simple we mean `**` and `*` stuff.
    #
    # We support either separate globs:
    #
    #   '**/*.rs' '*.md'
    #
    # or one big string with space-separated globs:
    #
    #   '**/*.rs' '*.md'
    #
    # Both will give the same result, one big regex:
    #
    #   '^.*/[^/]*\.rs$|^[^/]*\.md$'
    #
    for glob in $(echo "$GLOBS"); do
        # Escape special characters and convert to regex
        regex=$(echo "$glob" |\
            # We do the following transformations:
            #   1. escape common regex meta
            #   2. create a temporary token for **
            #   3. convert * to [^/]* (matches any character except /)
            #   4. convert ** to .*
            sed -e 's/[.[\^$()+{}|]/\\&/g' \
                -e 's#\*\*#__GLOBSTAR__#g' \
                -e 's#\*#[^/]*#g' \
                -e 's#__GLOBSTAR__#.*#g')

        # match the whole string, so we add ^ and $ at the beginning and end
        regex="^$regex\$"

        if [[ -z "$BIG_REGEX" ]]; then
            BIG_REGEX="$regex"
        else
            BIG_REGEX+="|$regex"
        fi

        REGEXES+=("$regex")

        LOG "Glob: $glob => Regex: $regex"
    done

    LOG "Big regex: $BIG_REGEX"
    LOG ""

    grep -E "$BIG_REGEX" <<< "$changed_files"

# Contents of .nightly-version
get-nightly-version:
    #!/usr/bin/env bash

    # This file should contain the version of the nightly toolchain to use.
    cat ../.nightly-version
