//! # `xyz.taluslabs.math.i64.mul@1`
//!
//! Standard Nexus Tool that multiplies two [i64] numbers and returns the result.
//!
//! ## Input
//!
//! - `a: i64`: The first number to multiply.
//! - `b: i64`: The second number to multiply.
//!
//! ## Output Variants
//!
//! - `ok`: The multiplication was successful.
//! - `err`: The multiplication failed due to overflow.
//!
//! ## Output Ports
//!
//! ### `ok`
//!
//! - `result: i64`: The result of the multiplication.
//!
//! ### `err`
//!
//! - `reason: string`: The reason for the error. This is always overflow.

use {
    nexus_toolkit::*,
    nexus_sdk::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Deserialize, JsonSchema)]
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

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.math.i64.mul@1")
    }

    fn path() -> &'static str {
        "/i64/mul"
    }

    async fn health() -> AnyResult<StatusCode> {
        // This tool has no external dependencies and as such, it is always
        // healthy if the endpoint is reachable.
        Ok(StatusCode::OK)
    }

    async fn invoke(Self::Input { a, b }: Self::Input) -> AnyResult<Self::Output> {
        match a.checked_mul(b) {
            Some(result) => Ok(Output::Ok { result }),
            None => Ok(Output::Err {
                reason: format!("Multiplying '{a}' and '{b}' results in an overflow"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_i64_mul() {
        let input = Input { a: 2, b: 3 };
        let output = I64Mul::invoke(input).await.unwrap();

        assert!(matches!(output, Output::Ok { result: 6 }));

        let input = Input { a: i64::MAX, b: 2 };
        let output = I64Mul::invoke(input).await.unwrap();

        assert!(matches!(output, Output::Err { .. }));

        let input = Input { a: i64::MIN, b: 2 };
        let output = I64Mul::invoke(input).await.unwrap();

        assert!(matches!(output, Output::Err { .. }));
    }
}
