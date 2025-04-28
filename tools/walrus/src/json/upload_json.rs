//! # `xyz.taluslabs.walrus.json.upload@1`
//!
//! Standard Nexus Tool that uploads a JSON file to Walrus and returns the blob ID.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The JSON data to upload
    json: String,
    /// The walrus publisher URL
    #[serde(default)]
    publisher_url: Option<String>,
    /// The URL of the aggregator to upload the JSON to
    #[serde(default)]
    aggregator_url: Option<String>,
    /// Number of epochs to store the data
    #[serde(default = "default_epochs")]
    epochs: u64,
    /// Optional address to which the created Blob object should be sent
    #[serde(default)]
    send_to_address: Option<String>,
}

fn default_epochs() -> u64 {
    1
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { blob_id: String },
    Err { reason: String },
}

pub(crate) struct UploadJson;

impl NexusTool for UploadJson {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.json.upload")
    }

    fn path() -> &'static str {
        "/json/upload"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let walrus_client = WalrusConfig::new()
            .with_publisher_url(input.publisher_url)
            .with_aggregator_url(input.aggregator_url)
            .build();

        let blob = walrus_client
            .upload_json(&input.json, input.epochs, input.send_to_address)
            .await;

        match blob {
            Ok(blob) => Output::Ok {
                blob_id: blob.blob_id,
            },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}
