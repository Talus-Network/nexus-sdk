//! # `xyz.taluslabs.walrus.json.upload_json@1`
//!
//! Standard Nexus Tool that uploads a JSON file to Walrus and returns the blob ID.

use {
    nexus_sdk::{fqn, walrus::WalrusClient, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The JSON data to upload
    json: String,
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
        fqn!("xyz.taluslabs.walrus.json.upload_json")
    }

    fn path() -> &'static str {
        "/json/upload"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let client = WalrusClient::new();

        let blob = client.upload_json(&input.json, 1000, None).await;

        match blob {
            Ok(blob) => {
                if let Some(blob) = blob.newly_created {
                    Output::Ok {
                        blob_id: blob.blob_object.blob_id,
                    }
                } else {
                    Output::Err {
                        reason: "No blob ID returned".to_string(),
                    }
                }
            }
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}
