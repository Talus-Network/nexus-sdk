use {
    crate::{
        sui,
        types::{
            ExecutionTerminalRecord,
            PublishedMoveEnum,
            RuntimeVertex,
            TypeName,
            WorkflowFailureClass,
        },
    },
    anyhow::anyhow,
    serde::de::DeserializeOwned,
};

/// Unwrap Sui's Move-JSON `{ fields: { ... } }` wrapper when present.
pub fn strip_fields_owned(value: serde_json::Value) -> serde_json::Value {
    let serde_json::Value::Object(mut object) = value else {
        return value;
    };

    match object.remove("fields") {
        Some(fields) if fields.is_object() => fields,
        Some(fields) => {
            object.insert("fields".to_string(), fields);
            serde_json::Value::Object(object)
        }
        None => serde_json::Value::Object(object),
    }
}

/// Parse a bool-like Move-JSON value.
pub fn parse_bool_value(value: &serde_json::Value) -> anyhow::Result<Option<bool>> {
    match value {
        serde_json::Value::Bool(flag) => Ok(Some(*flag)),
        serde_json::Value::String(flag) => Ok(Some(flag.parse()?)),
        serde_json::Value::Object(object) => {
            let unwrapped = strip_fields_owned(value.clone());
            if &unwrapped != value {
                return parse_bool_value(&unwrapped);
            }

            for key in ["value", "bool", "boolean"] {
                if let Some(nested) = object.get(key) {
                    return parse_bool_value(nested);
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Parse a string-like Move-JSON value.
pub fn parse_string_value(value: &serde_json::Value) -> anyhow::Result<Option<String>> {
    match value {
        serde_json::Value::String(text) => Ok(Some(text.clone())),
        serde_json::Value::Array(bytes) => {
            let mut out = Vec::with_capacity(bytes.len());

            for byte in bytes {
                let Some(byte) = byte.as_u64() else {
                    return Ok(None);
                };
                out.push(u8::try_from(byte)?);
            }

            Ok(Some(String::from_utf8(out)?))
        }
        serde_json::Value::Object(object) => {
            let unwrapped = strip_fields_owned(value.clone());
            if &unwrapped != value {
                return parse_string_value(&unwrapped);
            }

            for key in ["bytes", "value", "string", "ascii", "inner"] {
                if let Some(nested) = object.get(key) {
                    if let Some(parsed) = parse_string_value(nested)? {
                        return Ok(Some(parsed));
                    }
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Parse a u64-like Move-JSON value.
pub fn parse_u64_value(value: &serde_json::Value) -> anyhow::Result<Option<u64>> {
    match value {
        serde_json::Value::Number(number) => Ok(number.as_u64()),
        serde_json::Value::String(number) => Ok(Some(number.parse()?)),
        serde_json::Value::Object(object) => {
            let unwrapped = strip_fields_owned(value.clone());
            if &unwrapped != value {
                return parse_u64_value(&unwrapped);
            }

            for key in ["value", "inner", "u64", "number"] {
                if let Some(nested) = object.get(key) {
                    return parse_u64_value(nested);
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Parse a byte-vector-like Move-JSON value.
pub fn parse_byte_vector_value(value: &serde_json::Value) -> anyhow::Result<Option<Vec<u8>>> {
    match value {
        serde_json::Value::Array(bytes) => {
            let mut out = Vec::with_capacity(bytes.len());
            for byte in bytes {
                let Some(byte) = byte.as_u64() else {
                    return Ok(None);
                };
                out.push(u8::try_from(byte)?);
            }
            Ok(Some(out))
        }
        serde_json::Value::String(text) => {
            let text = text.strip_prefix("0x").unwrap_or(text);
            match hex::decode(text) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(_) => {
                    use base64::Engine as _;

                    match base64::engine::general_purpose::STANDARD.decode(text) {
                        Ok(bytes) => Ok(Some(bytes)),
                        Err(_) => Ok(None),
                    }
                }
            }
        }
        serde_json::Value::Object(object) => {
            let unwrapped = strip_fields_owned(value.clone());
            if &unwrapped != value {
                return parse_byte_vector_value(&unwrapped);
            }

            for key in ["bytes", "value", "vector", "inner"] {
                if let Some(nested) = object.get(key) {
                    if let Some(parsed) = parse_byte_vector_value(nested)? {
                        return Ok(Some(parsed));
                    }
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Parse a string option-like Move-JSON value.
pub fn parse_optional_string_value(
    value: &serde_json::Value,
) -> anyhow::Result<Option<Option<String>>> {
    if value.is_null() {
        return Ok(Some(None));
    }

    if let Some(parsed) = parse_string_value(value)? {
        return Ok(Some(Some(parsed)));
    }

    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    let Some(name) = parse_variant_name(value) else {
        return Ok(None);
    };

    match name.as_str() {
        "None" | "none" => Ok(Some(None)),
        "Some" | "some" => {
            for key in ["value", "inner", "some", "Some"] {
                if let Some(nested) = object.get(key) {
                    if let Some(parsed) = parse_string_value(nested)? {
                        return Ok(Some(Some(parsed)));
                    }
                }
            }

            for (key, nested) in object {
                if matches!(key.as_str(), "_variant_name" | "@variant") {
                    continue;
                }
                if let Some(parsed) = parse_string_value(nested)? {
                    return Ok(Some(Some(parsed)));
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Parse a `TypeName` from Move-JSON wrapper shapes.
pub fn parse_type_name_value(value: &serde_json::Value) -> anyhow::Result<Option<TypeName>> {
    match value {
        serde_json::Value::String(name) => Ok(Some(TypeName::new(name))),
        serde_json::Value::Object(object) => {
            let unwrapped = strip_fields_owned(value.clone());
            if &unwrapped != value {
                return parse_type_name_value(&unwrapped);
            }

            for key in ["name", "value", "string", "ascii", "inner"] {
                if let Some(nested) = object.get(key) {
                    if let Some(parsed) = parse_string_value(nested)? {
                        return Ok(Some(TypeName::new(&parsed)));
                    }
                }
            }

            Ok(None)
        }
        _ => Ok(None),
    }
}

/// Parse a `RuntimeVertex` from Move-JSON wrapper shapes.
pub fn parse_runtime_vertex_value(
    value: &serde_json::Value,
) -> anyhow::Result<Option<RuntimeVertex>> {
    if let Ok(parsed) = serde_json::from_value::<RuntimeVertex>(value.clone()) {
        return Ok(Some(parsed));
    }

    let unwrapped = strip_fields_owned(value.clone());
    if unwrapped != *value {
        if let Some(parsed) = parse_runtime_vertex_value(&unwrapped)? {
            return Ok(Some(parsed));
        }
    }

    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    let Some(name) = parse_variant_name(value) else {
        return Ok(None);
    };

    match name.as_str() {
        "Plain" | "plain" => {
            let Some(vertex) = object.get("vertex") else {
                return Ok(None);
            };

            let vertex = parse_type_name_value(vertex)?
                .ok_or_else(|| anyhow!("Could not parse plain runtime vertex name: {vertex}"))?;
            Ok(Some(RuntimeVertex::Plain {
                vertex: vertex.into(),
            }))
        }
        "WithIterator" | "with_iterator" => {
            let Some(vertex) = object.get("vertex") else {
                return Ok(None);
            };
            let Some(iteration) = object.get("iteration") else {
                return Ok(None);
            };
            let Some(out_of) = object.get("out_of") else {
                return Ok(None);
            };

            let vertex = parse_type_name_value(vertex)?
                .ok_or_else(|| anyhow!("Could not parse iterated runtime vertex name: {vertex}"))?;
            let iteration = parse_u64_value(iteration)?
                .ok_or_else(|| anyhow!("Could not parse runtime vertex iteration: {iteration}"))?;
            let out_of = parse_u64_value(out_of)?
                .ok_or_else(|| anyhow!("Could not parse runtime vertex out_of: {out_of}"))?;

            Ok(Some(RuntimeVertex::WithIterator {
                vertex: vertex.into(),
                iteration,
                out_of,
            }))
        }
        _ => Ok(None),
    }
}

/// Parse an execution terminal record from Move-JSON wrapper shapes.
pub fn parse_execution_terminal_record_value(
    value: &serde_json::Value,
) -> anyhow::Result<Option<ExecutionTerminalRecord>> {
    if let Ok(parsed) = serde_json::from_value::<ExecutionTerminalRecord>(value.clone()) {
        return Ok(Some(parsed));
    }

    let unwrapped = strip_fields_owned(value.clone());
    if unwrapped != *value {
        if let Some(parsed) = parse_execution_terminal_record_value(&unwrapped)? {
            return Ok(Some(parsed));
        }
    }

    let serde_json::Value::Object(object) = value else {
        return Ok(None);
    };

    if let (Some(vertex), Some(failure_class)) = (object.get("vertex"), object.get("failure_class"))
    {
        let vertex = parse_runtime_vertex_value(vertex)?
            .ok_or_else(|| anyhow!("Could not parse execution terminal record vertex: {vertex}"))?;
        let failure_class = parse_published_move_enum_value::<WorkflowFailureClass>(failure_class)?
            .ok_or_else(|| {
                anyhow!("Could not parse execution terminal record failure class: {failure_class}")
            })?;

        return Ok(Some(ExecutionTerminalRecord {
            vertex,
            failure_class,
        }));
    }

    for key in ["record", "fields", "inner", "value"] {
        if let Some(nested) = object.get(key) {
            if let Some(parsed) = parse_execution_terminal_record_value(nested)? {
                return Ok(Some(parsed));
            }
        }
    }

    Ok(None)
}

/// Parse a Sui address-like Move-JSON value.
pub fn parse_address_value(
    value: &serde_json::Value,
) -> anyhow::Result<Option<sui::types::Address>> {
    let Some(text) = parse_string_value(value)? else {
        return Ok(None);
    };

    Ok(Some(serde_json::from_value(serde_json::Value::String(
        text,
    ))?))
}

/// Parse a published Move enum value from wrapper forms.
pub fn parse_published_move_enum_value<T>(value: &serde_json::Value) -> anyhow::Result<Option<T>>
where
    T: DeserializeOwned,
{
    if value.is_null() {
        return Ok(None);
    }

    let parsed = serde_json::from_value::<PublishedMoveEnum<T>>(value.clone())?.0;
    Ok(Some(parsed))
}

/// Normalize JSON-quoted strings back into their plain string payload.
pub fn normalize_json_string(mut value: String) -> String {
    while let Ok(decoded) = serde_json::from_str::<String>(&value) {
        value = decoded;
    }

    value
}

fn parse_variant_name(value: &serde_json::Value) -> Option<String> {
    match strip_fields_owned(value.clone()) {
        serde_json::Value::String(name) => Some(name),
        serde_json::Value::Object(object) => object
            .get("_variant_name")
            .or_else(|| object.get("@variant"))
            .or_else(|| object.get("variant"))
            .and_then(|value| value.as_str().map(ToOwned::to_owned))
            .or_else(|| {
                if object.len() == 1 {
                    object.keys().next().cloned()
                } else {
                    None
                }
            }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::types::{MoveFields, MoveOption},
        serde::{Deserialize, Serialize},
        serde_json::json,
    };

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
    struct Foo {
        a: u64,
    }

    #[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
    enum DemoEnum {
        #[serde(rename = "retryable", alias = "Retryable")]
        Retryable,
    }

    #[test]
    fn helpers_parse_wrapped_runtime_vertex_and_scalars() {
        let vertex = parse_runtime_vertex_value(&json!({
            "fields": {
                "_variant_name": "WithIterator",
                "vertex": { "ascii": [118, 49] },
                "iteration": { "value": "2" },
                "out_of": { "u64": 5 }
            }
        }))
        .expect("runtime vertex should parse");
        assert_eq!(vertex, Some(RuntimeVertex::with_iterator("v1", 2, 5)));

        assert_eq!(
            parse_bool_value(&json!({"fields": {"value": "true"}})).unwrap(),
            Some(true)
        );
        assert_eq!(
            parse_u64_value(&json!({"fields": {"number": "9"}})).unwrap(),
            Some(9)
        );
        assert_eq!(
            parse_u64_value(&json!({"fields": {"inner": "10"}})).unwrap(),
            Some(10)
        );
        assert_eq!(
            parse_string_value(&json!({"fields": {"ascii": [98, 111, 111, 109]}})).unwrap(),
            Some("boom".to_string())
        );
    }

    #[test]
    fn helpers_parse_optional_string_bytes_and_enums() {
        assert_eq!(
            parse_optional_string_value(
                &json!({"fields": {"_variant_name": "Some", "value": "leader-1"}})
            )
            .unwrap(),
            Some(Some("leader-1".to_string()))
        );
        assert_eq!(
            parse_optional_string_value(&json!({"fields": {"None": {}}})).unwrap(),
            Some(None)
        );

        assert_eq!(
            parse_byte_vector_value(&json!({"fields": {"bytes": "0x0102ff"}})).unwrap(),
            Some(vec![1, 2, 255])
        );

        let parsed: Option<DemoEnum> =
            parse_published_move_enum_value(&json!({"fields": {"@variant": "Retryable"}})).unwrap();
        assert_eq!(parsed, Some(DemoEnum::Retryable));
    }

    #[test]
    fn helpers_parse_execution_terminal_record_from_wrapped_move_json() {
        let parsed = parse_execution_terminal_record_value(&json!({
            "fields": {
                "record": {
                    "fields": {
                        "vertex": {
                            "fields": {
                                "_variant_name": "WithIterator",
                                "vertex": { "ascii": [118, 49] },
                                "iteration": { "value": "2" },
                                "out_of": { "u64": 5 }
                            }
                        },
                        "failure_class": {
                            "fields": {
                                "@variant": "Retryable"
                            }
                        }
                    }
                }
            }
        }))
        .expect("execution terminal record should parse");

        assert_eq!(
            parsed,
            Some(ExecutionTerminalRecord {
                vertex: RuntimeVertex::with_iterator("v1", 2, 5),
                failure_class: WorkflowFailureClass::Retryable,
            })
        );
    }

    #[test]
    fn helpers_parse_execution_terminal_record_from_plain_json() {
        let parsed = parse_execution_terminal_record_value(&json!({
            "vertex": {
                "Plain": {
                    "vertex": {
                        "name": "terminal_vertex"
                    }
                }
            },
            "failure_class": "TerminalSubmissionFailure"
        }))
        .expect("plain execution terminal record should parse");

        assert_eq!(
            parsed,
            Some(ExecutionTerminalRecord {
                vertex: RuntimeVertex::plain("terminal_vertex"),
                failure_class: WorkflowFailureClass::TerminalSubmissionFailure,
            })
        );
    }

    #[test]
    fn helpers_parse_address_and_normalize_json_string() {
        let parsed = parse_address_value(&json!("0x2")).expect("address should parse");
        assert_eq!(parsed, Some(sui::types::Address::TWO));

        assert_eq!(normalize_json_string(r#""boom""#.to_string()), "boom");
        assert_eq!(normalize_json_string(r#""\"boom\"""#.to_string()), "boom");
    }

    #[test]
    fn strip_fields_owned_behaviour() {
        let value = json!("hello");
        assert_eq!(strip_fields_owned(value.clone()), value);

        let value = json!({"a": 1});
        assert_eq!(strip_fields_owned(value.clone()), value);

        let value = json!({"fields": {"a": 1}});
        assert_eq!(strip_fields_owned(value), json!({"a": 1}));

        let value = json!({"fields": 1, "x": 2});
        assert_eq!(strip_fields_owned(value.clone()), value);
    }

    #[test]
    fn move_fields_deserializes_json_with_or_without_wrapper() {
        let wrapped: MoveFields<Foo> =
            serde_json::from_value(json!({"fields": {"a": 1}})).expect("should unwrap fields");
        assert_eq!(wrapped.0, Foo { a: 1 });

        let plain: MoveFields<Foo> =
            serde_json::from_value(json!({"a": 2})).expect("should deserialize plain struct");
        assert_eq!(plain.0, Foo { a: 2 });
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn move_fields_deserializes_bcs_without_wrapper() {
        let bytes = bcs::to_bytes(&Foo { a: 3 }).expect("BCS serialization should succeed");
        let decoded: MoveFields<Foo> =
            bcs::from_bytes(&bytes).expect("BCS deserialization should succeed");
        assert_eq!(decoded.0, Foo { a: 3 });
    }

    #[test]
    fn move_option_deserializes_common_json_forms() {
        let none: MoveOption<u64> = serde_json::from_value(json!(null)).expect("null is accepted");
        assert_eq!(none.0, None);

        let array_some: MoveOption<u64> =
            serde_json::from_value(json!([5])).expect("array form is accepted");
        assert_eq!(array_some.0, Some(5));

        let vec_some: MoveOption<u64> =
            serde_json::from_value(json!({"vec": [7]})).expect("vec form is accepted");
        assert_eq!(vec_some.0, Some(7));

        let vec_cap: MoveOption<u64> =
            serde_json::from_value(json!({"Vec": [8]})).expect("Vec form is accepted");
        assert_eq!(vec_cap.0, Some(8));
    }

    #[test]
    fn move_option_deserializes_some_none_and_fallback_forms() {
        let some: MoveOption<u64> =
            serde_json::from_value(json!({"some": 9})).expect("some form is accepted");
        assert_eq!(some.0, Some(9));

        let none: MoveOption<u64> =
            serde_json::from_value(json!({"none": true})).expect("none form is accepted");
        assert_eq!(none.0, None);

        let none_cap: MoveOption<u64> =
            serde_json::from_value(json!({"None": {}})).expect("None form is accepted");
        assert_eq!(none_cap.0, None);

        let number: MoveOption<u64> =
            serde_json::from_value(json!(12)).expect("plain value form is accepted");
        assert_eq!(number.0, Some(12));

        let fallback: MoveOption<Foo> = serde_json::from_value(json!({"a": 42}))
            .expect("fallback object form should deserialize as T");
        assert_eq!(fallback.0, Some(Foo { a: 42 }));

        let some_wrapped: MoveOption<Foo> = serde_json::from_value(json!({
            "some": { "fields": { "a": 13 } }
        }))
        .expect("some inner fields wrapper should be tolerated");
        assert_eq!(some_wrapped.0, Some(Foo { a: 13 }));
    }

    #[test]
    fn move_option_deserializes_base64_byte_vector_json_forms() {
        let expected = {
            use sha2::{Digest, Sha256};
            Sha256::digest([]).to_vec()
        };
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &expected);

        let vec_some: MoveOption<Vec<u8>> = serde_json::from_value(json!({"vec": [encoded]}))
            .expect("Sui Move option byte-vector form is accepted");
        assert_eq!(vec_some.0, Some(expected.clone()));

        let some: MoveOption<Vec<u8>> = serde_json::from_value(json!({"some": encoded}))
            .expect("explicit some byte-vector form is accepted");
        assert_eq!(some.0, Some(expected));
    }

    #[test]
    fn move_option_deserializes_direct_byte_vector_payload() {
        let direct: MoveOption<Vec<u8>> =
            serde_json::from_value(json!([1, 2, 3])).expect("direct byte-vector form is accepted");
        assert_eq!(direct.0, Some(vec![1, 2, 3]));

        let option_layout: MoveOption<u64> =
            serde_json::from_value(json!([5])).expect("Move option array form remains accepted");
        assert_eq!(option_layout.0, Some(5));
    }

    #[cfg(feature = "bcs")]
    #[test]
    fn move_option_deserializes_bcs_vec_layout() {
        let bytes = bcs::to_bytes(&vec![1u64]).expect("BCS serialization should succeed");
        let decoded: MoveOption<u64> =
            bcs::from_bytes(&bytes).expect("BCS deserialization should succeed");
        assert_eq!(decoded.0, Some(1));

        let bytes = bcs::to_bytes(&Vec::<u64>::new()).expect("BCS serialization should succeed");
        let decoded: MoveOption<u64> =
            bcs::from_bytes(&bytes).expect("BCS deserialization should succeed");
        assert_eq!(decoded.0, None);
    }

    #[test]
    fn published_move_enum_deserializes_supported_human_readable_forms() {
        let string: PublishedMoveEnum<DemoEnum> =
            serde_json::from_value(json!("Retryable")).expect("string form is accepted");
        assert_eq!(string.0, DemoEnum::Retryable);

        let variant_tag: PublishedMoveEnum<DemoEnum> =
            serde_json::from_value(json!({"_variant_name": "Retryable"}))
                .expect("_variant_name form is accepted");
        assert_eq!(variant_tag.0, DemoEnum::Retryable);

        let at_variant: PublishedMoveEnum<DemoEnum> =
            serde_json::from_value(json!({"@variant": "Retryable"}))
                .expect("@variant form is accepted");
        assert_eq!(at_variant.0, DemoEnum::Retryable);

        let wrapped_fields: PublishedMoveEnum<DemoEnum> =
            serde_json::from_value(json!({"fields": {"_variant_name": "Retryable"}}))
                .expect("fields wrapper is accepted");
        assert_eq!(wrapped_fields.0, DemoEnum::Retryable);

        let singleton_key: PublishedMoveEnum<DemoEnum> =
            serde_json::from_value(json!({"Retryable": {}}))
                .expect("singleton object fallback is accepted");
        assert_eq!(singleton_key.0, DemoEnum::Retryable);
    }
}
