use {
    nexus_toolkit_rust::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Deserialize, JsonSchema)]
struct Input {
    // Add your Tool's input ports
}

#[derive(Serialize, JsonSchema)]
enum Output {
    Ok {
        // Add output ports for the `Ok` variant
    },
    // Add more output variants if needed
}

struct OpenaiChatCompletion;

impl NexusTool for OpenaiChatCompletion {
    type Input = Input;
    type Output = Output;

    fn fqn() -> &'static str {
        "xyz.taluslabs.llm.openai.chat-completion@1"
    }

    async fn health() -> AnyResult<StatusCode> {
        // The health endpoint should perform health checks on its dependencies.

        Ok(StatusCode::OK)
    }

    async fn invoke(_: Self::Input) -> AnyResult<Self::Output> {
        // Tool logic goes here.

        Ok(Output::Ok {})
    }
}

#[tokio::main]
async fn main() {
    bootstrap::<OpenaiChatCompletion>(([127, 0, 0, 1], 8080)).await;
}
