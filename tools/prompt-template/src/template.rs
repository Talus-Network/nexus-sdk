//! # `xyz.taluslabs.prompt.template.new@1`
//!
//! Tool that can create new prompt templates.

use {
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {}

/// Output model for the HTTP Generic tool
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {},
    // Err {},
}

pub(crate) struct PromptTemplate;

impl NexusTool for PromptTemplate {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.prompt.template.new@1")
    }

    fn path() -> &'static str {
        "/new"
    }

    fn description() -> &'static str {
        "Tool that can create new prompt templates."
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        Output::Ok {}
    }
}
