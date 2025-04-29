#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod client;
mod json;
mod verify_blob;

#[tokio::main]
async fn main() {
    bootstrap!([json::upload_json::UploadJson, verify_blob::VerifyBlob,]);
}
