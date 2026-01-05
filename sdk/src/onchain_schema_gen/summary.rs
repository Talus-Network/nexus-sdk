//! Summary-based schema generation for Move onchain tools.
//!
//! Generates input/output schemas by parsing the JSON output of `sui move summary`,
//! eliminating the need for RPC calls to introspect published packages.

use {
    anyhow::{anyhow, bail, Context, Result as AnyResult},
    serde::Deserialize,
    serde_json::{json, Map, Value},
    std::{collections::HashMap, path::Path, process::Command},
};

// ============================================================================
// Summary Type Definitions
// ============================================================================

/// Module identifier from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ModuleId {
    pub address: String,
    pub name: String,
}

/// Function parameter from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type_")]
    pub type_: SummaryType,
}

/// Function definition from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct FunctionSummary {
    #[serde(default)]
    pub doc: Option<String>,
    pub visibility: String,
    pub parameters: Vec<Parameter>,
    #[serde(rename = "return_")]
    pub return_: Vec<SummaryType>,
}

/// Enum variant field from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct VariantField {
    pub index: u32,
    #[serde(default)]
    pub doc: Option<String>,
    #[serde(rename = "type_")]
    pub type_: SummaryType,
}

/// Fields container for variants.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct VariantFields {
    pub positional_fields: bool,
    pub fields: HashMap<String, VariantField>,
}

/// Enum variant from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EnumVariant {
    pub index: u32,
    #[serde(default)]
    pub doc: Option<String>,
    pub fields: VariantFields,
}

/// Enum definition from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct EnumSummary {
    pub index: u32,
    #[serde(default)]
    pub doc: Option<String>,
    pub variants: HashMap<String, EnumVariant>,
}

/// Datatype module reference.
#[derive(Debug, Deserialize)]
pub struct DatatypeModule {
    pub address: String,
    pub name: String,
}

/// Datatype definition.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Datatype {
    pub module: DatatypeModule,
    pub name: String,
    #[serde(default)]
    pub type_arguments: Vec<SummaryType>,
}

/// Type representation in the summary JSON.
/// Can be a primitive string or a complex type object.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SummaryType {
    /// Primitive types like "u64", "bool", "address".
    Primitive(String),
    /// Complex types with structure.
    Complex(SummaryTypeComplex),
}

/// Complex type variants from the summary.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub enum SummaryTypeComplex {
    /// Reference type: [is_mutable, inner_type].
    Reference(bool, Box<SummaryType>),
    /// Datatype (struct/enum).
    Datatype(Datatype),
    /// Vector type (lowercase in JSON).
    #[serde(rename = "vector")]
    Vector(Box<SummaryType>),
    /// Type parameter.
    TypeParameter(u32),
}

/// Full module summary from `sui move summary`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ModuleSummary {
    pub id: ModuleId,
    #[serde(default)]
    pub doc: Option<String>,
    pub functions: HashMap<String, FunctionSummary>,
    #[serde(default)]
    pub enums: HashMap<String, EnumSummary>,
}

// ============================================================================
// Command Execution and Summary Parsing
// ============================================================================

/// Run `sui move summary` command and parse output for the specified module.
pub fn run_summary_command(package_path: &Path, module_name: &str) -> AnyResult<ModuleSummary> {
    validate_package_path(package_path)?;
    execute_summary_command(package_path)?;
    let summary_json = read_summary_file(package_path, module_name)?;
    find_module_in_summary(&summary_json, module_name)
}

/// Validate that the package path exists and contains a Move.toml file.
fn validate_package_path(package_path: &Path) -> AnyResult<()> {
    if !package_path.exists() {
        bail!("Package path does not exist: {}", package_path.display());
    }

    let move_toml = package_path.join("Move.toml");
    if !move_toml.exists() {
        bail!(
            "Move.toml not found in package path: {}",
            package_path.display()
        );
    }

    Ok(())
}

/// Execute the `sui move summary` command.
fn execute_summary_command(package_path: &Path) -> AnyResult<()> {
    let output = Command::new("sui")
        .args(["move", "summary"])
        .current_dir(package_path)
        .output()
        .context("Failed to execute 'sui move summary'")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("'sui move summary' failed: {}", stderr);
    }

    Ok(())
}

/// Read and parse the summary JSON file for the specified module.
fn read_summary_file(package_path: &Path, module_name: &str) -> AnyResult<Value> {
    let summary_file = package_path
        .join("package_summaries")
        .join(module_name)
        .join(format!("{}.json", module_name));

    if !summary_file.exists() {
        bail!(
            "Summary file not found: {}. Expected at package_summaries/{}/{}.json",
            summary_file.display(),
            module_name,
            module_name
        );
    }

    let summary_content = std::fs::read_to_string(&summary_file)
        .with_context(|| format!("Failed to read summary file: {}", summary_file.display()))?;

    serde_json::from_str(&summary_content).with_context(|| {
        format!(
            "Failed to parse summary JSON from {}",
            summary_file.display()
        )
    })
}

/// Find a specific module in the summary JSON output.
fn find_module_in_summary(summary: &Value, module_name: &str) -> AnyResult<ModuleSummary> {
    let Some(obj) = summary.as_object() else {
        // If not an object, try as array.
        if let Some(arr) = summary.as_array() {
            return find_module_in_array(arr, module_name);
        }
        bail!("Module '{}' not found in package summary", module_name);
    };

    // Check if this is a single module summary with an "id" field.
    if obj.contains_key("id") {
        let module: ModuleSummary =
            serde_json::from_value(summary.clone()).context("Failed to parse module summary")?;
        if module.id.name == module_name {
            return Ok(module);
        }
    }

    // Check if modules are nested by name.
    if let Some(module_value) = obj.get(module_name) {
        return serde_json::from_value(module_value.clone())
            .with_context(|| format!("Failed to parse module '{}'", module_name));
    }

    bail!("Module '{}' not found in package summary", module_name)
}

/// Find a module in an array of module summaries.
fn find_module_in_array(modules: &[Value], module_name: &str) -> AnyResult<ModuleSummary> {
    for module_value in modules {
        if let Ok(module) = serde_json::from_value::<ModuleSummary>(module_value.clone()) {
            if module.id.name == module_name {
                return Ok(module);
            }
        }
    }
    bail!("Module '{}' not found in package summary", module_name)
}

// ============================================================================
// Schema Generation
// ============================================================================

/// Generate input schema from the execute function parameters.
pub fn generate_input_schema_from_summary(module: &ModuleSummary) -> AnyResult<String> {
    let execute_fn = module
        .functions
        .get("execute")
        .ok_or_else(|| anyhow!("Function 'execute' not found in module"))?;

    let schema_map = build_input_schema_map(&execute_fn.parameters)?;
    let schema_json = Value::Object(schema_map);
    serde_json::to_string(&schema_json).context("Failed to serialize input schema")
}

/// Build the schema map from function parameters.
fn build_input_schema_map(parameters: &[Parameter]) -> AnyResult<Map<String, Value>> {
    let mut schema_map = Map::new();
    let mut param_index = 0;

    for (i, param) in parameters.iter().enumerate() {
        // Skip the first parameter (ProofOfUID) and TxContext parameters.
        if i == 0 || is_tx_context_type(&param.type_) {
            continue;
        }

        let param_schema = convert_type_to_schema(&param.type_)?;
        schema_map.insert(param_index.to_string(), param_schema);
        param_index += 1;
    }

    Ok(schema_map)
}

/// Generate output schema from the Output enum.
pub fn generate_output_schema_from_summary(module: &ModuleSummary) -> AnyResult<String> {
    let output_enum = module
        .enums
        .get("Output")
        .ok_or_else(|| anyhow!("Enum 'Output' not found in module"))?;

    let schema_map = build_output_schema_map(&output_enum.variants)?;
    let schema_json = Value::Object(schema_map);
    serde_json::to_string(&schema_json).context("Failed to serialize output schema")
}

/// Build the schema map from enum variants.
fn build_output_schema_map(
    variants: &HashMap<String, EnumVariant>,
) -> AnyResult<Map<String, Value>> {
    let mut schema_map = Map::new();

    for (variant_name, variant) in variants {
        let fields_schema = build_variant_fields_schema(&variant.fields)?;
        let variant_schema = json!({
            "type": "variant",
            "description": format!("{} variant", variant_name),
            "fields": fields_schema
        });
        schema_map.insert(variant_name.to_lowercase(), variant_schema);
    }

    Ok(schema_map)
}

/// Build schema map for variant fields, sorted by field index.
fn build_variant_fields_schema(variant_fields: &VariantFields) -> AnyResult<Map<String, Value>> {
    let mut fields_schema = Map::new();

    // Sort fields by index to maintain consistent ordering.
    let mut sorted_fields: Vec<_> = variant_fields.fields.iter().collect();
    sorted_fields.sort_by_key(|(_, field)| field.index);

    for (field_name, field) in sorted_fields {
        let field_schema = convert_type_to_schema(&field.type_)?;
        fields_schema.insert(field_name.clone(), field_schema);
    }

    Ok(fields_schema)
}

// ============================================================================
// Type Checking and Conversion
// ============================================================================

/// Check if a type is TxContext (should be excluded from input schema).
fn is_tx_context_type(type_: &SummaryType) -> bool {
    match type_ {
        SummaryType::Primitive(_) => false,
        SummaryType::Complex(SummaryTypeComplex::Datatype(datatype)) => is_tx_context(datatype),
        SummaryType::Complex(SummaryTypeComplex::Reference(_, inner)) => is_tx_context_type(inner),
        SummaryType::Complex(_) => false,
    }
}

/// Check if a datatype is TxContext.
fn is_tx_context(datatype: &Datatype) -> bool {
    matches!(
        (
            datatype.module.address.as_str(),
            datatype.module.name.as_str(),
            datatype.name.as_str()
        ),
        ("sui" | "0x2", "tx_context", "TxContext")
    )
}

/// Check if an address represents the standard library (0x1 or "std").
fn is_std_address(address: &str) -> bool {
    matches!(address, "0x1" | "std")
}

/// Check if an address represents the Sui framework (0x2 or "sui").
fn is_sui_address(address: &str) -> bool {
    matches!(address, "0x2" | "sui")
}

/// Convert a summary type to a JSON schema representation.
fn convert_type_to_schema(type_: &SummaryType) -> AnyResult<Value> {
    match type_ {
        SummaryType::Primitive(primitive_type) => convert_primitive_to_schema(primitive_type),
        SummaryType::Complex(complex_type) => convert_complex_to_schema(complex_type),
    }
}

/// Convert a primitive type string to schema.
fn convert_primitive_to_schema(primitive_type: &str) -> AnyResult<Value> {
    let (type_name, description) = match primitive_type {
        "bool" => ("bool", "Boolean value"),
        "u8" => ("u8", "8-bit unsigned integer"),
        "u16" => ("u16", "16-bit unsigned integer"),
        "u32" => ("u32", "32-bit unsigned integer"),
        "u64" => ("u64", "64-bit unsigned integer"),
        "u128" => ("u128", "128-bit unsigned integer"),
        "u256" => ("u256", "256-bit unsigned integer"),
        "address" => ("address", "Sui address"),
        "signer" => ("signer", "Transaction signer"),
        _ => bail!("Unknown primitive type: {}", primitive_type),
    };

    Ok(create_schema(type_name, description))
}

/// Helper to create a schema object with type and description.
fn create_schema(type_name: &str, description: &str) -> Value {
    json!({
        "type": type_name,
        "description": description
    })
}

/// Convert a complex type to schema.
fn convert_complex_to_schema(complex_type: &SummaryTypeComplex) -> AnyResult<Value> {
    match complex_type {
        SummaryTypeComplex::Reference(is_mutable, inner) => {
            convert_reference_to_schema(*is_mutable, inner)
        }
        SummaryTypeComplex::Datatype(datatype) => convert_datatype_to_schema(datatype),
        SummaryTypeComplex::Vector(element_type) => convert_vector_to_schema(element_type),
        SummaryTypeComplex::TypeParameter(_) => {
            Ok(create_schema("generic", "Generic type parameter"))
        }
    }
}

/// Convert a reference type to schema, adding mutability flag.
fn convert_reference_to_schema(is_mutable: bool, inner: &SummaryType) -> AnyResult<Value> {
    let mut inner_schema = convert_type_to_schema(inner)?;
    if let Value::Object(ref mut obj) = inner_schema {
        obj.insert("mutable".to_string(), Value::Bool(is_mutable));
    }
    Ok(inner_schema)
}

/// Convert a vector type to schema.
fn convert_vector_to_schema(element_type: &SummaryType) -> AnyResult<Value> {
    let element_schema = convert_type_to_schema(element_type)?;
    Ok(json!({
        "type": "vector",
        "description": "Vector of values",
        "element_type": element_schema
    }))
}

/// Convert a datatype to schema.
fn convert_datatype_to_schema(datatype: &Datatype) -> AnyResult<Value> {
    let address = &datatype.module.address;
    let module_name = &datatype.module.name;
    let type_name = &datatype.name;

    // Handle standard library types.
    if is_std_address(address) {
        return convert_std_type_to_schema(module_name, type_name);
    }

    // Handle Sui framework types.
    if is_sui_address(address) {
        return convert_sui_type_to_schema(module_name, type_name);
    }

    // Custom package types.
    Ok(create_schema(
        "object",
        &format!("{}::{}::{}", address, module_name, type_name),
    ))
}

/// Convert standard library (0x1) types to schema.
fn convert_std_type_to_schema(module_name: &str, type_name: &str) -> AnyResult<Value> {
    match (module_name, type_name) {
        ("string", "String") => Ok(create_schema("string", "0x1::string::String")),
        ("ascii", "String") => Ok(create_schema("string", "0x1::ascii::String")),
        _ => Ok(create_schema(
            "object",
            &format!("0x1::{}::{}", module_name, type_name),
        )),
    }
}

/// Convert Sui framework (0x2) types to schema.
fn convert_sui_type_to_schema(module_name: &str, type_name: &str) -> AnyResult<Value> {
    match (module_name, type_name) {
        ("object", "ID") => Ok(create_schema("object_id", "Sui object ID")),
        ("tx_context", "TxContext") => Ok(create_schema(
            "tx_context",
            "Transaction context (automatically provided)",
        )),
        _ => Ok(create_schema(
            "object",
            &format!("0x2::{}::{}", module_name, type_name),
        )),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_primitive_types() {
        let schema = convert_primitive_to_schema("u64").unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["description"], "64-bit unsigned integer");

        let schema = convert_primitive_to_schema("bool").unwrap();
        assert_eq!(schema["type"], "bool");
        assert_eq!(schema["description"], "Boolean value");

        let schema = convert_primitive_to_schema("address").unwrap();
        assert_eq!(schema["type"], "address");
        assert_eq!(schema["description"], "Sui address");
    }

    #[test]
    fn test_convert_unknown_primitive() {
        let result = convert_primitive_to_schema("unknown_type");
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_string_types() {
        // Test std::ascii::String.
        let datatype = Datatype {
            module: DatatypeModule {
                address: "std".to_string(),
                name: "ascii".to_string(),
            },
            name: "String".to_string(),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "0x1::ascii::String");

        // Test std::string::String.
        let datatype = Datatype {
            module: DatatypeModule {
                address: "std".to_string(),
                name: "string".to_string(),
            },
            name: "String".to_string(),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "0x1::string::String");
    }

    #[test]
    fn test_convert_sui_object_id() {
        let datatype = Datatype {
            module: DatatypeModule {
                address: "sui".to_string(),
                name: "object".to_string(),
            },
            name: "ID".to_string(),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "object_id");
        assert_eq!(schema["description"], "Sui object ID");
    }

    #[test]
    fn test_convert_custom_type() {
        let datatype = Datatype {
            module: DatatypeModule {
                address: "my_package".to_string(),
                name: "my_module".to_string(),
            },
            name: "MyStruct".to_string(),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "my_package::my_module::MyStruct");
    }

    #[test]
    fn test_convert_vector_type() {
        let element_type = SummaryType::Primitive("u64".to_string());
        let complex_type = SummaryTypeComplex::Vector(Box::new(element_type));
        let schema = convert_complex_to_schema(&complex_type).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "u64");
    }

    #[test]
    fn test_convert_reference_type() {
        // Mutable reference.
        let inner_type = SummaryType::Primitive("u64".to_string());
        let mutable_ref = SummaryTypeComplex::Reference(true, Box::new(inner_type));
        let schema = convert_complex_to_schema(&mutable_ref).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["mutable"], true);

        // Immutable reference.
        let inner_type = SummaryType::Primitive("u64".to_string());
        let immutable_ref = SummaryTypeComplex::Reference(false, Box::new(inner_type));
        let schema = convert_complex_to_schema(&immutable_ref).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["mutable"], false);
    }

    #[test]
    fn test_is_tx_context_type() {
        // Direct TxContext.
        let tx_ctx = SummaryType::Complex(SummaryTypeComplex::Datatype(Datatype {
            module: DatatypeModule {
                address: "sui".to_string(),
                name: "tx_context".to_string(),
            },
            name: "TxContext".to_string(),
            type_arguments: vec![],
        }));
        assert!(is_tx_context_type(&tx_ctx));

        // Reference to TxContext.
        let ref_tx_ctx =
            SummaryType::Complex(SummaryTypeComplex::Reference(true, Box::new(tx_ctx)));
        assert!(is_tx_context_type(&ref_tx_ctx));

        // Non-TxContext type.
        let other = SummaryType::Primitive("u64".to_string());
        assert!(!is_tx_context_type(&other));
    }

    #[test]
    fn test_address_helpers() {
        assert!(is_std_address("0x1"));
        assert!(is_std_address("std"));
        assert!(!is_std_address("sui"));

        assert!(is_sui_address("0x2"));
        assert!(is_sui_address("sui"));
        assert!(!is_sui_address("std"));
    }

    #[test]
    fn test_is_tx_context_datatype() {
        let tx_ctx_datatype = Datatype {
            module: DatatypeModule {
                address: "sui".to_string(),
                name: "tx_context".to_string(),
            },
            name: "TxContext".to_string(),
            type_arguments: vec![],
        };
        assert!(is_tx_context(&tx_ctx_datatype));

        // With 0x2 address.
        let tx_ctx_datatype_hex = Datatype {
            module: DatatypeModule {
                address: "0x2".to_string(),
                name: "tx_context".to_string(),
            },
            name: "TxContext".to_string(),
            type_arguments: vec![],
        };
        assert!(is_tx_context(&tx_ctx_datatype_hex));

        // Non-TxContext.
        let other_datatype = Datatype {
            module: DatatypeModule {
                address: "sui".to_string(),
                name: "object".to_string(),
            },
            name: "ID".to_string(),
            type_arguments: vec![],
        };
        assert!(!is_tx_context(&other_datatype));
    }
}
