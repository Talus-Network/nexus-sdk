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
            if address == &crate::sui::FRAMEWORK_PACKAGE_ID.to_string() {
                match (module.as_str(), name.as_str()) {
                    ("object", "ID") => Ok(json!({
                        "type": "object_id",
                        "description": "Sui object ID"
                    })),
                    ("object", "UID") => Ok(json!({
                        "type": "object_id",
                        "description": "Unique identifier for an object"
                    })),
                    ("coin", "Coin") => Ok(json!({
                        "type": "object",
                        "description": "Coin object reference"
                    })),
                    ("tx_context", "TxContext") => Ok(json!({
                        "type": "tx_context",
                        "description": "Transaction context (automatically provided)"
                    })),
                    _ => Ok(json!({
                        "type": "object",
                        "description": format!("{}::{}", module, name)
                    })),
                }
            } else if address == "0x1" {
                // Handle standard library types.
                match (module.as_str(), name.as_str()) {
                    ("string", "String") => Ok(json!({
                        "type": "string",
                        "description": "UTF-8 string"
                    })),
                    ("ascii", "String") => Ok(json!({
                        "type": "string",
                        "description": "ASCII string"
                    })),
                    _ => Ok(json!({
                        "type": "object",
                        "description": format!("{}::{}::{}", address, module, name)
                    })),
                }
            } else {
                // Custom struct types are treated as object references.
                Ok(json!({
                    "type": "object",
                    "description": format!("{}::{}::{}", address, module, name)
                }))
            }
        }
        MoveNormalizedType::Reference(inner_type) => {
            let inner_schema = convert_move_type_to_schema(inner_type)?;
            if let Value::Object(mut schema_obj) = inner_schema {
                schema_obj.insert("mutable".to_string(), Value::Bool(false));
                Ok(Value::Object(schema_obj))
            } else {
                Ok(json!({
                    "type": "reference",
                    "description": "Reference to an object",
                    "referenced_type": inner_schema
                }))
            }
        }
        MoveNormalizedType::MutableReference(inner_type) => {
            let inner_schema = convert_move_type_to_schema(inner_type)?;
            if let Value::Object(mut schema_obj) = inner_schema {
                schema_obj.insert("mutable".to_string(), Value::Bool(true));
                Ok(Value::Object(schema_obj))
            } else {
                Ok(json!({
                    "type": "mutable_reference",
                    "description": "Mutable reference to an object",
                    "referenced_type": inner_schema
                }))
            }
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
