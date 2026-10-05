//! Limits for transient HTTP execution data, independent of Move serialization.

/// Maximum combined content bytes in one resolved HTTP tool invocation.
/// The transport also has its own HTTP body limit.
pub const MAX_RESOLVED_INPUT_BYTES: usize = 8 * 1024 * 1024;
