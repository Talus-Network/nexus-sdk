//! Schema generation utilities for Move onchain tools.
//!
//! Provides introspection of Move modules to automatically generate
//! input/output schemas for tool registration.

mod input;
mod output;
mod types;

pub use {
    input::generate_input_schema,
    output::generate_output_schema,
    types::{convert_move_type_to_schema, is_dag_execution_param, is_tx_context_param},
};
