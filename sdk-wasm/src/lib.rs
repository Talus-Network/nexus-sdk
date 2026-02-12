use wasm_bindgen::prelude::*;

mod crypto;
mod dag_execute;
mod dag_publish;
mod dag_validate;
mod scheduler;
mod walrus;

pub use {crypto::*, dag_execute::*, dag_publish::*, dag_validate::*, scheduler::*, walrus::*};

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

/// Get the SDK-WASM version information
/// Updated to v0.5.0 with scheduler and tool registration support
#[wasm_bindgen]
pub fn get_sdk_version() -> String {
        serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"),
            "features": [
                "dag_validation",
                "dag_publish",
                "dag_execute",
                "crypto",
                "scheduler",
                "walrus"
            ],
            "cli_compatible_version": "0.5.0"
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
