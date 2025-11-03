#![doc = include_str!("../README.md")]

use {nexus_toolkit::bootstrap, template::TemplatingJinja};

mod template;

#[tokio::main]
async fn main() {
    bootstrap!(TemplatingJinja);
}
