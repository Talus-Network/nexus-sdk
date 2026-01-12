use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        onchain_schema_gen,
        nexus::error::NexusError,
        sui,
        transactions::tool,
    },
    serde::{Deserialize, Serialize},
    serde_json::{json, Map, Value},
    std::{
        collections::HashMap,
        io::{self, Write},
        path::{Path, PathBuf},
    },
};

/// Register a new onchain tool with automatic schema generation.
/// The input and output schemas are automatically generated from the Move package summary.
/// todo: merge this function with the existing `tool_register.rs` function.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn register_onchain_tool(
    package_path: PathBuf,
    package: sui::types::Address,
    module: sui::types::Identifier,
    fqn: ToolFqn,
    description: String,
    witness_id: sui::types::Address,
    collateral_coin: Option<sui::types::Address>,
    no_save: bool,
    sui_gas_coin: Option<sui::types::Address>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    command_title!(
        "Registering Onchain Tool '{fqn}' from package '{package}::{module}'",
        fqn = fqn,
        package = package,
        module = module
    );

    if collateral_coin.is_some() && collateral_coin == sui_gas_coin {
        return Err(NexusCliError::Any(anyhow!(
            "The coin used for collateral cannot be the same as the gas coin."
        )));
    }

    let nexus_client = get_nexus_client(sui_gas_coin, sui_gas_budget).await?;
    let signer = nexus_client.signer();
    let gas_config = nexus_client.gas_config();
    let address = signer.get_active_address();
    let nexus_objects = &*nexus_client.get_nexus_objects();
    let conf = CliConf::load().await.unwrap_or_default();
    let client = build_sui_grpc_client(&conf).await?;

    let collateral_coin = fetch_coin(client.clone(), address, collateral_coin, 1).await?;

    // Generate schemas from the Move package summary.
    let (input_schema, output_schema) = generate_schemas_from_summary(&package_path, &module_name)?;

    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::tx::TransactionBuilder::new();

    if let Err(e) = tool::register_on_chain_for_self(
        &mut tx,
        nexus_objects,
        package,
        module.as_str(),
        &input_schema,
        &output_schema,
        &fqn,
        &description,
        witness_id,
        &collateral_coin,
        address,
    ) {
        tx_handle.error();

        return Err(NexusCliError::Any(e));
    }

    tx_handle.success();

    let mut gas_coin = gas_config.acquire_gas_coin().await;

    tx.set_sender(address);
    tx.set_gas_budget(gas_config.get_budget());
    tx.set_gas_price(nexus_client.get_reference_gas_price());

    tx.add_gas_objects(vec![sui::tx::Input::owned(
        *gas_coin.object_id(),
        gas_coin.version(),
        *gas_coin.digest(),
    )]);

    let tx = tx.finish().map_err(|e| NexusCliError::Any(e.into()))?;

    let signature = signer.sign_tx(&tx).await.map_err(NexusCliError::Nexus)?;

    // Sign and submit the TX.
    let response = match signer.execute_tx(tx, signature, &mut gas_coin).await {
        Ok(response) => {
            gas_config.release_gas_coin(gas_coin).await;

            response
        }
        // If the tool is already registered, we don't want to fail the
        // command.
        Err(NexusError::Wallet(e)) if e.to_string().contains("register_off_chain_tool_") => {
            gas_config.release_gas_coin(gas_coin).await;

            notify_error!(
                "Tool '{fqn}' is already registered.",
                fqn = fqn.to_string().truecolor(100, 100, 100)
            );

            json_output(&json!({
                "tool_fqn": fqn,
                "already_registered": true,
            }))?;

            return Err(NexusCliError::Any(e));
        }
        // Any other error fails the tool registration but continues the
        // loop.
        Err(e) => {
            gas_config.release_gas_coin(gas_coin).await;

            notify_error!(
                "Failed to register tool '{fqn}': {error}",
                fqn = fqn.to_string().truecolor(100, 100, 100),
                error = e
            );

            return Err(NexusCliError::Nexus(e));
        }
    };

    // Extract the OwnerCap<OverTool> object ID.
    let over_tool_id = extract_over_tool_owner_cap(&response.objects, nexus_objects)?;

    // Save the owner caps to the CLI conf.
    if !no_save {
        save_tool_owner_caps(fqn.clone(), over_tool_id).await?;
    }

    json_output(&json!({
        "digest": response.digest,
        "tool_fqn": fqn,
        "package_address": package,
        "module_name": module,
        "witness_id": witness_id.to_string(),
        "description": description,
        "input_schema": input_schema,
        "output_schema": output_schema,
        "owner_cap_over_tool_id": over_tool_id,
        "owner_cap_over_gas_id": null,
        "already_registered": false,
    }))?;

    Ok(())
}

/// Validate that gas and collateral coins are different.
fn validate_gas_and_collateral_coins(
    gas_coin: &sui::Coin,
    collateral_coin: &sui::Coin,
) -> AnyResult<(), NexusCliError> {
    if gas_coin.coin_object_id == collateral_coin.coin_object_id {
        return Err(NexusCliError::Any(anyhow!(
            "Gas and collateral coins must be different."
        )));
    }
    Ok(())
}

/// Generate input and output schemas from the Move package summary.
fn generate_schemas_from_summary(
    package_path: &Path,
    module_name: &str,
) -> AnyResult<(String, String), NexusCliError> {
    // Run sui move summary command and parse the output.
    let summary_handle = loading!("Running 'sui move summary' on package...");
    let module_summary = match onchain_schema_gen::run_summary_command(package_path, module_name) {
        Ok(summary) => {
            summary_handle.success();
            summary
        }
        Err(e) => {
            summary_handle.error();
            return Err(NexusCliError::Any(e));
        }
    };

    // Generate input schema from the execute function parameters.
    let input_handle = loading!("Generating input schema from package summary...");
    let input_schema = match onchain_schema_gen::generate_input_schema_from_summary(&module_summary)
    {
        Ok(schema) => {
            input_handle.success();
            schema
        }
        Err(e) => {
            input_handle.error();
            return Err(NexusCliError::Any(e));
        }
    };

    // Generate output schema from the Output enum.
    let output_handle = loading!("Generating output schema from package summary...");
    let output_schema =
        match onchain_schema_gen::generate_output_schema_from_summary(&module_summary) {
            Ok(schema) => {
                output_handle.success();
                schema
            }
            Err(e) => {
                output_handle.error();
                return Err(NexusCliError::Any(e));
            }
        };

    Ok((input_schema, output_schema))
}

/// Extract the OwnerCap<OverTool> object ID from the transaction response.
fn extract_over_tool_owner_cap(
    objects: &[sui::types::Object],
    nexus_objects: &NexusObjects,
) -> AnyResult<sui::types::Address, NexusCliError> {
    // Find `CloneableOwnerCap<OverTool>` object ID
    let over_tool = objects.iter().find_map(|obj| {
        let sui::types::ObjectType::Struct(object_type) = obj.object_type() else {
            return None;
        };

        if *object_type.address() == nexus_objects.primitives_pkg_id
            && *object_type.module() == primitives::OwnerCap::CLONEABLE_OWNER_CAP.module
            && *object_type.name() == primitives::OwnerCap::CLONEABLE_OWNER_CAP.name
        {
            Some(obj.object_id())
        } else {
            None
        }
    });

    let Some(over_tool_id) = over_tool else {
        return Err(NexusCliError::Any(anyhow!(
            "Could not find the OwnerCap<OverTool> object ID in the transaction response."
        )));
    };

    notify_success!(
        "OwnerCap<OverTool> object ID: {id}",
        id = over_tool_id.to_string().truecolor(100, 100, 100)
    );

    notify_success!("Onchain tools use a different gas model. No OverGas cap was created.");

    Ok(over_tool_id)
}

/// Save the tool owner caps to the CLI configuration.
async fn save_tool_owner_caps(
    fqn: ToolFqn,
    over_tool_id: sui::types::Address,
) -> AnyResult<(), NexusCliError> {
    let save_handle = loading!("Saving the owner cap to the CLI configuration...");

    let mut conf = CliConf::load().await.unwrap_or_default();

    // For onchain tools, we only have OverTool cap, no OverGas cap.
    conf.tools.insert(
        fqn,
        ToolOwnerCaps {
            over_tool: over_tool_id,
            over_gas: None,
        },
    );

    if let Err(e) = conf.save().await {
        save_handle.error();
        return Err(NexusCliError::Any(e));
    }

    save_handle.success();

    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, nexus_sdk::test_utils::sui_mocks, std::sync::atomic::Ordering};

    #[test]
    fn test_build_final_schema_with_custom_names() {
        // Create a schema with integer keys and custom names using typed structs.
        let mut schema = HashMap::new();

        // Parameter 0: u64 with custom name "increment_amount".
        schema.insert(
            "0".to_string(),
            ParameterSchema {
                param_type: "u64".to_string(),
                description: "64-bit unsigned integer".to_string(),
                custom_name: Some("increment_amount".to_string()),
                mutable: None,
                parameter_index: Some("0".to_string()),
            },
        );

        // Parameter 1: object with custom name "counter".
        schema.insert(
            "1".to_string(),
            ParameterSchema {
                param_type: "object".to_string(),
                description: "Counter object reference".to_string(),
                custom_name: Some("counter".to_string()),
                mutable: Some(true),
                parameter_index: Some("1".to_string()),
            },
        );

        // Convert schema.
        let result = build_final_schema(schema).unwrap();

        // Verify custom names are used as keys.
        assert!(result.contains_key("increment_amount"));
        assert!(result.contains_key("counter"));
        assert!(!result.contains_key("0"));
        assert!(!result.contains_key("1"));

        // Verify metadata fields are removed.
        let increment_amount_param = result.get("increment_amount").unwrap().as_object().unwrap();
        assert!(!increment_amount_param.contains_key("custom_name"));
        assert!(!increment_amount_param.contains_key("parameter_index"));

        // Verify type and description are preserved.
        assert_eq!(increment_amount_param.get("type").unwrap(), "u64");
        assert_eq!(
            increment_amount_param.get("description").unwrap(),
            "64-bit unsigned integer"
        );

        // Verify mutable flag is preserved for counter.
        let counter_param = result.get("counter").unwrap().as_object().unwrap();
        assert_eq!(counter_param.get("mutable").unwrap(), true);
        assert!(!counter_param.contains_key("custom_name"));
        assert!(!counter_param.contains_key("parameter_index"));
    }

    #[test]
    fn test_build_final_schema_without_custom_names() {
        // Create a schema with integer keys but no custom names using typed structs.
        let mut schema = HashMap::new();

        // Parameter 0: u64 without custom name.
        schema.insert(
            "0".to_string(),
            ParameterSchema {
                param_type: "u64".to_string(),
                description: "64-bit unsigned integer".to_string(),
                custom_name: None,
                mutable: None,
                parameter_index: None,
            },
        );

        // Parameter 1: object without custom name.
        schema.insert(
            "1".to_string(),
            ParameterSchema {
                param_type: "object".to_string(),
                description: "Object reference".to_string(),
                custom_name: None,
                mutable: Some(true),
                parameter_index: None,
            },
        );

        // Convert schema.
        let result = build_final_schema(schema).unwrap();

        // Verify integer keys are preserved.
        assert!(result.contains_key("0"));
        assert!(result.contains_key("1"));

        // Verify all properties are preserved.
        let param0_result = result.get("0").unwrap().as_object().unwrap();
        assert_eq!(param0_result.get("type").unwrap(), "u64");
        assert_eq!(
            param0_result.get("description").unwrap(),
            "64-bit unsigned integer"
        );

        let param1_result = result.get("1").unwrap().as_object().unwrap();
        assert_eq!(param1_result.get("type").unwrap(), "object");
        assert_eq!(param1_result.get("mutable").unwrap(), true);
    }

    #[test]
    fn test_build_final_schema_mixed() {
        // Create a schema with some custom names and some without using typed structs.
        let mut schema = HashMap::new();

        // Parameter 0: with custom name.
        schema.insert(
            "0".to_string(),
            ParameterSchema {
                param_type: "u64".to_string(),
                description: "Amount parameter".to_string(),
                custom_name: Some("amount".to_string()),
                mutable: None,
                parameter_index: Some("0".to_string()),
            },
        );

        // Parameter 1: without custom name.
        schema.insert(
            "1".to_string(),
            ParameterSchema {
                param_type: "bool".to_string(),
                description: "Flag parameter".to_string(),
                custom_name: None,
                mutable: None,
                parameter_index: None,
            },
        );

        // Parameter 2: with custom name.
        schema.insert(
            "2".to_string(),
            ParameterSchema {
                param_type: "string".to_string(),
                description: "Message parameter".to_string(),
                custom_name: Some("message".to_string()),
                mutable: None,
                parameter_index: Some("2".to_string()),
            },
        );

        // Convert schema.
        let result = build_final_schema(schema).unwrap();

        // Verify mixed keys.
        assert!(result.contains_key("amount")); // Custom name.
        assert!(result.contains_key("1")); // Integer key preserved.
        assert!(result.contains_key("message")); // Custom name.
        assert!(!result.contains_key("0")); // Replaced by custom name.
        assert!(!result.contains_key("2")); // Replaced by custom name.

        // Verify no metadata in results.
        for (_, value) in result.iter() {
            let obj = value.as_object().unwrap();
            assert!(!obj.contains_key("custom_name"));
            assert!(!obj.contains_key("parameter_index"));
        }
    }

    #[test]
    fn test_customize_parameter_descriptions_json_mode() {
        // Set JSON mode to skip interactive prompts.
        JSON_MODE.store(true, Ordering::Relaxed);

        // Input schema based on onchain tool example.
        let input_schema = r#"{
            "0": {
                "type": "object",
                "description": "0x123::onchain_tool::RandomCounter",
                "mutable": true
            },
            "1": {
                "type": "u64",
                "description": "64-bit unsigned integer"
            }
        }"#;

        // Call customize function.
        let result = customize_parameter_descriptions(input_schema.to_string()).unwrap();

        // In JSON mode, schema should be returned unchanged.
        let input_value: Value = serde_json::from_str(input_schema).unwrap();
        let result_value: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(input_value, result_value);

        // Reset JSON mode.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_customize_parameter_descriptions_empty_schema() {
        // Set JSON mode to skip interactive prompts.
        JSON_MODE.store(true, Ordering::Relaxed);

        // Empty schema.
        let input_schema = "{}";

        // Call customize function.
        let result = customize_parameter_descriptions(input_schema.to_string()).unwrap();

        // Should return empty schema unchanged.
        assert_eq!(result, "{}");

        // Reset JSON mode.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_customize_output_variant_and_field_descriptions_json_mode() {
        // Set JSON mode to skip interactive prompts.
        JSON_MODE.store(true, Ordering::Relaxed);

        // Output schema based on onchain tool example.
        let output_schema = r#"{
            "ok": {
                "type": "variant",
                "description": "Ok variant",
                "fields": {
                    "old_count": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    },
                    "new_count": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    },
                    "increment": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    }
                }
            },
            "err": {
                "type": "variant",
                "description": "Err variant",
                "fields": {
                    "reason": {
                        "type": "string",
                        "description": "0x1::ascii::String"
                    }
                }
            },
            "largeincrement": {
                "type": "variant",
                "description": "LargeIncrement variant",
                "fields": {
                    "old_count": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    },
                    "new_count": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    },
                    "increment": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    },
                    "warning": {
                        "type": "string",
                        "description": "0x1::ascii::String"
                    }
                }
            }
        }"#;

        // Call customize function.
        let result =
            customize_output_variant_and_field_descriptions(output_schema.to_string()).unwrap();

        // In JSON mode, schema should be returned unchanged.
        let input_value: Value = serde_json::from_str(output_schema).unwrap();
        let result_value: Value = serde_json::from_str(&result).unwrap();

        assert_eq!(input_value, result_value);

        // Reset JSON mode.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_build_final_schema_preserves_all_fields() {
        // Test that all field types are properly preserved during conversion using typed structs.
        let mut schema = HashMap::new();

        // Parameter with various field types.
        schema.insert(
            "0".to_string(),
            ParameterSchema {
                param_type: "object".to_string(),
                description: "Test object".to_string(),
                custom_name: Some("my_param".to_string()),
                mutable: Some(true),
                parameter_index: Some("0".to_string()),
            },
        );

        // Convert.
        let result = build_final_schema(schema).unwrap();

        // Verify custom name is used and metadata removed.
        assert!(result.contains_key("my_param"));
        let my_param = result.get("my_param").unwrap().as_object().unwrap();

        // Verify all non-metadata fields are preserved.
        assert_eq!(my_param.get("type").unwrap(), "object");
        assert_eq!(my_param.get("description").unwrap(), "Test object");
        assert_eq!(my_param.get("mutable").unwrap(), true);

        // Verify metadata fields are removed.
        assert!(!my_param.contains_key("custom_name"));
        assert!(!my_param.contains_key("parameter_index"));
    }

    #[test]
    fn test_build_final_schema_with_none_custom_name() {
        // Test that None custom_name is treated as no custom name using typed structs.
        let mut schema = HashMap::new();

        schema.insert(
            "0".to_string(),
            ParameterSchema {
                param_type: "u64".to_string(),
                description: "Test param".to_string(),
                custom_name: None,
                mutable: None,
                parameter_index: None,
            },
        );

        // Convert.
        let result = build_final_schema(schema).unwrap();

        // Verify integer key is preserved when custom_name is None.
        assert!(result.contains_key("0"));
        assert_eq!(result.get("0").unwrap()["type"], "u64");
    }

    #[test]
    fn test_customize_parameter_descriptions_with_mock_input() {
        // Ensure JSON_MODE is off for this test.
        JSON_MODE.store(false, Ordering::Relaxed);

        // Create input schema.
        let input_schema = r#"{
            "0": {
                "type": "u64",
                "description": "64-bit unsigned integer"
            }
        }"#;

        // Mock user input: custom name "amount" + enter, then custom description "The amount to use" + enter.
        let mock_input = "amount\nThe amount to use\n";
        let mut reader = std::io::Cursor::new(mock_input.as_bytes());

        // Call the function with mock input.
        let result =
            customize_parameter_descriptions_with_reader(input_schema.to_string(), &mut reader)
                .unwrap();

        // Parse the result.
        let result_value: Value = serde_json::from_str(&result).unwrap();

        // Verify the custom name was applied.
        assert!(result_value.get("amount").is_some());
        assert!(result_value.get("0").is_none());

        // Verify the custom description was applied.
        let amount_param = result_value.get("amount").unwrap();
        assert_eq!(
            amount_param.get("description").unwrap().as_str().unwrap(),
            "The amount to use"
        );
        assert_eq!(amount_param.get("type").unwrap().as_str().unwrap(), "u64");

        // Reset JSON_MODE.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_customize_parameter_descriptions_keep_defaults() {
        // Ensure JSON_MODE is off.
        JSON_MODE.store(false, Ordering::Relaxed);

        // Create input schema.
        let input_schema = r#"{
            "0": {
                "type": "bool",
                "description": "Boolean flag"
            }
        }"#;

        // Mock user input: empty (press enter) for both name and description to keep defaults.
        let mock_input = "\n\n";
        let mut reader = std::io::Cursor::new(mock_input.as_bytes());

        // Call the function.
        let result =
            customize_parameter_descriptions_with_reader(input_schema.to_string(), &mut reader)
                .unwrap();

        // Parse the result.
        let result_value: Value = serde_json::from_str(&result).unwrap();

        // Verify integer key is preserved (no custom name).
        assert!(result_value.get("0").is_some());

        // Verify description is unchanged.
        let param = result_value.get("0").unwrap();
        assert_eq!(
            param.get("description").unwrap().as_str().unwrap(),
            "Boolean flag"
        );

        // Reset JSON_MODE.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_customize_output_with_mock_input() {
        // Ensure JSON_MODE is off.
        JSON_MODE.store(false, Ordering::Relaxed);

        // Create output schema with one variant.
        let output_schema = r#"{
            "ok": {
                "type": "variant",
                "description": "Ok variant",
                "fields": {
                    "count": {
                        "type": "u64",
                        "description": "64-bit unsigned integer"
                    }
                }
            }
        }"#;

        // Mock user input:
        // 1. "Success case\n" for variant description
        // 2. "The final count\n" for field description.
        let mock_input = "Success case\nThe final count\n";
        let mut reader = std::io::Cursor::new(mock_input.as_bytes());

        // Call the function.
        let result = customize_output_variant_and_field_descriptions_with_reader(
            output_schema.to_string(),
            &mut reader,
        )
        .unwrap();

        // Parse the result.
        let result_value: Value = serde_json::from_str(&result).unwrap();

        // Verify variant description was updated.
        let ok_variant = result_value.get("ok").unwrap();
        assert_eq!(
            ok_variant.get("description").unwrap().as_str().unwrap(),
            "Success case"
        );

        // Verify field description was updated.
        let fields = ok_variant.get("fields").unwrap();
        let count_field = fields.get("count").unwrap();
        assert_eq!(
            count_field.get("description").unwrap().as_str().unwrap(),
            "The final count"
        );

        // Reset JSON_MODE.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_customize_output_keep_defaults() {
        // Ensure JSON_MODE is off.
        JSON_MODE.store(false, Ordering::Relaxed);

        // Create output schema.
        let output_schema = r#"{
            "err": {
                "type": "variant",
                "description": "Error variant",
                "fields": {
                    "message": {
                        "type": "string",
                        "description": "Error message"
                    }
                }
            }
        }"#;

        // Mock user input: empty (press enter) for both variant and field to keep defaults.
        let mock_input = "\n\n";
        let mut reader = std::io::Cursor::new(mock_input.as_bytes());

        // Call the function.
        let result = customize_output_variant_and_field_descriptions_with_reader(
            output_schema.to_string(),
            &mut reader,
        )
        .unwrap();

        // Parse the result.
        let result_value: Value = serde_json::from_str(&result).unwrap();

        // Verify descriptions are unchanged.
        let err_variant = result_value.get("err").unwrap();
        assert_eq!(
            err_variant.get("description").unwrap().as_str().unwrap(),
            "Error variant"
        );

        let fields = err_variant.get("fields").unwrap();
        let message_field = fields.get("message").unwrap();
        assert_eq!(
            message_field.get("description").unwrap().as_str().unwrap(),
            "Error message"
        );

        // Reset JSON_MODE.
        JSON_MODE.store(false, Ordering::Relaxed);
    }

    #[test]
    fn test_extract_over_tool_owner_cap_success() {
        let mut rng = rand::thread_rng();
        let nexus_objects = sui_mocks::mock_nexus_objects();

        // Create a mock object vector with an OwnerCap<OverTool>.
        let owner_cap_id = sui::types::Address::generate(&mut rng);
        let owner_address = sui::types::Address::generate(&mut rng);

        let objects = vec![sui::types::Object::new(
            sui::types::ObjectData::Struct(
                sui::types::MoveStruct::new(
                    sui::types::StructTag::new(
                        nexus_objects.primitives_pkg_id,
                        sui::types::Identifier::from_static("owner_cap"),
                        sui::types::Identifier::from_static("CloneableOwnerCap"),
                        vec![],
                    ),
                    true,
                    0,
                    owner_cap_id.to_bcs().unwrap(),
                )
                .unwrap(),
            ),
            sui::types::Owner::Address(owner_address),
            sui::types::Digest::generate(&mut rng),
            1000,
        )];

        // Extract the owner cap.
        let result = extract_over_tool_owner_cap(&objects, &nexus_objects);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), owner_cap_id);
    }

    #[test]
    fn test_extract_over_tool_owner_cap_not_found() {
        let nexus_objects = sui_mocks::mock_nexus_objects();

        // Should fail because no owner cap is found.
        let result = extract_over_tool_owner_cap(&[], &nexus_objects);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Could not find the OwnerCap<OverTool> object ID"));
    }

    #[tokio::test]
    async fn test_generate_and_customize_schemas_integration() {
        use crate::test_utils;

        // Spin up the Sui instance.
        let test_utils::containers::SuiInstance {
            rpc_port,
            faucet_port,
            pg: _pg,
            container: _container,
            ..
        } = test_utils::containers::setup_sui_instance().await;

        let rpc_url = format!("http://127.0.0.1:{rpc_port}");
        let faucet_url = format!("http://127.0.0.1:{faucet_port}/gas");

        let mut rng = rand::thread_rng();

        // Create a wallet and request some gas tokens.
        let pk = sui::crypto::Ed25519PrivateKey::generate(&mut rng);
        let addr = pk.public_key().derive_address();

        test_utils::faucet::request_tokens(&faucet_url, addr)
            .await
            .expect("Failed to request tokens from faucet.");

        let (gas_coin, _) = test_utils::gas::fetch_gas_coins(&rpc_url, addr)
            .await
            .expect("Failed to fetch gas coin.")
            .into_iter()
            .next()
            .unwrap();

        // Publish test onchain_tool package.
        let response = test_utils::contracts::publish_move_package(
            &pk,
            &rpc_url,
            "../sdk/tests/move/onchain_tool_test",
            gas_coin,
        )
        .await;

        let pkg_id = response
            .objects
            .iter()
            .find_map(|c| match c.data() {
                sui::types::ObjectData::Package(m) => Some(m.id),
                _ => None,
            })
            .expect("Move package must be published");

        // Enable JSON mode to skip interactive prompts.
        JSON_MODE.store(true, Ordering::Relaxed);

        let client = Arc::new(Mutex::new(
            sui::grpc::Client::new(format!("http://127.0.0.1:{rpc_port}"))
                .expect("Failed to create Sui gRPC client"),
        ));

        // Generate and customize schemas.
        let result = generate_and_customize_schemas(
            client,
            pkg_id.to_string().parse().unwrap(),
            "onchain_tool",
        )
        .await;

        // Reset JSON mode.
        JSON_MODE.store(false, Ordering::Relaxed);

        // Should succeed.
        assert!(result.is_ok());
        let (input_schema, output_schema) = result.unwrap();

        // Verify input schema is valid JSON.
        let input_json: serde_json::Value =
            serde_json::from_str(&input_schema).expect("Input schema should be valid JSON");
        assert!(input_json.is_object());

        // Verify output schema is valid JSON.
        let output_json: serde_json::Value =
            serde_json::from_str(&output_schema).expect("Output schema should be valid JSON");
        assert!(output_json.is_object());

        // Verify input schema has expected parameters (counter and increase_with).
        // After skipping ProofOfUID and TxContext, we should have 2 parameters.
        assert_eq!(input_json.as_object().unwrap().len(), 2);

        // Verify output schema has expected variants.
        let output_obj = output_json.as_object().unwrap();
        assert!(output_obj.contains_key("ok") || output_obj.contains_key("err"));
    }

    #[tokio::test]
    async fn test_save_tool_owner_caps_success() {
        let mut rng = rand::thread_rng();
        // Create a test FQN and object ID.
        let fqn = "com.example.testtool@1".parse::<ToolFqn>().unwrap();
        let over_tool_id = sui::types::Address::generate(&mut rng);

        // Call save_tool_owner_caps.
        let result = save_tool_owner_caps(fqn.clone(), over_tool_id).await;

        // Should succeed.
        assert!(result.is_ok());
    }
}
