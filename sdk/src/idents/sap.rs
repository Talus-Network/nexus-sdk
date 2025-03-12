//! This module is slightly different than others as it only defines the
//! generic interface of SAPs. The packages and modules are retrieved at
//! runtime.

use crate::sui;

// == Nexus Interface V1 ==

pub struct SapV1;

impl SapV1 {
    /// Confirm walk eval with the SAP.
    pub const CONFIRM_TOOL_EVAL_FOR_WALK: &sui::MoveIdentStr =
        sui::move_ident_str!("confirm_tool_eval_for_walk");
    /// Get the SAP worksheet so that we can stamp it.
    pub const WORKSHEET: &sui::MoveIdentStr = sui::move_ident_str!("worksheet");
}
