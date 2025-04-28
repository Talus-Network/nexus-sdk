//! # `xyz.taluslabs.walrus.file.upload@1`
//!
//! Standard Nexus Tool that uploads a file to Walrus and returns the blob ID.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
    /// The path to the file to upload
    file_path: String,
    /// The walrus publisher URL
    #[serde(default)]
    publisher_url: Option<String>,
    /// The number of epochs to store the file
    #[serde(default = "default_epochs")]
    epochs: u64,
    /// Optional address to which the created Blob object should be sent
    #[serde(default)]
    send_to: Option<String>,
}

fn default_epochs() -> u64 {
    1000
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { blob_id: String },
    Err { reason: String },
}

pub(crate) struct UploadFile;

impl NexusTool for UploadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.file.upload")
    }

    fn path() -> &'static str {
        "/file/upload"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let client = WalrusConfig::new()
            .with_publisher_url(input.publisher_url)
            .build();

        let blob = client
            .upload_file(
                &PathBuf::from(&input.file_path),
                input.epochs,
                input.send_to,
            )
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
