//! Shared contract for HTTP Tool execution, defined by the SDK.
//!
//! The CLI, Leader and Toolkit use the same limits. Inputs and outputs each have
//! an independent budget across all their ports and values. Changing these
//! limits requires a coordinated SDK and runtime release; they are not local
//! operator settings and do not change the published Move protocol limits.

/// Maximum combined content bytes in one resolved input set or output set.
/// Count inline and downloaded bytes equally; Object IDs count as 32 bytes.
pub const MAX_RESOLVED_DATA_BYTES: usize = 8 * 1024 * 1024;

/// Maximum encoded HTTP invocation body, including base64 and port metadata.
/// This envelope accommodates the resolved data budget with encoding overhead.
/// Canonical signed responses retain their separate Move encoded output limit.
pub const MAX_INVOKE_BODY_BYTES: u64 = 12 * 1024 * 1024;
