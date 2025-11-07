//! Input schema generation for Move onchain tools.

use {
    super::types::{convert_move_type_to_schema, is_tx_context_param},
    anyhow::{anyhow, Result as AnyResult},
    serde_json::{Map, Value},
};

/// Generate input schema by introspecting the execute function's parameters.
///
/// This function fetches the Move module from the chain and analyzes the
/// execute function's parameters to generate a JSON schema. It automatically
/// skips the first parameter (Promise/ProofOfUID) and the last parameter (TxContext).
pub async fn generate_input_schema(
    sui: &crate::sui::Client,
    package_address: crate::sui::ObjectID,
    module_name: &str,
    execute_function: &str,
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

    // Find the execute function.
    let execute_func = normalized_module
        .exposed_functions
        .get(execute_function)
        .ok_or_else(|| {
            anyhow!(
                "Function '{}' not found in module '{}'",
                execute_function,
                module_name
            )
        })?;

    // Parse function parameters.
    let mut schema_map = Map::new();
    let mut param_index = 0;

    for (i, param_type) in execute_func.parameters.iter().enumerate() {
        let is_tx_context = is_tx_context_param(param_type);

        // Skip the first parameter (Promise/ProofOfUID) and the last parameter (TxContext).
        if i == 0 || is_tx_context {
            continue;
        }

        let param_schema = convert_move_type_to_schema(param_type)?;

        // Store parameter information with index as the default name.
        let param_obj = match param_schema {
            Value::Object(obj) => obj,
            other => {
                let mut new_obj = Map::new();
                new_obj.insert("type".to_string(), other);
                new_obj
            }
        };

        schema_map.insert(param_index.to_string(), Value::Object(param_obj));
        param_index += 1;
    }

    let schema_json = Value::Object(schema_map);
    let schema_string = serde_json::to_string(&schema_json)?;

    Ok(schema_string)
}
