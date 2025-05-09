#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod client;
mod download_file;
mod read_json;
mod upload_file;
mod upload_json;
mod verify_blob;

#[tokio::main]
async fn main() {
    bootstrap!([
        upload_file::UploadFile,
        upload_json::UploadJson,
        read_json::ReadJson,
        verify_blob::VerifyBlob,
        download_file::DownloadFile,
    ])
}
