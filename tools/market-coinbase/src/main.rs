#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod coinbase_client;
mod error;
mod market;

#[tokio::main]
async fn main() {
    bootstrap!([market::get_spot_price::GetSpotPrice,]);
}
