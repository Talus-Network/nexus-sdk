#
# just
#
# Command runner for project-specific tasks.
# <https://github.com/casey/just>
#

# Commands concerning Nexus CLI
mod cli 'cli/.just'

# Commands concerning Nexus Rust Toolkit
mod toolkit-rust 'toolkit-rust/.just'

[private]
_default:
    @just --list
