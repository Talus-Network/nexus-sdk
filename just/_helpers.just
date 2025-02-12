
[private]
_check-cargo:
    #!/bin/bash
    if ! command -v cargo &> /dev/null
    then
        echo "cargo could not be found"
        echo "Install cargo from https://doc.rust-lang.org/cargo/getting-started/installation.html"
        exit 1
    fi
