#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod client;
mod upload_file;
mod upload_json;

#[tokio::main]
async fn main() {
    bootstrap!([upload_json::UploadJson, upload_file::UploadFile])
}
