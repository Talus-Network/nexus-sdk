use {
    crate::{
        command_title,
        display::json_output,
        loading,
        notify_error,
        notify_success,
        prelude::*,
        sui::*,
        tool::tool_register_offchain,
    },
    nexus_sdk::{
        idents::{primitives, workflow},
        onchain_schema_gen,
        sui,
        transactions::tool,
    },
    serde_json::json,
    std::path::{Path, PathBuf},
};

/// Register a new onchain tool with automatic schema generation.
/// The input and output schemas are automatically generated from the Move package summary.
/// todo: merge this function with the existing `tool_register.rs` function.
/// https://github.com/Talus-Network/nexus/issues/501
pub(crate) async fn register_onchain_tool(
    package_path: PathBuf,
    module_id: sui::MoveModuleId,
    fqn: ToolFqn,
    description: String,
    witness_id: sui::ObjectID,
    collateral_coin: Option<sui::ObjectID>,
    no_save: bool,
    sui_gas_coin: Option<sui::ObjectID>,
    sui_gas_budget: u64,
) -> AnyResult<(), NexusCliError> {
    // Extract package address and module name from ModuleId.
    let package_address = sui::ObjectID::from(*module_id.address());
    let module_name = module_id.name().to_string();

    command_title!(
        "Registering Onchain Tool '{fqn}' from package '{package_address}'",
        fqn = fqn,
        package_address = package_address
    );

    // Load CLI configuration.
    let mut conf = CliConf::load().await.unwrap_or_default();

    // Nexus objects must be present in the configuration.
    let objects = &get_nexus_objects(&mut conf).await?;

    // Create wallet context, Sui client and find the active address.
    let mut wallet = create_wallet_context(&conf.sui.wallet_path, conf.sui.net).await?;
    let sui = build_sui_client(&conf.sui).await?;
    let address = wallet.active_address().map_err(NexusCliError::Any)?;

    // Fetch gas and collateral coin objects.
    let (gas_coin, collateral_coin) = tool_register_offchain::fetch_gas_and_collateral_coins(
        &sui,
        address,
        sui_gas_coin,
        collateral_coin,
    )
    .await?;

    // Validate that gas and collateral coins are different.
    validate_gas_and_collateral_coins(&gas_coin, &collateral_coin)?;

    // Fetch reference gas price.
    let reference_gas_price = fetch_reference_gas_price(&sui).await?;

    // Generate schemas from the Move package summary.
    let (input_schema, output_schema) = generate_schemas_from_summary(&package_path, &module_name)?;

    // Build the registration transaction.
    let tx_data = build_registration_transaction(
        objects,
        package_address,
        module_name.clone(),
        input_schema.clone(),
        output_schema.clone(),
        &fqn,
        description.clone(),
        witness_id,
        &collateral_coin,
        &gas_coin,
        address,
        sui_gas_budget,
        reference_gas_price,
    )?;

    // Execute the registration transaction.
    let response = match execute_registration_transaction(&sui, &wallet, tx_data, &fqn).await? {
        Some(response) => response,
        None => return Ok(()), // Tool already registered.
    };

    // Extract the OwnerCap<OverTool> object ID.
    let over_tool_id = extract_over_tool_owner_cap(&response, objects)?;

    // Save the owner caps to the CLI conf.
    if !no_save {
        save_tool_owner_caps(fqn.clone(), over_tool_id).await?;
    }

    json_output(&json!({
        "digest": response.digest,
        "tool_fqn": fqn,
        "package_address": package_address.to_string(),
        "module_name": module_name,
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

/// Build the registration transaction and return TransactionData.
fn build_registration_transaction(
    objects: &NexusObjects,
    package_address: sui::ObjectID,
    module_name: String,
    input_schema: String,
    output_schema: String,
    fqn: &ToolFqn,
    description: String,
    witness_id: sui::ObjectID,
    collateral_coin: &sui::Coin,
    gas_coin: &sui::Coin,
    address: sui::Address,
    sui_gas_budget: u64,
    reference_gas_price: u64,
) -> AnyResult<sui::TransactionData, NexusCliError> {
    let tx_handle = loading!("Crafting transaction...");

    let mut tx = sui::ProgrammableTransactionBuilder::new();

    if let Err(e) = tool::register_on_chain_for_self(
        &mut tx,
        objects,
        package_address,
        module_name,
        input_schema,
        output_schema,
        fqn,
        description,
        witness_id,
        collateral_coin,
        address.into(),
    ) {
        tx_handle.error();
        return Err(NexusCliError::Any(e));
    }

    tx_handle.success();

    let tx_data = sui::TransactionData::new_programmable(
        address,
        vec![gas_coin.object_ref()],
        tx.finish(),
        sui_gas_budget,
        reference_gas_price,
    );

    Ok(tx_data)
}

/// Sign and execute the registration transaction with specific error handling.
/// Returns None if the tool is already registered (handled gracefully).
async fn execute_registration_transaction(
    sui: &sui::Client,
    wallet: &sui::WalletContext,
    tx_data: sui::TransactionData,
    fqn: &ToolFqn,
) -> AnyResult<Option<sui::TransactionBlockResponse>, NexusCliError> {
    match sign_and_execute_transaction(sui, wallet, tx_data).await {
        Ok(response) => Ok(Some(response)),
        // If the tool is already registered, we don't want to fail the command.
        Err(NexusCliError::Any(e)) if e.to_string().contains("register_on_chain_tool_") => {
            notify_error!(
                "Tool '{fqn}' is already registered.",
                fqn = fqn.to_string().truecolor(100, 100, 100)
            );

            json_output(&json!({
                "tool_fqn": fqn,
                "already_registered": true,
            }))?;

            Ok(None)
        }
        // Any other error fails the tool registration.
        Err(e) => {
            notify_error!(
                "Failed to register tool '{fqn}': {error}",
                fqn = fqn.to_string().truecolor(100, 100, 100),
                error = e
            );

            Err(e)
        }
    }
}

/// Extract the OwnerCap<OverTool> object ID from the transaction response.
fn extract_over_tool_owner_cap(
    response: &sui::TransactionBlockResponse,
    objects: &NexusObjects,
) -> AnyResult<sui::ObjectID, NexusCliError> {
    // Parse the owner cap object IDs from the response.
    let owner_caps = response
        .object_changes
        .as_ref()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|change| match change {
            sui::ObjectChange::Created {
                object_type,
                object_id,
                ..
            } if object_type.address == *objects.primitives_pkg_id
                && object_type.module
                    == primitives::OwnerCap::CLONEABLE_OWNER_CAP.module.into()
                && object_type.name == primitives::OwnerCap::CLONEABLE_OWNER_CAP.name.into() =>
            {
                Some((*object_id, object_type.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    // Find `CloneableOwnerCap<OverTool>` object ID.
    let over_tool = owner_caps.iter().find_map(|(object_id, object_type)| {
        match object_type.type_params.first() {
            Some(sui::MoveTypeTag::Struct(what_for))
                if what_for.module == workflow::ToolRegistry::OVER_TOOL.module.into()
                    && what_for.name == workflow::ToolRegistry::OVER_TOOL.name.into() =>
            {
                Some(object_id)
            }
            _ => None,
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

    Ok(*over_tool_id)
}

/// Save the tool owner caps to the CLI configuration.
async fn save_tool_owner_caps(
    fqn: ToolFqn,
    over_tool_id: sui::ObjectID,
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
    fn test_validate_gas_and_collateral_coins_different() {
        // Create two different coins.
        let gas_coin = sui_mocks::mock_sui_coin(1000);
        let collateral_coin = sui_mocks::mock_sui_coin(2000);

        // Should succeed because coins are different.
        let result = validate_gas_and_collateral_coins(&gas_coin, &collateral_coin);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_gas_and_collateral_coins_same() {
        // Create a single coin.
        let coin = sui_mocks::mock_sui_coin(1000);

        // Should fail because both references point to the same coin.
        let result = validate_gas_and_collateral_coins(&coin, &coin);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Gas and collateral coins must be different"));
    }

    #[test]
    fn test_extract_over_tool_owner_cap_success() {
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let primitives_pkg_id = nexus_objects.primitives_pkg_id;

        // Create a mock transaction response with owner cap object.
        let owner_cap_id = sui::ObjectID::random();

        let object_type = sui::MoveStructTag {
            address: primitives_pkg_id.into(),
            module: primitives::OwnerCap::CLONEABLE_OWNER_CAP.module.into(),
            name: primitives::OwnerCap::CLONEABLE_OWNER_CAP.name.into(),
            type_params: vec![sui::MoveTypeTag::Struct(Box::new(sui::MoveStructTag {
                address: nexus_objects.workflow_pkg_id.into(),
                module: workflow::ToolRegistry::OVER_TOOL.module.into(),
                name: workflow::ToolRegistry::OVER_TOOL.name.into(),
                type_params: vec![],
            }))],
        };

        let response = sui::TransactionBlockResponse {
            digest: sui::TransactionDigest::random(),
            transaction: None,
            raw_transaction: vec![],
            effects: None,
            events: None,
            object_changes: Some(vec![sui::ObjectChange::Created {
                sender: sui::Address::random_for_testing_only(),
                owner: sui::Owner::AddressOwner(sui::Address::random_for_testing_only()),
                object_type,
                object_id: owner_cap_id,
                version: 1.into(),
                digest: sui::ObjectDigest::random(),
            }]),
            balance_changes: None,
            timestamp_ms: None,
            confirmed_local_execution: None,
            checkpoint: None,
            errors: vec![],
            raw_effects: vec![],
        };

        // Extract the owner cap.
        let result = extract_over_tool_owner_cap(&response, &nexus_objects);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), owner_cap_id);
    }

    #[test]
    fn test_extract_over_tool_owner_cap_not_found() {
        let nexus_objects = sui_mocks::mock_nexus_objects();

        // Create a mock transaction response with no owner cap.
        let response = sui::TransactionBlockResponse {
            digest: sui::TransactionDigest::random(),
            transaction: None,
            raw_transaction: vec![],
            effects: None,
            events: None,
            object_changes: Some(vec![]),
            balance_changes: None,
            timestamp_ms: None,
            confirmed_local_execution: None,
            checkpoint: None,
            errors: vec![],
            raw_effects: vec![],
        };

        // Should fail because no owner cap is found.
        let result = extract_over_tool_owner_cap(&response, &nexus_objects);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Could not find the OwnerCap<OverTool> object ID"));
    }

    #[test]
    fn test_extract_over_tool_owner_cap_empty_object_changes() {
        let nexus_objects = sui_mocks::mock_nexus_objects();

        // Create a response with None object_changes.
        let response = sui::TransactionBlockResponse {
            digest: sui::TransactionDigest::random(),
            transaction: None,
            raw_transaction: vec![],
            effects: None,
            events: None,
            object_changes: None,
            balance_changes: None,
            timestamp_ms: None,
            confirmed_local_execution: None,
            checkpoint: None,
            errors: vec![],
            raw_effects: vec![],
        };

        // Should fail because no owner cap is found.
        let result = extract_over_tool_owner_cap(&response, &nexus_objects);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_registration_transaction_success() {
        // Create mock objects.
        let nexus_objects = sui_mocks::mock_nexus_objects();
        let package_address = sui::ObjectID::random();
        let module_name = "test_module".to_string();
        let input_schema = r#"{"0":{"type":"u64","description":"Amount"}}"#.to_string();
        let output_schema =
            r#"{"ok":{"type":"variant","description":"Success","fields":{}}}"#.to_string();
        let fqn = "com.example.testtool@1".parse::<ToolFqn>().unwrap();
        let description = "Test tool description".to_string();
        let witness_id = sui::ObjectID::random();
        let collateral_coin = sui_mocks::mock_sui_coin(5000);
        let gas_coin = sui_mocks::mock_sui_coin(10000);
        let address = sui::Address::random_for_testing_only();
        let sui_gas_budget = 100000000;
        let reference_gas_price = 1000;

        // Build the transaction.
        let result = build_registration_transaction(
            &nexus_objects,
            package_address,
            module_name.clone(),
            input_schema.clone(),
            output_schema.clone(),
            &fqn,
            description.clone(),
            witness_id,
            &collateral_coin,
            &gas_coin,
            address,
            sui_gas_budget,
            reference_gas_price,
        );

        // Should succeed and return TransactionData.
        // We verify the function succeeds with valid inputs.
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_generate_and_customize_schemas_integration() {
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
            "../sdk/tests/move/onchain_tool_test",
            gas_coin,
        )
        .await;

        let changes = response
            .object_changes
            .expect("TX response must have object changes");

        let pkg_id = *changes
            .iter()
            .find_map(|c| match c {
                sui::ObjectChange::Published { package_id, .. } => Some(package_id),
                _ => None,
            })
            .expect("Move package must be published");

        // Enable JSON mode to skip interactive prompts.
        JSON_MODE.store(true, Ordering::Relaxed);

        // Generate and customize schemas.
        let result = generate_and_customize_schemas(&sui, pkg_id, "onchain_tool").await;

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
        // Create a test FQN and object ID.
        let fqn = "com.example.testtool@1".parse::<ToolFqn>().unwrap();
        let over_tool_id = sui::ObjectID::random();

        // Call save_tool_owner_caps.
        let result = save_tool_owner_caps(fqn.clone(), over_tool_id).await;

        // Should succeed.
        assert!(result.is_ok());
    }
}
