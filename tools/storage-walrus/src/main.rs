#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod client;
mod upload_json;
mod verify_blob;

#[tokio::main]
async fn main() {
    bootstrap!([upload_json::UploadJson, verify_blob::VerifyBlob])
}
