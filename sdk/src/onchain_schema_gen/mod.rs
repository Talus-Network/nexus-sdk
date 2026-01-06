//! Schema generation utilities for Move onchain tools.
//!
//! Generates input/output schemas by parsing the JSON output of `sui move summary`,
//! eliminating the need for RPC calls to introspect published packages.

mod summary;

pub use summary::{
    generate_input_schema_from_summary,
    generate_output_schema_from_summary,
    run_summary_command,
    ModuleSummary,
};
