//! # `xyz.taluslabs.walrus.file.upload@1`
//!
//! Standard Nexus Tool that uploads a file to Walrus and returns the blob ID.

use {
    crate::client::WalrusConfig,
    nexus_sdk::{fqn, walrus::StorageInfo, ToolFqn},
    nexus_toolkit::*,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize},
    std::path::PathBuf,
    thiserror::Error,
};

/// Errors that can occur during file upload
#[derive(Error, Debug)]
pub enum UploadFileError {
    #[error("Failed to upload file: {0}")]
    UploadError(#[from] anyhow::Error),
    #[error("Invalid file data: {0}")]
    InvalidFile(String),
}

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
    1
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Output {
    Ok {
        blob_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        end_epoch: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        newly_created: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        already_certified: Option<bool>,
        // if the blob is already certified, this will be the tx digest of the blob object
        #[serde(skip_serializing_if = "Option::is_none")]
        tx_digest: Option<String>,
        // if the blob is newly created, this will be the sui object ID of the blob object
        #[serde(skip_serializing_if = "Option::is_none")]
        sui_object_id: Option<String>,
    },
    Err {
        reason: String,
    },
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
        match self.upload(input).await {
            Ok(storage_info) => handle_successful_upload(storage_info),
            Err(e) => Output::Err {
                reason: e.to_string(),
            },
        }
    }
}

/// Handles the successful upload case by extracting the blob ID from the storage info
fn handle_successful_upload(storage_info: StorageInfo) -> Output {
    if let Some(newly_created) = storage_info.newly_created {
        Output::Ok {
            end_epoch: None,
            newly_created: Some(true),
            blob_id: newly_created.blob_object.blob_id,
            tx_digest: None,
            sui_object_id: Some(newly_created.blob_object.id),
            already_certified: None,
        }
    } else if let Some(already_certified) = storage_info.already_certified {
        Output::Ok {
            end_epoch: Some(already_certified.end_epoch),
            newly_created: Some(false),
            blob_id: already_certified.blob_id,
            tx_digest: Some(already_certified.event.tx_digest),
            sui_object_id: None,
            already_certified: Some(true),
        }
    } else {
        Output::Err {
            reason: "Neither newly created nor already certified".to_string(),
        }
    }
}

impl UploadFile {
    async fn upload(&self, input: Input) -> Result<StorageInfo, UploadFileError> {
        let walrus_client = WalrusConfig::new()
            .with_publisher_url(input.publisher_url)
            .build();

        let storage_info = walrus_client
            .upload_file(
                &PathBuf::from(&input.file_path),
                input.epochs,
                input.send_to,
            )
            .await?;

        Ok(storage_info)
    }
}
