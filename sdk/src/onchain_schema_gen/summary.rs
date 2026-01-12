//! Summary-based schema generation for Move onchain tools.
//!
//! Generates input/output schemas by parsing the JSON output of `sui move summary`,
//! eliminating the need for RPC calls to introspect published packages.

#[cfg(test)]
use move_model_2::summary::ModuleId;
use {
    anyhow::{anyhow, bail, Context, Result as AnyResult},
    indexmap::IndexMap,
    move_cli::base::summary::{Summary, SummaryOutputFormat},
    move_model_2::summary::{Datatype, Fields, Module, Parameter, Type, Variant},
    move_package::BuildConfig,
    move_symbol_pool::Symbol,
    serde_json::{json, Map, Value},
    std::path::Path,
    sui_move_build::{implicit_deps, set_sui_flavor, SuiPackageHooks},
    sui_package_management::system_package_versions::latest_system_packages,
};

// ============================================================================
// Command Execution and Summary Parsing
// ============================================================================

/// Run `sui move summary` command and parse output for the specified module.
pub fn run_summary_command(package_path: &Path, module_name: &str) -> AnyResult<Module> {
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

/// Execute the summary generation using the move-cli library directly.
fn execute_summary_command(package_path: &Path) -> AnyResult<()> {
    let summary = Summary {
        output_format: SummaryOutputFormat::Json,
        output_directory: "package_summaries".to_string(),
        bytecode: false,
    };

    // Register Sui package hooks.
    move_package::package_hooks::register_package_hooks(Box::new(SuiPackageHooks));

    let mut config = BuildConfig::default();

    // Set the Sui flavor.
    if let Some(err_msg) = set_sui_flavor(&mut config) {
        bail!(err_msg);
    }

    // Configure Sui framework dependencies (0x1 std, 0x2 sui framework).
    config.implicit_dependencies = implicit_deps(latest_system_packages());

    summary
        .execute(
            Some(package_path),
            config,
            None::<&()>,
            None::<fn(&mut _) -> anyhow::Result<()>>,
        )
        .context("Failed to execute summary generation")?;

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
fn find_module_in_summary(summary: &Value, module_name: &str) -> AnyResult<Module> {
    let Some(obj) = summary.as_object() else {
        // If not an object, try as array.
        if let Some(arr) = summary.as_array() {
            return find_module_in_array(arr, module_name);
        }
        bail!("Module '{}' not found in package summary", module_name);
    };

    // Check if this is a single module summary with an "id" field.
    if obj.contains_key("id") {
        let module: Module =
            serde_json::from_value(summary.clone()).context("Failed to parse module summary")?;
        if module.id.name.as_ref() == module_name {
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
fn find_module_in_array(modules: &[Value], module_name: &str) -> AnyResult<Module> {
    for module_value in modules {
        if let Ok(module) = serde_json::from_value::<Module>(module_value.clone()) {
            if module.id.name.as_ref() == module_name {
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
pub fn generate_input_schema_from_summary(module: &Module) -> AnyResult<String> {
    let execute_symbol = Symbol::from("execute");
    let execute_fn = module
        .functions
        .get(&execute_symbol)
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

        let mut param_schema = convert_type_to_schema(&param.type_)?;
        // Add index field to preserve parameter ordering for PTB construction.
        if let Value::Object(ref mut obj) = param_schema {
            obj.insert("index".to_string(), Value::Number(param_index.into()));
        }

        // Handle optional parameter name (FromSource).
        let param_name = param
            .name
            .as_ref()
            .map(|s| s.as_ref().to_string())
            .unwrap_or_else(|| format!("param_{}", i));

        schema_map.insert(param_name, param_schema);
        param_index += 1;
    }

    Ok(schema_map)
}

/// Generate output schema from the Output enum.
pub fn generate_output_schema_from_summary(module: &Module) -> AnyResult<String> {
    let output_symbol = Symbol::from("Output");
    let output_enum = module
        .enums
        .get(&output_symbol)
        .ok_or_else(|| anyhow!("Enum 'Output' not found in module"))?;

    let schema_map = build_output_schema_map(&output_enum.variants)?;
    let schema_json = Value::Object(schema_map);
    serde_json::to_string(&schema_json).context("Failed to serialize output schema")
}

/// Build the schema map from enum variants.
fn build_output_schema_map(variants: &IndexMap<Symbol, Variant>) -> AnyResult<Map<String, Value>> {
    let mut schema_map = Map::new();

    for (variant_name, variant) in variants {
        let fields_schema = build_variant_fields_schema(&variant.fields)?;
        let variant_name_str = variant_name.as_ref();
        let variant_schema = json!({
            "type": "variant",
            "description": format!("{} variant", variant_name_str),
            "fields": fields_schema
        });
        schema_map.insert(variant_name_str.to_lowercase(), variant_schema);
    }

    Ok(schema_map)
}

/// Build schema map for variant fields, sorted by field index.
fn build_variant_fields_schema(variant_fields: &Fields) -> AnyResult<Map<String, Value>> {
    let mut fields_schema = Map::new();

    // IndexMap preserves insertion order, and fields are already sorted by index.
    for (field_name, field) in &variant_fields.fields {
        let field_schema = convert_type_to_schema(&field.type_)?;
        fields_schema.insert(field_name.as_ref().to_string(), field_schema);
    }

    Ok(fields_schema)
}

// ============================================================================
// Type Checking and Conversion
// ============================================================================

/// Check if a type is TxContext (should be excluded from input schema).
fn is_tx_context_type(type_: &Type) -> bool {
    match type_ {
        Type::Datatype(datatype) => is_tx_context(datatype),
        Type::Reference(_, inner) => is_tx_context_type(inner),
        _ => false,
    }
}

/// Check if a datatype is TxContext.
fn is_tx_context(datatype: &Datatype) -> bool {
    matches!(
        (
            datatype.module.address.as_ref(),
            datatype.module.name.as_ref(),
            datatype.name.as_ref()
        ),
        ("sui" | "0x2", "tx_context", "TxContext")
    )
}

/// Check if an address represents the standard library (0x1 or "std").
fn is_std_address(address: &Symbol) -> bool {
    matches!(address.as_ref(), "0x1" | "std")
}

/// Check if an address represents the Sui framework (0x2 or "sui").
fn is_sui_address(address: &Symbol) -> bool {
    matches!(address.as_ref(), "0x2" | "sui")
}

/// Convert a summary type to a JSON schema representation.
fn convert_type_to_schema(type_: &Type) -> AnyResult<Value> {
    match type_ {
        Type::Bool => Ok(create_schema("bool", "Boolean value")),
        Type::U8 => Ok(create_schema("u8", "8-bit unsigned integer")),
        Type::U16 => Ok(create_schema("u16", "16-bit unsigned integer")),
        Type::U32 => Ok(create_schema("u32", "32-bit unsigned integer")),
        Type::U64 => Ok(create_schema("u64", "64-bit unsigned integer")),
        Type::U128 => Ok(create_schema("u128", "128-bit unsigned integer")),
        Type::U256 => Ok(create_schema("u256", "256-bit unsigned integer")),
        Type::Address => Ok(create_schema("address", "Sui address")),
        Type::Signer => Ok(create_schema("signer", "Transaction signer")),
        Type::Datatype(datatype) => convert_datatype_to_schema(datatype),
        Type::Vector(element_type) => convert_vector_to_schema(element_type),
        Type::Reference(is_mutable, inner) => convert_reference_to_schema(*is_mutable, inner),
        Type::TypeParameter(_) | Type::NamedTypeParameter(_) => {
            Ok(create_schema("generic", "Generic type parameter"))
        }
        Type::Tuple(types) => {
            if types.is_empty() {
                Ok(create_schema("unit", "Unit type"))
            } else {
                Ok(create_schema("tuple", "Tuple type"))
            }
        }
        Type::Fun(_, _) => Ok(create_schema("function", "Function type")),
        Type::Any => Ok(create_schema("any", "Any type")),
    }
}

/// Helper to create a schema object with type and description.
fn create_schema(type_name: &str, description: &str) -> Value {
    json!({
        "type": type_name,
        "description": description
    })
}

/// Convert a reference type to schema, adding mutability flag.
fn convert_reference_to_schema(is_mutable: bool, inner: &Type) -> AnyResult<Value> {
    let mut inner_schema = convert_type_to_schema(inner)?;
    if let Value::Object(ref mut obj) = inner_schema {
        obj.insert("mutable".to_string(), Value::Bool(is_mutable));
    }
    Ok(inner_schema)
}

/// Convert a vector type to schema.
fn convert_vector_to_schema(element_type: &Type) -> AnyResult<Value> {
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
        &format!(
            "{}::{}::{}",
            address.as_ref(),
            module_name.as_ref(),
            type_name.as_ref()
        ),
    ))
}

/// Convert standard library (0x1) types to schema.
fn convert_std_type_to_schema(module_name: &Symbol, type_name: &Symbol) -> AnyResult<Value> {
    match (module_name.as_ref(), type_name.as_ref()) {
        ("string", "String") => Ok(create_schema("string", "0x1::string::String")),
        ("ascii", "String") => Ok(create_schema("string", "0x1::ascii::String")),
        _ => Ok(create_schema(
            "object",
            &format!("0x1::{}::{}", module_name.as_ref(), type_name.as_ref()),
        )),
    }
}

/// Convert Sui framework (0x2) types to schema.
fn convert_sui_type_to_schema(module_name: &Symbol, type_name: &Symbol) -> AnyResult<Value> {
    match (module_name.as_ref(), type_name.as_ref()) {
        ("object", "ID") => Ok(create_schema("object_id", "Sui object ID")),
        ("tx_context", "TxContext") => Ok(create_schema(
            "tx_context",
            "Transaction context (automatically provided)",
        )),
        _ => Ok(create_schema(
            "object",
            &format!("0x2::{}::{}", module_name.as_ref(), type_name.as_ref()),
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
        let schema = convert_type_to_schema(&Type::U64).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["description"], "64-bit unsigned integer");

        let schema = convert_type_to_schema(&Type::Bool).unwrap();
        assert_eq!(schema["type"], "bool");
        assert_eq!(schema["description"], "Boolean value");

        let schema = convert_type_to_schema(&Type::Address).unwrap();
        assert_eq!(schema["type"], "address");
        assert_eq!(schema["description"], "Sui address");
    }

    #[test]
    fn test_convert_string_types() {
        // Test std::ascii::String.
        let datatype = Datatype {
            module: ModuleId {
                address: Symbol::from("std"),
                name: Symbol::from("ascii"),
            },
            name: Symbol::from("String"),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "0x1::ascii::String");

        // Test std::string::String.
        let datatype = Datatype {
            module: ModuleId {
                address: Symbol::from("std"),
                name: Symbol::from("string"),
            },
            name: Symbol::from("String"),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "0x1::string::String");
    }

    #[test]
    fn test_convert_sui_object_id() {
        let datatype = Datatype {
            module: ModuleId {
                address: Symbol::from("sui"),
                name: Symbol::from("object"),
            },
            name: Symbol::from("ID"),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "object_id");
        assert_eq!(schema["description"], "Sui object ID");
    }

    #[test]
    fn test_convert_custom_type() {
        let datatype = Datatype {
            module: ModuleId {
                address: Symbol::from("my_package"),
                name: Symbol::from("my_module"),
            },
            name: Symbol::from("MyStruct"),
            type_arguments: vec![],
        };
        let schema = convert_datatype_to_schema(&datatype).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "my_package::my_module::MyStruct");
    }

    #[test]
    fn test_convert_vector_type() {
        let vector_type = Type::Vector(Box::new(Type::U64));
        let schema = convert_type_to_schema(&vector_type).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "u64");
    }

    #[test]
    fn test_convert_reference_type() {
        // Mutable reference.
        let mutable_ref = Type::Reference(true, Box::new(Type::U64));
        let schema = convert_type_to_schema(&mutable_ref).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["mutable"], true);

        // Immutable reference.
        let immutable_ref = Type::Reference(false, Box::new(Type::U64));
        let schema = convert_type_to_schema(&immutable_ref).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["mutable"], false);
    }

    #[test]
    fn test_is_tx_context_type() {
        // Direct TxContext.
        let tx_ctx = Type::Datatype(Box::new(Datatype {
            module: ModuleId {
                address: Symbol::from("sui"),
                name: Symbol::from("tx_context"),
            },
            name: Symbol::from("TxContext"),
            type_arguments: vec![],
        }));
        assert!(is_tx_context_type(&tx_ctx));

        // Reference to TxContext.
        let ref_tx_ctx = Type::Reference(true, Box::new(tx_ctx));
        assert!(is_tx_context_type(&ref_tx_ctx));

        // Non-TxContext type.
        let other = Type::U64;
        assert!(!is_tx_context_type(&other));
    }

    #[test]
    fn test_address_helpers() {
        assert!(is_std_address(&Symbol::from("0x1")));
        assert!(is_std_address(&Symbol::from("std")));
        assert!(!is_std_address(&Symbol::from("sui")));

        assert!(is_sui_address(&Symbol::from("0x2")));
        assert!(is_sui_address(&Symbol::from("sui")));
        assert!(!is_sui_address(&Symbol::from("std")));
    }

    #[test]
    fn test_is_tx_context_datatype() {
        let tx_ctx_datatype = Datatype {
            module: ModuleId {
                address: Symbol::from("sui"),
                name: Symbol::from("tx_context"),
            },
            name: Symbol::from("TxContext"),
            type_arguments: vec![],
        };
        assert!(is_tx_context(&tx_ctx_datatype));

        // With 0x2 address.
        let tx_ctx_datatype_hex = Datatype {
            module: ModuleId {
                address: Symbol::from("0x2"),
                name: Symbol::from("tx_context"),
            },
            name: Symbol::from("TxContext"),
            type_arguments: vec![],
        };
        assert!(is_tx_context(&tx_ctx_datatype_hex));

        // Non-TxContext.
        let other_datatype = Datatype {
            module: ModuleId {
                address: Symbol::from("sui"),
                name: Symbol::from("object"),
            },
            name: Symbol::from("ID"),
            type_arguments: vec![],
        };
        assert!(!is_tx_context(&other_datatype));
    }
}
