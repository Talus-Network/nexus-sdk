#![doc = include_str!("../README.md")]

use nexus_toolkit::bootstrap;

mod i64;

#[tokio::main]
async fn main() {
    bootstrap!([
        i64::add::I64Add,
        i64::mul::I64Mul,
        i64::cmp::I64Cmp,
        i64::sum::I64Sum,
    ])
}
