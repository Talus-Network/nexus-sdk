#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod http;
mod models;

#[tokio::main]
async fn main() {
    bootstrap!([
        http::Http,
    ]);
}
