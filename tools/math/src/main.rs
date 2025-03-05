//! # `xyz.taluslabs.math.*`
//!
//! This module contains tools for mathematical operations. They are divided
//! into modules based on the datatype of the input.

use nexus_toolkit::bootstrap;

mod i64;

#[tokio::main]
async fn main() {
    bootstrap!([
        i64::add::I64Add,
        i64::mul::I64Mul,
        i64::cmp::I64Cmp,
        //
    ])
}
