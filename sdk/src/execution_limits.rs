//! Shared contract for HTTP Tool execution, defined by the SDK.
//!
//! The CLI, Leader and Toolkit use the same limits. Inputs and outputs each have
//! an independent budget across all their ports and values. Changing these
//! data budgets requires a coordinated SDK and runtime release. HTTP admission
//! limits remain operator settings. Neither changes the published Move limits.

/// Maximum combined content bytes in one resolved input set or output set.
/// Count inline and downloaded bytes equally; Object IDs count as 32 bytes.
pub const MAX_RESOLVED_DATA_BYTES: usize = 8 * 1024 * 1024;

/// Default HTTP invocation body limit, including base64 and port metadata.
/// This envelope accommodates the resolved data budget with encoding overhead.
/// Canonical signed responses retain their separate Move encoded output limit.
pub const MAX_INVOKE_BODY_BYTES: u64 = 12 * 1024 * 1024;
