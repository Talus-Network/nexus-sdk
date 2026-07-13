use wasm_bindgen::prelude::*;

mod crypto;
mod dag_execute;
mod dag_publish;
mod dag_validate;
mod scheduler;
mod walrus;

pub use {
    crypto::*, dag_execute::*, dag_publish::*, dag_validate::*, scheduler::*, walrus::*,
};

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

/// JSON metadata about this WASM build (for JS feature detection).
#[wasm_bindgen]
pub fn get_sdk_version() -> String {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "features": [
            "dag_validation",
            "dag_publish",
            "dag_execute",
            "sui_and_tool_keys",
            "signed_http_helpers",
            "scheduler",
            "walrus_upload"
        ],
    })
    .to_string()
}

// A macro to provide `println!(..)`-style syntax for `console.log` logging.
#[allow(unused_macros)]
macro_rules! console_log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[allow(unused_imports)]
pub(crate) use console_log;
