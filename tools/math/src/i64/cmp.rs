//! # `xyz.taluslabs.math.i64.cmp@1`
//!
//! Standard Nexus Tool that compares two [`i64`] numbers and returns the result.

use {
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::cmp::Ordering,
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
    Gt { a: i64, b: i64 },
    Eq { a: i64, b: i64 },
    Lt { a: i64, b: i64 },
}

pub(crate) struct I64Cmp;

impl NexusTool for I64Cmp {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.math.i64.cmp@1")
    }

    fn path() -> &'static str {
        "/i64/cmp"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        // This tool has no external dependencies and as such, it is always
        // healthy if the endpoint is reachable.
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, Self::Input { a, b }: Self::Input) -> Self::Output {
        match a.cmp(&b) {
            Ordering::Greater => Output::Gt { a, b },
            Ordering::Equal => Output::Eq { a, b },
            Ordering::Less => Output::Lt { a, b },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_i64_cmp() {
        let tool: I64Cmp = I64Cmp::new().await;

        let input = Input { a: 1, b: 2 };
        let output = tool.invoke(input).await;

        assert!(matches!(output, Output::Lt { a: 1, b: 2 }));

        let input = Input { a: 2, b: 1 };
        let output = tool.invoke(input).await;

        assert!(matches!(output, Output::Gt { a: 2, b: 1 }));

        let input = Input { a: 1, b: 1 };
        let output = tool.invoke(input).await;

        assert!(matches!(output, Output::Eq { a: 1, b: 1 }));
    }
}
