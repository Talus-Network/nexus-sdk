//! Output schema generation for Move onchain tools.

use anyhow::{anyhow, Result as AnyResult};
use serde_json::{json, Map, Value};

use super::types::convert_move_type_to_schema;

/// Generate output schema by introspecting the Move module's Output enum.
///
/// This function fetches the Move module from the chain and analyzes the
/// Output enum to generate a JSON schema. Each variant becomes a key in the
/// schema with its fields represented as nested schema objects.
pub async fn generate_output_schema(
    sui: &crate::sui::Client,
    package_address: crate::sui::ObjectID,
    module_name: &str,
    output_enum_name: &str,
) -> AnyResult<String> {
    // Fetch all normalized Move modules for the package.
    let all_modules = sui
        .read_api()
        .get_normalized_move_modules_by_package(package_address)
        .await?;

    // Find the specific module.
    let normalized_module = all_modules.get(module_name).ok_or_else(|| {
        anyhow!(
            "Module '{}' not found in package '{}'",
            module_name,
            package_address
        )
    })?;

    // Find the Output enum.
    let output_enum = normalized_module
        .enums
        .get(output_enum_name)
        .ok_or_else(|| {
            anyhow!(
                "Enum '{}' not found in module '{}'",
                output_enum_name,
                module_name
            )
        })?;

    // Parse the enum variants from the normalized enum.
    let mut schema_map = Map::new();

    // Iterate through each variant in the enum.
    for (variant_name, variant_fields) in &output_enum.variants {
        let mut fields_schema = Map::new();

        // Convert each field in the variant to schema.
        for field in variant_fields {
            let field_schema = convert_move_type_to_schema(&field.type_)?;
            fields_schema.insert(field.name.clone(), field_schema);
        }

        // Create the variant schema.
        let variant_schema = json!({
            "type": "variant",
            "description": format!("{} variant", variant_name),
            "fields": fields_schema
        });

        schema_map.insert(variant_name.to_lowercase(), variant_schema);
    }

    let schema_json = Value::Object(schema_map);
    let schema_string = serde_json::to_string(&schema_json)?;

    Ok(schema_string)
}

