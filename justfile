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

# Commands concerning Nexus Types library
mod types 'types/.just'

[private]
_default:
    @just --list
