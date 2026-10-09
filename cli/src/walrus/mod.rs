//! CLI arguments, local files and presentation for the SDK storage client.

mod args;
mod command;
pub(crate) mod receipt;
mod settings;
mod upload;

pub(crate) use {
    args::{UploadArgs, WalrusCommand},
    command::handle,
    receipt::{print_task_receipt, read_bounded},
    settings::Settings,
    upload::Uploader,
};
