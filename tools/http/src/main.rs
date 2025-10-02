#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod errors;
mod http;
mod http_client;
mod models;
mod utils;

#[tokio::main]
async fn main() {
    bootstrap!([http::Http,]);
}
