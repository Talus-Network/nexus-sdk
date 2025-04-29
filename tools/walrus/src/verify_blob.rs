//! # `xyz.taluslabs.walrus.utils.verify_blob@1`
//!
//! Standard Nexus Tool that verifies a blob.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    thiserror::Error,
};

#[derive(Error, Debug)]
pub enum VerifyBlobError {
    #[error("Failed to verify blob: {0}")]
    VerificationError(#[from] anyhow::Error),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    blob_id: String,
    #[serde(default)]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { verified: bool },
    Err { reason: String },
}

pub(crate) struct VerifyBlob;

impl NexusTool for VerifyBlob {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.utils.verify_blob@1")
    }

    fn path() -> &'static str {
        "/verify-blob"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.verify_blob(input).await {
            Ok(verified) => Output::Ok { verified },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

impl VerifyBlob {
    async fn verify_blob(&self, input: Input) -> Result<bool, VerifyBlobError> {
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let is_verified = walrus_client.verify_blob(&input.blob_id).await?;

        Ok(is_verified)
    }
}
