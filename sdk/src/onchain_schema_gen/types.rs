//! Type conversion utilities for Move types to JSON schema.

use anyhow::Result as AnyResult;
use serde_json::{json, Value};

/// Convert a Sui Move normalized type to a JSON schema representation.
///
/// This function introspects Move types and generates corresponding JSON schema
/// representations that can be used for tool registration and validation.
pub fn convert_move_type_to_schema(move_type: &crate::sui::MoveNormalizedType) -> AnyResult<Value> {
    use crate::sui::MoveNormalizedType;

    match move_type {
        MoveNormalizedType::Bool => Ok(json!({
            "type": "bool",
            "description": "Boolean value"
        })),
        MoveNormalizedType::U8 => Ok(json!({
            "type": "u8",
            "description": "8-bit unsigned integer"
        })),
        MoveNormalizedType::U16 => Ok(json!({
            "type": "u16",
            "description": "16-bit unsigned integer"
        })),
        MoveNormalizedType::U32 => Ok(json!({
            "type": "u32",
            "description": "32-bit unsigned integer"
        })),
        MoveNormalizedType::U64 => Ok(json!({
            "type": "u64",
            "description": "64-bit unsigned integer"
        })),
        MoveNormalizedType::U128 => Ok(json!({
            "type": "u128",
            "description": "128-bit unsigned integer"
        })),
        MoveNormalizedType::U256 => Ok(json!({
            "type": "u256",
            "description": "256-bit unsigned integer"
        })),
        MoveNormalizedType::Address => Ok(json!({
            "type": "address",
            "description": "Sui address"
        })),
        MoveNormalizedType::Signer => Ok(json!({
            "type": "signer",
            "description": "Transaction signer"
        })),
        MoveNormalizedType::Vector(inner_type) => {
            let inner_schema = convert_move_type_to_schema(inner_type)?;
            Ok(json!({
                "type": "vector",
                "description": "Vector of values",
                "element_type": inner_schema
            }))
        }
        MoveNormalizedType::Struct {
            address,
            module,
            name,
            type_arguments: _,
        } => {
            if address == "0x1" {
                match (module.as_str(), name.as_str()) {
                    ("string", "String") => Ok(json!({
                        "type": "string",
                        "description": "UTF-8 string"
                    })),
                    ("ascii", "String") => Ok(json!({
                        "type": "string",
                        "description": "ASCII string"
                    })),
                    // All other standard library types are object references.
                    _ => Ok(json!({
                        "type": "object",
                        "description": format!("0x1::{}::{}", module, name)
                    })),
                }
            } else if address == "0x2" || address == &crate::sui::FRAMEWORK_PACKAGE_ID.to_string() {
                match (module.as_str(), name.as_str()) {
                    ("object", "ID") => Ok(json!({
                        "type": "object_id",
                        "description": "Sui object ID"
                    })),
                    ("tx_context", "TxContext") => Ok(json!({
                        "type": "tx_context",
                        "description": "Transaction context (automatically provided)"
                    })),
                    _ => Ok(json!({
                        "type": "object",
                        "description": format!("0x2::{}::{}", module, name)
                    })),
                }
            } else {
                Ok(json!({
                    "type": "object",
                    "description": format!("{}::{}::{}", address, module, name)
                }))
            }
        }
        MoveNormalizedType::Reference(inner_type) => {
            // References don't change the fundamental type for transaction building.
            // &u64 is still u64, &MyStruct is still MyStruct.
            let mut inner_schema = convert_move_type_to_schema(inner_type)?;
            if let Value::Object(ref mut schema_obj) = inner_schema {
                schema_obj.insert("mutable".to_string(), Value::Bool(false));
            }
            Ok(inner_schema)
        }
        MoveNormalizedType::MutableReference(inner_type) => {
            let mut inner_schema = convert_move_type_to_schema(inner_type)?;
            if let Value::Object(ref mut schema_obj) = inner_schema {
                schema_obj.insert("mutable".to_string(), Value::Bool(true));
            }
            Ok(inner_schema)
        }
        MoveNormalizedType::TypeParameter(_) => Ok(json!({
            "type": "generic",
            "description": "Generic type parameter"
        })),
    }
}

/// Check if a parameter is TxContext (should be excluded from the inputschema).
pub fn is_tx_context_param(move_type: &crate::sui::MoveNormalizedType) -> bool {
    use crate::sui::MoveNormalizedType;

    let struct_type = match move_type {
        MoveNormalizedType::Struct { .. } => Some(move_type),
        MoveNormalizedType::MutableReference(inner) | MoveNormalizedType::Reference(inner) => {
            if let MoveNormalizedType::Struct { .. } = inner.as_ref() {
                Some(inner.as_ref())
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(MoveNormalizedType::Struct {
        address,
        module,
        name,
        ..
    }) = struct_type
    {
        (address == "0x2" || address == &crate::sui::FRAMEWORK_PACKAGE_ID.to_string())
            && module == "tx_context"
            && name == "TxContext"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sui::MoveNormalizedType;

    // Helper function to create a struct type.
    fn make_struct(address: &str, module: &str, name: &str) -> MoveNormalizedType {
        MoveNormalizedType::Struct {
            address: address.to_string(),
            module: module.to_string(),
            name: name.to_string(),
            type_arguments: vec![],
        }
    }

    #[test]
    fn test_primitive_types() {
        // Bool.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::Bool).unwrap();
        assert_eq!(schema["type"], "bool");

        // U8.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::U8).unwrap();
        assert_eq!(schema["type"], "u8");

        // U16.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::U16).unwrap();
        assert_eq!(schema["type"], "u16");

        // U32.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::U32).unwrap();
        assert_eq!(schema["type"], "u32");

        // U64.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::U64).unwrap();
        assert_eq!(schema["type"], "u64");

        // U128.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::U128).unwrap();
        assert_eq!(schema["type"], "u128");

        // U256.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::U256).unwrap();
        assert_eq!(schema["type"], "u256");

        // Address.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::Address).unwrap();
        assert_eq!(schema["type"], "address");

        // Signer.
        let schema = convert_move_type_to_schema(&MoveNormalizedType::Signer).unwrap();
        assert_eq!(schema["type"], "signer");
    }

    #[test]
    fn test_vector_types() {
        // Vector<u8>.
        let vec_u8 = MoveNormalizedType::Vector(Box::new(MoveNormalizedType::U8));
        let schema = convert_move_type_to_schema(&vec_u8).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "u8");

        // Vector<bool>.
        let vec_bool = MoveNormalizedType::Vector(Box::new(MoveNormalizedType::Bool));
        let schema = convert_move_type_to_schema(&vec_bool).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "bool");

        // Nested vector Vector<Vector<u64>>.
        let vec_vec_u64 = MoveNormalizedType::Vector(Box::new(MoveNormalizedType::Vector(
            Box::new(MoveNormalizedType::U64),
        )));
        let schema = convert_move_type_to_schema(&vec_vec_u64).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "vector");
        assert_eq!(schema["element_type"]["element_type"]["type"], "u64");
    }

    #[test]
    fn test_standard_library_string_types() {
        // 0x1::string::String.
        let utf8_string = make_struct("0x1", "string", "String");
        let schema = convert_move_type_to_schema(&utf8_string).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "UTF-8 string");

        // 0x1::ascii::String.
        let ascii_string = make_struct("0x1", "ascii", "String");
        let schema = convert_move_type_to_schema(&ascii_string).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "ASCII string");
    }

    #[test]
    fn test_standard_library_other_types() {
        // 0x1::option::Option.
        let option_type = make_struct("0x1", "option", "Option");
        let schema = convert_move_type_to_schema(&option_type).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "0x1::option::Option");
    }

    #[test]
    fn test_sui_framework_object_id() {
        // 0x2::object::ID.
        let object_id = make_struct("0x2", "object", "ID");
        let schema = convert_move_type_to_schema(&object_id).unwrap();
        assert_eq!(schema["type"], "object_id");
        assert_eq!(schema["description"], "Sui object ID");
    }

    #[test]
    fn test_sui_framework_tx_context() {
        // 0x2::tx_context::TxContext.
        let tx_context = make_struct("0x2", "tx_context", "TxContext");
        let schema = convert_move_type_to_schema(&tx_context).unwrap();
        assert_eq!(schema["type"], "tx_context");
        assert_eq!(
            schema["description"],
            "Transaction context (automatically provided)"
        );
    }

    #[test]
    fn test_sui_framework_coin() {
        // 0x2::coin::Coin .
        let coin = make_struct("0x2", "coin", "Coin");
        let schema = convert_move_type_to_schema(&coin).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "0x2::coin::Coin");
    }

    #[test]
    fn test_sui_framework_uid() {
        // 0x2::object::UID.
        let uid = make_struct("0x2", "object", "UID");
        let schema = convert_move_type_to_schema(&uid).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "0x2::object::UID");
    }

    #[test]
    fn test_sui_framework_other_types() {
        // 0x2::balance::Balance.
        let balance = make_struct("0x2", "balance", "Balance");
        let schema = convert_move_type_to_schema(&balance).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "0x2::balance::Balance");

        // 0x2::transfer::Transfer.
        let transfer = make_struct("0x2", "transfer", "Transfer");
        let schema = convert_move_type_to_schema(&transfer).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["description"], "0x2::transfer::Transfer");
    }

    #[test]
    fn test_custom_package_types() {
        // Custom package type.
        let custom = make_struct("0x123456789abcdef", "my_module", "MyStruct");
        let schema = convert_move_type_to_schema(&custom).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(
            schema["description"],
            "0x123456789abcdef::my_module::MyStruct"
        );
    }

    #[test]
    fn test_reference_types() {
        // Immutable reference to u64.
        let ref_u64 = MoveNormalizedType::Reference(Box::new(MoveNormalizedType::U64));
        let schema = convert_move_type_to_schema(&ref_u64).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["mutable"], false);

        // Immutable reference to object.
        let object = make_struct("0x2", "coin", "Coin");
        let ref_object = MoveNormalizedType::Reference(Box::new(object));
        let schema = convert_move_type_to_schema(&ref_object).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["mutable"], false);
    }

    #[test]
    fn test_mutable_reference_types() {
        // Mutable reference to u64.
        let mut_ref_u64 = MoveNormalizedType::MutableReference(Box::new(MoveNormalizedType::U64));
        let schema = convert_move_type_to_schema(&mut_ref_u64).unwrap();
        assert_eq!(schema["type"], "u64");
        assert_eq!(schema["mutable"], true);

        // Mutable reference to object.
        let object = make_struct("0x2", "coin", "Coin");
        let mut_ref_object = MoveNormalizedType::MutableReference(Box::new(object));
        let schema = convert_move_type_to_schema(&mut_ref_object).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["mutable"], true);
    }

    #[test]
    fn test_type_parameter() {
        // Generic type parameter.
        let generic = MoveNormalizedType::TypeParameter(0);
        let schema = convert_move_type_to_schema(&generic).unwrap();
        assert_eq!(schema["type"], "generic");
    }

    #[test]
    fn test_is_tx_context_param_struct() {
        // Direct TxContext struct.
        let tx_context = make_struct("0x2", "tx_context", "TxContext");
        assert!(is_tx_context_param(&tx_context));

        // Non-TxContext struct.
        let coin = make_struct("0x2", "coin", "Coin");
        assert!(!is_tx_context_param(&coin));
    }

    #[test]
    fn test_is_tx_context_param_reference() {
        // Immutable reference to TxContext.
        let tx_context = make_struct("0x2", "tx_context", "TxContext");
        let ref_tx_context = MoveNormalizedType::Reference(Box::new(tx_context));
        assert!(is_tx_context_param(&ref_tx_context));

        // Mutable reference to TxContext.
        let tx_context = make_struct("0x2", "tx_context", "TxContext");
        let mut_ref_tx_context = MoveNormalizedType::MutableReference(Box::new(tx_context));
        assert!(is_tx_context_param(&mut_ref_tx_context));
    }

    #[test]
    fn test_is_tx_context_param_primitive() {
        // Primitives are not TxContext.
        assert!(!is_tx_context_param(&MoveNormalizedType::U64));
        assert!(!is_tx_context_param(&MoveNormalizedType::Bool));
        assert!(!is_tx_context_param(&MoveNormalizedType::Address));
    }

    #[test]
    fn test_complex_nested_types() {
        // Vector of references to objects.
        let object = make_struct("0x2", "coin", "Coin");
        let ref_object = MoveNormalizedType::Reference(Box::new(object));
        let vec_ref_object = MoveNormalizedType::Vector(Box::new(ref_object));
        let schema = convert_move_type_to_schema(&vec_ref_object).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "object");
        assert_eq!(schema["element_type"]["mutable"], false);
    }

    #[test]
    fn test_schema_structure_completeness() {
        // Verify that all schemas have both type and description fields.
        let test_types = vec![
            MoveNormalizedType::Bool,
            MoveNormalizedType::U8,
            MoveNormalizedType::U256,
            MoveNormalizedType::Address,
            make_struct("0x1", "string", "String"),
            make_struct("0x2", "object", "ID"),
            make_struct("0x2", "coin", "Coin"),
        ];

        for move_type in test_types {
            let schema = convert_move_type_to_schema(&move_type).unwrap();
            assert!(schema.get("type").is_some(), "Schema missing 'type' field");
            assert!(
                schema.get("description").is_some(),
                "Schema missing 'description' field"
            );
        }
    }
}
