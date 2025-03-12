//! # `xyz.taluslabs.math.i64.cmp@1`
//!
//! Standard Nexus Tool that compares two [i64] numbers and returns the result.
//!
//! ## Input
//!
//! - `a: i64`: The first number to compare.
//! - `b: i64`: The second number to compare.
//!
//! ## Output Variants
//!
//! - `gt`: The first number is greater than the second.
//! - `eq`: The first number is equal to the second.
//! - `lt`: The first number is less than the second.
//!
//! ## Output Ports
//!
//! Each Output Variant has the following output ports:
//!
//! - `a: i64`: The first number.
//! - `b: i64`: The second number.

use {
    nexus_toolkit::*,
    nexus_sdk::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::cmp::Ordering,
};

#[derive(Deserialize, JsonSchema)]
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

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.math.i64.cmp@1")
    }

    fn path() -> &'static str {
        "/i64/cmp"
    }

    async fn health() -> AnyResult<StatusCode> {
        // This tool has no external dependencies and as such, it is always
        // healthy if the endpoint is reachable.
        Ok(StatusCode::OK)
    }

    async fn invoke(Self::Input { a, b }: Self::Input) -> AnyResult<Self::Output> {
        match a.cmp(&b) {
            Ordering::Greater => Ok(Output::Gt { a, b }),
            Ordering::Equal => Ok(Output::Eq { a, b }),
            Ordering::Less => Ok(Output::Lt { a, b }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_i64_cmp() {
        let input = Input { a: 1, b: 2 };
        let output = I64Cmp::invoke(input).await.unwrap();

        assert!(matches!(output, Output::Lt { a: 1, b: 2 }));

        let input = Input { a: 2, b: 1 };
        let output = I64Cmp::invoke(input).await.unwrap();

        assert!(matches!(output, Output::Gt { a: 2, b: 1 }));

        let input = Input { a: 1, b: 1 };
        let output = I64Cmp::invoke(input).await.unwrap();

        assert!(matches!(output, Output::Eq { a: 1, b: 1 }));
    }
}
