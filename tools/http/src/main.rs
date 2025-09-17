#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod errors;
mod helpers;
mod http;
mod http_client;
mod models;

#[tokio::main]
async fn main() {
    bootstrap!([http::Http,]);
}
