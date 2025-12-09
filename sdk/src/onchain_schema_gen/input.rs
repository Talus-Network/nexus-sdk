//! Input schema generation for Move onchain tools.

use {
    super::types::{convert_move_type_to_schema, is_tx_context_param},
    crate::sui,
    anyhow::{anyhow, bail, Result as AnyResult},
    serde_json::{Map, Value},
    std::sync::Arc,
    tokio::sync::Mutex,
};

/// Generate input schema by introspecting the execute function's parameters.
///
/// This function fetches the Move module from the chain and analyzes the
/// execute function's parameters to generate a JSON schema. It automatically
/// skips the first parameter (ProofOfUID) and the last parameter (TxContext).
pub async fn generate_input_schema(
    client: Arc<Mutex<sui::grpc::Client>>,
    package_address: sui::types::Address,
    module_name: &str,
    execute_function: &str,
) -> AnyResult<String> {
    let request = sui::grpc::GetPackageRequest::default().with_package_id(package_address);

    let mut client = client.lock().await;

    // Fetch all normalized Move modules for the package.
    let Some(package) = client
        .package_client()
        .get_package(request)
        .await
        .map(|resp| resp.into_inner().package)?
    else {
        bail!("Package '{}' not found", package_address)
    };

    drop(client);

    let all_modules = package.modules();

    // Find the specific module.
    let normalized_module = all_modules
        .into_iter()
        .find(|m| m.name() == module_name)
        .ok_or_else(|| {
            anyhow!(
                "Module '{}' not found in package '{}'",
                module_name,
                package_address
            )
        })?;

    // Find the execute function.
    let execute_func = normalized_module
        .functions()
        .into_iter()
        .find(|f| f.name() == execute_function)
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

    for (i, param_type) in execute_func.parameters().iter().enumerate() {
        let signature = param_type.body_opt().ok_or_else(|| {
            anyhow!(
                "Parameter type missing body in function '{}' of module '{}'",
                execute_function,
                module_name
            )
        })?;

        let is_tx_context = is_tx_context_param(signature);

        // Skip the first parameter (ProofOfUID) and the last parameter (TxContext).
        if i == 0 || is_tx_context {
            continue;
        }

        let param_schema = convert_move_type_to_schema(signature)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "test_utils")]
    async fn test_generate_input_schema_from_published_package() {
        use crate::test_utils;

        // Spin up the Sui instance.
        let (_container, rpc_port, faucet_port) =
            test_utils::containers::setup_sui_instance().await;

        // Create a wallet and request some gas tokens.
        let (mut wallet, _) = test_utils::wallet::create_ephemeral_wallet_context(rpc_port)
            .expect("Failed to create a wallet.");
        let sui = wallet.get_client().await.expect("Could not get Sui client");

        let addr = wallet
            .active_address()
            .expect("Failed to get active address.");

        test_utils::faucet::request_tokens(&format!("http://127.0.0.1:{faucet_port}/gas"), addr)
            .await
            .expect("Failed to request tokens from faucet.");

        let gas_coin = test_utils::gas::fetch_gas_coins(&sui, addr)
            .await
            .expect("Failed to fetch gas coin.")
            .into_iter()
            .next()
            .unwrap();

        // Publish test onchain_tool package.
        let response = test_utils::contracts::publish_move_package(
            &mut wallet,
            "tests/move/onchain_tool_test",
            gas_coin,
        )
        .await;

        let changes = response
            .object_changes
            .expect("TX response must have object changes");

        let pkg_id = *changes
            .iter()
            .find_map(|c| match c {
                crate::sui::ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .expect("Move package must be published");

        let client = Arc::new(Mutex::new(
            sui::grpc::Client::new(&format!("http://127.0.0.1:{rpc_port}"))
                .expect("Could not create gRPC client"),
        ));

        // Generate input schema for the onchain_tool::execute function.
        let schema_str = generate_input_schema(
            client,
            pkg_id.to_string().parse().unwrap(),
            "onchain_tool",
            "execute",
        )
        .await
        .expect("Failed to generate input schema");

        // Parse the schema.
        let schema: serde_json::Value =
            serde_json::from_str(&schema_str).expect("Failed to parse schema JSON");

        // Verify schema structure.
        // The execute function has signature:
        // execute(worksheet: &mut ProofOfUID, counter: &mut RandomCounter, increase_with: u64, _ctx: &mut TxContext)
        // After skipping ProofOfUID (first) and TxContext (last), we should have:
        // - Parameter 0: counter (&mut RandomCounter) - object type, mutable
        // - Parameter 1: increase_with (u64).

        // Check parameter 0 (counter).
        let param0 = schema
            .get("0")
            .expect("Schema should have parameter 0 (counter)");
        assert_eq!(param0["type"], "object");
        assert!(param0["description"]
            .as_str()
            .unwrap()
            .contains("RandomCounter"));

        // Check parameter 1 (increase_with).
        let param1 = schema
            .get("1")
            .expect("Schema should have parameter 1 (increase_with)");
        assert_eq!(param1["type"], "u64");
        assert_eq!(param1["description"], "64-bit unsigned integer");

        // Verify only 2 parameters (ProofOfUID and TxContext were skipped).
        assert_eq!(schema.as_object().unwrap().len(), 2);
    }
}
