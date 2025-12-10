//! Output schema generation for Move onchain tools.

use {
    super::types::convert_move_type_to_schema,
    crate::sui,
    anyhow::{anyhow, bail, Result as AnyResult},
    serde_json::{json, Map, Value},
    std::sync::Arc,
    tokio::sync::Mutex,
};

/// Generate output schema by introspecting the Move module's Output enum.
///
/// This function fetches the Move module from the chain and analyzes the
/// Output enum to generate a JSON schema. Each variant becomes a key in the
/// schema with its fields represented as nested schema objects.
pub async fn generate_output_schema(
    client: Arc<Mutex<sui::grpc::Client>>,
    package_address: sui::types::Address,
    module_name: &str,
    output_enum_name: &str,
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
        bail!("Package '{package_address}' not found")
    };

    drop(client);

    let all_modules = package.modules();

    // Find the specific module.
    let normalized_module = all_modules
        .iter()
        .find(|m| m.name() == module_name)
        .ok_or_else(|| {
            anyhow!("Module '{module_name}' not found in package '{package_address}'")
        })?;

    // Find the Output enum.
    let output_enum = normalized_module
        .datatypes()
        .iter()
        .find(|kind| kind.name() == output_enum_name)
        .ok_or_else(|| anyhow!("Enum '{output_enum_name}' not found in module '{module_name}'"))?;

    // Parse the enum variants from the normalized enum.
    let mut schema_map = Map::new();

    // Iterate through each variant in the enum.
    for variant in &output_enum.variants {
        let Some(variant_name) = variant.name_opt() else {
            bail!("Variant name missing in enum '{output_enum_name}'")
        };

        let variant_fields = &variant.fields;

        let mut fields_schema = Map::new();

        // Convert each field in the variant to schema.
        for field in variant_fields {
            let Some(field_type) = &field.r#type else {
                bail!(
                    "Field type missing body in variant '{variant_name}' of enum '{output_enum_name}'"
                )
            };

            let Some(field_name) = field.name_opt() else {
                bail!("Field name missing in variant '{variant_name}' of enum '{output_enum_name}'")
            };

            let field_schema = convert_move_type_to_schema(field_type)?;
            fields_schema.insert(field_name.to_string(), field_schema);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(feature = "test_utils")]
    async fn test_generate_output_schema_from_published_package() {
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

        // Generate output schema for the onchain_tool::Output enum.
        let schema_str = generate_output_schema(
            client,
            pkg_id.to_string().parse().unwrap(),
            "onchain_tool",
            "Output",
        )
        .await
        .expect("Failed to generate output schema");

        // Parse the schema.
        let schema: serde_json::Value =
            serde_json::from_str(&schema_str).expect("Failed to parse schema JSON");

        // Verify schema structure.
        // The Output enum has three variants: Ok, Err, LargeIncrement.

        // Check "ok" variant.
        let ok_variant = schema.get("ok").expect("Schema should have 'ok' variant");
        assert_eq!(ok_variant["type"], "variant");
        assert!(ok_variant["description"].as_str().unwrap().contains("Ok"));

        let ok_fields = ok_variant["fields"].as_object().unwrap();
        assert_eq!(ok_fields["old_count"]["type"], "u64");
        assert_eq!(ok_fields["new_count"]["type"], "u64");
        assert_eq!(ok_fields["increment"]["type"], "u64");

        // Check "err" variant.
        let err_variant = schema.get("err").expect("Schema should have 'err' variant");
        assert_eq!(err_variant["type"], "variant");
        assert!(err_variant["description"].as_str().unwrap().contains("Err"));

        let err_fields = err_variant["fields"].as_object().unwrap();
        assert_eq!(err_fields["reason"]["type"], "string");

        // Check "largeincrement" variant (lowercase).
        let large_variant = schema
            .get("largeincrement")
            .expect("Schema should have 'largeincrement' variant");
        assert_eq!(large_variant["type"], "variant");
        assert!(large_variant["description"]
            .as_str()
            .unwrap()
            .contains("LargeIncrement"));

        let large_fields = large_variant["fields"].as_object().unwrap();
        assert_eq!(large_fields["old_count"]["type"], "u64");
        assert_eq!(large_fields["new_count"]["type"], "u64");
        assert_eq!(large_fields["increment"]["type"], "u64");
        assert_eq!(large_fields["warning"]["type"], "string");

        // Verify all three variants are present.
        assert_eq!(schema.as_object().unwrap().len(), 3);
    }
}
