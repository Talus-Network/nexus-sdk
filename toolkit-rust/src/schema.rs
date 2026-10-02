//! Compile caller supplied schemas without filesystem or network access.

use {
    jsonschema::{Retrieve, Uri, Validator},
    serde_json::Value,
    std::{error::Error, fmt},
};

struct DenyExternalReferences;

impl Retrieve for DenyExternalReferences {
    fn retrieve(&self, _uri: &Uri<String>) -> Result<Value, Box<dyn Error + Send + Sync>> {
        Err("external schema references are disabled".into())
    }
}

/// A schema compilation failure with no embedded schema values or file contents.
#[derive(Debug, Clone, Copy)]
pub struct InvalidSchema;

impl fmt::Display for InvalidSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Invalid JSON schema; external references are disabled")
    }
}

impl Error for InvalidSchema {}

/// Compile a schema, allowing references within the supplied document only.
///
/// An explicit retriever keeps this policy effective even if another dependency
/// enables the library's default network or filesystem features.
pub fn compile(schema: &Value) -> Result<Validator, InvalidSchema> {
    jsonschema::options()
        .with_retriever(DenyExternalReferences)
        .build(schema)
        .map_err(|_| InvalidSchema)
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn internal_references_work() {
        let validator = compile(&json!({
            "$defs": {"value": {"type": "integer"}},
            "$ref": "#/$defs/value"
        }))
        .unwrap();
        assert!(validator.is_valid(&json!(42)));
        assert!(!validator.is_valid(&json!("42")));
    }

    #[test]
    fn external_references_cannot_read_files_or_make_requests() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("schema.json");
        std::fs::write(&file, "true").unwrap();
        for reference in [
            format!("file://{}", file.display()),
            "http://127.0.0.1:1/schema".into(),
            "https://example.com/schema".into(),
        ] {
            assert!(compile(&json!({"$ref": reference})).is_err());
        }
    }

    #[test]
    fn invalid_schema_returns_a_sanitized_error() {
        let schema = json!({"type": "PRIVATE_TEST_MARKER"});
        let error = compile(&schema).unwrap_err().to_string();
        assert!(!error.contains("PRIVATE_TEST_MARKER"));
        assert!(compile(&json!({"type": 42})).is_err());
    }
}
