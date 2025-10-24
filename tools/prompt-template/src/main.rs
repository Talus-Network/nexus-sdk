#![doc = include_str!("../README.md")]

use {nexus_toolkit::bootstrap, template::PromptTemplate};

mod template;

#[tokio::main]
async fn main() {
    bootstrap!(PromptTemplate);
}
