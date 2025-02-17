#
# $ just tools
#
# Commands related to native Nexus Tools.
#

import '../just/_helpers.just'

[private]
_default:
    @just --list tools

# Runs the `llm.opeanai.chat-completion` native Nexus Tool.
llm-openai-chat-completion: _check-cargo
    @cargo run --bin llm-openai-chat-completion
