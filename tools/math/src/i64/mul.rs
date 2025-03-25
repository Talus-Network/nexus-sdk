//! # `xyz.taluslabs.math.i64.mul@1`
//!
//! Standard Nexus Tool that multiplies two [`i64`] numbers and returns the result.

use {
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    a: i64,
    b: i64,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { result: i64 },
    Err { reason: String },
}

pub(crate) struct I64Mul;

impl NexusTool for I64Mul {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.math.i64.mul@1")
    }

    fn path() -> &'static str {
        "/i64/mul"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        // This tool has no external dependencies and as such, it is always
        // healthy if the endpoint is reachable.
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, Self::Input { a, b }: Self::Input) -> Self::Output {
        match a.checked_mul(b) {
            Some(result) => Output::Ok { result },
            None => Output::Err {
                reason: format!("Multiplying '{a}' and '{b}' results in an overflow"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_i64_mul() {
        let tool = I64Mul::new().await;

        let input = Input { a: 2, b: 3 };
        let output = tool.invoke(input).await;

        assert!(matches!(output, Output::Ok { result: 6 }));

        let input = Input { a: i64::MAX, b: 2 };
        let output = tool.invoke(input).await;

        assert!(matches!(output, Output::Err { .. }));

        let input = Input { a: i64::MIN, b: 2 };
        let output = tool.invoke(input).await;

        assert!(matches!(output, Output::Err { .. }));

        assert!(matches!(tool.health().await, Ok(StatusCode::OK)));
    }
}
