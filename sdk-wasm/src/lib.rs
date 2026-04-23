use wasm_bindgen::prelude::*;

mod crypto;
mod dag_execute;
mod dag_publish;
mod dag_validate;

pub use {crypto::*, dag_execute::*, dag_publish::*, dag_validate::*};

// Called when the wasm module is instantiated
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
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
