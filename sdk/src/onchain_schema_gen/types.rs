//! Type conversion utilities for Move types to JSON schema.

use {
    crate::{
        idents::{move_std, sui_framework},
        sui,
    },
    anyhow::{anyhow, bail, Result as AnyResult},
    serde_json::{json, Value},
};

/// Convert a Sui Move normalized type to a JSON schema representation.
///
/// This function introspects Move types and generates corresponding JSON schema
/// representations that can be used for tool registration and validation.
pub fn convert_move_type_to_schema(move_type: &sui::grpc::OpenSignatureBody) -> AnyResult<Value> {
    use sui::grpc::open_signature_body::Type;

    let Some(kind) = move_type.r#type else {
        bail!("Move type missing kind");
    };

    match kind.try_into() {
        Ok(Type::Address) => Ok(json!({
            "type": "address",
            "description": "Sui address"
        })),
        Ok(Type::Bool) => Ok(json!({
            "type": "bool",
            "description": "Boolean value"
        })),
        Ok(Type::U8) => Ok(json!({
            "type": "u8",
            "description": "8-bit unsigned integer"
        })),
        Ok(Type::U16) => Ok(json!({
            "type": "u16",
            "description": "16-bit unsigned integer"
        })),
        Ok(Type::U32) => Ok(json!({
            "type": "u32",
            "description": "32-bit unsigned integer"
        })),
        Ok(Type::U64) => Ok(json!({
            "type": "u64",
            "description": "64-bit unsigned integer"
        })),
        Ok(Type::U128) => Ok(json!({
            "type": "u128",
            "description": "128-bit unsigned integer"
        })),
        Ok(Type::U256) => Ok(json!({
            "type": "u256",
            "description": "256-bit unsigned integer"
        })),
        Ok(Type::Vector) => {
            let inner_type = move_type
                .type_parameter_instantiation
                .first()
                .ok_or_else(|| anyhow!("Vector type missing inner type"))?;

            let inner_schema = convert_move_type_to_schema(inner_type)?;

            Ok(json!({
                "type": "vector",
                "description": "Vector of values",
                "element_type": inner_schema
            }))
        }
        Ok(Type::Datatype) => {
            let struct_tag: sui::types::StructTag = move_type
                .type_name_opt()
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| anyhow!("Datatype type missing or invalid StructTag"))?;

            if *struct_tag.address() == move_std::PACKAGE_ID {
                match (struct_tag.module().as_str(), struct_tag.name().as_str()) {
                    ("string", "String") => Ok(json!({
                        "type": "string",
                        "description": "0x1::string::String"
                    })),
                    ("ascii", "String") => Ok(json!({
                        "type": "string",
                        "description": "0x1::ascii::String"
                    })),
                    _ => Ok(json!({
                        "type": "object",
                        "description": format!("0x1::{}::{}", struct_tag.module(), struct_tag.name())
                    })),
                }
            } else if *struct_tag.address() == sui_framework::PACKAGE_ID {
                match (struct_tag.module().as_str(), struct_tag.name().as_str()) {
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
                        "description": format!("0x2::{}::{}", struct_tag.module(), struct_tag.name())
                    })),
                }
            } else {
                Ok(json!({
                    "type": "object",
                    "description": format!("{}::{}::{}", struct_tag.address(), struct_tag.module(), struct_tag.name())
                }))
            }
        }
        Ok(Type::Parameter) => Ok(json!({
            "type": "generic",
            "description": "Generic type parameter"
        })),
        _ => {
            bail!("Unsupported Move type for schema conversion: {kind:?}");
        }
    }
}

/// Check if a parameter is TxContext (should be excluded from the inputschema).
pub fn is_tx_context_param(move_type: &sui::grpc::OpenSignatureBody) -> bool {
    let Some(type_name) = move_type.type_name_opt() else {
        return false;
    };

    let maybe_struct_tag: Option<sui::types::StructTag> = type_name.parse().ok();

    if let Some(struct_tag) = maybe_struct_tag {
        *struct_tag.address() == sui_framework::PACKAGE_ID
            && struct_tag.module().as_str() == "tx_context"
            && struct_tag.name().as_str() == "TxContext"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use {super::*, sui::grpc::open_signature_body::Type};

    // Helper function to create a struct type.
    fn make_struct(address: &str, module: &str, name: &str) -> sui::grpc::OpenSignatureBody {
        sui::grpc::OpenSignatureBody::default()
            .with_type(Type::Datatype)
            .with_type_name(format!("{}::{}::{}", address, module, name))
    }

    fn make_prim(kind: Type) -> sui::grpc::OpenSignatureBody {
        sui::grpc::OpenSignatureBody::default().with_type(kind)
    }

    fn make_vec(inner: sui::grpc::OpenSignatureBody) -> sui::grpc::OpenSignatureBody {
        sui::grpc::OpenSignatureBody::default()
            .with_type(Type::Vector)
            .with_type_parameter_instantiation(vec![inner])
    }

    #[test]
    fn test_primitive_types() {
        // Bool.
        let schema = convert_move_type_to_schema(&make_prim(Type::Bool)).unwrap();
        assert_eq!(schema["type"], "bool");

        // U8.
        let schema = convert_move_type_to_schema(&make_prim(Type::U8)).unwrap();
        assert_eq!(schema["type"], "u8");

        // U16.
        let schema = convert_move_type_to_schema(&make_prim(Type::U16)).unwrap();
        assert_eq!(schema["type"], "u16");

        // U32.
        let schema = convert_move_type_to_schema(&make_prim(Type::U32)).unwrap();
        assert_eq!(schema["type"], "u32");

        // U64.
        let schema = convert_move_type_to_schema(&make_prim(Type::U64)).unwrap();
        assert_eq!(schema["type"], "u64");

        // U128.
        let schema = convert_move_type_to_schema(&make_prim(Type::U128)).unwrap();
        assert_eq!(schema["type"], "u128");

        // U256.
        let schema = convert_move_type_to_schema(&make_prim(Type::U256)).unwrap();
        assert_eq!(schema["type"], "u256");

        // Address.
        let schema = convert_move_type_to_schema(&make_prim(Type::Address)).unwrap();
        assert_eq!(schema["type"], "address");
    }

    #[test]
    fn test_vector_types() {
        // Vector<u8>.
        let vec_u8 = make_vec(make_prim(Type::U8));
        let schema = convert_move_type_to_schema(&vec_u8).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "u8");

        // Vector<bool>.
        let vec_bool = make_vec(make_prim(Type::Bool));
        let schema = convert_move_type_to_schema(&vec_bool).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "bool");

        // Nested vector Vector<Vector<u64>>.
        let vec_vec_u64 = make_vec(make_vec(make_prim(Type::U64)));
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
        assert_eq!(schema["description"], "0x1::string::String");

        // 0x1::ascii::String.
        let ascii_string = make_struct("0x1", "ascii", "String");
        let schema = convert_move_type_to_schema(&ascii_string).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(schema["description"], "0x1::ascii::String");
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
            format!(
                "{}::my_module::MyStruct",
                sui::types::Address::from_static("0x123456789abcdef")
            )
        );
    }

    #[test]
    fn test_type_parameter() {
        // Generic type parameter.
        let generic = make_prim(Type::Parameter);
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
    fn test_is_tx_context_param_primitive() {
        // Primitives are not TxContext.
        assert!(!is_tx_context_param(&make_prim(Type::U64)));
        assert!(!is_tx_context_param(&make_prim(Type::Bool)));
        assert!(!is_tx_context_param(&make_prim(Type::Address)));
    }

    #[test]
    fn test_complex_nested_types() {
        // Vector  to objects.
        let vector = make_vec(make_struct("0x2", "coin", "Coin"));
        let schema = convert_move_type_to_schema(&vector).unwrap();
        assert_eq!(schema["type"], "vector");
        assert_eq!(schema["element_type"]["type"], "object");
    }

    #[test]
    fn test_schema_structure_completeness() {
        // Verify that all schemas have both type and description fields.
        let test_types = vec![
            make_prim(Type::Bool),
            make_prim(Type::U8),
            make_prim(Type::U256),
            make_prim(Type::Address),
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
