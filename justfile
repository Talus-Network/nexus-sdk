#
# just
#
# Command runner for project-specific tasks.
# <https://github.com/casey/just>
#

# Commands concerning Nexus CLI
mod cli 'cli/.just'

# Commands concerning Nexus Toolkit for Rust
mod toolkit-rust 'toolkit-rust/.just'

# Commands concerning the Nexus SDK
mod sdk 'sdk/.just'

# Commands concerning native Nexus Tools
mod tools 'tools/.just'

# Pre-commit hooks
mod pre-commit '.pre-commit/.just'

# Helpers
mod helpers 'helpers/helpers.just'

[private]
_default:
    @just --list
