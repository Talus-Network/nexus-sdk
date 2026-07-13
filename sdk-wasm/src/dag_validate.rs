use {
    nexus_sdk::dag::{json::parse_dag_spec, validator},
    wasm_bindgen::prelude::*,
};

/// WASM-exported DAG validation result
#[wasm_bindgen]
pub struct ValidationResult {
    is_valid: bool,
    error_message: Option<String>,
}

#[wasm_bindgen]
impl ValidationResult {
    #[wasm_bindgen(getter)]
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    #[wasm_bindgen(getter)]
    pub fn error_message(&self) -> Option<String> {
        self.error_message.clone()
    }
}

/// Validate a DAG from JSON string
#[wasm_bindgen]
pub fn validate_dag_from_json(dag_json: &str) -> ValidationResult {
    // Parse JSON string into a typed DAG spec
    let dag = match parse_dag_spec(dag_json) {
        Ok(dag) => dag,
        Err(e) => {
            return ValidationResult {
                is_valid: false,
                error_message: Some(format!("JSON parsing error: {}", e)),
            };
        }
    };

    // Validate the DAG using the SDK validator
    match validator::validate(&dag) {
        Ok(_) => ValidationResult {
            is_valid: true,
            error_message: None,
        },
        Err(e) => ValidationResult {
            is_valid: false,
            error_message: Some(format!("{}", e)),
        },
    }
}
