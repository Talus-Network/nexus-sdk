#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod client;
mod file;
mod json;

#[tokio::main]
async fn main() {
    bootstrap!([json::upload_json::UploadJson, file::upload_file::UploadFile,])
}
