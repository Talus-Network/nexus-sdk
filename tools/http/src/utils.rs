//! Helper functions for HTTP tool

use {
    crate::models::{HttpJsonSchema, SchemaValidationDetails},
    serde_json::Value,
};

/// Validate JSON data against a schema and return detailed results
pub(crate) fn validate_schema_detailed(
    schema_def: &HttpJsonSchema,
    json_data: &Value,
) -> Result<SchemaValidationDetails, SchemaValidationDetails> {
    // Convert schema to JSON value for validation
    let schema_value =
        serde_json::to_value(&schema_def.schema).map_err(|_| SchemaValidationDetails {
            name: schema_def.name.clone(),
            description: schema_def.description.clone(),
            strict: schema_def.strict,
            valid: false,
            errors: vec!["Schema serialization failed".to_string()],
        })?;

    // Validate using jsonschema
    let validator =
        jsonschema::validator_for(&schema_value).map_err(|e| SchemaValidationDetails {
            name: schema_def.name.clone(),
            description: schema_def.description.clone(),
            strict: schema_def.strict,
            valid: false,
            errors: vec![format!("Schema compilation failed: {}", e)],
        })?;

    let validation_result = validator.validate(json_data);
    match validation_result {
        Ok(_) => Ok(SchemaValidationDetails {
            name: schema_def.name.clone(),
            description: schema_def.description.clone(),
            strict: schema_def.strict,
            valid: true,
            errors: vec![],
        }),
        Err(_) => {
            let error_messages: Vec<String> = validator
                .iter_errors(json_data)
                .map(|e| e.to_string())
                .collect();
            Ok(SchemaValidationDetails {
                name: schema_def.name.clone(),
                description: schema_def.description.clone(),
                strict: schema_def.strict,
                valid: false,
                errors: error_messages,
            })
        }
    }
}
