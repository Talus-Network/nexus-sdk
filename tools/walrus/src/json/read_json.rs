//! # `xyz.taluslabs.walrus.json.read@1`
//!
//! Standard Nexus Tool that reads a JSON file from Walrus and returns the JSON data.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    thiserror::Error,
};

/// Errors that can occur during JSON upload
#[derive(Error, Debug)]
pub enum ReadJsonError {
    #[error("Failed to read JSON: {0}")]
    ReadError(#[from] anyhow::Error),
    #[error("Invalid JSON data: {0}")]
    InvalidJson(String),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The blob ID of the JSON file to read
    blob_id: String,

    /// The URL of the Walrus aggregator to read the JSON from
    #[serde(default)]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { json: Value, text: String },
    Err { reason: String },
}

pub(crate) struct ReadJson;

impl NexusTool for ReadJson {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.json.read@1")
    }

    fn path() -> &'static str {
        "/json/read"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        match self.read(input).await {
            Ok(string_result) => {
                let json_data = serde_json::from_str(&string_result);

                match json_data {
                    Ok(json) => Output::Ok {
                        json,
                        text: string_result,
                    },
                    Err(e) => Output::Err {
                        reason: ReadJsonError::InvalidJson(e.to_string()).to_string(),
                    },
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

impl ReadJson {
    async fn read(&self, input: Input) -> Result<String, ReadJsonError> {
        let walrus_client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let storage_info = walrus_client.read_json(&input.blob_id).await;

        match storage_info {
            Ok(string_result) => Ok(string_result),
            Err(e) => Err(ReadJsonError::ReadError(e)),
        }
    }
}
