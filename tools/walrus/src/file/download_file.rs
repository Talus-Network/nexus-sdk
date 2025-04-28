//! # `xyz.taluslabs.walrus.file.download@1`
//!
//! Standard Nexus Tool that downloads a file from Walrus and saves it to a local path.

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
    /// The blob ID of the file to download
    blob_id: String,
    /// The path to save the file to
    output_path: PathBuf,
    /// The URL of the aggregator to upload the JSON to
    #[serde(default)]
    aggregator_url: Option<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok { contents: String },
    Err { reason: String },
}

pub(crate) struct DownloadFile;

impl NexusTool for DownloadFile {
    type Input = Input;
    type Output = Output;

    async fn new() -> Self {
        Self {}
    }

    fn fqn() -> ToolFqn {
        fqn!("xyz.taluslabs.walrus.file.download")
    }

    fn path() -> &'static str {
        "/file/download"
    }

    async fn health(&self) -> AnyResult<StatusCode> {
        Ok(StatusCode::OK)
    }

    async fn invoke(&self, input: Self::Input) -> Self::Output {
        let client = WalrusConfig::new()
            .with_aggregator_url(input.aggregator_url)
            .build();

        let contents = client
            .download_file(&input.blob_id, &input.output_path)
            .await;

        match contents {
            Ok(_) => Output::Ok {
                contents: format!(
                    "File downloaded to {} successfully",
                    input.output_path.display()
                ),
            },
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}
